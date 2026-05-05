use constants::IO_ENGINE_LABEL;
use k8s_openapi::api::{apps::v1::DaemonSet, core::v1::Node};
use kube::{
    api::{Api, ListParams},
    Client,
};
use snafu::Snafu;
use std::collections::BTreeSet;
use upgrade::common::{error::Error as UpgradeError, kube::client::paginated_list};

/// The Mayastor-specific node-label key that the helm chart uses by
/// default to target io-engine pods.
const PREFERRED_REMOVE_KEY: &str = "openebs.io/engine";

/// Errors that can occur while running the pre-flight check.
#[derive(Debug, Snafu)]
pub enum LabelCheckError {
    /// Failed to list io-engine DaemonSets in the install namespace.
    #[snafu(display("Failed to list io-engine DaemonSet(s) in namespace {namespace}: {source}"))]
    ListDaemonSets {
        source: UpgradeError,
        namespace: String,
    },
    /// Failed to fetch the Kubernetes Node object.
    #[snafu(display("Failed to fetch Kubernetes Node {node_id}: {source}"))]
    GetNode {
        source: kube::Error,
        node_id: String,
    },
    /// At least one io-engine DaemonSet's nodeSelector is fully satisfied
    /// by the Kubernetes Node's labels.
    #[snafu(display(
        "Cannot delete storage node '{node_id}': io-engine DaemonSet(s) [{ds_names}] \
         still consider this node a scheduling target — every nodeSelector label is \
         present on the Kubernetes Node: [{labels}]. Remove at least one nodeSelector \
         label required by each of these DaemonSets to detarget; for example: \
         `kubectl label node {node_id} {example_remove_arg}`"
    ))]
    NodeStillTargeted {
        node_id: String,
        ds_names: String,
        labels: String,
        example_remove_arg: String,
    },
    #[snafu(display(
        "Node '{node_id}' (DaemonSet(s) [{ds_names}]): the matched-labels set was \
         unexpectedly empty. Workaround: inspect the io-engine DaemonSet's nodeSelector \
         with `kubectl -n {namespace} get ds -l {IO_ENGINE_LABEL} -o yaml` and remove the \
         corresponding label(s) from the node manually."
    ))]
    EmptyMatchedLabels {
        node_id: String,
        ds_names: String,
        namespace: String,
    },
}

/// Verify that no io-engine DaemonSet with a non-empty nodeSelector matches this node’s labels.
///
/// Returns `Ok(())` when:
/// * No io-engine DaemonSet exists in `namespace` (check skipped).
/// * The Kubernetes Node `node_id` does not exist in the cluster (check skipped).
/// * The node exists but no DaemonSet's `nodeSelector` is fully satisfied by its
///   labels (check passes).
pub async fn ensure_node_unlabelled(
    client: Client,
    namespace: &str,
    node_id: &str,
) -> Result<(), LabelCheckError> {
    let ds_api: Api<DaemonSet> = Api::namespaced(client.clone(), namespace);
    let mut daemonsets: Vec<DaemonSet> = Vec::new();
    paginated_list(
        ds_api,
        &mut daemonsets,
        Some(ListParams::default().labels(IO_ENGINE_LABEL)),
    )
    .await
    .map_err(|source| LabelCheckError::ListDaemonSets {
        source,
        namespace: namespace.into(),
    })?;

    if daemonsets.is_empty() {
        return Ok(());
    }

    // 2. Fetch the Kubernetes Node. Missing Node → trivially passes.
    let node_api: Api<Node> = Api::all(client);
    let node = match node_api.get_opt(node_id).await {
        Ok(Some(node)) => node,
        Ok(None) => return Ok(()),
        Err(source) => {
            return Err(LabelCheckError::GetNode {
                source,
                node_id: node_id.into(),
            });
        }
    };

    let node_labels = node.metadata.labels.unwrap_or_default();

    // 3. For each DaemonSet that declares a nodeSelector, check whether
    //    every entry in it is matched by a label on the Node (AND).
    let mut targeting: Vec<String> = Vec::new();
    let mut matched_kvs: BTreeSet<(String, String)> = BTreeSet::new();

    for ds in &daemonsets {
        let ds_name = match ds.metadata.name.as_deref() {
            Some(n) => n,
            None => continue,
        };

        // Skip DSes with no nodeSelector.
        let Some(selector) = ds
            .spec
            .as_ref()
            .and_then(|s| s.template.spec.as_ref())
            .and_then(|ps| ps.node_selector.as_ref())
            .filter(|sel| !sel.is_empty())
        else {
            continue;
        };

        let all_match = selector
            .iter()
            .all(|(k, v)| node_labels.get(k).map(|nv| nv == v).unwrap_or(false));
        if !all_match {
            continue;
        }

        targeting.push(ds_name.to_string());
        for (k, v) in selector {
            matched_kvs.insert((k.clone(), v.clone()));
        }
    }

    if targeting.is_empty() {
        return Ok(());
    }

    let ds_names = targeting.join(", ");
    let labels = matched_kvs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");
    // Prefer the canonical `openebs.io/engine` opt-in label
    // when it's among the matches, otherwise fall back to the
    // alphabetically-first matched key for determinism. Safe to unwrap:
    // `targeting` non-empty (checked above) implies at least one
    // nodeSelector entry was inserted into `matched_kvs` in the loop.
    let preferred_kv = matched_kvs
        .iter()
        .find(|(k, _)| k.as_str() == PREFERRED_REMOVE_KEY)
        .or_else(|| matched_kvs.iter().next());
    let (preferred_key, _) = match preferred_kv {
        Some(kv) => kv,
        None => {
            return Err(LabelCheckError::EmptyMatchedLabels {
                node_id: node_id.into(),
                ds_names,
                namespace: namespace.into(),
            });
        }
    };
    let example_remove_arg = format!("{preferred_key}-");

    Err(LabelCheckError::NodeStillTargeted {
        node_id: node_id.into(),
        ds_names,
        labels,
        example_remove_arg,
    })
}
