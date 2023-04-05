use crate::{
    common::{
        constants::{AGENT_CORE_LABEL, DRAIN_FOR_UPGRADE, IO_ENGINE_LABEL, PRODUCT},
        error::{
            DrainStorageNode, EmptyPodNodeName, EmptyPodSpec, ListPodsWithLabel, PodDelete, Result,
            StorageNodeUncordon, ValidatingPodReadyStatus,
        },
        kube_client::KubeClientSet,
        rest_client::RestClientSet,
    },
    upgrade::utils::{all_pods_are_ready, is_draining, is_rebuilding},
};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{DeleteParams, ListParams, ObjectList},
    ResourceExt,
};
use snafu::{ensure, ResultExt};
use std::time::Duration;
use utils::{API_REST_LABEL, ETCD_LABEL};

/// Upgrade data plane by controlled restart of io-engine pods
pub(crate) async fn upgrade_data_plane(namespace: String, rest_endpoint: String) -> Result<()> {
    let k8s_client = KubeClientSet::builder()
        .with_namespace(namespace.clone())
        .build()
        .await?;

    let rest_client = RestClientSet::new_with_url(rest_endpoint)?;

    let io_engine_listparam = ListParams::default().labels(IO_ENGINE_LABEL);
    let namespace = namespace.clone();
    let initial_io_engine_pod_list: ObjectList<Pod> = k8s_client
        .pods_api()
        .list(&io_engine_listparam)
        .await
        .context(ListPodsWithLabel {
            label: IO_ENGINE_LABEL.to_string(),
            namespace: namespace.clone(),
        })?;
    for pod in initial_io_engine_pod_list.iter() {
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

        tracing::info!(
            pod.name = %pod.name_any(),
            node.name = %node_name,
            "Upgrade starting for data-plane pod"
        );

        // Issue node drain command
        issue_node_drain(node_name, &rest_client).await?;

        // Wait for node drain to complete across the cluster.
        wait_node_drain(&rest_client).await?;

        // Wait for any rebuild to complete.
        wait_for_rebuild(&rest_client).await?;

        // restart the data plane pod
        restart_data_plane(node_name, pod, &k8s_client).await?;

        // Uncordon the drained node
        uncordon_node(node_name, &rest_client).await?;

        // validate the new pod is up and running
        verify_data_plane_pod_is_running(node_name, namespace.clone(), &k8s_client).await?;

        // Validate the control plane pod is up and running
        is_control_plane_running(namespace.clone(), &k8s_client).await?;
    }
    Ok(())
}

/// Uncordon storage Node.
async fn uncordon_node(node_name: &str, rest_client: &RestClientSet) -> Result<()> {
    rest_client
        .nodes_api()
        .delete_node_cordon(node_name, DRAIN_FOR_UPGRADE)
        .await
        .context(StorageNodeUncordon {
            node_name: node_name.to_string(),
        })?;

    tracing::info!(node.name = node_name, "{PRODUCT} Node is uncordoned");

    Ok(())
}

/// Issue delete command on dataplane pods.
async fn restart_data_plane(node_name: &str, pod: &Pod, k8s_client: &KubeClientSet) -> Result<()> {
    // Deleting the io-engine pod
    let pod_name = pod.name_any();
    tracing::info!(
        pod.name = pod_name.clone(),
        node.name = node_name,
        "Deleting the pod"
    );
    k8s_client
        .pods_api()
        .delete(pod_name.as_str(), &DeleteParams::default())
        .await
        .context(PodDelete {
            name: pod_name,
            node: node_name.to_string(),
        })?;
    Ok(())
}

/// Wait for the data plane pod to come up on the given node.
async fn wait_node_drain(rest_client: &RestClientSet) -> Result<()> {
    while is_draining(rest_client).await? {
        tokio::time::sleep(Duration::from_secs(10_u64)).await;
    }
    Ok(())
}

/// Wait for all the node drain process to complete.
async fn verify_data_plane_pod_is_running(
    node_name: &str,
    namespace: String,
    k8s_client: &KubeClientSet,
) -> Result<()> {
    // Validate the new pod is up and running
    while is_data_plane_pod_running(node_name, namespace.clone(), k8s_client).await? {
        tokio::time::sleep(Duration::from_secs(10_u64)).await;
    }
    Ok(())
}

/// Wait for the rebuild to complete if any.
async fn wait_for_rebuild(rest_client: &RestClientSet) -> Result<()> {
    // Wait for 60 seconds for any rebuilds to kick in.
    tokio::time::sleep(Duration::from_secs(60_u64)).await;
    while is_rebuilding(rest_client).await? {
        tokio::time::sleep(Duration::from_secs(10_u64)).await;
    }
    Ok(())
}

/// Issue the node drain command on the node.
async fn issue_node_drain(node_name: &str, rest_client: &RestClientSet) -> Result<()> {
    rest_client
        .nodes_api()
        .put_node_drain(node_name, DRAIN_FOR_UPGRADE)
        .await
        .context(DrainStorageNode {
            node_name: node_name.to_string(),
        })?;

    tracing::info!(node.name = %node_name, "Drain started for {PRODUCT} Node");

    Ok(())
}

/// Validate if io-engine DaemonSet Pod is running.
async fn is_data_plane_pod_running(
    node: &str,
    namespace: String,
    k8s_client: &KubeClientSet,
) -> Result<bool> {
    let mut data_plane_pod_running = false;
    let io_engine_listparam = ListParams::default().labels(IO_ENGINE_LABEL);
    let initial_io_engine_pod_list: ObjectList<Pod> = k8s_client
        .pods_api()
        .list(&io_engine_listparam)
        .await
        .context(ListPodsWithLabel {
            label: IO_ENGINE_LABEL.to_string(),
            namespace: namespace.clone(),
        })?;

    for pod in initial_io_engine_pod_list.iter() {
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
        if node != node_name {
            continue;
        } else {
            match pod
                .status
                .as_ref()
                .and_then(|status| status.conditions.as_ref())
            {
                Some(conditions) => {
                    for condition in conditions {
                        if condition.type_.eq("Ready") {
                            if condition.status.eq("True") {
                                data_plane_pod_running = true;
                                break;
                            }
                            data_plane_pod_running = false;
                        } else {
                            continue;
                        }
                    }
                }
                None => {
                    data_plane_pod_running = false;
                }
            }
        }
    }
    Ok(data_plane_pod_running)
}

/// Validate if control-plane pods are running -- etcd, agent-core, api-rest.
async fn is_control_plane_running(namespace: String, k8s_client: &KubeClientSet) -> Result<()> {
    let pod_list: ObjectList<Pod> = k8s_client
        .pods_api()
        .list(&ListParams::default().labels(AGENT_CORE_LABEL))
        .await
        .context(ListPodsWithLabel {
            label: AGENT_CORE_LABEL.to_string(),
            namespace: namespace.clone(),
        })?;
    let core_result = all_pods_are_ready(pod_list);
    ensure!(
        core_result.0,
        ValidatingPodReadyStatus {
            name: core_result.1,
            namespace: core_result.2,
        }
    );

    let pod_list: ObjectList<Pod> = k8s_client
        .pods_api()
        .list(&ListParams::default().labels(API_REST_LABEL))
        .await
        .context(ListPodsWithLabel {
            label: API_REST_LABEL.to_string(),
            namespace: namespace.clone(),
        })?;
    let rest_result = all_pods_are_ready(pod_list);
    ensure!(
        rest_result.0,
        ValidatingPodReadyStatus {
            name: rest_result.1,
            namespace: rest_result.2,
        }
    );

    let pod_list: ObjectList<Pod> = k8s_client
        .pods_api()
        .list(&ListParams::default().labels(ETCD_LABEL))
        .await
        .context(ListPodsWithLabel {
            label: ETCD_LABEL.to_string(),
            namespace: namespace.clone(),
        })?;
    let etcd_result = all_pods_are_ready(pod_list);
    ensure!(
        etcd_result.0,
        ValidatingPodReadyStatus {
            name: etcd_result.1,
            namespace: etcd_result.2,
        }
    );

    Ok(())
}
