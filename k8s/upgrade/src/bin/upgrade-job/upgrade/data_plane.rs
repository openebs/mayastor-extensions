use crate::{
    common::{
        constants::{
            AGENT_CORE_LABEL, CHART_VERSION_LABEL_KEY, CORDON_FOR_ANA_CHECK, DRAIN_FOR_UPGRADE,
            IO_ENGINE_LABEL, PRODUCT,
        },
        error::{
            DrainStorageNode, EmptyPodNodeName, EmptyPodSpec, EmptyStorageNodeSpec, GetStorageNode,
            ListNodesWithLabel, ListStorageNodes, PodDelete, Result, StorageNodeUncordon,
            TooManyIoEnginePods,
        },
        kube_client as KubeClient,
        rest_client::RestClientSet,
    },
    upgrade::utils::{
        all_pods_are_ready, cordon_storage_node, list_all_volumes, rebuild_result,
        uncordon_storage_node, RebuildResult,
    },
};
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::{
    api::{Api, DeleteParams, ListParams, ObjectList},
    ResourceExt,
};
use openapi::models::CordonDrainState;
use snafu::ResultExt;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;
use utils::{API_REST_LABEL, CSI_NODE_NVME_ANA, ETCD_LABEL};

/// Upgrade data plane by controlled restart of io-engine pods
pub(crate) async fn upgrade_data_plane(
    namespace: String,
    rest_endpoint: String,
    upgrade_target_version: String,
    ha_is_enabled: bool,
    yet_to_upgrade_io_engine_label: String,
    yet_to_upgrade_io_engine_pods: Vec<Pod>,
) -> Result<()> {
    // This makes data-plane upgrade idempotent.
    if yet_to_upgrade_io_engine_pods.is_empty() {
        info!("Skipping data-plane upgrade: All data-plane Pods are already upgraded");
        return Ok(());
    }

    // If here, then there is a need to proceed to data-plane upgrade.

    // Generate storage REST API client.
    let rest_client = RestClientSet::new_with_url(rest_endpoint)?;

    info!("Starting data-plane upgrade...");

    info!(
        "Trying to remove upgrade {PRODUCT} Node Drain label from {PRODUCT} Nodes, \
        if any left over from previous upgrade attempts..."
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

    loop {
        let initial_io_engine_pod_list: Vec<Pod> = KubeClient::list_pods(
            namespace.clone(),
            Some(yet_to_upgrade_io_engine_label.clone()),
            None,
        )
        .await?;

        // Infinite loop exit.
        if initial_io_engine_pod_list.is_empty() {
            break;
        }

        for pod in initial_io_engine_pod_list.iter() {
            // Validate the control plane pod is up and running before we start.
            verify_control_plane_is_running(namespace.clone(), &upgrade_target_version).await?;

            // Fetch the node name on which the io-engine pod is running
            let node_name = pod
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
                .as_ref()
                .ok_or(
                    EmptyPodNodeName {
                        name: pod.name_any(),
                        namespace: namespace.clone(),
                    }
                    .build(),
                )?
                .as_str();

            info!(
                pod.name = %pod.name_any(),
                node.name = %node_name,
                "Starting upgrade for the data-plane pod"
            );

            // Wait for any rebuild to complete
            wait_for_rebuild(node_name, &rest_client).await?;

            if is_node_drainable(
                ha_is_enabled,
                node_name,
                KubeClient::nodes_api().await?,
                &rest_client,
            )
            .await?
            {
                // Issue node drain command if NVMe Ana is enabled.
                drain_storage_node(node_name, &rest_client).await?;
            }

            // restart the data plane pod
            delete_data_plane_pod(node_name, pod, namespace.clone()).await?;

            // validate the new pod is up and running
            verify_data_plane_pod_is_running(node_name, namespace.clone(), &upgrade_target_version)
                .await?;

            // Uncordon the drained node
            uncordon_drained_storage_node(node_name, &rest_client).await?;
        }

        info!("Checking to see if new {PRODUCT} Nodes have been added to the cluster, which require upgrade");
    }

    info!("Successfully upgraded data-plane!");

    Ok(())
}

/// Uncordon storage Node by removing drain label.
async fn uncordon_drained_storage_node(node_id: &str, rest_client: &RestClientSet) -> Result<()> {
    let drain_label_for_upgrade: String = DRAIN_FOR_UPGRADE.to_string();
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
                    .delete_node_cordon(node_id, DRAIN_FOR_UPGRADE)
                    .await
                    .context(StorageNodeUncordon {
                        node_id: node_id.to_string(),
                    })?;

                info!(node.id = %node_id,
                    label = %DRAIN_FOR_UPGRADE,
                    "Removed drain label from {PRODUCT} Node"
                );
            }
            _ => return Ok(()),
        }
        sleep(sleep_duration).await;
    }
}

/// Issue delete command on dataplane pods.
async fn delete_data_plane_pod(node_name: &str, pod: &Pod, namespace: String) -> Result<()> {
    let k8s_pods_api = KubeClient::pods_api(namespace.as_str()).await?;

    // Deleting the io-engine pod
    let pod_name = pod.name_any();
    info!(
        pod.name = pod_name.clone(),
        node.name = node_name,
        "Deleting the pod"
    );
    k8s_pods_api
        .delete(pod_name.as_str(), &DeleteParams::default())
        .await
        .context(PodDelete {
            name: pod_name,
            node: node_name.to_string(),
        })?;
    info!(node.name = %node_name, "Pod delete command issued");
    Ok(())
}

/// Wait for all the node drain process to complete.
async fn verify_data_plane_pod_is_running(
    node_name: &str,
    namespace: String,
    upgrade_target_version: &String,
) -> Result<()> {
    let duration = Duration::from_secs(5_u64);
    // Validate the new pod is up and running
    info!(node.name = %node_name, "Waiting for data-plane Pod to come to Ready state");
    while !data_plane_pod_is_running(node_name, namespace.clone(), upgrade_target_version).await? {
        sleep(duration).await;
    }
    Ok(())
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
    let drain_label_for_upgrade: String = DRAIN_FOR_UPGRADE.to_string();
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
                info!(node.id = %node_id, "Waiting for {PRODUCT} Node drain to complete");
                // Wait for node drain to complete.
                sleep(sleep_duration).await;
            }
            Some(CordonDrainState::drainedstate(drain_state))
                if drain_state.drainlabels.contains(&drain_label_for_upgrade) =>
            {
                info!(node.id = %node_id, "Drain completed for {PRODUCT} Node");
                return Ok(());
            }
            _ => {
                rest_client
                    .nodes_api()
                    .put_node_drain(node_id, DRAIN_FOR_UPGRADE)
                    .await
                    .context(DrainStorageNode {
                        node_id: node_id.to_string(),
                    })?;

                info!(node.id = %node_id, "Drain started for {PRODUCT} Node");
            }
        }
    }
}

/// Validate if io-engine DaemonSet Pod is running.
async fn data_plane_pod_is_running(
    node: &str,
    namespace: String,
    upgrade_target_version: &String,
) -> Result<bool> {
    let node_name_pod_field = format!("spec.nodeName={node}");
    let pod_label = format!("{IO_ENGINE_LABEL},{CHART_VERSION_LABEL_KEY}={upgrade_target_version}");

    let pod_list: Vec<Pod> =
        KubeClient::list_pods(namespace, Some(pod_label), Some(node_name_pod_field)).await?;

    if pod_list.is_empty() {
        return Ok(false);
    }

    if pod_list.len() != 1 {
        return TooManyIoEnginePods { node_name: node }.fail();
    }

    Ok(all_pods_are_ready(pod_list))
}

async fn verify_control_plane_is_running(
    namespace: String,
    upgrade_target_version: &String,
) -> Result<()> {
    let duration = Duration::from_secs(3_u64);
    while !control_plane_is_running(namespace.clone(), upgrade_target_version).await? {
        sleep(duration).await;
    }

    Ok(())
}

/// Validate if control-plane pods are running -- etcd, agent-core, api-rest.
async fn control_plane_is_running(
    namespace: String,
    upgrade_target_version: &String,
) -> Result<bool> {
    let agent_core_selector_label =
        format!("{AGENT_CORE_LABEL},{CHART_VERSION_LABEL_KEY}={upgrade_target_version}");
    let pod_list: Vec<Pod> =
        KubeClient::list_pods(namespace.clone(), Some(agent_core_selector_label), None).await?;
    let core_is_ready = all_pods_are_ready(pod_list);

    let api_rest_selector_label =
        format!("{API_REST_LABEL},{CHART_VERSION_LABEL_KEY}={upgrade_target_version}");
    let pod_list: Vec<Pod> =
        KubeClient::list_pods(namespace.clone(), Some(api_rest_selector_label), None).await?;
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
    k8s_nodes_api: Api<Node>,
    rest_client: &RestClientSet,
) -> Result<bool> {
    if !ha_is_enabled {
        info!("HA is disabled, disabling drain for node {}", node_name);
        return Ok(false);
    }

    let ana_disabled_label = format!("{CSI_NODE_NVME_ANA}=false");
    let ana_disabled_filter = ListParams::default().labels(ana_disabled_label.as_str());
    let ana_disabled_nodes =
        k8s_nodes_api
            .list(&ana_disabled_filter)
            .await
            .context(ListNodesWithLabel {
                label: ana_disabled_label,
            })?;

    if ana_disabled_nodes.items.is_empty() {
        info!(
            "There are no ANA-incapable nodes in this cluster, it is safe to drain node {}",
            node_name
        );
        // If there are no ANA-disabled nodes, it should be safe to drain.
        return Ok(true);
    }

    cordon_storage_node(node_name, CORDON_FOR_ANA_CHECK, rest_client).await?;
    let result = frontend_nodes_ana_check(node_name, ana_disabled_nodes, rest_client).await;
    uncordon_storage_node(node_name, CORDON_FOR_ANA_CHECK, rest_client).await?;

    result
}

/// Returns true if any of the frontend nodes of the target at node_name have the label for
/// ANA-incapability.
async fn frontend_nodes_ana_check(
    node_name: &str,
    ana_disabled_nodes: ObjectList<Node>,
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
