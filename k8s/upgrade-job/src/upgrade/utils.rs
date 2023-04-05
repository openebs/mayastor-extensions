use crate::common::{
    error::{EmptyStorageNodeSpec, ListStorageVolumes, Result},
    rest_client::RestClientSet,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::ObjectList, ResourceExt};
use openapi::models::CordonDrainState;
use snafu::ResultExt;

/// Function to find whether any node drain is in progress.
pub(crate) async fn is_draining(rest_client: &RestClientSet) -> Result<bool> {
    let mut is_draining = false;
    let nodes = rest_client
        .nodes_api()
        .get_nodes()
        .await
        .context(ListStorageVolumes)?;

    let nodelist = nodes.into_body();
    for node in nodelist {
        let node_spec = node
            .spec
            .ok_or(EmptyStorageNodeSpec { node_id: node.id }.build())?;

        is_draining = match node_spec.cordondrainstate {
            Some(CordonDrainState::cordonedstate(_)) => false,
            Some(CordonDrainState::drainingstate(_)) => true,
            Some(CordonDrainState::drainedstate(_)) => false,
            None => false,
        };
        if is_draining {
            break;
        }
    }
    Ok(is_draining)
}

/// Function to check for any volume rebuild in progress across the cluster
pub(crate) async fn is_rebuilding(rest_client: &RestClientSet) -> Result<bool> {
    // The number of volumes to get per request.
    let max_entries = 200;
    let mut starting_token = Some(0_isize);

    // The last paginated request will set the `starting_token` to `None`.
    while starting_token.is_some() {
        let vols = rest_client
            .volumes_api()
            .get_volumes(max_entries, None, starting_token)
            .await
            .context(ListStorageVolumes)?;

        let volumes = vols.into_body();
        starting_token = volumes.next_token;
        for volume in volumes.entries {
            if let Some(target) = &volume.state.target {
                if target
                    .children
                    .iter()
                    .any(|child| child.rebuild_progress.is_some())
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// This function returns 'true' only if all of the containers in the Pods contained in the
/// ObjectList<Pod> have their Ready status.condition value set to true.
pub(crate) fn all_pods_are_ready(pod_list: ObjectList<Pod>) -> (bool, String, String) {
    let not_ready_warning = |pod_name: &String, namespace: &String| {
        tracing::warn!("Couldn't verify the ready condition of io-engine Pod '{}' in namespace '{}' to be true", pod_name, namespace);
    };
    for pod in pod_list.iter() {
        match &pod
            .status
            .as_ref()
            .and_then(|status| status.conditions.as_ref())
        {
            Some(conditions) => {
                for condition in *conditions {
                    if condition.type_.eq("Ready") {
                        if condition.status.eq("True") {
                            break;
                        }
                        not_ready_warning(&pod.name_any(), &pod.namespace().unwrap_or_default());
                        return (false, pod.name_any(), pod.namespace().unwrap_or_default());
                    } else {
                        continue;
                    }
                }
            }
            None => {
                not_ready_warning(&pod.name_any(), &pod.namespace().unwrap_or_default());
                return (false, pod.name_any(), pod.namespace().unwrap_or_default());
            }
        }
    }
    (true, "".to_string(), "".to_string())
}
