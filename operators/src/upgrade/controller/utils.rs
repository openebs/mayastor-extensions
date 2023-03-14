use k8s_openapi::api::core::v1::Pod;
use kube::{api::ObjectList, ResourceExt};
use openapi::clients::tower::ApiClient;
use std::collections::HashSet;
use tracing::{error, warn};

use crate::upgrade::common::error::Error;

/// This function checks if a set of of Pod uids belong to the Pods from a given
/// kube::api::ObjectList<Pod>.
pub(crate) fn at_least_one_pod_uid_exists(
    uid_set: &HashSet<&String>,
    pod_list: ObjectList<Pod>,
) -> bool {
    for pod in pod_list.iter() {
        match pod.metadata.uid.as_ref() {
            Some(uid) => {
                if uid_set.contains(uid) {
                    return true;
                }
            }
            None => {
                // It's ok to panic here because all kubernetes objects should have uids.
                panic!("Could not list all io-engine Pods' metadata.uids");
            }
        }
    }

    false
}

/// This function returns 'true' only if all of the containers in the Pods contained in the
/// ObjectList<Pod> have their Ready status.condition value set to true.
pub(crate) fn all_pods_are_ready(pod_list: ObjectList<Pod>) -> (bool, String, String) {
    let not_ready_warning = |pod_name: &String, namespace: &String| {
        warn!("Couldn't verify the ready condition of io-engine Pod '{}' in namespace '{}' to be true", pod_name, namespace);
    };
    for pod in pod_list.iter() {
        match &pod
            .status
            .as_ref()
            .and_then(|status| status.conditions.as_ref())
        {
            Some(conditions) => {
                for condition in *conditions {
                    if condition.type_.eq("Ready") && condition.status.eq("True") {
                        continue;
                    } else {
                        not_ready_warning(&pod.name_any(), &pod.namespace().unwrap_or_default());
                        return (false, pod.name_any(), pod.namespace().unwrap_or_default());
                    }
                }
            }
            None => {
                not_ready_warning(&pod.name_any(), &pod.namespace().unwrap_or_default());
                return (false, pod.name_any(), pod.namespace().unwrap_or_default());
            }
        }
    }
    (false, "".to_string(), "".to_string())
}

/// This function checks if at least one volume is published.
pub(crate) async fn at_least_one_volume_is_published(
    rest_client: ApiClient,
) -> Result<bool, Error> {
    // The number of volumes to get per request.
    let max_entries = 200;
    let mut starting_token = Some(0_isize);
    let mut volumes = Vec::with_capacity(max_entries as usize);

    // The last paginated request will set the `starting_token` to `None`.
    while starting_token.is_some() {
        let vols = rest_client
            .volumes_api()
            .get_volumes(max_entries, None, starting_token)
            .await
            .map_err(|error| {
                error!(?error, "Failed to list volumes");
                error
            })?;

        let v = vols.into_body();
        volumes.extend(v.entries);
        starting_token = v.next_token;
    }

    // Checking if spec.target exists, spec.target exists only for published volumes.
    for volume in volumes.into_iter() {
        if volume.spec.target.is_some() {
            return Ok(true);
        }
    }

    Ok(false)
}
