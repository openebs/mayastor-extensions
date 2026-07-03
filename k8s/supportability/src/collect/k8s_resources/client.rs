use super::k8s_operators::DiskPool;
use crate::collect::k8s_resources::common::KUBERNETES_HOST_LABEL_KEY;

use futures::future;
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, StatefulSet},
    core::v1::{Event, Node, Pod},
};
use kube::{
    api::{DynamicObject, ListParams},
    core::GroupVersionKind,
    discovery,
    discovery::{verbs, Scope},
    Api, Client, Resource,
};
use std::{collections::HashSet, convert::TryFrom};

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
#[allow(unused)]
pub enum K8sResourceError {
    ClientConfigError(kube::config::KubeconfigError),
    InferConfigError(kube::config::InferConfigError),
    InClusterError(kube::config::InClusterError),
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

impl From<kube::config::InClusterError> for K8sResourceError {
    fn from(e: kube::config::InClusterError) -> K8sResourceError {
        K8sResourceError::InClusterError(e)
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
pub struct ClientSet {
    client: kube::Client,
    namespace: String,
}

impl ClientSet {
    /// Create a new ClientSet, from the config file if provided, otherwise with default.
    pub(crate) async fn new(
        kubeconfig: crate::KubeConfigArgs,
        namespace: String,
    ) -> Result<Self, K8sResourceError> {
        let config = match kubeconfig.path {
            Some(config_path) => {
                let kube_config = kube::config::Kubeconfig::read_from(&config_path)
                    .map_err(|e| -> K8sResourceError { e.into() })?;
                kube::Config::from_custom_kubeconfig(kube_config, &kubeconfig.opts).await?
            }
            None => {
                if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
                    kube::Config::incluster()?
                } else {
                    kube::Config::from_kubeconfig(&kubeconfig.opts).await?
                }
            }
        };
        let client = Client::try_from(config)?;
        Ok(Self { client, namespace })
    }

    /// Get a clone of the inner `kube::Client`.
    pub fn kube_client(&self) -> kube::Client {
        self.client.clone()
    }
    /// Get a reference to the namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Get a new api for a `dynamic_object` for the provided GVK.
    pub async fn dynamic_object_api(
        &self,
        namespace: Option<&str>,
        group: &str,
        version: &str,
        kind: &str,
    ) -> Result<Option<Api<DynamicObject>>, K8sResourceError> {
        let gvk = GroupVersionKind::gvk(group, version, kind);

        match discovery::pinned_kind(&self.kube_client(), &gvk).await {
            Ok((ar, caps)) => {
                if !caps.supports_operation(verbs::LIST) {
                    return Ok(None);
                }

                let api = match (namespace, caps.scope) {
                    (Some(ns), Scope::Namespaced) => {
                        Api::namespaced_with(self.kube_client(), ns, &ar)
                    }
                    (None, Scope::Cluster) => Api::all_with(self.kube_client(), &ar),
                    _ => return Ok(None),
                };

                Ok(Some(api))
            }
            // For any discovery and API 404 we should not error out.
            Err(kube::Error::Api(ref api_err)) if api_err.code == 404 => Ok(None),
            Err(kube::Error::Discovery(_)) => Ok(None),
            Err(e) => Err(K8sResourceError::ClientError(e)),
        }
    }

    /// Get pods when given a label selector that has multiple labels comma seperated.
    pub(crate) async fn get_pods_for_multiple_labels(
        &self,
        label_selectors: &str,
        field_selector: &str,
    ) -> Result<Vec<Pod>, K8sResourceError> {
        let selectors: Vec<&str> = label_selectors
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        let futs = selectors
            .into_iter()
            .map(|sel| self.get_pods(sel, field_selector));

        let results = future::join_all(futs).await;

        let mut all_pods = Vec::new();
        let mut seen_ids = HashSet::new();

        for res in results {
            let pods = res?;
            for pod in pods {
                if let Some(key) = pod
                    .metadata
                    .uid
                    .clone()
                    .or_else(|| pod.metadata.name.clone())
                {
                    if seen_ids.insert(key) {
                        all_pods.push(pod);
                    }
                }
            }
        }

        Ok(all_pods)
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
                Some(ref token) if !token.is_empty() => {
                    list_params = list_params.continue_token(token)
                }
                _ => break,
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

        // Attempt to retrieve the VolumeSnapshotClass API
        let vsc_api_opt = self
            .dynamic_object_api(
                None,
                SNAPSHOT_GROUP,
                SNAPSHOT_VERSION,
                VOLUME_SNAPSHOT_CLASS,
            )
            .await?;

        // If the API is not available, return an empty list
        let vsc_api = match vsc_api_opt {
            Some(api) => api,
            None => return Ok(vec![]),
        };

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

        let vsc_api_opt = self
            .dynamic_object_api(
                None,
                SNAPSHOT_GROUP,
                SNAPSHOT_VERSION,
                VOLUME_SNAPSHOT_CONTENT,
            )
            .await?;

        // If the API is not available, return an empty list
        let vsc_api = match vsc_api_opt {
            Some(api) => api,
            None => return Ok(vec![]),
        };

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
                Some(ref token) if !token.is_empty() => {
                    list_params = list_params.continue_token(token)
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
                Some(ref token) if !token.is_empty() => {
                    list_params = list_params.continue_token(token)
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

    /// Get the helm release version from the rest's deployment.
    pub(crate) async fn rest_version(&self) -> Result<String, K8sResourceError> {
        let rest = constants::API_REST_LABEL_SELECTOR;
        let deployments = self.get_deployments(rest, "").await?;
        let rest_depl = deployments
            .first()
            .ok_or(K8sResourceError::CustomError(format!(
                "Failed to find deployment {rest}"
            )))?;
        let labels = rest_depl.metadata.labels.as_ref();
        match labels.and_then(|l| l.get(&constants::helm_release_version_key())) {
            Some(version) => Ok(version.to_owned()),
            None => Err(K8sResourceError::CustomError(
                "Helm Release Version key not found".into(),
            )),
        }
    }
}
