use crate::common::{
    constants::product_train,
    error::{
        CordonStorageNode, EmptyStorageNodeSpec, GetStorageNode, ListStorageVolumes, Result,
        StorageNodeUncordon,
    },
    rest_client::RestClientSet,
};
use k8s_openapi::api::{apps::v1::DaemonSet, core::v1::Pod};
use kube::ResourceExt;
use openapi::models::{CordonDrainState, Volume, VolumeStatus};
use snafu::ResultExt;
use std::{
    collections::HashSet,
    fmt::{Display, Formatter},
    time::Duration,
};
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

        let mut volume_over_nodes = HashSet::new();
        for volume in unhealthy_volumes.iter() {
            let target = match volume.state.target.as_ref() {
                Some(t) => t,
                None => continue,
            };

            volume_over_nodes.insert(target.node.as_str());

            for topology in volume.state.replica_topology.values() {
                if let Some(node) = topology.node.as_ref() {
                    volume_over_nodes.insert(node);
                }
            }

            if volume_over_nodes.contains(node_name) {
                match replica_rebuild_count(volume) {
                    0 => {
                        for _i in 0..11 {
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
        if volume_over_nodes.is_empty() {
            break;
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
pub(crate) fn all_pods_are_ready(pod_list: Vec<Pod>) -> bool {
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

/// The reason why the rollout of a DaemonSet is not complete. The reasons are checked in the order
/// in which they are listed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RolloutIncompleteReason {
    /// The DaemonSet has no .status.
    StatusAbsent,
    /// The DaemonSet controller has not observed the latest spec of the DaemonSet.
    GenerationNotObserved,
    /// Some of the nodes which should run a Pod of the DaemonSet don't have one.
    PodsMissing,
    /// Some of the Pods of the DaemonSet are not created from its latest Pod template.
    PodsNotUpdated,
    /// Some of the Pods of the DaemonSet are not available.
    PodsNotAvailable,
}

impl Display for RolloutIncompleteReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::StatusAbsent => "the DaemonSet has no status",
            Self::GenerationNotObserved => {
                "the DaemonSet controller has not observed the latest DaemonSet spec"
            }
            Self::PodsMissing => "some of the nodes which should run a Pod don't have one",
            Self::PodsNotUpdated => "some of the Pods are not up-to-date",
            Self::PodsNotAvailable => "some of the Pods are not available",
        })
    }
}

/// The rollout state of a DaemonSet. The rollout is complete when the DaemonSet controller has
/// observed the latest spec of the DaemonSet, and all of the nodes which should run a Pod of the
/// DaemonSet run an up-to-date and available Pod. These are the rules which
/// 'kubectl rollout status' uses for DaemonSets. It can't be used for the io-engine DaemonSet,
/// because it only supports the RollingUpdate update strategy.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct DaemonSetRollout {
    pub(crate) generation: i64,
    pub(crate) observed_generation: i64,
    pub(crate) desired: i32,
    pub(crate) current: i32,
    pub(crate) ready: i32,
    pub(crate) updated: i32,
    pub(crate) available: i32,
    pub(crate) incomplete_reason: Option<RolloutIncompleteReason>,
}

impl DaemonSetRollout {
    /// Returns true if the rollout of the DaemonSet is complete.
    pub(crate) fn is_complete(&self) -> bool {
        self.incomplete_reason.is_none()
    }

    /// Returns true if the DaemonSet controller has observed the latest generation of the
    /// DaemonSet. The .status of the DaemonSet reflects its latest spec only after this is true.
    pub(crate) fn generation_is_observed(&self) -> bool {
        !matches!(
            self.incomplete_reason,
            Some(
                RolloutIncompleteReason::StatusAbsent
                    | RolloutIncompleteReason::GenerationNotObserved
            )
        )
    }
}

impl From<&DaemonSet> for DaemonSetRollout {
    fn from(ds: &DaemonSet) -> Self {
        let generation = ds.metadata.generation.unwrap_or_default();
        let Some(status) = ds.status.as_ref() else {
            return Self {
                generation,
                incomplete_reason: Some(RolloutIncompleteReason::StatusAbsent),
                ..Default::default()
            };
        };

        let mut rollout = Self {
            generation,
            observed_generation: status.observed_generation.unwrap_or_default(),
            desired: status.desired_number_scheduled,
            current: status.current_number_scheduled,
            ready: status.number_ready,
            updated: status.updated_number_scheduled.unwrap_or_default(),
            available: status.number_available.unwrap_or_default(),
            incomplete_reason: None,
        };
        rollout.incomplete_reason = if rollout.generation > rollout.observed_generation {
            Some(RolloutIncompleteReason::GenerationNotObserved)
        } else if rollout.current < rollout.desired {
            Some(RolloutIncompleteReason::PodsMissing)
        } else if rollout.updated < rollout.desired {
            Some(RolloutIncompleteReason::PodsNotUpdated)
        } else if rollout.available < rollout.desired {
            Some(RolloutIncompleteReason::PodsNotAvailable)
        } else {
            None
        };

        rollout
    }
}

impl Display for DaemonSetRollout {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.incomplete_reason {
            Some(reason) => write!(f, "{reason}")?,
            None => f.write_str("the rollout is complete")?,
        }
        write!(
            f,
            " (desired: {}, current: {}, ready: {}, up-to-date: {}, available: {}, \
            generation: {}, observed generation: {})",
            self.desired,
            self.current,
            self.ready,
            self.updated,
            self.available,
            self.generation,
            self.observed_generation
        )
    }
}

/// Returns the name of the node which a DaemonSet Pod runs on, or is meant to run on. The
/// DaemonSet controller sets the node affinity of its Pods to their node, so that this is known
/// even before the Pod is scheduled. This is the same as the DaemonSet controller's
/// GetTargetNodeName().
pub(crate) fn pod_target_node(pod: &Pod) -> Option<String> {
    let spec = pod.spec.as_ref()?;
    if let Some(node_name) = spec.node_name.as_ref().filter(|name| !name.is_empty()) {
        return Some(node_name.clone());
    }

    spec.affinity
        .as_ref()?
        .node_affinity
        .as_ref()?
        .required_during_scheduling_ignored_during_execution
        .as_ref()?
        .node_selector_terms
        .iter()
        .flat_map(|term| term.match_fields.iter().flatten())
        .find(|requirement| requirement.key == "metadata.name" && requirement.operator == "In")
        .and_then(|requirement| match requirement.values.as_deref() {
            Some([node_name]) => Some(node_name.clone()),
            _ => None,
        })
}

/// Returns true if the Ready condition of the Pod is true.
pub(crate) fn pod_is_ready(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|status| status.conditions.as_ref())
        .is_some_and(|conditions| {
            conditions
                .iter()
                .any(|condition| condition.type_ == "Ready" && condition.status == "True")
        })
}

/// Describes the Pods which are not Ready, along with the nodes which they run on or are meant to
/// run on, and their phase. At most 'limit' Pods are described.
pub(crate) fn describe_not_ready_pods(pods: &[Pod], limit: usize) -> String {
    let not_ready_pods: Vec<&Pod> = pods.iter().filter(|pod| !pod_is_ready(pod)).collect();
    if not_ready_pods.is_empty() {
        return "none".to_string();
    }

    let mut description = not_ready_pods
        .iter()
        .take(limit)
        .map(|pod| {
            format!(
                "{} (node: {}, phase: {})",
                pod.name_any(),
                pod_target_node(pod).as_deref().unwrap_or("unknown"),
                pod.status
                    .as_ref()
                    .and_then(|status| status.phase.as_deref())
                    .unwrap_or("Unknown")
            )
        })
        .collect::<Vec<String>>()
        .join(", ");
    if not_ready_pods.len() > limit {
        description.push_str(format!(" and {} more", not_ready_pods.len() - limit).as_str());
    }

    description
}

/// Cordon storage node.
pub(crate) async fn cordon_storage_node(
    node_id: &str,
    cordon_label: &str,
    rest_client: &RestClientSet,
) -> Result<()> {
    let cordon_label = cordon_label.to_string();

    let storage_node = rest_client
        .nodes_api()
        .get_node(node_id)
        .await
        .context(GetStorageNode {
            node_id: node_id.to_string(),
        })?;

    match storage_node
        .into_body()
        .spec
        .ok_or(
            EmptyStorageNodeSpec {
                node_id: node_id.to_string(),
            }
            .build(),
        )?
        .cordondrainstate
    {
        Some(CordonDrainState::cordonedstate(cordon_state))
            if cordon_state.cordonlabels.contains(&cordon_label) =>
        {
            info!(node.id = %node_id, "{} Node is already cordoned", product_train());
        }
        _ => {
            rest_client
                .nodes_api()
                .put_node_cordon(node_id, cordon_label.as_str())
                .await
                .context(CordonStorageNode {
                    node_id: node_id.to_string(),
                })?;

            info!(node.id = %node_id, "Put cordon label for {} Node", product_train());
        }
    }

    Ok(())
}

/// Uncordon storage Node.
pub(crate) async fn uncordon_storage_node(
    node_id: &str,
    cordon_label: &str,
    rest_client: &RestClientSet,
) -> Result<()> {
    let cordon_label = cordon_label.to_string();
    let storage_node = rest_client
        .nodes_api()
        .get_node(node_id)
        .await
        .context(GetStorageNode {
            node_id: node_id.to_string(),
        })?;

    match storage_node
        .into_body()
        .spec
        .ok_or(
            EmptyStorageNodeSpec {
                node_id: node_id.to_string(),
            }
            .build(),
        )?
        .cordondrainstate
    {
        Some(CordonDrainState::cordonedstate(cordon_state))
            if cordon_state.cordonlabels.contains(&cordon_label) =>
        {
            rest_client
                .nodes_api()
                .delete_node_cordon(node_id, cordon_label.as_str())
                .await
                .context(StorageNodeUncordon {
                    node_id: node_id.to_string(),
                })?;

            info!(
                node.id = %node_id,
                label = %cordon_label,
                "Removed cordon label from {} Node",
                product_train()
            );
        }
        _ => info!(
                node.id = %node_id,
                label = %cordon_label,
                "Cordon label absent from {} Node", product_train()
        ),
    }

    Ok(())
}

/// List all Storage volumes. Paginated responses from the Storage REST.
pub(crate) async fn list_all_volumes(rest_client: &RestClientSet) -> Result<Vec<Volume>> {
    let mut volumes: Vec<Volume> = Vec::new();
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

        let vols = vols.into_body();
        volumes.extend(vols.entries);

        starting_token = vols.next_token;
    }

    Ok(volumes)
}

#[cfg(test)]
mod tests {
    use super::{
        describe_not_ready_pods, pod_is_ready, pod_target_node, DaemonSetRollout,
        RolloutIncompleteReason,
    };
    use k8s_openapi::{
        api::{
            apps::v1::{DaemonSet, DaemonSetStatus},
            core::v1::{
                Affinity, NodeAffinity, NodeSelector, NodeSelectorRequirement, NodeSelectorTerm,
                Pod, PodCondition, PodSpec, PodStatus,
            },
        },
        apimachinery::pkg::apis::meta::v1::ObjectMeta,
    };

    /// Builds a DaemonSet with a .metadata.generation and an optional .status.
    fn daemonset(generation: i64, status: Option<DaemonSetStatus>) -> DaemonSet {
        DaemonSet {
            metadata: ObjectMeta {
                generation: Some(generation),
                ..Default::default()
            },
            status,
            ..Default::default()
        }
    }

    /// Builds a DaemonSetStatus with an observed generation and the desired, current, updated and
    /// available Pod counts.
    fn status(
        observed_generation: Option<i64>,
        desired: i32,
        current: i32,
        updated: Option<i32>,
        available: Option<i32>,
    ) -> Option<DaemonSetStatus> {
        Some(DaemonSetStatus {
            observed_generation,
            desired_number_scheduled: desired,
            current_number_scheduled: current,
            number_ready: available.unwrap_or_default(),
            updated_number_scheduled: updated,
            number_available: available,
            ..Default::default()
        })
    }

    #[test]
    fn daemonset_rollout() {
        use RolloutIncompleteReason::*;

        let test_cases = [
            (
                "complete",
                daemonset(2, status(Some(2), 3, 3, Some(3), Some(3))),
                None,
            ),
            (
                "no desired Pods",
                daemonset(1, status(Some(1), 0, 0, None, None)),
                None,
            ),
            ("status absent", daemonset(1, None), Some(StatusAbsent)),
            (
                "generation not observed",
                daemonset(3, status(Some(2), 3, 3, Some(3), Some(3))),
                Some(GenerationNotObserved),
            ),
            (
                "observed generation absent",
                daemonset(1, status(None, 0, 0, None, None)),
                Some(GenerationNotObserved),
            ),
            (
                "Pods missing",
                daemonset(2, status(Some(2), 3, 2, Some(2), Some(2))),
                Some(PodsMissing),
            ),
            (
                "Pods not updated",
                daemonset(2, status(Some(2), 3, 3, Some(2), Some(3))),
                Some(PodsNotUpdated),
            ),
            (
                "updated Pod count absent",
                daemonset(2, status(Some(2), 3, 3, None, Some(3))),
                Some(PodsNotUpdated),
            ),
            (
                "Pods not available",
                daemonset(2, status(Some(2), 3, 3, Some(3), Some(2))),
                Some(PodsNotAvailable),
            ),
            (
                "available Pod count absent",
                daemonset(2, status(Some(2), 3, 3, Some(3), None)),
                Some(PodsNotAvailable),
            ),
        ];

        for (name, ds, expected_reason) in test_cases {
            let rollout = DaemonSetRollout::from(&ds);
            assert_eq!(rollout.incomplete_reason, expected_reason, "{name}");
            assert_eq!(rollout.is_complete(), expected_reason.is_none(), "{name}");
            assert_eq!(
                rollout.generation_is_observed(),
                !matches!(expected_reason, Some(StatusAbsent | GenerationNotObserved)),
                "{name}"
            );
        }
    }

    #[test]
    fn daemonset_rollout_counts() {
        let rollout =
            DaemonSetRollout::from(&daemonset(4, status(Some(4), 3, 2, Some(1), Some(2))));
        assert_eq!(
            rollout,
            DaemonSetRollout {
                generation: 4,
                observed_generation: 4,
                desired: 3,
                current: 2,
                ready: 2,
                updated: 1,
                available: 2,
                incomplete_reason: Some(RolloutIncompleteReason::PodsMissing),
            }
        );
        assert_eq!(
            rollout.to_string(),
            "some of the nodes which should run a Pod don't have one (desired: 3, current: 2, \
            ready: 2, up-to-date: 1, available: 2, generation: 4, observed generation: 4)"
        );
    }

    /// Builds a NodeSelectorRequirement.
    fn requirement(key: &str, operator: &str, values: &[&str]) -> NodeSelectorRequirement {
        NodeSelectorRequirement {
            key: key.to_string(),
            operator: operator.to_string(),
            values: Some(values.iter().map(ToString::to_string).collect()),
        }
    }

    /// Builds a Pod with an optional .spec.nodeName, and an optional required node affinity whose
    /// node selector terms have the given matchFields.
    fn pod(
        name: &str,
        node_name: Option<&str>,
        match_fields_terms: Option<Vec<Vec<NodeSelectorRequirement>>>,
    ) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..Default::default()
            },
            spec: Some(PodSpec {
                node_name: node_name.map(ToString::to_string),
                affinity: match_fields_terms.map(|terms| Affinity {
                    node_affinity: Some(NodeAffinity {
                        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                            node_selector_terms: terms
                                .into_iter()
                                .map(|match_fields| NodeSelectorTerm {
                                    match_fields: Some(match_fields),
                                    ..Default::default()
                                })
                                .collect(),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn pod_target_nodes() {
        let node_affinity =
            |node: &str| Some(vec![vec![requirement("metadata.name", "In", &[node])]]);

        let test_cases = [
            (
                "scheduled",
                pod("p", Some("node-1"), node_affinity("node-1")),
                Some("node-1"),
            ),
            (
                "scheduled without affinity",
                pod("p", Some("node-1"), None),
                Some("node-1"),
            ),
            (
                "not scheduled",
                pod("p", None, node_affinity("node-2")),
                Some("node-2"),
            ),
            (
                "empty node name",
                pod("p", Some(""), node_affinity("node-2")),
                Some("node-2"),
            ),
            (
                "node in a later term",
                pod(
                    "p",
                    None,
                    Some(vec![
                        vec![],
                        vec![requirement("metadata.name", "In", &["node-3"])],
                    ]),
                ),
                Some("node-3"),
            ),
            ("no affinity", pod("p", None, None), None),
            (
                "many nodes",
                pod(
                    "p",
                    None,
                    Some(vec![vec![requirement(
                        "metadata.name",
                        "In",
                        &["node-1", "node-2"],
                    )]]),
                ),
                None,
            ),
            (
                "not in operator",
                pod(
                    "p",
                    None,
                    Some(vec![vec![requirement(
                        "metadata.name",
                        "NotIn",
                        &["node-1"],
                    )]]),
                ),
                None,
            ),
            (
                "no spec",
                Pod {
                    spec: None,
                    ..pod("p", Some("node-1"), None)
                },
                None,
            ),
        ];

        for (name, pod, expected_node) in test_cases {
            assert_eq!(pod_target_node(&pod).as_deref(), expected_node, "{name}");
        }
    }

    /// Sets the phase and an optional Ready condition on a Pod.
    fn with_status(mut pod: Pod, phase: &str, ready: Option<bool>) -> Pod {
        pod.status = Some(PodStatus {
            phase: Some(phase.to_string()),
            conditions: ready.map(|ready| {
                vec![PodCondition {
                    type_: "Ready".to_string(),
                    status: if ready { "True" } else { "False" }.to_string(),
                    ..Default::default()
                }]
            }),
            ..Default::default()
        });
        pod
    }

    #[test]
    fn not_ready_pods() {
        let pods = [
            with_status(pod("ready", Some("node-1"), None), "Running", Some(true)),
            with_status(
                pod("not-ready", Some("node-2"), None),
                "Running",
                Some(false),
            ),
            with_status(
                pod(
                    "pending",
                    None,
                    Some(vec![vec![requirement("metadata.name", "In", &["node-3"])]]),
                ),
                "Pending",
                None,
            ),
            pod("no-status", None, None),
        ];

        assert!(pod_is_ready(&pods[0]));
        assert!(!pod_is_ready(&pods[1]));
        assert!(!pod_is_ready(&pods[2]));
        assert!(!pod_is_ready(&pods[3]));

        assert_eq!(
            describe_not_ready_pods(&pods, 3),
            "not-ready (node: node-2, phase: Running), pending (node: node-3, phase: Pending), \
            no-status (node: unknown, phase: Unknown)"
        );
        assert_eq!(
            describe_not_ready_pods(&pods, 1),
            "not-ready (node: node-2, phase: Running) and 2 more"
        );
        assert_eq!(describe_not_ready_pods(&pods[..1], 3), "none");
        assert_eq!(describe_not_ready_pods(&[], 3), "none");
    }
}
