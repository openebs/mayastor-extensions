use crate::{
    common::{
        constants::{
            cordon_ana_check, drain_for_upgrade, io_engine_daemonset_name, product_train,
            AGENT_CORE_LABEL, IO_ENGINE_DAEMONSET_OBSERVED_GENERATION_TIMEOUT,
            IO_ENGINE_DAEMONSET_POLL_INTERVAL, IO_ENGINE_DAEMONSET_ROLLOUT_TIMEOUT,
            IO_ENGINE_LABEL, IO_ENGINE_POD_CREATION_TIMEOUT, IO_ENGINE_POD_TERMINATION_TIMEOUT,
            MAX_IO_ENGINE_PODS_IN_ERROR,
        },
        error::{
            DrainStorageNode, EmptyDaemonSetUid, EmptyPodSpec, EmptyStorageNodeSpec, GetPod,
            GetStorageNode, IoEngineGenerationNotObserved, IoEnginePodsNotRestartable,
            IoEngineRolloutIncomplete, ListStorageNodes, PodDelete, Result, StorageNodeUncordon,
            TooManyIoEnginePods,
        },
        kube::client as KubeClient,
        rest_client::RestClientSet,
    },
    upgrade_utils::{
        all_pods_are_ready, cordon_storage_node, daemonset_node_skip_reason,
        describe_not_ready_pods, join_with_limit, list_all_volumes, pod_is_controlled_by,
        pod_is_stuck_terminating, pod_is_terminating, pod_target_node, rebuild_result,
        stuck_terminating_pod, uncordon_storage_node, DaemonSetRollout, NodeSkipReason,
        RebuildResult,
    },
};
use constants::DS_CONTROLLER_REVISION_HASH_LABEL_KEY;
use k8s_openapi::{
    api::{
        apps::v1::DaemonSet,
        core::v1::{Node, Pod},
    },
    chrono::Utc,
};
use kube::{
    api::{DeleteParams, Preconditions},
    core::PartialObjectMeta,
    ResourceExt,
};
use openapi::models::CordonDrainState;
use snafu::ResultExt;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{error, info, warn};
use utils::{csi_node_nvme_ana, API_REST_LABEL, ETCD_LABEL};

/// Upgrade data plane by controlled restart of io-engine pods
pub async fn upgrade_data_plane(
    namespace: String,
    release_name: String,
    rest_endpoint: String,
    ha_is_enabled: bool,
) -> Result<()> {
    let io_engine_ds_name = io_engine_daemonset_name(release_name.as_str());

    // The helm upgrade doesn't wait for the DaemonSet controller to create the ControllerRevision
    // for the latest Pod template of the io-engine DaemonSet. The DaemonSet controller creates it
    // before it marks the latest generation of the DaemonSet as observed.
    let io_engine_ds = wait_for_io_engine_generation_to_be_observed(
        io_engine_ds_name.as_str(),
        namespace.as_str(),
    )
    .await?;

    // Only the ControllerRevisions and the Pods of this DaemonSet are considered, in case there
    // are other io-engine DaemonSets in the namespace.
    let io_engine_ds_uid = io_engine_ds.uid().ok_or(
        EmptyDaemonSetUid {
            name: io_engine_ds_name.clone(),
            namespace: namespace.clone(),
        }
        .build(),
    )?;

    let latest_io_engine_ctrl_rev_hash = KubeClient::latest_controller_revision_hash(
        namespace.clone(),
        Some(IO_ENGINE_LABEL.to_string()),
        None,
        Some(io_engine_ds_uid.clone()),
        DS_CONTROLLER_REVISION_HASH_LABEL_KEY.to_string(),
    )
    .await?;

    let io_engine = IoEngineDaemonSet {
        name: io_engine_ds_name,
        namespace: namespace.clone(),
        uid: io_engine_ds_uid,
        latest_revision_hash: latest_io_engine_ctrl_rev_hash,
    };

    // This makes data-plane upgrade idempotent. The io-engine Pods which the DaemonSet is yet to
    // create are not in the list of Pods, so the DaemonSet's rollout has to be complete as well.
    // The outdated io-engine Pods which are terminating, e.g. stuck on a finalizer, are already
    // restarted.
    if io_engine
        .list_outdated_pods()
        .await?
        .iter()
        .all(pod_is_terminating)
        && DaemonSetRollout::from(&io_engine_ds).is_complete()
    {
        info!("Skipping data-plane upgrade: All data-plane Pods are already upgraded");
        return Ok(());
    }

    // If here, then there is a need to proceed to data-plane upgrade.

    // Generate storage REST API client.
    let rest_client = RestClientSet::new_with_url(rest_endpoint)?;

    info!("Starting data-plane upgrade...");

    info!(
        "Trying to remove upgrade {product} Node Drain label from {product} Nodes, \
        if any left over from previous upgrade attempts...",
        product = product_train()
    );

    let storage_nodes_resp = rest_client
        .nodes_api()
        .get_nodes(None)
        .await
        .context(ListStorageNodes)?;
    let storage_nodes = storage_nodes_resp.body();
    for storage_node in storage_nodes {
        uncordon_drained_storage_node(storage_node.id.as_str(), &rest_client).await?;
    }

    // This is when the data-plane upgrade stopped making progress, i.e. when it last found no
    // io-engine Pods which it could restart, while the io-engine DaemonSet hadn't finished rolling
    // out.
    let mut stalled_since: Option<Instant> = None;
    loop {
        let initial_io_engine_pod_list: Vec<Pod> = io_engine.list_outdated_pods().await?;

        // These describe the io-engine Pods which are not restarted, because the io-engine
        // DaemonSet would not re-create them.
        let mut not_restartable_pods: Vec<String> = Vec::new();
        let mut restarted_pods = false;

        for pod in initial_io_engine_pod_list.iter() {
            // The list of Pods may be stale by now, because restarting a Pod takes a while.
            let Some(pod) = refresh_pod(pod, namespace.as_str()).await? else {
                continue;
            };

            // The Pod is already being deleted, e.g. it was restarted earlier, but it is stuck
            // terminating.
            if pod_is_terminating(&pod) {
                io_engine.wait_for_terminating_pod(&pod).await?;
                continue;
            }

            // Fetch the node name on which the io-engine pod is running
            let node_name = match pod
                .spec
                .as_ref()
                .ok_or(
                    EmptyPodSpec {
                        name: pod.name_any(),
                        namespace: namespace.clone(),
                    }
                    .build(),
                )?
                .node_name
                .as_deref()
            {
                Some(node_name) if !node_name.is_empty() => node_name,
                // The Pod is not scheduled to its node, so there is no io-engine running for it.
                _ => {
                    restarted_pods |=
                        restart_unscheduled_data_plane_pod(&pod, namespace.as_str()).await?;
                    continue;
                }
            };

            // Deleting such a Pod would leave its node without an io-engine.
            if let Some(description) = io_engine
                .describe_not_restartable_pod(&pod, node_name)
                .await?
            {
                not_restartable_pods.push(description);
                continue;
            }

            // Validate the control plane pod is up and running before we start.
            verify_control_plane_is_running(namespace.clone()).await?;

            info!(
                pod.name = %pod.name_any(),
                node.name = %node_name,
                "Starting upgrade for the data-plane pod"
            );

            // Wait for any rebuild to complete
            wait_for_rebuild(node_name, &rest_client).await?;

            if is_node_drainable(ha_is_enabled, node_name, &rest_client).await? {
                // Issue node drain command if NVMe Ana is enabled.
                drain_storage_node(node_name, &rest_client).await?;
            }

            // Check again, because waiting for rebuilds and draining the node take a while.
            if let Some(description) = io_engine
                .describe_not_restartable_pod(&pod, node_name)
                .await?
            {
                uncordon_drained_storage_node(node_name, &rest_client).await?;
                not_restartable_pods.push(description);
                continue;
            }

            // restart the data plane pod. The precondition makes sure that a different Pod with
            // the same name is not deleted.
            delete_data_plane_pod(
                node_name,
                &pod,
                namespace.as_str(),
                Preconditions {
                    uid: pod.uid(),
                    resource_version: None,
                },
            )
            .await?;

            // validate the new pod is up and running
            io_engine
                .verify_data_plane_pod_is_running(node_name)
                .await?;

            // Uncordon the drained node
            uncordon_drained_storage_node(node_name, &rest_client).await?;

            restarted_pods = true;
        }

        if restarted_pods {
            stalled_since = None;
            info!(
                "Checking to see if new {} Nodes have been added to the cluster, which require upgrade",
                product_train()
            );
            continue;
        }

        // The io-engine Pods which the DaemonSet is yet to create, and the up-to-date io-engine
        // Pods which are not available, are not in the list of Pods.
        let rollout = DaemonSetRollout::from(&io_engine.get().await?);

        // Infinite loop exit. The outdated io-engine Pods which are terminating, e.g. stuck on a
        // finalizer, are already restarted.
        if initial_io_engine_pod_list.iter().all(pod_is_terminating) && rollout.is_complete() {
            break;
        }

        let stall_start = *stalled_since.get_or_insert_with(Instant::now);
        if stall_start.elapsed() >= IO_ENGINE_DAEMONSET_ROLLOUT_TIMEOUT {
            let timeout =
                humantime::format_duration(IO_ENGINE_DAEMONSET_ROLLOUT_TIMEOUT).to_string();
            if !not_restartable_pods.is_empty() {
                return IoEnginePodsNotRestartable {
                    name: io_engine.name.clone(),
                    namespace: io_engine.namespace.clone(),
                    timeout,
                    pods: join_with_limit(
                        not_restartable_pods.as_slice(),
                        MAX_IO_ENGINE_PODS_IN_ERROR,
                    ),
                }
                .fail();
            }

            return IoEngineRolloutIncomplete {
                name: io_engine.name.clone(),
                namespace: io_engine.namespace.clone(),
                timeout,
                rollout: rollout.to_string(),
                not_ready_pods: io_engine.describe_not_ready_io_engine_pods().await,
            }
            .fail();
        }

        if not_restartable_pods.is_empty() {
            info!(
                %rollout,
                "Waiting for the io-engine DaemonSet '{}' to finish rolling out",
                io_engine.name
            );
        } else {
            info!(
                %rollout,
                "Waiting to restart io-engine Pods which the io-engine DaemonSet '{}' would not \
                re-create on their nodes",
                io_engine.name
            );
        }
        sleep(IO_ENGINE_DAEMONSET_POLL_INTERVAL).await;
    }

    info!("Successfully upgraded data-plane!");

    Ok(())
}

/// The io-engine DaemonSet of the helm release which is being upgraded.
struct IoEngineDaemonSet {
    name: String,
    namespace: String,
    uid: String,
    /// The hash of the DaemonSet's latest ControllerRevision, i.e. of its latest Pod template.
    latest_revision_hash: String,
}

impl IoEngineDaemonSet {
    /// GETs the DaemonSet.
    async fn get(&self) -> Result<DaemonSet> {
        KubeClient::get_daemonset(self.name.as_str(), self.namespace.as_str()).await
    }

    /// Lists the Pods of the DaemonSet which match the label selector and the field selector.
    async fn list_pods(
        &self,
        label_selector: String,
        field_selector: Option<String>,
    ) -> Result<Vec<Pod>> {
        let mut pods =
            KubeClient::list_pods(self.namespace.clone(), Some(label_selector), field_selector)
                .await?;
        pods.retain(|pod| pod_is_controlled_by(pod, self.uid.as_str()));
        Ok(pods)
    }

    /// Lists the Pods of the DaemonSet which are not created from its latest Pod template.
    async fn list_outdated_pods(&self) -> Result<Vec<Pod>> {
        self.list_pods(
            format!(
                "{IO_ENGINE_LABEL},{DS_CONTROLLER_REVISION_HASH_LABEL_KEY}!={}",
                self.latest_revision_hash
            ),
            None,
        )
        .await
    }

    /// Lists the Pods of the DaemonSet which run on, or are meant to run on, the node. This
    /// includes the Pods which are terminating.
    async fn node_pods(&self, node_name: &str) -> Result<Vec<Pod>> {
        let mut pods = self.list_pods(IO_ENGINE_LABEL.to_string(), None).await?;
        pods.retain(|pod| pod_target_node(pod).as_deref() == Some(node_name));
        Ok(pods)
    }

    /// Returns the reason for which the DaemonSet doesn't run a Pod on the node, or None if it
    /// does.
    async fn node_skip_reason(&self, node_name: &str) -> Result<Option<NodeSkipReason>> {
        let Some(node) = KubeClient::get_node(node_name).await? else {
            return Ok(Some(NodeSkipReason::NodeNotFound));
        };

        Ok(daemonset_node_skip_reason(&self.get().await?, &node))
    }

    /// Describes the Pod of the DaemonSet if the DaemonSet would not re-create it on its node,
    /// e.g. because the node has a NoSchedule taint which the DaemonSet's Pods don't tolerate.
    async fn describe_not_restartable_pod(
        &self,
        pod: &Pod,
        node_name: &str,
    ) -> Result<Option<String>> {
        let Some(reason) = self.node_skip_reason(node_name).await? else {
            return Ok(None);
        };

        warn!(
            pod.name = %pod.name_any(),
            node.name = %node_name,
            %reason,
            "Not restarting the data-plane pod, because the io-engine DaemonSet would not re-create it"
        );
        Ok(Some(format!(
            "{} (node: {node_name}, reason: {reason})",
            pod.name_any()
        )))
    }

    /// Describes the Pods of the DaemonSet which are not Ready. This is meant for error messages,
    /// so a failure to list the Pods is logged instead of being returned.
    async fn describe_not_ready_io_engine_pods(&self) -> String {
        match self.list_pods(IO_ENGINE_LABEL.to_string(), None).await {
            Ok(pods) => describe_not_ready_pods(pods.as_slice(), MAX_IO_ENGINE_PODS_IN_ERROR),
            Err(error) => {
                warn!(%error, "Failed to list the io-engine Pods");
                "unknown".to_string()
            }
        }
    }

    /// Waits for the DaemonSet to replace its Pod which is already terminating, if the Pod is
    /// scheduled to its node. This doesn't wait for a Pod which is stuck terminating, e.g. because
    /// a finalizer is not removed from it.
    async fn wait_for_terminating_pod(&self, pod: &Pod) -> Result<()> {
        let target_node = pod_target_node(pod).unwrap_or_else(|| "unknown".to_string());
        if pod_is_stuck_terminating(pod, IO_ENGINE_POD_TERMINATION_TIMEOUT, Utc::now()) {
            warn!(
                pod.name = %pod.name_any(),
                node.name = %target_node,
                finalizers = ?pod.finalizers(),
                "Skipping the data-plane pod, because it is stuck terminating"
            );
            return Ok(());
        }

        let node_name = pod
            .spec
            .as_ref()
            .and_then(|spec| spec.node_name.as_deref())
            .filter(|node_name| !node_name.is_empty());
        match node_name {
            Some(node_name) => {
                info!(
                    pod.name = %pod.name_any(),
                    node.name = %node_name,
                    "Waiting for the data-plane pod which is already terminating to be replaced"
                );
                self.verify_data_plane_pod_is_running(node_name).await
            }
            // There is no io-engine running for a Pod which is not scheduled to its node.
            None => {
                info!(
                    pod.name = %pod.name_any(),
                    node.name = %target_node,
                    "Waiting for the data-plane pod which is not scheduled to its node to be deleted"
                );
                Ok(())
            }
        }
    }

    /// Waits for the up-to-date Pod of the DaemonSet on the node to be Ready. This stops waiting
    /// if the DaemonSet would not create a Pod on the node, and there are no Pods of the DaemonSet
    /// left on the node, e.g. because the node is removed from the cluster. This also stops
    /// waiting, and logs an error, if the DaemonSet doesn't create a Pod on the node in time, e.g.
    /// because an admission webhook rejects it, or if a Pod on the node is stuck terminating, so
    /// that the rest of the io-engine Pods are restarted.
    async fn verify_data_plane_pod_is_running(&self, node_name: &str) -> Result<()> {
        let (name, namespace) = (self.name.as_str(), self.namespace.as_str());
        // This is when the DaemonSet was last found to have no Pods on the node.
        let mut no_pods_since: Option<Instant> = None;
        // Validate the new pod is up and running
        info!(node.name = %node_name, "Waiting for data-plane Pod to come to Ready state");
        while !self.data_plane_pod_is_running(node_name).await? {
            let node_pods = self.node_pods(node_name).await?;
            if node_pods.is_empty() {
                if let Some(reason) = self.node_skip_reason(node_name).await? {
                    warn!(
                        node.name = %node_name,
                        %reason,
                        "Not waiting for a data-plane Pod on the node, because the io-engine DaemonSet would not create one"
                    );
                    return Ok(());
                }

                let no_pods_start = *no_pods_since.get_or_insert_with(Instant::now);
                if no_pods_start.elapsed() >= IO_ENGINE_POD_CREATION_TIMEOUT {
                    error!(
                        node.name = %node_name,
                        timeout = %humantime::format_duration(IO_ENGINE_POD_CREATION_TIMEOUT),
                        "The io-engine DaemonSet '{name}' has not created a data-plane Pod on the \
                        node, e.g. because an admission webhook or policy rejects it, see the \
                        DaemonSet's events with 'kubectl -n {namespace} describe daemonset \
                        {name}'. Moving on to the rest of the data-plane Pods"
                    );
                    return Ok(());
                }
            } else {
                no_pods_since = None;
                if let Some(pod) = stuck_terminating_pod(
                    node_pods.as_slice(),
                    self.latest_revision_hash.as_str(),
                    IO_ENGINE_POD_TERMINATION_TIMEOUT,
                    Utc::now(),
                ) {
                    error!(
                        pod.name = %pod.name_any(),
                        node.name = %node_name,
                        finalizers = ?pod.finalizers(),
                        timeout = %humantime::format_duration(IO_ENGINE_POD_TERMINATION_TIMEOUT),
                        "The data-plane pod is stuck terminating, e.g. because of a finalizer, or \
                        because its node is unreachable, so the io-engine DaemonSet '{name}' may \
                        not create a new data-plane Pod on the node. Moving on to the rest of the \
                        data-plane Pods"
                    );
                    return Ok(());
                }
            }
            sleep(IO_ENGINE_DAEMONSET_POLL_INTERVAL).await;
        }
        Ok(())
    }

    /// Validate if io-engine DaemonSet Pod is running.
    async fn data_plane_pod_is_running(&self, node: &str) -> Result<bool> {
        let node_name_pod_field = format!("spec.nodeName={node}");
        let pod_label = format!(
            "{IO_ENGINE_LABEL},{DS_CONTROLLER_REVISION_HASH_LABEL_KEY}={}",
            self.latest_revision_hash
        );

        let mut pod_list: Vec<Pod> = self.list_pods(pod_label, Some(node_name_pod_field)).await?;
        // A Pod which is terminating is not the node's io-engine any more, even if it is stuck
        // terminating.
        pod_list.retain(|pod| !pod_is_terminating(pod));

        if pod_list.is_empty() {
            return Ok(false);
        }

        if pod_list.len() != 1 {
            return TooManyIoEnginePods { node_name: node }.fail();
        }

        Ok(all_pods_are_ready(pod_list))
    }
}

/// Waits for the DaemonSet controller to observe the latest generation of the io-engine DaemonSet,
/// and returns the DaemonSet.
async fn wait_for_io_engine_generation_to_be_observed(
    name: &str,
    namespace: &str,
) -> Result<DaemonSet> {
    let wait_start = Instant::now();
    loop {
        let ds = KubeClient::get_daemonset(name, namespace).await?;
        let rollout = DaemonSetRollout::from(&ds);
        if rollout.generation_is_observed() {
            return Ok(ds);
        }

        if wait_start.elapsed() >= IO_ENGINE_DAEMONSET_OBSERVED_GENERATION_TIMEOUT {
            return IoEngineGenerationNotObserved {
                name,
                namespace,
                timeout: humantime::format_duration(
                    IO_ENGINE_DAEMONSET_OBSERVED_GENERATION_TIMEOUT,
                )
                .to_string(),
                rollout: rollout.to_string(),
            }
            .fail();
        }

        info!(
            %rollout,
            "Waiting for the DaemonSet controller to observe the latest spec of the io-engine DaemonSet '{name}'"
        );
        sleep(IO_ENGINE_DAEMONSET_POLL_INTERVAL).await;
    }
}

/// Returns the latest state of the Pod, or None if the Pod no longer exists. A Pod with the same
/// name and a different UID is a different Pod.
async fn refresh_pod(pod: &Pod, namespace: &str) -> Result<Option<Pod>> {
    let pod_name = pod.name_any();
    let latest_pod = KubeClient::pods_api(namespace)
        .await?
        .get_opt(pod_name.as_str())
        .await
        .context(GetPod {
            pod_name,
            pod_namespace: namespace.to_string(),
        })?;

    Ok(latest_pod.filter(|latest_pod| latest_pod.uid() == pod.uid()))
}

/// Deletes an io-engine Pod which is not scheduled to its node, so that the DaemonSet re-creates
/// it from the latest Pod template. There is no io-engine running for such a Pod, so there is no
/// need to wait for volume rebuilds, or to drain its node. Returns true if the Pod is deleted.
async fn restart_unscheduled_data_plane_pod(pod: &Pod, namespace: &str) -> Result<bool> {
    let target_node = pod_target_node(pod).unwrap_or_else(|| "unknown".to_string());
    info!(
        pod.name = %pod.name_any(),
        node.name = %target_node,
        "Restarting the data-plane pod which is not scheduled to its node"
    );
    // The preconditions make sure that the Pod is deleted only if it hasn't changed since it was
    // fetched, e.g. it hasn't been scheduled to its node since.
    delete_data_plane_pod(
        target_node.as_str(),
        pod,
        namespace,
        Preconditions {
            uid: pod.uid(),
            resource_version: pod.resource_version(),
        },
    )
    .await
}

/// Uncordon storage Node by removing drain label.
async fn uncordon_drained_storage_node(node_id: &str, rest_client: &RestClientSet) -> Result<()> {
    let drain_label_for_upgrade: String = drain_for_upgrade();
    let sleep_duration = Duration::from_secs(1_u64);
    loop {
        let storage_node =
            rest_client
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
            Some(CordonDrainState::drainedstate(drain_state))
                if drain_state.drainlabels.contains(&drain_label_for_upgrade) =>
            {
                rest_client
                    .nodes_api()
                    .delete_node_cordon(node_id, &drain_for_upgrade())
                    .await
                    .context(StorageNodeUncordon {
                        node_id: node_id.to_string(),
                    })?;

                info!(node.id = %node_id,
                    label = %drain_for_upgrade(),
                    "Removed drain label from {} Node", product_train()
                );
            }
            _ => return Ok(()),
        }
        sleep(sleep_duration).await;
    }
}

/// Issue delete command on dataplane pods. The Pod is deleted only if it matches the
/// preconditions. Returns false if the Pod no longer exists, or if it doesn't match the
/// preconditions any more.
async fn delete_data_plane_pod(
    node_name: &str,
    pod: &Pod,
    namespace: &str,
    preconditions: Preconditions,
) -> Result<bool> {
    let k8s_pods_api = KubeClient::pods_api(namespace).await?;

    // Deleting the io-engine pod
    let pod_name = pod.name_any();
    info!(
        pod.name = pod_name.clone(),
        node.name = node_name,
        "Deleting the pod"
    );
    let delete_params = DeleteParams {
        preconditions: Some(preconditions),
        ..Default::default()
    };
    match k8s_pods_api.delete(pod_name.as_str(), &delete_params).await {
        Ok(_) => {
            info!(node.name = %node_name, "Pod delete command issued");
            Ok(true)
        }
        // The Pod is not found, or it doesn't match the preconditions.
        Err(kube::Error::Api(response)) if matches!(response.code, 404 | 409) => {
            info!(
                pod.name = %pod_name,
                node.name = %node_name,
                reason = %response.message,
                "The pod was not deleted, because it no longer exists or it has changed"
            );
            Ok(false)
        }
        Err(error) => Err(error).context(PodDelete {
            name: pod_name,
            node: node_name.to_string(),
        }),
    }
}

/// Wait for the rebuild to complete if any.
async fn wait_for_rebuild(node_name: &str, rest_client: &RestClientSet) -> Result<()> {
    // Wait for 60 seconds for any rebuilds to kick in.
    sleep(Duration::from_secs(60_u64)).await;

    let mut result = RebuildResult::default();
    loop {
        let rebuild = rebuild_result(rest_client, &mut result.discarded_volumes, node_name).await?;

        if rebuild.rebuilding {
            info!(node.name = %node_name, "Waiting for volume rebuilds to complete");
            sleep(Duration::from_secs(10_u64)).await;
        } else {
            break;
        }
    }
    info!(node.name = %node_name, "No volume rebuilds in progress");
    Ok(())
}

/// Issue the node drain command on the node.
async fn drain_storage_node(node_id: &str, rest_client: &RestClientSet) -> Result<()> {
    let drain_label_for_upgrade: String = drain_for_upgrade();
    let sleep_duration = Duration::from_secs(5_u64);
    loop {
        let storage_node =
            rest_client
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
            Some(CordonDrainState::drainingstate(drain_state))
                if drain_state.drainlabels.contains(&drain_label_for_upgrade) =>
            {
                info!(node.id = %node_id, "Waiting for {} Node drain to complete", product_train());
                // Wait for node drain to complete.
                sleep(sleep_duration).await;
            }
            Some(CordonDrainState::drainedstate(drain_state))
                if drain_state.drainlabels.contains(&drain_label_for_upgrade) =>
            {
                info!(node.id = %node_id, "Drain completed for {} Node", product_train());
                return Ok(());
            }
            _ => {
                rest_client
                    .nodes_api()
                    .put_node_drain(node_id, &drain_for_upgrade())
                    .await
                    .context(DrainStorageNode {
                        node_id: node_id.to_string(),
                    })?;

                info!(node.id = %node_id, "Drain started for {} Node", product_train());
            }
        }
    }
}

async fn verify_control_plane_is_running(namespace: String) -> Result<()> {
    let duration = Duration::from_secs(3_u64);
    while !control_plane_is_running(namespace.clone()).await? {
        sleep(duration).await;
    }

    Ok(())
}

/// Validate if control-plane pods are running -- etcd, agent-core, api-rest.
async fn control_plane_is_running(namespace: String) -> Result<bool> {
    let pod_list: Vec<Pod> =
        KubeClient::list_pods(namespace.clone(), Some(AGENT_CORE_LABEL.to_string()), None).await?;
    let core_is_ready = all_pods_are_ready(pod_list);

    let pod_list: Vec<Pod> =
        KubeClient::list_pods(namespace.clone(), Some(API_REST_LABEL.to_string()), None).await?;
    let rest_is_ready = all_pods_are_ready(pod_list);

    let pod_list: Vec<Pod> =
        KubeClient::list_pods(namespace, Some(ETCD_LABEL.to_string()), None).await?;
    let etcd_is_ready = all_pods_are_ready(pod_list);

    Ok(core_is_ready && rest_is_ready && etcd_is_ready)
}

/// Decides if a specific node is drainable during data-plane upgrade, based on multiple factors:
/// 1. Helm value state for HA feature
/// 2. NVMe ANA is not enabled for frontend nodes of a volume target.
async fn is_node_drainable(
    ha_is_enabled: bool,
    node_name: &str,
    rest_client: &RestClientSet,
) -> Result<bool> {
    if !ha_is_enabled {
        info!("HA is disabled, disabling drain for node {}", node_name);
        return Ok(false);
    }

    let ana_disabled_label = format!("{}=false", csi_node_nvme_ana());
    let ana_disabled_nodes =
        KubeClient::list_nodes_metadata(Some(ana_disabled_label), None).await?;

    if ana_disabled_nodes.is_empty() {
        info!(
            "There are no ANA-incapable nodes in this cluster, it is safe to drain node {}",
            node_name
        );
        // If there are no ANA-disabled nodes, it should be safe to drain.
        return Ok(true);
    }

    cordon_storage_node(node_name, &cordon_ana_check(), rest_client).await?;
    let result = frontend_nodes_ana_check(node_name, ana_disabled_nodes, rest_client).await;
    uncordon_storage_node(node_name, &cordon_ana_check(), rest_client).await?;

    result
}

/// Returns true if any of the frontend nodes of the target at node_name have the label for
/// ANA-incapability.
async fn frontend_nodes_ana_check(
    node_name: &str,
    ana_disabled_nodes: Vec<PartialObjectMeta<Node>>,
    rest_client: &RestClientSet,
) -> Result<bool> {
    let volumes = list_all_volumes(rest_client).await?;
    let frontend_nodes = volumes.into_iter().fold(vec![], |mut acc, volume| {
        if let Some(target) = volume.spec.target {
            // Check to see if the target for the volume is on the node which
            // we're trying to upgrade.
            if target.node.eq(node_name) {
                if let Some(frontend_nodes) = target.frontend_nodes {
                    frontend_nodes.into_iter().for_each(|n| acc.push(n.name));
                }
            }
        }
        acc
    });

    if ana_disabled_nodes
        .into_iter()
        .any(|node| frontend_nodes.contains(&node.name_any()))
    {
        // There is a frontend_node which has a label saying ANA is absent for that
        // node. Not safe to drain.
        info!(
            "At least one frontend_node for a volume-target at node {} \
is ANA-incapable, disabling drain for node {}",
            node_name, node_name
        );
        return Ok(false);
    }

    // All of the targets' volumes' frontend_nodes are not amongst the nodes with ANA
    // disabled. Safe to drain this node.
    info!(
        "No frontend_nodes for node {} are ANA-incapable, it is safe to drain node {}",
        node_name, node_name
    );

    Ok(true)
}
