use crate::common::{
    constants::CHART_VERSION_LABEL_KEY,
    error::{
        HelmChartVersionLabelHasNoValue, ListStorageVolumes, NoNamespaceInPod, Result, SemverParse,
    },
    rest_client::RestClientSet,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::ObjectList, ResourceExt};
use openapi::models::{Volume, VolumeStatus};
use semver::{Version, VersionReq};
use snafu::ResultExt;
use std::{collections::HashSet, time::Duration};
use tracing::{info, warn};

/// Contains the Rebuild Results.
#[derive(Default)]
pub(crate) struct RebuildResult {
    pub(crate) rebuilding: bool,
    pub(crate) discarded_volumes: Vec<Volume>,
}

/// Function to check for any volume rebuild in progress across the cluster.
pub(crate) async fn rebuild_result(
    rest_client: &RestClientSet,
    stale_volumes: &mut Vec<Volume>,
    node_name: &str,
) -> Result<RebuildResult> {
    loop {
        let unhealthy_volumes = list_unhealthy_volumes(rest_client, stale_volumes).await?;
        if unhealthy_volumes.is_empty() {
            break;
        }

        for volume in unhealthy_volumes.iter() {
            let target = if let Some(target) = volume.state.target.as_ref() {
                target
            } else {
                continue;
            };

            let mut volume_over_nodes = HashSet::new();
            volume_over_nodes.insert(target.node.as_str());

            for (_, topology) in volume.state.replica_topology.iter() {
                if let Some(node) = topology.node.as_ref() {
                    volume_over_nodes.insert(node);
                }
            }

            if volume_over_nodes.contains(node_name) {
                match replica_rebuild_count(volume) {
                    0 => {
                        for _i in 0 .. 11 {
                            // wait for a minute for any rebuild to start
                            tokio::time::sleep(Duration::from_secs(60_u64)).await;
                            let count = replica_rebuild_count(volume);
                            if count > 0 {
                                return Ok(RebuildResult {
                                    rebuilding: true,
                                    discarded_volumes: stale_volumes.clone(),
                                });
                            }
                        }
                        stale_volumes.push(volume.clone());
                    }
                    _ => {
                        return Ok(RebuildResult {
                            rebuilding: true,
                            discarded_volumes: stale_volumes.to_vec(),
                        })
                    }
                }
            }
        }
    }
    Ok(RebuildResult {
        rebuilding: false,
        discarded_volumes: stale_volumes.to_vec(),
    })
}

/// Return the list of unhealthy volumes.
pub(crate) async fn list_unhealthy_volumes(
    rest_client: &RestClientSet,
    discarded_volumes: &[Volume],
) -> Result<Vec<Volume>> {
    let mut unhealthy_volumes: Vec<Volume> = Vec::new();
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
            match volume.state.status {
                VolumeStatus::Faulted | VolumeStatus::Degraded => {
                    unhealthy_volumes.push(volume);
                }
                _ => continue,
            }
        }
    }
    unhealthy_volumes.retain(|v| !discarded_volumes.contains(v));
    Ok(unhealthy_volumes)
}

/// Count of number of replica rebuilding.
pub(crate) fn replica_rebuild_count(volume: &Volume) -> i32 {
    let mut rebuild_count = 0;
    if let Some(target) = &volume.state.target {
        for child in target.children.iter() {
            if child.rebuild_progress.is_some() {
                rebuild_count += 1;
            }
        }
        if rebuild_count > 0 {
            info!(
                "Rebuilding {} of {} replicas for volume {}",
                rebuild_count,
                target.children.len(),
                volume.spec.uuid
            );
        }
    }
    rebuild_count
}

/// This function returns 'true' only if all of the containers in the Pods contained in the
/// ObjectList<Pod> have their Ready status.condition value set to true.
pub(crate) fn all_pods_are_ready(pod_list: ObjectList<Pod>) -> bool {
    let not_ready_warning = |pod_name: &String, namespace: &String| {
        warn!(
            "Couldn't verify the ready condition of Pod '{}' in namespace '{}' to be true",
            pod_name, namespace
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
                            info!(pod.name = %pod_name, "Pod is Ready");
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

/// Checks to see if all of io-engine Pods are already upgraded to the version of the local helm
/// chart.
pub(crate) async fn data_plane_is_upgraded(
    to_version: &str,
    io_engine_pod_list: &ObjectList<Pod>,
) -> Result<bool> {
    let to_version_requirement: VersionReq =
        VersionReq::parse(to_version).context(SemverParse {
            version_string: to_version.to_string(),
        })?;

    for pod in io_engine_pod_list {
        let version_str = pod.labels().get(CHART_VERSION_LABEL_KEY).ok_or(
            HelmChartVersionLabelHasNoValue {
                pod_name: pod.name_any(),
                namespace: pod.namespace().ok_or(
                    NoNamespaceInPod {
                        pod_name: pod.name_any(),
                        context: "checking to see if data-plane Pods are already upgraded"
                            .to_string(),
                    }
                    .build(),
                )?,
            }
            .build(),
        )?;
        let version = Version::parse(version_str).context(SemverParse {
            version_string: version_str.clone(),
        })?;
        if !to_version_requirement.matches(&version) {
            return Ok(false);
        }
    }

    Ok(true)
}
