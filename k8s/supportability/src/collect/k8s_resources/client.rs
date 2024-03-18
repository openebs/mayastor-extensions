use crate::collect::k8s_resources::common::KUBERNETES_HOST_LABEL_KEY;
use k8s_operators::diskpool::crd::DiskPool;

use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, StatefulSet},
    core::v1::{Event, Node, Pod},
};
use kube::{
    api::{DynamicObject, ListParams},
    discovery::{verbs, Scope},
    Api, Client, Discovery, Resource,
};
use std::{collections::HashMap, convert::TryFrom};

const SNAPSHOT_GROUP: &str = "snapshot.storage.k8s.io";
const SNAPSHOT_VERSION: &str = "v1";
const VOLUME_SNAPSHOT_CLASS: &str = "VolumeSnapshotClass";
const VOLUME_SNAPSHOT_CONTENT: &str = "VolumeSnapshotContent";
const DRIVER: &str = "driver";
const SPEC: &str = "spec";

/// K8sResourceError holds errors that can obtain while fetching
/// information of Kubernetes Objects
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub(crate) enum K8sResourceError {
    ClientConfigError(kube::config::KubeconfigError),
    InferConfigError(kube::config::InferConfigError),
    ClientError(kube::Error),
    ResourceError(Box<dyn std::error::Error>),
    CustomError(String),
}

impl From<kube::config::KubeconfigError> for K8sResourceError {
    fn from(e: kube::config::KubeconfigError) -> K8sResourceError {
        K8sResourceError::ClientConfigError(e)
    }
}

impl From<kube::config::InferConfigError> for K8sResourceError {
    fn from(e: kube::config::InferConfigError) -> K8sResourceError {
        K8sResourceError::InferConfigError(e)
    }
}

impl From<kube::Error> for K8sResourceError {
    fn from(e: kube::Error) -> K8sResourceError {
        K8sResourceError::ClientError(e)
    }
}

impl From<Box<dyn std::error::Error>> for K8sResourceError {
    fn from(e: Box<dyn std::error::Error>) -> K8sResourceError {
        K8sResourceError::ResourceError(e)
    }
}

impl From<String> for K8sResourceError {
    fn from(e: String) -> K8sResourceError {
        K8sResourceError::CustomError(e)
    }
}

impl K8sResourceError {
    /// Returns K8sResourceError from provided message
    pub fn invalid_k8s_resource_value(err: String) -> Self {
        Self::CustomError(err)
    }
}

/// ClientSet is wrapper Kubernetes clientset and namespace of mayastor service
#[derive(Clone)]
pub(crate) struct ClientSet {
    client: kube::Client,
    namespace: String,
}

impl ClientSet {
    /// Create a new ClientSet, from the config file if provided, otherwise with default.
    pub(crate) async fn new(
        kube_config_path: Option<std::path::PathBuf>,
        namespace: String,
    ) -> Result<Self, K8sResourceError> {
        let config = match kube_config_path {
            Some(config_path) => {
                let kube_config = kube::config::Kubeconfig::read_from(&config_path)
                    .map_err(|e| -> K8sResourceError { e.into() })?;
                kube::Config::from_custom_kubeconfig(kube_config, &Default::default()).await?
            }
            None => kube::Config::infer().await?,
        };
        let client = Client::try_from(config)?;
        Ok(Self { client, namespace })
    }

    /// Get a clone of the inner `kube::Client`.
    pub(crate) fn kube_client(&self) -> kube::Client {
        self.client.clone()
    }
    /// Get a reference to the namespace.
    pub(crate) fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Get a new api for a `dynamic_object` for the provided GVK.
    pub(crate) async fn dynamic_object_api(
        &self,
        namespace: Option<&str>,
        group_name: &str,
        version: &str,
        kind: &str,
    ) -> Result<Api<DynamicObject>, K8sResourceError> {
        let discovery = Discovery::new(self.kube_client()).run().await?;
        for group in discovery.groups() {
            if group.name() == group_name {
                for (ar, caps) in group.recommended_resources() {
                    if !caps.supports_operation(verbs::LIST) {
                        continue;
                    }
                    if ar.version == version && ar.kind == kind {
                        let result = match namespace {
                            None if caps.scope == Scope::Cluster => {
                                Ok(Api::all_with(self.kube_client(), &ar))
                            }
                            Some(ns) if caps.scope == Scope::Namespaced => {
                                Ok(Api::namespaced_with(self.kube_client(), ns, &ar))
                            }
                            _ => Err(K8sResourceError::CustomError(format!(
                                "DynamicObject Api not available for {kind} of {group_name}/{version}"
                            ))),
                        };
                        return result;
                    }
                }
            }
        }
        Err(K8sResourceError::CustomError(format!(
            "DynamicObject Api not available for {kind} of {group_name}/{version}"
        )))
    }

    /// Fetch node objects from API-server then form and return map of node name to node object
    pub(crate) async fn get_nodes_map(&self) -> Result<HashMap<String, Node>, K8sResourceError> {
        let node_api: Api<Node> = Api::all(self.client.clone());
        let nodes = node_api.list(&ListParams::default()).await?;
        let mut node_map = HashMap::new();
        for node in nodes.items {
            node_map.insert(
                node.metadata
                    .name
                    .as_ref()
                    .ok_or_else(|| {
                        K8sResourceError::CustomError("Unable to get node name".to_string())
                    })?
                    .clone(),
                node,
            );
        }
        Ok(node_map)
    }

    /// Fetch list of pods associated to given label_selector & field_selector
    pub(crate) async fn get_pods(
        &self,
        label_selector: &str,
        field_selector: &str,
    ) -> Result<Vec<Pod>, K8sResourceError> {
        let mut list_params = ListParams::default()
            .labels(label_selector)
            .fields(field_selector)
            .limit(100);

        let mut pods: Vec<Pod> = vec![];

        let pods_api: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);
        // Paginate to get 100 contents at a time
        loop {
            let mut result = pods_api.list(&list_params).await?;
            pods.append(&mut result.items);
            match result.metadata.continue_ {
                None => break,
                Some(token) => list_params = list_params.continue_token(token.as_str()),
            };
        }
        Ok(pods)
    }

    /// get the k8s pod api for pod operations, like logs_stream
    pub(crate) async fn get_pod_api(&self) -> Api<Pod> {
        Api::namespaced(self.client.clone(), &self.namespace)
    }

    /// Fetch list of disk pools associated to given names if None is provided then
    /// all results will be returned
    pub(crate) async fn list_pools(
        &self,
        label_selector: Option<&str>,
        field_selector: Option<&str>,
    ) -> Result<Vec<DiskPool>, K8sResourceError> {
        let list_params = ListParams::default()
            .labels(label_selector.unwrap_or_default())
            .fields(field_selector.unwrap_or_default());
        let pools_api: Api<DiskPool> = Api::namespaced(self.client.clone(), &self.namespace);
        let pools = match pools_api.list(&list_params).await {
            Ok(val) => val,
            Err(kube_error) => match kube_error {
                kube::Error::Api(e) => {
                    if e.code == 404 {
                        return Ok(vec![]);
                    }
                    return Err(K8sResourceError::ClientError(kube::Error::Api(e)));
                }
                _ => return Err(K8sResourceError::ClientError(kube_error)),
            },
        };
        Ok(pools.items)
    }

    /// Fetch list of volume snapshot classes based on the driver if provided.
    pub(crate) async fn list_volumesnapshot_classes(
        &self,
        driver_selector: Option<&str>,
        label_selector: Option<&str>,
        field_selector: Option<&str>,
    ) -> Result<Vec<DynamicObject>, K8sResourceError> {
        let list_params = ListParams::default()
            .labels(label_selector.unwrap_or_default())
            .fields(field_selector.unwrap_or_default());
        let vsc_api: Api<DynamicObject> = self
            .dynamic_object_api(
                None,
                SNAPSHOT_GROUP,
                SNAPSHOT_VERSION,
                VOLUME_SNAPSHOT_CLASS,
            )
            .await?;
        let vscs = match vsc_api.list(&list_params).await {
            Ok(val) => val,
            Err(kube_error) => match kube_error {
                kube::Error::Api(e) => {
                    if e.code == 404 {
                        return Ok(vec![]);
                    }
                    return Err(K8sResourceError::ClientError(kube::Error::Api(e)));
                }
                _ => return Err(K8sResourceError::ClientError(kube_error)),
            },
        };
        Ok(vscs
            .items
            .into_iter()
            .filter(|item| match driver_selector {
                None => true,
                Some(driver_selector) => match item.data.get(DRIVER) {
                    None => false,
                    Some(value) => match value.as_str() {
                        None => false,
                        Some(driver) => driver == driver_selector,
                    },
                },
            })
            .collect())
    }

    /// Fetch list of volume snapshot contents based on the driver if provided.
    pub(crate) async fn list_volumesnapshotcontents(
        &self,
        driver_selector: Option<&str>,
        label_selector: Option<&str>,
        field_selector: Option<&str>,
    ) -> Result<Vec<DynamicObject>, K8sResourceError> {
        let mut list_params = ListParams::default()
            .labels(label_selector.unwrap_or_default())
            .fields(field_selector.unwrap_or_default())
            .limit(2);
        let vsc_api: Api<DynamicObject> = self
            .dynamic_object_api(
                None,
                SNAPSHOT_GROUP,
                SNAPSHOT_VERSION,
                VOLUME_SNAPSHOT_CONTENT,
            )
            .await?;

        let mut vscs_filtered: Vec<DynamicObject> = vec![];
        loop {
            let vscs = match vsc_api.list(&list_params).await {
                Ok(val) => val,
                Err(kube_error) => match kube_error {
                    kube::Error::Api(e) => {
                        if e.code == 404 {
                            return Ok(vec![]);
                        }
                        return Err(K8sResourceError::ClientError(kube::Error::Api(e)));
                    }
                    _ => return Err(K8sResourceError::ClientError(kube_error)),
                },
            };
            vscs_filtered.append(
                &mut vscs
                    .items
                    .into_iter()
                    .filter(|item| match driver_selector {
                        None => true,
                        Some(driver_selector) => match item.data.get(SPEC) {
                            None => false,
                            Some(value) => match value.get(DRIVER) {
                                None => false,
                                Some(value) => match value.as_str() {
                                    None => false,
                                    Some(driver) => driver == driver_selector,
                                },
                            },
                        },
                    })
                    .collect(),
            );
            match vscs.metadata.continue_ {
                Some(token) if !token.is_empty() => {
                    list_params = list_params.continue_token(token.as_str())
                }
                _ => break,
            };
        }
        Ok(vscs_filtered)
    }

    /// Fetch list of k8s events associated to given label_selector & field_selector
    pub(crate) async fn get_events(
        &self,
        label_selector: &str,
        field_selector: &str,
    ) -> Result<Vec<Event>, K8sResourceError> {
        let mut list_params = ListParams::default()
            .labels(label_selector)
            .fields(field_selector)
            .limit(100);

        let mut events: Vec<Event> = vec![];

        let events_api: Api<Event> = Api::namespaced(self.client.clone(), &self.namespace);
        // Paginate to get 100 contents at a time
        loop {
            let mut result = events_api.list(&list_params).await?;
            events.append(&mut result.items);
            match result.metadata.continue_ {
                Some(token) if !token.is_empty() => {
                    list_params = list_params.continue_token(token.as_str())
                }
                _ => break,
            };
        }

        Ok(events)
    }

    /// Fetch list of deployments associated to given label_selector & field_selector
    pub(crate) async fn get_deployments(
        &self,
        label_selector: &str,
        field_selector: &str,
    ) -> Result<Vec<Deployment>, K8sResourceError> {
        let list_params = ListParams::default()
            .labels(label_selector)
            .fields(field_selector);

        let deployments_api: Api<Deployment> =
            Api::namespaced(self.client.clone(), &self.namespace);
        let deployments = deployments_api.list(&list_params).await?;
        Ok(deployments.items)
    }

    /// Fetch list of daemonsets associated to given label_selector & field_selector
    pub(crate) async fn get_daemonsets(
        &self,
        label_selector: &str,
        field_selector: &str,
    ) -> Result<Vec<DaemonSet>, K8sResourceError> {
        let list_params = ListParams::default()
            .labels(label_selector)
            .fields(field_selector);

        let ds_api: Api<DaemonSet> = Api::namespaced(self.client.clone(), &self.namespace);
        let daemonsets = ds_api.list(&list_params).await?;
        Ok(daemonsets.items)
    }

    /// Fetch list of statefulsets associated to given label_selector & field_selector
    pub(crate) async fn get_statefulsets(
        &self,
        label_selector: &str,
        field_selector: &str,
    ) -> Result<Vec<StatefulSet>, K8sResourceError> {
        let list_params = ListParams::default()
            .labels(label_selector)
            .fields(field_selector);

        let sts_api: Api<StatefulSet> = Api::namespaced(self.client.clone(), &self.namespace);
        let statefulsets = sts_api.list(&list_params).await?;
        Ok(statefulsets.items)
    }

    /// Returns the hostname of provided node name by reading from Kubernetes
    /// object labels
    pub(crate) async fn get_hostname(&self, node_name: &str) -> Result<String, K8sResourceError> {
        let node_api: Api<Node> = Api::all(self.client.clone());
        let node = node_api.get(node_name).await?;

        // Labels will definitely exists on Kubernetes node object
        let labels = node.meta().labels.as_ref().ok_or_else(|| {
            K8sResourceError::CustomError(format!("No labels available on node '{node_name}'"))
        })?;

        let reqired_label_value = labels
            .get(KUBERNETES_HOST_LABEL_KEY)
            .ok_or_else(|| {
                K8sResourceError::CustomError(format!(
                    "Node '{KUBERNETES_HOST_LABEL_KEY}' label not found on node {node_name}"
                ))
            })?
            .as_str();
        Ok(reqired_label_value.to_string())
    }

    /// Get node name from a specified hostname
    pub(crate) async fn get_nodename(&self, host_name: &str) -> Result<String, K8sResourceError> {
        let node_api: Api<Node> = Api::all(self.client.clone());
        let node = node_api
            .list(
                &ListParams::default()
                    .labels(format!("{KUBERNETES_HOST_LABEL_KEY}={host_name}").as_str()),
            )
            .await?;
        if node.items.is_empty() {
            return Err(K8sResourceError::CustomError(format!(
                "No node found for hostname {host_name}"
            )));
        }
        // Since object fetched from Kube-apiserver node name will always exist
        if let Some(node) = node.items.first() {
            Ok(node
                .metadata
                .name
                .clone()
                .expect("Node Name should exist in kube-apiserver"))
        } else {
            Err(K8sResourceError::CustomError(format!(
                "No node found for hostname {host_name}"
            )))
        }
    }
}
