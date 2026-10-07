use crate::common::{
    constants::product_train,
    error::{
        CordonStorageNode, EmptyStorageNodeSpec, GetStorageNode, ListStorageVolumes, Result,
        StorageNodeUncordon,
    },
    rest_client::RestClientSet,
};
use constants::DS_CONTROLLER_REVISION_HASH_LABEL_KEY;
use k8s_openapi::{
    api::{
        apps::v1::DaemonSet,
        core::v1::{
            Node, NodeSelectorRequirement, NodeSelectorTerm, Pod, PodSpec, Taint, Toleration,
        },
    },
    chrono::{DateTime, Utc},
};
use kube::ResourceExt;
use openapi::models::{CordonDrainState, Volume, VolumeStatus};
use snafu::ResultExt;
use std::{
    collections::{BTreeMap, HashSet},
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

/// Returns true if the Pod is being deleted.
pub(crate) fn pod_is_terminating(pod: &Pod) -> bool {
    pod.metadata.deletion_timestamp.is_some()
}

/// Returns true if the Pod is still terminating 'timeout' after its .metadata.deletionTimestamp,
/// e.g. because a finalizer is not removed from it. The deletionTimestamp already includes the
/// Pod's termination grace period.
pub(crate) fn pod_is_stuck_terminating(pod: &Pod, timeout: Duration, now: DateTime<Utc>) -> bool {
    pod.metadata
        .deletion_timestamp
        .as_ref()
        .is_some_and(|deletion_timestamp| {
            // This is an error if the deletionTimestamp is yet to come.
            (now - deletion_timestamp.0)
                .to_std()
                .is_ok_and(|overdue| overdue >= timeout)
        })
}

/// Returns a Pod of a DaemonSet on a node which is stuck terminating (see
/// pod_is_stuck_terminating()), unless the DaemonSet has replaced it with a Pod which is created
/// from its latest Pod template, i.e. with the revision hash 'latest_revision_hash', and which is
/// not terminating. The DaemonSet may not create a new Pod on the node until the stuck Pod is gone.
pub(crate) fn stuck_terminating_pod<'a>(
    node_pods: &'a [Pod],
    latest_revision_hash: &str,
    timeout: Duration,
    now: DateTime<Utc>,
) -> Option<&'a Pod> {
    let replaced = node_pods.iter().any(|pod| {
        !pod_is_terminating(pod)
            && pod
                .labels()
                .get(DS_CONTROLLER_REVISION_HASH_LABEL_KEY)
                .map(String::as_str)
                == Some(latest_revision_hash)
    });
    if replaced {
        return None;
    }

    node_pods
        .iter()
        .find(|pod| pod_is_stuck_terminating(pod, timeout, now))
}

/// Describes the Pods which are not Ready, along with the nodes which they run on or are meant to
/// run on, their phase, and whether they are terminating. At most 'limit' Pods are described.
pub(crate) fn describe_not_ready_pods(pods: &[Pod], limit: usize) -> String {
    let not_ready_pods: Vec<String> = pods
        .iter()
        .filter(|pod| !pod_is_ready(pod))
        .map(|pod| {
            format!(
                "{} (node: {}, phase: {}{})",
                pod.name_any(),
                pod_target_node(pod).as_deref().unwrap_or("unknown"),
                pod.status
                    .as_ref()
                    .and_then(|status| status.phase.as_deref())
                    .unwrap_or("Unknown"),
                if pod_is_terminating(pod) {
                    ", terminating"
                } else {
                    ""
                }
            )
        })
        .collect();
    if not_ready_pods.is_empty() {
        return "none".to_string();
    }

    join_with_limit(not_ready_pods.as_slice(), limit)
}

/// Joins the items with commas. The items after the first 'limit' items are only counted.
pub(crate) fn join_with_limit(items: &[String], limit: usize) -> String {
    let mut joined = items
        .iter()
        .take(limit)
        .map(String::as_str)
        .collect::<Vec<&str>>()
        .join(", ");
    if items.len() > limit {
        joined.push_str(format!(" and {} more", items.len() - limit).as_str());
    }

    joined
}

/// Returns true if the object with the UID 'owner_uid' is the controller of the Pod, e.g. the
/// DaemonSet which created the Pod.
pub(crate) fn pod_is_controlled_by(pod: &Pod, owner_uid: &str) -> bool {
    pod.owner_references()
        .iter()
        .any(|owner| owner.controller == Some(true) && owner.uid == owner_uid)
}

/// The reasons for which a DaemonSet doesn't run a Pod on a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NodeSkipReason {
    /// The node doesn't exist.
    NodeNotFound,
    /// The DaemonSet's Pod template sets the name of a different node.
    NodeName,
    /// The node doesn't match the node selector or the required node affinity of the DaemonSet's
    /// Pod template.
    NodeAffinity,
    /// The DaemonSet's Pods don't tolerate this NoSchedule or NoExecute taint of the node.
    UntoleratedTaint(String),
}

impl Display for NodeSkipReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound => f.write_str("the node doesn't exist"),
            Self::NodeName => f.write_str("the DaemonSet's Pod template is for a different node"),
            Self::NodeAffinity => {
                f.write_str("the node doesn't match the DaemonSet's node selector or node affinity")
            }
            Self::UntoleratedTaint(taint) => {
                write!(
                    f,
                    "the DaemonSet's Pods don't tolerate the node's taint '{taint}'"
                )
            }
        }
    }
}

/// These are the (key, effect) pairs of the tolerations which the DaemonSet controller adds to
/// the Pods of every DaemonSet. All of them use the Exists operator. This is the same as the
/// DaemonSet controller's AddOrUpdateDaemonPodTolerations().
const DAEMONSET_POD_DEFAULT_TOLERATIONS: [(&str, &str); 6] = [
    ("node.kubernetes.io/not-ready", "NoExecute"),
    ("node.kubernetes.io/unreachable", "NoExecute"),
    ("node.kubernetes.io/disk-pressure", "NoSchedule"),
    ("node.kubernetes.io/memory-pressure", "NoSchedule"),
    ("node.kubernetes.io/pid-pressure", "NoSchedule"),
    ("node.kubernetes.io/unschedulable", "NoSchedule"),
];

/// This is the (key, effect) pair of the toleration which the DaemonSet controller adds to the
/// Pods of DaemonSets which use the host's network. It uses the Exists operator.
const DAEMONSET_HOST_NETWORK_POD_DEFAULT_TOLERATION: (&str, &str) =
    ("node.kubernetes.io/network-unavailable", "NoSchedule");

/// Returns the reason for which the DaemonSet controller doesn't run a Pod of the DaemonSet on the
/// node, or None if it does. The DaemonSet controller doesn't create a Pod on a node for which
/// there is a reason, e.g. after the Pod which was on the node is deleted. This is the same as the
/// 'shouldRun' result of the DaemonSet controller's NodeShouldRunDaemonPod().
pub(crate) fn daemonset_node_skip_reason(ds: &DaemonSet, node: &Node) -> Option<NodeSkipReason> {
    let default_pod_spec = PodSpec::default();
    let pod_spec = ds
        .spec
        .as_ref()
        .and_then(|spec| spec.template.spec.as_ref())
        .unwrap_or(&default_pod_spec);
    let node_name = node.metadata.name.as_deref().unwrap_or_default();

    if pod_spec
        .node_name
        .as_deref()
        .is_some_and(|name| !name.is_empty() && name != node_name)
    {
        return Some(NodeSkipReason::NodeName);
    }

    if !node_matches_required_node_affinity(pod_spec, node) {
        return Some(NodeSkipReason::NodeAffinity);
    }

    node.spec
        .as_ref()
        .and_then(|spec| spec.taints.as_ref())
        .into_iter()
        .flatten()
        .filter(|taint| taint.effect == "NoSchedule" || taint.effect == "NoExecute")
        .find(|taint| !daemonset_pod_tolerates_taint(pod_spec, taint))
        .map(|taint| NodeSkipReason::UntoleratedTaint(describe_taint(taint)))
}

/// Returns true if the node matches the node selector and the required node affinity of the Pod
/// spec. This is the same as Kubernetes' RequiredNodeAffinity.Match().
fn node_matches_required_node_affinity(pod_spec: &PodSpec, node: &Node) -> bool {
    let no_labels = BTreeMap::new();
    let labels = node.metadata.labels.as_ref().unwrap_or(&no_labels);
    let node_name = node.metadata.name.as_deref().unwrap_or_default();

    let matches_node_selector = pod_spec
        .node_selector
        .iter()
        .flatten()
        .all(|(key, value)| labels.get(key) == Some(value));
    if !matches_node_selector {
        return false;
    }

    match pod_spec
        .affinity
        .as_ref()
        .and_then(|affinity| affinity.node_affinity.as_ref())
        .and_then(|node_affinity| {
            node_affinity
                .required_during_scheduling_ignored_during_execution
                .as_ref()
        }) {
        // The node selector terms are ORed.
        Some(node_selector) => node_selector
            .node_selector_terms
            .iter()
            .any(|term| node_selector_term_matches(term, labels, node_name)),
        None => true,
    }
}

/// Returns true if the node's labels and name match all of the requirements of the node selector
/// term. A term without requirements, or with an invalid requirement, matches no node.
fn node_selector_term_matches(
    term: &NodeSelectorTerm,
    labels: &BTreeMap<String, String>,
    node_name: &str,
) -> bool {
    let match_expressions = term.match_expressions.as_deref().unwrap_or_default();
    let match_fields = term.match_fields.as_deref().unwrap_or_default();
    if match_expressions.is_empty() && match_fields.is_empty() {
        return false;
    }

    match_expressions
        .iter()
        .all(|requirement| node_label_requirement_matches(requirement, labels))
        && match_fields
            .iter()
            .all(|requirement| node_field_requirement_matches(requirement, node_name))
}

/// Returns true if the node's labels match the node selector requirement. An invalid requirement
/// matches no node.
fn node_label_requirement_matches(
    requirement: &NodeSelectorRequirement,
    labels: &BTreeMap<String, String>,
) -> bool {
    let values = requirement.values.as_deref().unwrap_or_default();
    let label = labels.get(requirement.key.as_str());
    match requirement.operator.as_str() {
        "In" if !values.is_empty() => label.is_some_and(|label| values.contains(label)),
        "NotIn" if !values.is_empty() => label.is_none_or(|label| !values.contains(label)),
        "Exists" if values.is_empty() => label.is_some(),
        "DoesNotExist" if values.is_empty() => label.is_none(),
        operator @ ("Gt" | "Lt") => {
            let ([value], Some(label)) = (values, label) else {
                return false;
            };
            match (label.parse::<i64>(), value.parse::<i64>()) {
                (Ok(label), Ok(value)) if operator == "Gt" => label > value,
                (Ok(label), Ok(value)) => label < value,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Returns true if the node's name matches the node selector requirement. 'metadata.name' is the
/// only field which node selector requirements support. An invalid requirement matches no node.
fn node_field_requirement_matches(requirement: &NodeSelectorRequirement, node_name: &str) -> bool {
    let field = match requirement.key.as_str() {
        "metadata.name" => node_name,
        _ => "",
    };
    match (requirement.operator.as_str(), requirement.values.as_deref()) {
        ("In", Some([value])) => field == value.as_str(),
        ("NotIn", Some([value])) => field != value.as_str(),
        _ => false,
    }
}

/// Returns true if the Pods of a DaemonSet with this Pod template spec tolerate the taint. These
/// Pods have the tolerations of the Pod template, and the ones which the DaemonSet controller adds
/// to them.
fn daemonset_pod_tolerates_taint(pod_spec: &PodSpec, taint: &Taint) -> bool {
    let host_network_toleration = pod_spec
        .host_network
        .unwrap_or_default()
        .then_some(&DAEMONSET_HOST_NETWORK_POD_DEFAULT_TOLERATION);
    let tolerated_by_default = DAEMONSET_POD_DEFAULT_TOLERATIONS
        .iter()
        .chain(host_network_toleration)
        .any(|(key, effect)| taint.key == *key && taint.effect == *effect);

    tolerated_by_default
        || pod_spec
            .tolerations
            .iter()
            .flatten()
            .any(|toleration| toleration_tolerates_taint(toleration, taint))
}

/// Returns true if the toleration tolerates the taint. This is the same as Kubernetes'
/// Toleration.ToleratesTaint().
fn toleration_tolerates_taint(toleration: &Toleration, taint: &Taint) -> bool {
    let effect = toleration.effect.as_deref().unwrap_or_default();
    if !effect.is_empty() && effect != taint.effect {
        return false;
    }

    let key = toleration.key.as_deref().unwrap_or_default();
    if !key.is_empty() && key != taint.key {
        return false;
    }

    match toleration.operator.as_deref().unwrap_or_default() {
        "" | "Equal" => {
            toleration.value.as_deref().unwrap_or_default()
                == taint.value.as_deref().unwrap_or_default()
        }
        "Exists" => true,
        // The Gt and Lt operators need the alpha TaintTolerationComparisonOperators feature gate.
        // These operators don't tolerate any taint while it's disabled, which is the default. This
        // errs on the side of not restarting a Pod which may not be re-created.
        _ => false,
    }
}

/// Describes a taint in the same format as kubectl, i.e. 'key=value:effect'.
fn describe_taint(taint: &Taint) -> String {
    match taint.value.as_deref() {
        Some(value) if !value.is_empty() => format!("{}={value}:{}", taint.key, taint.effect),
        _ => format!("{}:{}", taint.key, taint.effect),
    }
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
        daemonset_node_skip_reason, describe_not_ready_pods, pod_is_controlled_by, pod_is_ready,
        pod_is_stuck_terminating, pod_is_terminating, pod_target_node, stuck_terminating_pod,
        DaemonSetRollout, NodeSkipReason, RolloutIncompleteReason,
    };
    use constants::DS_CONTROLLER_REVISION_HASH_LABEL_KEY;
    use k8s_openapi::{
        api::{
            apps::v1::{DaemonSet, DaemonSetSpec, DaemonSetStatus},
            core::v1::{
                Affinity, Node, NodeAffinity, NodeSelector, NodeSelectorRequirement,
                NodeSelectorTerm, NodeSpec, Pod, PodCondition, PodSpec, PodStatus, PodTemplateSpec,
                Taint, Toleration,
            },
        },
        apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference, Time},
        chrono::{DateTime, TimeDelta, Utc},
    };
    use kube::ResourceExt;
    use std::{collections::BTreeMap, time::Duration};

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

        let mut terminating = with_status(pod("terminating", Some("node-4"), None), "Failed", None);
        terminating.metadata.deletion_timestamp = Some(Time(DateTime::default()));
        assert_eq!(
            describe_not_ready_pods(&[terminating], 3),
            "terminating (node: node-4, phase: Failed, terminating)"
        );
    }

    /// Builds a Pod of a DaemonSet on a node, created from the Pod template with the revision hash
    /// 'hash', with an optional .metadata.deletionTimestamp.
    fn daemonset_pod(name: &str, hash: &str, deletion_timestamp: Option<DateTime<Utc>>) -> Pod {
        let mut pod = pod(name, Some("node-1"), None);
        pod.metadata.labels = Some(BTreeMap::from([(
            DS_CONTROLLER_REVISION_HASH_LABEL_KEY.to_string(),
            hash.to_string(),
        )]));
        pod.metadata.deletion_timestamp = deletion_timestamp.map(Time);
        pod
    }

    #[test]
    fn pods_stuck_terminating() {
        let now = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        let timeout = Duration::from_secs(300);
        let ago = |seconds: i64| Some(now - TimeDelta::seconds(seconds));

        let running = daemonset_pod("running", "old", None);
        let in_grace_period = daemonset_pod("in-grace-period", "old", ago(-30));
        let terminating = daemonset_pod("terminating", "old", ago(299));
        let stuck = daemonset_pod("stuck", "old", ago(300));

        assert!(!pod_is_terminating(&running));
        assert!(pod_is_terminating(&in_grace_period));
        assert!(pod_is_terminating(&stuck));

        assert!(!pod_is_stuck_terminating(&running, timeout, now));
        assert!(!pod_is_stuck_terminating(&in_grace_period, timeout, now));
        assert!(!pod_is_stuck_terminating(&terminating, timeout, now));
        assert!(pod_is_stuck_terminating(&stuck, timeout, now));

        let replacement = daemonset_pod("replacement", "latest", None);
        let terminating_replacement = daemonset_pod("terminating-replacement", "latest", ago(10));

        let test_cases = [
            ("no pods", vec![], None),
            ("stuck", vec![stuck.clone()], Some("stuck")),
            ("not stuck yet", vec![terminating.clone()], None),
            ("replaced", vec![stuck.clone(), replacement.clone()], None),
            (
                "replacement is terminating",
                vec![stuck.clone(), terminating_replacement],
                Some("stuck"),
            ),
            ("outdated pod", vec![running, stuck], Some("stuck")),
            ("up-to-date pod", vec![replacement], None),
        ];

        for (name, pods, expected_pod) in test_cases {
            assert_eq!(
                stuck_terminating_pod(pods.as_slice(), "latest", timeout, now)
                    .map(|pod| pod.name_any())
                    .as_deref(),
                expected_pod,
                "{name}"
            );
        }
    }

    #[test]
    fn pods_controlled_by() {
        let owned_by = |uid: &str, controller: Option<bool>| Pod {
            metadata: ObjectMeta {
                owner_references: Some(vec![OwnerReference {
                    uid: uid.to_string(),
                    controller,
                    ..Default::default()
                }]),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(pod_is_controlled_by(&owned_by("ds-1", Some(true)), "ds-1"));
        assert!(!pod_is_controlled_by(&owned_by("ds-2", Some(true)), "ds-1"));
        assert!(!pod_is_controlled_by(
            &owned_by("ds-1", Some(false)),
            "ds-1"
        ));
        assert!(!pod_is_controlled_by(&owned_by("ds-1", None), "ds-1"));
        assert!(!pod_is_controlled_by(&Pod::default(), "ds-1"));
    }

    /// Builds a Node with labels, and taints as (key, value, effect).
    fn node(name: &str, labels: &[(&str, &str)], taints: &[(&str, Option<&str>, &str)]) -> Node {
        Node {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                labels: Some(
                    labels
                        .iter()
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                ),
                ..Default::default()
            },
            spec: Some(NodeSpec {
                taints: Some(
                    taints
                        .iter()
                        .map(|(key, value, effect)| Taint {
                            key: key.to_string(),
                            value: value.map(ToString::to_string),
                            effect: effect.to_string(),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Builds a toleration from its key, operator, value and effect.
    fn toleration(
        key: Option<&str>,
        operator: Option<&str>,
        value: Option<&str>,
        effect: Option<&str>,
    ) -> Toleration {
        Toleration {
            key: key.map(ToString::to_string),
            operator: operator.map(ToString::to_string),
            value: value.map(ToString::to_string),
            effect: effect.map(ToString::to_string),
            ..Default::default()
        }
    }

    /// Builds a required node affinity with node selector terms, which are made of the
    /// (matchExpressions, matchFields) pairs.
    fn required_node_affinity(
        terms: Vec<(Vec<NodeSelectorRequirement>, Vec<NodeSelectorRequirement>)>,
    ) -> Option<Affinity> {
        Some(Affinity {
            node_affinity: Some(NodeAffinity {
                required_during_scheduling_ignored_during_execution: Some(NodeSelector {
                    node_selector_terms: terms
                        .into_iter()
                        .map(|(match_expressions, match_fields)| NodeSelectorTerm {
                            match_expressions: Some(match_expressions),
                            match_fields: Some(match_fields),
                        })
                        .collect(),
                }),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    #[test]
    fn daemonset_node_skip_reasons() {
        use NodeSkipReason::*;

        let engine_label = [("openebs.io/engine", "mayastor")];
        let engine_node_selector = Some(BTreeMap::from([(
            "openebs.io/engine".to_string(),
            "mayastor".to_string(),
        )]));
        let label_affinity = |requirement: NodeSelectorRequirement| {
            required_node_affinity(vec![(vec![requirement], vec![])])
        };
        let maintenance_taint = [("example.com/maintenance", Some("true"), "NoSchedule")];
        let untolerated_maintenance_taint = Some(UntoleratedTaint(
            "example.com/maintenance=true:NoSchedule".to_string(),
        ));
        let with_tolerations = |tolerations: Vec<Toleration>| PodSpec {
            tolerations: Some(tolerations),
            ..Default::default()
        };

        let test_cases = [
            (
                "no constraints",
                PodSpec::default(),
                node("node-1", &[], &[]),
                None,
            ),
            (
                "node selector matches",
                PodSpec {
                    node_selector: engine_node_selector.clone(),
                    ..Default::default()
                },
                node("node-1", &engine_label, &[]),
                None,
            ),
            (
                "node selector label is absent",
                PodSpec {
                    node_selector: engine_node_selector.clone(),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                Some(NodeAffinity),
            ),
            (
                "node selector label has a different value",
                PodSpec {
                    node_selector: engine_node_selector.clone(),
                    ..Default::default()
                },
                node("node-1", &[("openebs.io/engine", "none")], &[]),
                Some(NodeAffinity),
            ),
            (
                "template is for this node",
                PodSpec {
                    node_name: Some("node-1".to_string()),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                None,
            ),
            (
                "template is for a different node",
                PodSpec {
                    node_name: Some("node-2".to_string()),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                Some(NodeName),
            ),
            (
                "In matches",
                PodSpec {
                    affinity: label_affinity(requirement("zone", "In", &["a", "b"])),
                    ..Default::default()
                },
                node("node-1", &[("zone", "b")], &[]),
                None,
            ),
            (
                "In doesn't match",
                PodSpec {
                    affinity: label_affinity(requirement("zone", "In", &["a", "b"])),
                    ..Default::default()
                },
                node("node-1", &[("zone", "c")], &[]),
                Some(NodeAffinity),
            ),
            (
                "In without values is invalid",
                PodSpec {
                    affinity: label_affinity(requirement("zone", "In", &[])),
                    ..Default::default()
                },
                node("node-1", &[("zone", "a")], &[]),
                Some(NodeAffinity),
            ),
            (
                "NotIn matches an absent label",
                PodSpec {
                    affinity: label_affinity(requirement("zone", "NotIn", &["a"])),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                None,
            ),
            (
                "NotIn doesn't match",
                PodSpec {
                    affinity: label_affinity(requirement("zone", "NotIn", &["a"])),
                    ..Default::default()
                },
                node("node-1", &[("zone", "a")], &[]),
                Some(NodeAffinity),
            ),
            (
                "Exists matches",
                PodSpec {
                    affinity: label_affinity(requirement("zone", "Exists", &[])),
                    ..Default::default()
                },
                node("node-1", &[("zone", "a")], &[]),
                None,
            ),
            (
                "DoesNotExist doesn't match",
                PodSpec {
                    affinity: label_affinity(requirement("zone", "DoesNotExist", &[])),
                    ..Default::default()
                },
                node("node-1", &[("zone", "a")], &[]),
                Some(NodeAffinity),
            ),
            (
                "Gt matches",
                PodSpec {
                    affinity: label_affinity(requirement("cores", "Gt", &["8"])),
                    ..Default::default()
                },
                node("node-1", &[("cores", "16")], &[]),
                None,
            ),
            (
                "Lt doesn't match",
                PodSpec {
                    affinity: label_affinity(requirement("cores", "Lt", &["8"])),
                    ..Default::default()
                },
                node("node-1", &[("cores", "16")], &[]),
                Some(NodeAffinity),
            ),
            (
                "Gt with a label which isn't an integer",
                PodSpec {
                    affinity: label_affinity(requirement("cores", "Gt", &["8"])),
                    ..Default::default()
                },
                node("node-1", &[("cores", "many")], &[]),
                Some(NodeAffinity),
            ),
            (
                "terms are ORed",
                PodSpec {
                    affinity: required_node_affinity(vec![
                        (vec![requirement("zone", "In", &["a"])], vec![]),
                        (vec![requirement("zone", "In", &["b"])], vec![]),
                    ]),
                    ..Default::default()
                },
                node("node-1", &[("zone", "b")], &[]),
                None,
            ),
            (
                "requirements of a term are ANDed",
                PodSpec {
                    affinity: required_node_affinity(vec![(
                        vec![
                            requirement("zone", "In", &["a"]),
                            requirement("disk", "Exists", &[]),
                        ],
                        vec![requirement("metadata.name", "In", &["node-1"])],
                    )]),
                    ..Default::default()
                },
                node("node-1", &[("zone", "a")], &[]),
                Some(NodeAffinity),
            ),
            (
                "an empty term matches no node",
                PodSpec {
                    affinity: required_node_affinity(vec![(vec![], vec![])]),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                Some(NodeAffinity),
            ),
            (
                "no terms match no node",
                PodSpec {
                    affinity: required_node_affinity(vec![]),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                Some(NodeAffinity),
            ),
            (
                "node name field matches",
                PodSpec {
                    affinity: required_node_affinity(vec![(
                        vec![],
                        vec![requirement("metadata.name", "In", &["node-1"])],
                    )]),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                None,
            ),
            (
                "node name field doesn't match",
                PodSpec {
                    affinity: required_node_affinity(vec![(
                        vec![],
                        vec![requirement("metadata.name", "NotIn", &["node-1"])],
                    )]),
                    ..Default::default()
                },
                node("node-1", &[], &[]),
                Some(NodeAffinity),
            ),
            (
                "node selector and node affinity must both match",
                PodSpec {
                    node_selector: engine_node_selector.clone(),
                    affinity: label_affinity(requirement("zone", "In", &["a"])),
                    ..Default::default()
                },
                node("node-1", &[("zone", "a")], &[]),
                Some(NodeAffinity),
            ),
            (
                "untolerated NoSchedule taint",
                PodSpec::default(),
                node("node-1", &[], &maintenance_taint),
                untolerated_maintenance_taint.clone(),
            ),
            (
                "untolerated NoExecute taint without a value",
                PodSpec::default(),
                node("node-1", &[], &[("example.com/evict", None, "NoExecute")]),
                Some(UntoleratedTaint("example.com/evict:NoExecute".to_string())),
            ),
            (
                "untolerated PreferNoSchedule taint",
                PodSpec::default(),
                node(
                    "node-1",
                    &[],
                    &[("example.com/maintenance", None, "PreferNoSchedule")],
                ),
                None,
            ),
            (
                "taint tolerated with Exists",
                with_tolerations(vec![toleration(
                    Some("example.com/maintenance"),
                    Some("Exists"),
                    None,
                    None,
                )]),
                node("node-1", &[], &maintenance_taint),
                None,
            ),
            (
                "taint tolerated with Equal",
                with_tolerations(vec![toleration(
                    Some("example.com/maintenance"),
                    Some("Equal"),
                    Some("true"),
                    Some("NoSchedule"),
                )]),
                node("node-1", &[], &maintenance_taint),
                None,
            ),
            (
                "taint tolerated without an operator",
                with_tolerations(vec![toleration(
                    Some("example.com/maintenance"),
                    None,
                    Some("true"),
                    None,
                )]),
                node("node-1", &[], &maintenance_taint),
                None,
            ),
            (
                "toleration for a different value",
                with_tolerations(vec![toleration(
                    Some("example.com/maintenance"),
                    Some("Equal"),
                    Some("false"),
                    None,
                )]),
                node("node-1", &[], &maintenance_taint),
                untolerated_maintenance_taint.clone(),
            ),
            (
                "toleration for a different effect",
                with_tolerations(vec![toleration(
                    Some("example.com/maintenance"),
                    Some("Exists"),
                    None,
                    Some("NoExecute"),
                )]),
                node("node-1", &[], &maintenance_taint),
                untolerated_maintenance_taint.clone(),
            ),
            (
                "toleration for a different key",
                with_tolerations(vec![toleration(
                    Some("example.com/other"),
                    Some("Exists"),
                    None,
                    None,
                )]),
                node("node-1", &[], &maintenance_taint),
                untolerated_maintenance_taint.clone(),
            ),
            (
                "toleration for every taint",
                with_tolerations(vec![toleration(None, Some("Exists"), None, None)]),
                node("node-1", &[], &maintenance_taint),
                None,
            ),
            (
                "Gt toleration",
                with_tolerations(vec![toleration(
                    Some("example.com/level"),
                    Some("Gt"),
                    Some("1"),
                    None,
                )]),
                node(
                    "node-1",
                    &[],
                    &[("example.com/level", Some("2"), "NoSchedule")],
                ),
                Some(UntoleratedTaint(
                    "example.com/level=2:NoSchedule".to_string(),
                )),
            ),
            (
                "taints tolerated by every DaemonSet",
                PodSpec::default(),
                node(
                    "node-1",
                    &[],
                    &[
                        ("node.kubernetes.io/not-ready", None, "NoExecute"),
                        ("node.kubernetes.io/unreachable", None, "NoExecute"),
                        ("node.kubernetes.io/disk-pressure", None, "NoSchedule"),
                        ("node.kubernetes.io/memory-pressure", None, "NoSchedule"),
                        ("node.kubernetes.io/pid-pressure", None, "NoSchedule"),
                        ("node.kubernetes.io/unschedulable", None, "NoSchedule"),
                    ],
                ),
                None,
            ),
            (
                "network unavailable with the host's network",
                PodSpec {
                    host_network: Some(true),
                    ..Default::default()
                },
                node(
                    "node-1",
                    &[],
                    &[("node.kubernetes.io/network-unavailable", None, "NoSchedule")],
                ),
                None,
            ),
            (
                "network unavailable without the host's network",
                PodSpec::default(),
                node(
                    "node-1",
                    &[],
                    &[("node.kubernetes.io/network-unavailable", None, "NoSchedule")],
                ),
                Some(UntoleratedTaint(
                    "node.kubernetes.io/network-unavailable:NoSchedule".to_string(),
                )),
            ),
        ];

        for (name, pod_spec, node, expected_reason) in test_cases {
            let ds = DaemonSet {
                spec: Some(DaemonSetSpec {
                    template: PodTemplateSpec {
                        spec: Some(pod_spec),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert_eq!(
                daemonset_node_skip_reason(&ds, &node),
                expected_reason,
                "{name}"
            );
        }

        assert_eq!(
            UntoleratedTaint("example.com/maintenance=true:NoSchedule".to_string()).to_string(),
            "the DaemonSet's Pods don't tolerate the node's taint \
            'example.com/maintenance=true:NoSchedule'"
        );
    }
}
