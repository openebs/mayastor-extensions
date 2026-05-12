use super::k8s_operators::DiskPool;
use crate::{
    collect::{
        k8s_resources::client::{ClientSet, K8sResourceError},
        logs::create_directory_if_not_exist,
    },
    log,
};
use utils::csi_plugin_name;

use k8s_openapi::{
    api::{apps::v1, core::v1::Event},
    apimachinery::pkg::apis::meta::v1::MicroTime,
};
use kube::Resource;
use serde::Serialize;
use std::{
    collections::HashSet,
    fs::File,
    io::Write,
    iter::FromIterator,
    path::{Path, PathBuf},
};

/// K8s resource dumper client
#[derive(Clone)]
pub struct K8sResourceDumperClient {
    k8s_client: ClientSet,
}

/// Errors pertaining to k8s resource dumper module
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
#[allow(unused)]
pub enum K8sResourceDumperError {
    K8sResourceError(K8sResourceError),
    IOError(std::io::Error),
    YamlSerializationError(serde_yaml::Error),
    JsonSerializationError(serde_json::Error),
    // Used to hold stack of multiple errors and used to continue collecting information
    MultipleErrors(Vec<K8sResourceDumperError>),
    Generic(String),
}

impl From<std::io::Error> for K8sResourceDumperError {
    fn from(e: std::io::Error) -> K8sResourceDumperError {
        K8sResourceDumperError::IOError(e)
    }
}

impl From<serde_yaml::Error> for K8sResourceDumperError {
    fn from(e: serde_yaml::Error) -> K8sResourceDumperError {
        K8sResourceDumperError::YamlSerializationError(e)
    }
}

impl From<serde_json::Error> for K8sResourceDumperError {
    fn from(e: serde_json::Error) -> K8sResourceDumperError {
        K8sResourceDumperError::JsonSerializationError(e)
    }
}

impl From<K8sResourceError> for K8sResourceDumperError {
    fn from(e: K8sResourceError) -> K8sResourceDumperError {
        K8sResourceDumperError::K8sResourceError(e)
    }
}

/// Newtype to wrap k8s DaemonSet
#[derive(Serialize)]
pub(crate) struct DaemonSet(v1::DaemonSet);
/// Newtype to wrap k8s Deployment
#[derive(Serialize)]
pub(crate) struct Deployment(v1::Deployment);
/// Newtype to wrap k8s StatefulSet
#[derive(Serialize)]
pub(crate) struct StatefulSet(v1::StatefulSet);

/// Trait to get the entity names
pub(crate) trait EntityName: Serialize {
    fn name(&self) -> String;
}

impl EntityName for DaemonSet {
    fn name(&self) -> String {
        self.0.metadata.name.as_ref().unwrap().to_string()
    }
}

impl EntityName for Deployment {
    fn name(&self) -> String {
        self.0.metadata.name.as_ref().unwrap().to_string()
    }
}

impl EntityName for StatefulSet {
    fn name(&self) -> String {
        self.0.metadata.name.as_ref().unwrap().to_string()
    }
}

impl K8sResourceDumperClient {
    /// Get a new k8s resource dumper client.
    pub(crate) async fn new(k8s_client: ClientSet) -> Self {
        Self { k8s_client }
    }

    /// dump the kubernetes resources like deployments, daemonsets,
    /// pods, statefulsets, events.
    pub(crate) async fn dump_common_k8s_resources(
        &self,
        root_path: String,
    ) -> Result<(), K8sResourceDumperError> {
        // Create the root dir path
        let mut root_dir = PathBuf::from(root_path);
        root_dir.push("k8s_resources");
        create_directory_if_not_exist(&root_dir)?;

        // Create the configurations path
        let mut configurations_path = root_dir.to_path_buf();
        configurations_path.push("configurations");
        // Create the configurations directory
        create_directory_if_not_exist(&configurations_path)?;

        let mut errors = Vec::new();

        log("Collecting K8s resources...");

        // Fetch all events in provided NAMESPACE
        if let Err(error) = get_k8s_events(&self.k8s_client, &root_dir).await {
            log(format!("\t Failed to collect k8s events, {error:?}"));
            errors.push(error);
        }

        // Fetch all Daemonsets in provided NAMESPACE
        if let Err(error) = get_k8s_daemonsets(&self.k8s_client, &configurations_path).await {
            log(format!("\t Failed to collect k8s daemonsets, {error:?}"));
            errors.push(error)
        }

        // Fetch all Deployments in provided NAMESPACE
        if let Err(error) = get_k8s_deployments(&self.k8s_client, &configurations_path).await {
            log(format!("\t Failed to collect k8s deployments, {error:?}"));
            errors.push(error)
        }

        // Fetch all StatefulSets in provided NAMESPACE
        if let Err(error) = get_k8s_statefulsets(&self.k8s_client, &configurations_path).await {
            log(format!("\t Failed to collect k8s statefulsets, {error:?}"));
            errors.push(error)
        }

        // Fetch all Pods in provided NAMESPACE
        if let Err(error) = get_k8s_pod_configurations(&self.k8s_client, &root_dir).await {
            log(format!(
                "\t Failed to collect k8s pod configurations, {error:?}"
            ));
            errors.push(error)
        }

        if !errors.is_empty() {
            return Err(K8sResourceDumperError::MultipleErrors(errors));
        }
        log("Completed collection of k8s resources");
        Ok(())
    }

    /// Dump the mayastor specific k8s resources, like diskpools, vs_class and vs_conts.
    pub(crate) async fn dump_mayastor_k8s_resources(
        &self,
        root_path: &Path,
        required_pools: Option<Vec<String>>,
    ) -> Result<(), K8sResourceDumperError> {
        // Create the root dir path
        create_directory_if_not_exist(root_path)?;

        let mut errors = Vec::new();

        log("Collecting mayastor specific k8s resources...");
        // Fetch all DiskPools in provided NAMESPACE
        if let Err(error) = get_k8s_diskpools(&self.k8s_client, root_path, required_pools).await {
            log(format!(
                "\t Failed to collect mayastor diskpools, {error:?}"
            ));
            errors.push(error)
        }

        // Fetch all VolumeSnapshotClasses for mayastor csi driver
        if let Err(error) = get_k8s_vs_classes(&self.k8s_client, root_path, csi_plugin_name()).await
        {
            log(format!(
                "\t Failed to collect mayastor volume snapshot classes, {error:?}"
            ));
            errors.push(error)
        }

        // Fetch all VolumeSnapshotContents for mayastor csi driver
        if let Err(error) =
            get_k8s_vsnapshot_contents(&self.k8s_client, root_path, csi_plugin_name()).await
        {
            log(format!(
                "\t Failed to collect mayastor volume snapshot contents, {error:?}"
            ));
            errors.push(error)
        }

        if !errors.is_empty() {
            return Err(K8sResourceDumperError::MultipleErrors(errors));
        }
        log("Completed collection of mayastor specific k8s resources");
        Ok(())
    }

    /// Get the k8s clientset.
    pub fn client_set(&self) -> &ClientSet {
        &self.k8s_client
    }
}

/// create the app specific yamls
fn create_app_configurations<T: EntityName>(
    apps: Vec<T>,
    dir_path: PathBuf,
) -> Result<(), K8sResourceDumperError> {
    for app in apps {
        let serialized = match serde_yaml::to_string(&app) {
            Ok(value) => value,
            Err(error) => {
                log(format!(
                    "Error serializing the app: {}, error: {error}",
                    app.name(),
                ));
                continue;
            }
        };
        match create_file_and_write(dir_path.clone(), format!("{}.yaml", app.name()), serialized) {
            Ok(_) => {}
            Err(error) => {
                log(format!(
                    "Error creating or writing file for the app: {}, error: {error}",
                    app.name(),
                ));
                continue;
            }
        }
    }
    Ok(())
}

/// kubectl's way of ensuring we always have a time to be used for sorting
/// ref: https://github.com/kubernetes/kubectl/blob/f0ce177e80077eb167dd17febe4b9a6c157c5684/pkg/cmd/events/events.go#L294-L319
fn event_time(event: &Event) -> MicroTime {
    if event.series.is_some() {
        return event
            .series
            .as_ref()
            .unwrap()
            .last_observed_time
            .as_ref()
            .unwrap()
            .clone();
    }
    if event.last_timestamp.is_some() {
        return MicroTime(event.last_timestamp.as_ref().unwrap().0);
    }
    event.event_time.as_ref().unwrap().clone()
}

async fn get_k8s_daemonsets(
    k8s_client: &ClientSet,
    configurations_path: &Path,
) -> Result<(), K8sResourceDumperError> {
    // Fetch all Daemonsets in provided NAMESPACE
    log("\t Collecting daemonsets configuration");
    match k8s_client.get_daemonsets("", "").await {
        Ok(daemonsets) => {
            // Create all daemonsets configurations
            create_app_configurations(
                daemonsets.into_iter().map(DaemonSet).collect(),
                configurations_path.to_path_buf(),
            )?;
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}

async fn get_k8s_deployments(
    k8s_client: &ClientSet,
    configurations_path: &Path,
) -> Result<(), K8sResourceDumperError> {
    // Fetch all Deployments in provided NAMESPACE
    log("\t Collecting deployments configuration");
    match k8s_client.get_deployments("", "").await {
        Ok(deploys) => {
            // Create all deployment configurations
            create_app_configurations(
                deploys.into_iter().map(Deployment).collect(),
                configurations_path.to_path_buf(),
            )?;
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}

async fn get_k8s_statefulsets(
    k8s_client: &ClientSet,
    configurations_path: &Path,
) -> Result<(), K8sResourceDumperError> {
    // Fetch all StatefulSets in provided NAMESPACE
    log("\t Collecting statefulsets configuration");
    match k8s_client.get_statefulsets("", "").await {
        Ok(statefulsets) => {
            // Create all statefulsets configurations
            create_app_configurations(
                statefulsets.into_iter().map(StatefulSet).collect(),
                configurations_path.to_path_buf(),
            )?;
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}

async fn get_k8s_diskpools(
    k8s_client: &ClientSet,
    root_dir: &Path,
    required_pools: Option<Vec<String>>,
) -> Result<(), K8sResourceDumperError> {
    // Fetch all DiskPools in provided NAMESPACE
    log("\t Collecting mayastor diskpool resources");
    match k8s_client.list_pools(None, None).await {
        Ok(disk_pools) => {
            let filtered_pools = match required_pools {
                Some(p_names) => {
                    let names: HashSet<String> = HashSet::from_iter(p_names);
                    disk_pools
                        .into_iter()
                        .filter(|p| names.contains(p.meta().name.as_ref().unwrap()))
                        .collect::<Vec<DiskPool>>()
                }
                None => disk_pools,
            };
            // NOTE: Unmarshalling object recevied from K8s API-server will not fail
            create_file_and_write(
                root_dir.to_path_buf(),
                "disk_pools.yaml".to_string(),
                serde_yaml::to_string(&filtered_pools)?,
            )
            .map_err(K8sResourceDumperError::IOError)?;
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}

async fn get_k8s_pod_configurations(
    k8s_client: &ClientSet,
    root_dir: &Path,
) -> Result<(), K8sResourceDumperError> {
    // Fetch all Pods in provided NAMESPACE
    log("\t Collecting Kubernetes pod resources");
    match k8s_client.get_pods("", "").await {
        Ok(pods) => {
            create_file_and_write(
                root_dir.to_path_buf(),
                "pods.yaml".to_string(),
                serde_yaml::to_string(&pods)?,
            )
            .map_err(K8sResourceDumperError::IOError)?;
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}

async fn get_k8s_events(
    k8s_client: &ClientSet,
    root_dir: &Path,
) -> Result<(), K8sResourceDumperError> {
    // Fetch all events in provided NAMESPACE
    log("\t Collecting Kubernetes events");
    match k8s_client.get_events("", "").await {
        Ok(mut events) => {
            // Sort the events based on event_time
            events.sort_unstable_by_key(event_time);
            // NOTE: Unmarshalling object recevied from K8s API-server will not fail
            create_file_and_write(
                root_dir.to_path_buf(),
                "k8s_events.json".to_string(),
                serde_json::to_string_pretty(&events)?,
            )
            .map_err(K8sResourceDumperError::IOError)?;
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}

/// Creates a file and writes the passed content in it
pub fn create_file_and_write(
    mut file_path: PathBuf,
    file_name: String,
    content: String,
) -> Result<(), std::io::Error> {
    file_path.push(file_name);
    let mut file = File::create(file_path)?;
    file.write_all(content.as_bytes())?;
    file.flush().unwrap();
    Ok(())
}

/// Get all the volume snapshot classes for a given driver.
pub async fn get_k8s_vs_classes(
    k8s_client: &ClientSet,
    root_dir: &Path,
    driver_selector: String,
) -> Result<(), K8sResourceDumperError> {
    log("\t Collecting Kubernetes VolumeSnapshotClass resources");
    match k8s_client
        .list_volumesnapshot_classes(Some(&driver_selector), None, None)
        .await
    {
        Ok(vsclasses) => {
            // NOTE: Unmarshalling object received from K8s API-server will not fail
            if !vsclasses.is_empty() {
                create_file_and_write(
                    root_dir.to_path_buf(),
                    "volume_snapshot_classes.yaml".to_string(),
                    serde_yaml::to_string(&vsclasses)?,
                )
                .map_err(K8sResourceDumperError::IOError)?;
            }
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}

/// Get all the volume snapshot contents for a given driver.
pub async fn get_k8s_vsnapshot_contents(
    k8s_client: &ClientSet,
    root_dir: &Path,
    driver_selector: String,
) -> Result<(), K8sResourceDumperError> {
    log("\t Collecting Kubernetes VolumeSnapshotContents resources");
    match k8s_client
        .list_volumesnapshotcontents(Some(&driver_selector), None, None)
        .await
    {
        Ok(vscs) => {
            // NOTE: Unmarshalling object received from K8s API-server will not fail
            if !vscs.is_empty() {
                create_file_and_write(
                    root_dir.to_path_buf(),
                    "volume_snapshot_contents.yaml".to_string(),
                    serde_yaml::to_string(&vscs)?,
                )
                .map_err(K8sResourceDumperError::IOError)?;
            }
            Ok(())
        }
        Err(error) => Err(K8sResourceDumperError::K8sResourceError(error)),
    }
}
