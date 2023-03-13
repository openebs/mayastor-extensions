use crate::upgrade::{
    common::{
        constants::{
            AGENT_CORE_POD_LABEL, API_REST_POD_LABEL, DRAIN_FOR_UPGRADE, ETCD_POD_LABEL,
            IO_ENGINE_POD_LABEL,
        },
        error::{Error, RestError},
    },
    config::UpgradeConfig,
    controller::{
        reconciler::{is_draining, is_node_cordoned, is_rebuilding},
        utils::all_pods_are_ready,
    },
    k8s::crd::v0::UpgradePhase,
};
use actix_web::{
    body::BoxBody,
    get,
    http::{header::ContentType, StatusCode},
    put, HttpRequest, HttpResponse, Responder, ResponseError,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{DeleteParams, ListParams, ObjectList},
    Api, ResourceExt,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, time::Duration};
use tracing::{error, info};
/// Upgrade to be returned for get calls.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Upgrade {
    name: String,
    current_version: String,
    target_version: String,
    components_state: HashMap<String, HashMap<String, UpgradePhase>>,
    state: String,
}

impl Upgrade {
    /// This adds a name to the Upgrade instance.
    fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// This adds a source version to the Upgrade instance.
    fn with_current_version(mut self, current_version: String) -> Self {
        self.current_version = current_version;
        self
    }

    /// This adds a target version to the Upgrade instance.
    fn with_target_version(mut self, target_version: String) -> Self {
        self.target_version = target_version;
        self
    }

    /// This adds a state to the Upgrade instance.
    fn with_state(mut self, state: String) -> Self {
        self.state = state;
        self
    }
}

impl Responder for Upgrade {
    type Body = BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        let res_body = serde_json::to_string(&self)
            .map_err(|err| Error::SerdeDeserialization { source: err })
            .unwrap();

        // Create HttpResponse and set Content Type
        HttpResponse::Ok()
            .content_type(ContentType::json())
            .body(res_body)
    }
}

/// Implement ResponseError for RestError.
impl ResponseError for RestError {
    fn status_code(&self) -> StatusCode {
        StatusCode::NOT_FOUND
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let body = serde_json::to_string(&self)
            .map_err(|err| Error::SerdeDeserialization { source: err })
            .unwrap();
        let res = HttpResponse::new(self.status_code());
        res.set_body(BoxBody::new(body))
    }
}

/// Implement Display for RestError.
impl Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Put request for upgrade.
#[put("/upgrade")]
pub async fn apply_upgrade() -> Result<HttpResponse, RestError> {
    match UpgradeConfig::get_config()
        .k8s_client()
        .create_upgrade_action_resource()
        .await
    {
        Ok(u) => {
            info!(
                name = u.metadata.name.as_ref().unwrap(),
                namespace = u.metadata.namespace.as_ref().unwrap(),
                "Created UpgradeAction CustomResource"
            );
            let res = Upgrade::default()
                .with_name(u.name_any())
                .with_current_version(u.spec.current_version().to_string())
                .with_target_version(u.spec.target_version().to_string());
            let res_body = serde_json::to_string(&res).map_err(|error| {
                RestError::default().with_error(format!(
                    "error: {}",
                    Error::SerdeDeserialization { source: error }
                ))
            })?;

            return Ok(HttpResponse::Ok()
                .content_type(ContentType::json())
                .body(res_body));
        }
        Err(error) => {
            error!(?error, "Failed to create UpgradeAction resource");
            let err = RestError::default()
                .with_error("Unable to create UpgradeAction resource".to_string());
            Err(err)
        }
    }
}

/// Upgrade data plane by controlled restart of io-engine pods
pub async fn upgrade_data_plane() -> Result<(), Error> {
    let pods: Api<Pod> = Api::namespaced(
        UpgradeConfig::get_config().k8s_client().client(),
        UpgradeConfig::get_config().namespace(),
    );

    let io_engine_listparam = ListParams::default().labels(IO_ENGINE_POD_LABEL);
    let initial_io_engine_pod_list: ObjectList<Pod> = pods.list(&io_engine_listparam).await?;
    for pod in initial_io_engine_pod_list.iter() {
        // Fetch the node name on which the io-engine pod is running
        let node_name = pod
            .spec
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("pod.spec is empty"))?
            .node_name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("pod.spec.node_name is empty"))?
            .as_str();

        tracing::info!(
            pod.name = %pod.metadata.name.clone().unwrap_or_default(),
            node.name = %node_name,
            "Upgrade starting for data-plane pod"
        );

        let is_node_cordoned = is_node_cordoned(node_name).await?;

        // Issue node drain command
        issue_node_drain(node_name).await?;

        // Wait for node drain to complete across the cluster.
        wait_node_drain().await?;

        // Wait for any rebuild to complete.
        wait_for_rebuild().await?;

        // restart the data plane pod
        restart_data_plane(node_name, pod).await?;

        // Uncordon the drained node
        if !is_node_cordoned {
            uncordon_node(node_name).await?;
        }

        // validate the new pod is up and running
        verify_data_plane_pod_is_running(node_name).await?;

        // Validate the control plane pod is up and running
        is_control_plane_running().await?
    }
    Ok(())
}

pub async fn uncordon_node(node_name: &str) -> Result<(), Error> {
    match UpgradeConfig::get_config()
        .rest_client()
        .nodes_api()
        .delete_node_cordon(node_name, DRAIN_FOR_UPGRADE)
        .await
    {
        Ok(_) => {
            tracing::info!(node.name = node_name, "Node is uncordoned");
            Ok(())
        }
        Err(_) => Err(Error::NodeUncordonError {
            node_name: node_name.to_string(),
        }),
    }
}

/// Issue delete command on dataplane pods.
pub async fn restart_data_plane(node_name: &str, pod: &Pod) -> Result<(), Error> {
    let pods: Api<Pod> = Api::namespaced(
        UpgradeConfig::get_config().k8s_client().client(),
        UpgradeConfig::get_config().namespace(),
    );
    // Deleting the io-engine pod
    let pod_name = pod.metadata.name.as_ref().unwrap();
    tracing::info!(
        pod.name = pod_name,
        node.name = node_name,
        "Deleting the pod"
    );
    pods.delete(pod_name, &DeleteParams::default())
        .await?
        .map_right(|s| tracing::info!(pod.name = pod_name, "Deleted Pod: {:?}", s));
    Ok(())
}

/// Wait for the data plane pod to come up on the given node.
pub async fn wait_node_drain() -> Result<(), Error> {
    while is_draining().await? {
        tokio::time::sleep(Duration::from_secs(10_u64)).await;
    }
    Ok(())
}

/// Wait for all the node drain process to complete.
pub async fn verify_data_plane_pod_is_running(node_name: &str) -> Result<(), Error> {
    // Validate the new pod is up and running
    while is_data_plane_pod_running(node_name).await? {
        tokio::time::sleep(Duration::from_secs(10_u64)).await;
    }
    Ok(())
}

///  Wait for the rebuild to complete if any
pub async fn wait_for_rebuild() -> Result<(), Error> {
    // Wait for 60 seconds for any rebuilds to kick in.
    tokio::time::sleep(Duration::from_secs(60_u64)).await;
    while is_rebuilding().await? {
        tokio::time::sleep(Duration::from_secs(10_u64)).await;
    }
    Ok(())
}

/// Issue the node drain command on the node.
pub async fn issue_node_drain(node_name: &str) -> Result<(), Error> {
    match UpgradeConfig::get_config()
        .rest_client()
        .nodes_api()
        .put_node_drain(node_name, DRAIN_FOR_UPGRADE)
        .await
    {
        Ok(_) => {
            tracing::info!(
                node.name = %node_name,
                "Drain started"
            );
            Ok(())
        }
        Err(_) => Err(Error::NodeDrainError {
            node_name: node_name.to_string(),
        }),
    }
}

pub async fn is_data_plane_pod_running(node: &str) -> Result<bool, Error> {
    let mut data_plane_pod_running = false;
    let pods: Api<Pod> = Api::namespaced(
        UpgradeConfig::get_config().k8s_client().client(),
        UpgradeConfig::get_config().namespace(),
    );
    let io_engine_listparam = ListParams::default().labels(IO_ENGINE_POD_LABEL);
    let initial_io_engine_pod_list: ObjectList<Pod> = pods.list(&io_engine_listparam).await?;
    //let data_plane_pod_running =
    for pod in initial_io_engine_pod_list.iter() {
        // Fetch the node name on which the io-engine pod is running
        let node_name = pod
            .spec
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("pod.spec is empty"))?
            .node_name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("pod.spec.node_name is empty"))?
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
                        if condition.type_.eq("Ready") && condition.status.eq("True") {
                            data_plane_pod_running = true
                        } else {
                            data_plane_pod_running = false;
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

pub async fn is_control_plane_running() -> Result<(), Error> {
    let pods: Api<Pod> = Api::namespaced(
        UpgradeConfig::get_config().k8s_client().client(),
        UpgradeConfig::get_config().namespace(),
    );

    let pod_list: ObjectList<Pod> = pods
        .list(&ListParams::default().labels(AGENT_CORE_POD_LABEL))
        .await?;
    let core_result = all_pods_are_ready(pod_list);
    if !core_result.0 {
        return Err(Error::AgentCorePodNotRunning {
            pod: core_result.1,
            namespace: core_result.2,
        });
    }

    let pod_list: ObjectList<Pod> = pods
        .list(&ListParams::default().labels(API_REST_POD_LABEL))
        .await?;
    let rest_result = all_pods_are_ready(pod_list);
    if !rest_result.0 {
        return Err(Error::ApiRestPodNotRunning {
            pod: rest_result.1,
            namespace: rest_result.2,
        });
    }

    let pod_list: ObjectList<Pod> = pods
        .list(&ListParams::default().labels(ETCD_POD_LABEL))
        .await?;
    let etcd_result = all_pods_are_ready(pod_list);
    if !etcd_result.0 {
        return Err(Error::EtcdPodNotRunning {
            pod: etcd_result.1,
            namespace: etcd_result.2,
        });
    }
    Ok(())
}

/// Get  upgrade.
#[get("/upgrade")]
pub async fn get_upgrade() -> impl Responder {
    match UpgradeConfig::get_config()
        .k8s_client()
        .get_upgrade_action_resource()
        .await
    {
        Ok(u) => {
            let status = match &u.status {
                Some(status) => status.state().to_string(),
                None => "<Empty>".to_string(),
            };

            let res = Upgrade::default()
                .with_name(u.name_any())
                .with_current_version(u.spec.current_version().to_string())
                .with_target_version(u.spec.target_version().to_string())
                .with_state(status);
            Ok(res)
        }
        Err(error) => {
            error!(?error, "Failed to GET UpgradeAction resource");
            let err = RestError::default()
                .with_error("Unable to create UpgradeAction resource".to_string());
            Err(err)
        }
    }
}
