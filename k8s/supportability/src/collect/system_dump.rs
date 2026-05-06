use crate::collect::k8s_resources::client::ClientSet;
use crate::collect::rest_wrapper;
use crate::{
    collect::{
        archive, common,
        common::DumpConfig,
        error::Error,
        k8s_resources::k8s_resource_dump::K8sResourceDumperClient,
        logs::{LogCollection, Logger},
        persistent_store::etcd::EtcdStore,
        resources::{
            node::NodeClientWrapper, pool::PoolClientWrapper,
            snapshot::VolumeSnapshotClientWrapper, volume::VolumeClientWrapper, Resourcer,
        },
        rest_wrapper::RestClient,
        utils::{flush_tool_log_file, init_tool_log_file, write_to_log_file},
    },
    log,
};

use futures::future;
use std::path::Path;
use std::{path::PathBuf, process};

/// SystemDumper interacts with various services to collect information like mayastor resource(s),
/// logs of mayastor service and state of mayastor artifacts in etcd
pub struct SystemDumper {
    rest_client: Option<RestClient>,
    archive: archive::Archive,
    dir_path: String,
    logger: Box<dyn Logger>,
    k8s_resource_dumper: K8sResourceDumperClient,
    etcd_dumper: Option<EtcdStore>,
    logging_label_selectors: String,
}

impl SystemDumper {
    /// Instantiate new system dumper by performing following actions:
    /// 1.1 Create new archive in given directory and create temporary directory
    /// in given directory to generate dump files
    /// 1.2 Instantiate all required objects to interact with various other modules
    pub async fn get_or_panic_system_dumper(config: DumpConfig, archive_prefix: &str) -> Self {
        // Creates a temporary directory inside user provided directory, to store
        // artifacts. If creation is failed then we can't continue the process.
        let new_dir = match common::create_and_get_tmp_directory(
            config.output_directory().to_string(),
            archive_prefix,
        ) {
            Ok(val) => val,
            Err(e) => {
                println!("Failed to create temporary directory to dump information, error: {e:?}");
                process::exit(1);
            }
        };

        // Create and initialise the support tool log file
        init_tool_log_file(PathBuf::from(format!("{new_dir}/support_tool_logs.log")))
            .expect("Support Tool Log file should be created");

        log(format!("Plugin {}", utils::version_info_str!()));

        // Creates an arcive file to dump mayastor resource information. If creation
        // of archive is failed then we can't continue process
        let archive = match archive::Archive::new(
            Some(config.output_directory().to_string()),
            archive_prefix,
        ) {
            Ok(val) => val,
            Err(err) => {
                log(format!("Failed to create archive archive, error: {err:?}"));
                process::exit(1);
            }
        };

        let logger = match LogCollection::new_logger(
            config.kubeconfig().clone(),
            config.namespace().to_string(),
            config.loki_uri().cloned(),
            *config.since(),
            *config.timeout(),
            config.tenant_id().to_string(),
        )
        .await
        {
            Ok(val) => val,
            Err(err) => {
                log(format!(
                    "Failed to initialize logging service, error: {err:?}"
                ));
                process::exit(1);
            }
        };

        let k8s_resource_dumper = match K8sResourceDumperClient::new(
            config.kubeconfig().clone(),
            config.namespace().to_string(),
        )
        .await
        {
            Ok(val) => val,
            Err(err) => {
                log(format!(
                    "Failed to instantiate K8s resource dumper, error: {err:?}"
                ));
                process::exit(1);
            }
        };

        let etcd_dumper = match EtcdStore::new(
            config.kubeconfig().clone(),
            config.etcd_uri().cloned(),
            config.namespace().to_string(),
        )
        .await
        {
            Ok(val) => Some(val),
            Err(err) => {
                log(format!("Failed to initialize etcd client, error: {err:?}"));
                None
            }
        };

        let rest_client = match kube_proxy::ConfigBuilder::default_api_rest()
            .with_kube_config(config.kube_config_path().cloned())
            .with_context(config.kube_config_opts().context.clone())
            .with_timeout(Some((*config.timeout()).into()))
            .with_target_mod(|t| t.with_namespace(config.namespace()))
            .build()
            .await
        {
            Ok(config) => Some(rest_wrapper::RestClient::new_with_config(config)),
            Err(error) => {
                log(format!("Can't create rest client: {error}"));
                None
            }
        };

        SystemDumper {
            rest_client,
            archive,
            dir_path: new_dir,
            logger,
            k8s_resource_dumper,
            etcd_dumper,
            logging_label_selectors: config.logging_label_selectors().to_string(),
        }
    }

    /// Collect and dump loki logs across all logging services.
    pub async fn collect_and_dump_loki_logs(&mut self) -> Result<(), Error> {
        log(format!(
            "Label selectors : {}",
            self.logging_label_selectors
        ));
        // Fetch required logging resources
        let resources = self
            .logger
            .get_logging_services(self.logging_label_selectors.clone())
            .await?;

        let _ = write_to_log_file(format!(
            "Collecting logs of following services: \n {resources:#?}"
        ));

        log("Collecting logs...".to_string());
        if let Err(error) = self
            .logger
            .fetch_and_dump_logs(resources, self.dir_path.clone())
            .await
        {
            log("Error occurred while collecting logs".to_string());
            return Err(Error::LogCollectionError(error));
        }
        log("Completed collection of logs".to_string());
        Ok(())
    }

    /// Copies the temporary directory into archive and delete temporary directory
    pub fn fill_archive_and_delete_tmp(&mut self) -> Result<(), Error> {
        // Log which is visible in archive system log file
        let _ = write_to_log_file("Will copy temporary directory content to archive".to_string());
        // Flush log file before copying contents
        flush_tool_log_file()?;

        // Copy folder into archive
        self.archive
            .copy_to_archive(self.dir_path.clone(), ".".to_string())
            .map_err(|e| {
                log(format!(
                    "Failed to move content into archive file, error: {e}"
                ));
                e
            })?;

        self.delete_temporary_directory().map_err(|e| {
            log(format!(
                "Failed to delete temporary directory, error: {e:?}"
            ));
            e
        })?;
        Ok(())
    }

    /// Dumps the state of the system.
    pub async fn dump_common_k8s_resources(&mut self) -> Result<(), Error> {
        self.k8s_resource_dumper
            .dump_common_k8s_resources(self.dir_path.clone())
            .await?;
        Ok(())
    }

    fn delete_temporary_directory(&self) -> Result<(), Error> {
        std::fs::remove_dir_all(self.dir_path.clone())?;
        Ok(())
    }

    /// Get the k8s client set.
    pub fn k8s_client(&self) -> &ClientSet {
        self.k8s_resource_dumper.client_set()
    }

    /// Get the root dir path.
    pub fn dir_path(&self) -> PathBuf {
        PathBuf::from(&self.dir_path)
    }
}

/// Mayastor specific dump operations.
impl SystemDumper {
    pub async fn dump_mayastor(&mut self) -> Result<(), Error> {
        let mut errors: Vec<Error> = Vec::new();

        let mut path: PathBuf = std::path::PathBuf::new();
        path.push(self.dir_path.clone());
        path.push("mayastor");

        if let Err(e) = self.dump_mayastor_resource_topology(&path).await {
            errors.push(e);
        }

        if let Err(e) = self.dump_mayastor_k8s_resources(&path).await {
            errors.push(e);
        }

        if let Err(e) = self.dump_mayastor_etcd(&path).await {
            errors.push(e);
        }

        if !errors.is_empty() {
            log("Failed to dump system state".to_string());
            return Err(Error::MultipleErrors(errors));
        }
        Ok(())
    }

    /// Dump Mayastor resource topology
    pub(crate) async fn dump_mayastor_resource_topology(
        &mut self,
        path: &Path,
    ) -> Result<(), Error> {
        let rest_client = match self.rest_client.clone() {
            None => {
                log(
                    "Skipping topology information collection as rest client is not available"
                        .to_string(),
                );
                return Err(Error::Generic("Failed to get rest client".to_string()));
            }
            Some(client) => client,
        };

        let mut errors: Vec<Error> = Vec::new();

        log("Collecting topology information...".to_string());
        // Dump information of all volume topologies exist in the system
        match VolumeClientWrapper::new(rest_client.clone())
            .get_topologer(None)
            .await
        {
            Ok(topologer) => {
                log("\t Collecting volume topology information".to_string());
                let mut vol_topo_path = path.to_path_buf();
                vol_topo_path.push("topology");
                vol_topo_path.push("volume");

                let _ = topologer.dump_topology_info(vol_topo_path).map_err(|e| {
                    log(format!(
                        "\t Failed to dump volume topology information, {e:?}"
                    ));
                    errors.push(Error::ResourceError(e));
                });
            }
            Err(e) => errors.push(Error::ResourceError(e)),
        };

        match VolumeSnapshotClientWrapper::new(rest_client.clone())
            .get_topologer(None)
            .await
        {
            Ok(topologer) => {
                log("\t Collecting snapshot topology information".to_string());
                let mut vol_snap_topo_path = path.to_path_buf();
                vol_snap_topo_path.push("topology");
                vol_snap_topo_path.push("snapshot");

                let _ = topologer
                    .dump_topology_info(vol_snap_topo_path)
                    .map_err(|e| {
                        log(format!(
                            "\t Failed to dump snapshot topology information, {e:?}"
                        ));
                        errors.push(Error::ResourceError(e));
                    });
            }
            Err(e) => errors.push(Error::ResourceError(e)),
        };

        // Dump information of all pools topologies exist in the system
        match PoolClientWrapper::new(rest_client.clone())
            .get_topologer(None)
            .await
        {
            Ok(topologer) => {
                log("\t Collecting pool topology information".to_string());
                let mut pool_topo_path = path.to_path_buf();
                pool_topo_path.push("topology");
                pool_topo_path.push("pool");

                let _ = topologer.dump_topology_info(pool_topo_path).map_err(|e| {
                    log(format!(
                        "\t Failed to dump pool topology information, {e:?}"
                    ));
                    errors.push(Error::ResourceError(e));
                });
            }
            Err(e) => errors.push(Error::ResourceError(e)),
        };

        match NodeClientWrapper::new(rest_client)
            .get_topologer(None)
            .await
        {
            Ok(topologer) => {
                log("\t Collecting node topology information".to_string());
                let mut node_topo_path = path.to_path_buf();
                node_topo_path.push("topology");
                node_topo_path.push("node");

                let _ = topologer.dump_topology_info(node_topo_path).map_err(|e| {
                    log(format!(
                        "\t Failed to dump node topology information, {e:?}"
                    ));
                    errors.push(Error::ResourceError(e));
                });
                Some(topologer)
            }
            Err(e) => {
                errors.push(Error::ResourceError(e));
                None
            }
        };
        log("Completed collection of topology information".to_string());
        Ok(())
    }

    /// Dumps the mayastor specific resources.
    pub(crate) async fn dump_mayastor_k8s_resources(&mut self, path: &Path) -> Result<(), Error> {
        self.k8s_resource_dumper
            .dump_mayastor_k8s_resources(path, None)
            .await?;
        Ok(())
    }

    /// Dumps the mayastor etcd.
    pub(crate) async fn dump_mayastor_etcd(&mut self, path: &Path) -> Result<(), Error> {
        let _ = future::try_join_all(self.etcd_dumper.as_mut().map(|etcd_store| {
            log("Collecting mayastor specific information from Etcd...".to_string());
            etcd_store.dump(path.to_path_buf(), false)
        }))
        .await
        .map_err(|e| {
            log(format!(
                "\t Failed to collect etcd dump information, error: {e:?}"
            ));
            Error::EtcdDumpError(e)
        })?;

        Ok(())
    }
}
