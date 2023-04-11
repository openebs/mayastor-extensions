use crate::common::{
    error::{ListStorageVolumes, Result},
    rest_client::RestClientSet,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::ObjectList, ResourceExt};
use snafu::ResultExt;

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
pub(crate) fn all_pods_are_ready(pod_list: ObjectList<Pod>) -> bool {
    let not_ready_warning = |pod_name: &String, namespace: &String| {
        tracing::warn!(
            "Couldn't verify the ready condition of Pod '{}' in namespace '{}' to be true",
            pod_name,
            namespace
        );
    };
    for pod in pod_list.into_iter() {
        match &pod
            .status
            .as_ref()
            .and_then(|status| status.conditions.as_ref())
        {
            Some(conditions) => {
                for condition in *conditions {
                    if condition.type_.eq("Ready") {
                        if condition.status.eq("True") {
                            let pod_name = pod.name_any();
                            tracing::info!(pod.name = %pod_name, "Pod is Ready");
                            break;
                        }
                        not_ready_warning(&pod.name_any(), &pod.namespace().unwrap_or_default());
                        return false;
                    } else {
                        continue;
                    }
                }
            }
            None => {
                not_ready_warning(&pod.name_any(), &pod.namespace().unwrap_or_default());
                return false;
            }
        }
    }
    true
}
