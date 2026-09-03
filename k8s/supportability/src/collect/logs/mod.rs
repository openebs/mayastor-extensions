mod k8s_log;
mod loki;

pub use loki::{LokiClient, LokiError};

use crate::collect::{
    k8s_resources::client::{ClientSet, K8sResourceError},
    logs::k8s_log::{K8sLoggerClient, K8sLoggerError},
    utils::log,
};

use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use std::{collections::HashSet, path::Path};

/// Error that can occur while interacting with logs module
#[derive(Debug)]
#[allow(unused)]
pub enum LogError {
    Loki(loki::LokiError),
    K8sResource(K8sResourceError),
    K8sLogger(K8sLoggerError),
    IOError(std::io::Error),
    Custom(String),
    MultipleErrors(Vec<LogError>),
}

impl From<loki::LokiError> for LogError {
    fn from(e: loki::LokiError) -> LogError {
        LogError::Loki(e)
    }
}

impl From<K8sResourceError> for LogError {
    fn from(e: K8sResourceError) -> LogError {
        LogError::K8sResource(e)
    }
}

impl From<K8sLoggerError> for LogError {
    fn from(e: K8sLoggerError) -> LogError {
        LogError::K8sLogger(e)
    }
}

impl From<String> for LogError {
    fn from(e: String) -> LogError {
        LogError::Custom(e)
    }
}

impl From<std::io::Error> for LogError {
    fn from(e: std::io::Error) -> LogError {
        LogError::IOError(e)
    }
}

/// Contains fields to identify cluster resources
#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub(crate) struct LogResource {
    /// Defines the name of the service to fetch logs
    pub(crate) container_name: String,

    /// Identifiy hostname of the service
    pub(crate) host_name: Option<String>,

    /// Uniquely identifies the service via label selector
    pub(crate) label_selector: String,

    /// States the type of the service(mayastor/agents/...)
    pub(crate) service_type: String,
}

/// LogCollection is a wrapper around internal service of log collection
pub(crate) struct LogCollection {
    loki_client: Option<loki::LokiClient>,
    k8s_logger_client: K8sLoggerClient,
}

impl LogCollection {
    /// new create new instance of Logger service based on provided arguments
    /// param 'kube_config_path' --> Holds path to kubernetes config required to interact with
    /// Kube-API server param 'namespace' --> Defines the namespace of the product
    /// param 'loki_uri' --> Defines the address of loki instance
    /// param 'since'  --> Defines period from which logs needs to collect
    /// param 'timeout' --> Specifies the timeout while interacting with Loki Service
    /// param 'tenant_id' --> Specifies the tenant_id while interacting with Loki Service
    pub(crate) async fn new_logger(
        kubeconfig_args: crate::KubeConfigArgs,
        namespace: String,
        loki_uri: Option<String>,
        since: humantime::Duration,
        timeout: humantime::Duration,
        tenant_id: String,
    ) -> Result<Box<dyn Logger>, LogError> {
        let client_set = ClientSet::new(kubeconfig_args.clone(), namespace.clone()).await?;
        Ok(Box::new(Self {
            loki_client: loki::LokiClient::new(
                loki_uri,
                kubeconfig_args,
                namespace,
                since,
                timeout,
                tenant_id,
                false,
            )
            .await,
            k8s_logger_client: K8sLoggerClient::new(client_set),
        }))
    }

    async fn pod_logging_resources(&self, pod: Pod) -> Result<HashSet<LogResource>, LogError> {
        let mut logging_resources = HashSet::new();

        let service_name = pod
            .metadata
            .labels
            .as_ref()
            .ok_or_else(|| {
                K8sResourceError::invalid_k8s_resource_value(format!(
                    "No labels found in pod {:?}",
                    pod.metadata.name
                ))
            })?
            .get("app")
            .unwrap_or(&"".to_string())
            .clone();

        let spec = pod.spec.ok_or_else(|| {
            K8sResourceError::invalid_k8s_resource_value("Pod spec not found".to_string())
        })?;

        let hostname = spec.node_name;

        for container in spec.containers {
            logging_resources.insert(LogResource {
                container_name: container.name,
                host_name: hostname.clone(),
                label_selector: format!("app={service_name}"),
                service_type: service_name.clone(),
            });
        }

        Ok(logging_resources)
    }

    async fn get_logging_resources(
        &self,
        pods: Vec<Pod>,
    ) -> Result<HashSet<LogResource>, LogError> {
        let mut logging_resources = HashSet::new();

        for pod in pods {
            match self.pod_logging_resources(pod.clone()).await {
                Ok(resources) => logging_resources.extend(resources),
                Err(error) => log(format!(
                    "Skipping the pod {:?} due to error: {error:?}",
                    pod.metadata.name
                )),
            }
        }
        Ok(logging_resources)
    }
}

#[async_trait(?Send)]
impl Logger for LogCollection {
    // Fetch logs of requested resource and dump into files
    async fn fetch_and_dump_logs(
        &mut self,
        resources: HashSet<LogResource>,
        working_dir: String,
    ) -> Result<(), LogError> {
        let mut errors = Vec::new();
        for resource in resources.iter() {
            log(format!(
                "\t Collecting logs of service: {}, container: {} of host: {:?}",
                resource.service_type, resource.container_name, resource.host_name,
            ));
            let service_dir = std::path::Path::new(&working_dir.clone())
                .join("logs")
                .join(resource.service_type.clone());

            create_directory_if_not_exist(&service_dir)?;
            if let Some(loki_client) = &mut self.loki_client {
                let _ = loki_client
                    .fetch_and_dump_logs(
                        resource.label_selector.clone(),
                        resource.container_name.clone(),
                        resource.host_name.clone(),
                        service_dir.clone(),
                    )
                    .await.map_err(|e| {
                    log(format!(
                        "\t Failed to collect historical logs of service: {}, container: {} of: host {:?}",
                        resource.service_type, resource.container_name, resource.host_name,
                    ));
                    errors.push(LogError::Loki(e));
                });
            }

            let _ = self
                .k8s_logger_client
                .dump_pod_logs(
                    resource.label_selector.as_str(),
                    service_dir.clone(),
                    resource.host_name.clone(),
                    &[resource.container_name.as_str()],
                )
                .await
                .map_err(|e| {
                    log(format!(
                        "\t Failed to collect current logs of service: {}, container: {} of: host {:?}",
                        resource.service_type, resource.container_name, resource.host_name,
                    ));
                    errors.push(LogError::K8sLogger(e));
                });
        }
        if !errors.is_empty() {
            return Err(LogError::MultipleErrors(errors));
        }
        Ok(())
    }

    async fn get_logging_services(
        &self,
        logging_label_selectors: String,
    ) -> Result<HashSet<LogResource>, LogError> {
        let pods = self
            .k8s_logger_client
            .get_k8s_clientset()
            .get_pods_for_multiple_labels(&logging_label_selectors, "")
            .await?;

        self.get_logging_resources(pods).await
    }

    fn loki_client_mut(&mut self) -> Option<&mut LokiClient> {
        self.loki_client.as_mut()
    }
}

/// Logger contains functionality to interact with service and fetch logs for requested service
#[async_trait(?Send)]
pub(crate) trait Logger {
    async fn fetch_and_dump_logs(
        &mut self,
        resources: HashSet<LogResource>,
        working_dir: String,
    ) -> Result<(), LogError>;
    async fn get_logging_services(
        &self,
        logging_label_selectors: String,
    ) -> Result<HashSet<LogResource>, LogError>;
    /// Return a mutable reference to the inner Loki client, if one was successfully connected
    /// at construction time. Returns `None` when Loki was not found or could not be reached.
    fn loki_client_mut(&mut self) -> Option<&mut LokiClient>;
}

/// Creates specified directory path if not already exist
pub fn create_directory_if_not_exist(dir_path: &Path) -> Result<(), std::io::Error> {
    if !dir_path.exists() {
        std::fs::create_dir_all(dir_path)?;
    }
    Ok(())
}
