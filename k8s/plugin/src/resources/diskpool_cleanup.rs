// DiskPool CR cleanup operations.
//
// Deletes orphaned DiskPool CRs by issuing a DELETE and watching for
// the pool operator's finalizer handler to complete — the same approach
// kubectl uses.  The operator detects that the backing pool spec is
// gone (404 from the control plane), strips the finalizer, and
// Kubernetes garbage-collects the CR.

// TODO: Now that we have workspace pinned versions for dependency cargo crates, we may use the
// DiskPool via the module tree via Cargo.toml, instead of include!-ing the code file here.
/// The DiskPool definition.
#[allow(dead_code)]
#[allow(clippy::empty_line_after_outer_attr)]
mod crd {
    include!(
        "../../../../dependencies/control-plane/k8s/operators/src/pool/diskpool/crd/v1beta3.rs"
    );
}

/// The DiskPool quantity module.
#[allow(dead_code)]
#[allow(clippy::empty_line_after_outer_attr)]
mod quantity {
    include!(
        "../../../../dependencies/control-plane/k8s/operators/src/pool/diskpool/crd/quantity.rs"
    );
}

use anyhow::anyhow;
use crd::DiskPool;
use http::StatusCode;
use kube::{
    api::{Api, DeleteParams, ListParams},
    runtime::wait::{await_condition, conditions::is_deleted},
    Client, ResourceExt,
};
use snafu::Snafu;
/// Errors that can occur during DiskPool CR cleanup.
#[derive(Debug, Snafu)]
pub enum CleanupError {
    /// A Kubernetes API call failed.
    #[snafu(display("Kubernetes API error in namespace {namespace}: {source}"))]
    Kube {
        source: kube::Error,
        namespace: String,
    },
    /// The watch stream failed while waiting for CR deletion.
    #[snafu(display("Watch error in namespace {namespace}: {source}"))]
    Watch {
        source: kube::runtime::wait::Error,
        namespace: String,
    },
}

/// List the names of all DiskPool CRs in `namespace` whose `spec.node` matches
/// `node_id`.
///
/// This queries Kubernetes directly, so it works even when the control-plane
/// node spec has already been deleted (i.e. `get_node_pools` via REST would
/// return NOT_FOUND or an empty list).
///
/// Results are fetched in pages to avoid a single large response.
pub async fn list_diskpool_ids_for_node(
    client: Client,
    namespace: &str,
    node_id: &str,
) -> Result<Vec<String>, CleanupError> {
    let api: Api<DiskPool> = Api::namespaced(client, namespace);
    let max_entries: u32 = 500;
    let mut params = ListParams::default().limit(max_entries);
    let mut pool_ids = Vec::with_capacity(max_entries as usize);

    loop {
        let page = api
            .list(&params)
            .await
            .map_err(|source| CleanupError::Kube {
                source,
                namespace: namespace.into(),
            })?;

        for pool in page.items {
            if pool.spec.node == node_id {
                if let Some(name) = pool.metadata.name {
                    pool_ids.push(name);
                }
            }
        }

        match page.metadata.continue_.as_deref() {
            Some("") | None => break,
            Some(token) => params = params.continue_token(token),
        }
    }

    Ok(pool_ids)
}

/// Delete a DiskPool CR, applying a timeout and not-found semantics.
///
/// This is the high-level entry point used by the plugin's delete commands.
/// It wraps [`delete_diskpool_cr`] with:
///
/// - A `timeout` enforced via [`tokio::time::timeout`].
/// - `pool_deleted`: whether the backing pool spec was already removed via
///   REST.  When `true`, a missing CR is acceptable (already clean).  When
///   both are absent the operation is an error unless `ignore_not_found`.
/// - `ignore_not_found`: suppress the "both missing" error, matching the
///   `--ignore-not-found` CLI flag.
pub async fn cleanup_dsp(
    client: Client,
    namespace: &str,
    pool_id: &str,
    timeout: humantime::Duration,
    pool_deleted: bool,
    ignore_not_found: bool,
) -> Result<(), anyhow::Error> {
    let cr_deleted = tokio::time::timeout(*timeout, delete_diskpool_cr(client, namespace, pool_id))
        .await
        .map_err(|_| {
            anyhow!(
                "Timed out after {timeout} waiting for DiskPool CR {pool_id} deletion \
             in namespace {namespace}. Check that the pool operator is running."
            )
        })?
        .map_err(|e| anyhow!("Failed to delete DiskPool {pool_id}: {e}"))?
        .is_some();

    // Both sides missing is an error (unless suppressed).
    if !cr_deleted && !pool_deleted {
        return if ignore_not_found {
            Ok(())
        } else {
            Err(anyhow!(
                "Pool {pool_id} not found (pool spec and DiskPool CR are both missing)"
            ))
        };
    }

    Ok(())
}

/// Delete a DiskPool CR and wait for the operator to process the
/// finalizer.
///
/// Issues a DELETE on the CR which sets `deletionTimestamp`.  The
/// operator's finalizer handler runs, detects the pool is missing from
/// the control plane, strips the finalizer, and Kubernetes
/// garbage-collects the CR.  This function watches (like kubectl) until
/// the CR is actually gone.
///
/// Returns `Ok(None)` if the CR does not exist.
///
/// **This function does not enforce a timeout.**  The caller should wrap
/// it with `tokio::time::timeout` or similar.
pub async fn delete_diskpool_cr(
    client: Client,
    namespace: &str,
    pool_id: &str,
) -> Result<Option<()>, CleanupError> {
    let api: Api<DiskPool> = Api::namespaced(client, namespace);

    let resp = match api.delete(pool_id, &DeleteParams::default()).await {
        Ok(resp) => resp,
        Err(kube::Error::Api(resp)) if resp.code == StatusCode::NOT_FOUND => return Ok(None),
        Err(source) => {
            return Err(CleanupError::Kube {
                source,
                namespace: namespace.into(),
            })
        }
    };

    // Left: CR still exists (finalizer blocking deletion) — watch until gone.
    // Right: CR was deleted immediately (no finalizers) — already done.
    if let Some(cr) = resp.left() {
        if let Some(uid) = cr.uid() {
            await_condition(api, pool_id, is_deleted(&uid))
                .await
                .map_err(|source| CleanupError::Watch {
                    source,
                    namespace: namespace.into(),
                })?;
        }
    }

    Ok(Some(()))
}
