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

use crd::DiskPool;
use http::StatusCode;
use kube::{
    api::{Api, DeleteParams},
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
