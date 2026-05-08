use crate::{
    collect::{
        archive, common,
        common::DumpConfig,
        error::Error,
        persistent_store::{etcd::EtcdStore, EtcdError},
        utils::{init_no_log_file, init_tool_log_file},
    },
    log, OutputFormat,
};

use std::{path::PathBuf, process};

/// Dumper interacts with various services to collect information like mayastor resource(s),
/// mayastor service logs and state of mayastor artifacts and mayastor specific artifacts from
/// etcd
pub(crate) struct ResourceDumper {
    archive: archive::Archive,
    dir_path: String,
    etcd_dumper: Option<EtcdStore>,
    output_format: OutputFormat,
}

impl ResourceDumper {
    /// Instantiate new dumper by performing following actions:
    /// 1.1 Create new archive in given directory and create temporary directory
    /// in given directory to generate dump files
    /// 1.2 Instantiate all required objects to interact with various other modules
    pub(crate) async fn get_or_panic_resource_dumper(
        config: DumpConfig,
        archive_prefix: &str,
    ) -> Self {
        // creates a temporary directory inside given directory
        let (new_dir, output_directory) = match config.output_format() {
            OutputFormat::Tar => {
                let new_dir = match common::create_and_get_tmp_directory(
                    config.output_directory().to_string(),
                    archive_prefix,
                ) {
                    Ok(val) => val,
                    Err(e) => {
                        println!(
                                "Failed to create temporary directory to dump information, error: {e:?}"
                            );
                        process::exit(1);
                    }
                };

                // Create and initialise the support tool log file
                if let Err(e) =
                    init_tool_log_file(PathBuf::from(format!("{new_dir}/support_tool_logs.log")))
                {
                    println!("Encountered error while creating log file: {e} ");
                    process::exit(1);
                }

                (new_dir, Some(config.output_directory().to_string()))
            }
            OutputFormat::Stdout => {
                init_no_log_file();
                ("".into(), None)
            }
        };

        let archive = match archive::Archive::new(output_directory, archive_prefix) {
            Ok(val) => val,
            Err(err) => {
                log(format!("Failed to create archive, {err:?}"));
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

        ResourceDumper {
            archive,
            dir_path: new_dir,
            etcd_dumper,
            output_format: config.output_format().clone(),
        }
    }

    /// Dumps information associated to given resource(s)
    pub(crate) async fn dump_etcd(&mut self) -> Result<(), Error> {
        let mut path: PathBuf = std::path::PathBuf::new();
        path.push(self.dir_path.clone());

        self.etcd_dumper
            .as_mut()
            .ok_or_else(|| EtcdError::Custom("etcd not configured".into()))?
            .dump(path, matches!(self.output_format, OutputFormat::Stdout))
            .await
            .map_err(|e| {
                log(format!(
                    "Failed to collect etcd dump information, error: {e:?}"
                ));
                e
            })?;
        log("Completed collection of etcd dump information");

        if matches!(self.output_format, OutputFormat::Tar) {
            self.archive
                .copy_to_archive(self.dir_path.clone(), ".".to_string())
                .map_err(|e| {
                    log(format!(
                        "Failed to move content into archive file, error: {e}"
                    ));
                    e
                })?;

            let _ = self.delete_temporary_directory().map_err(|e| {
                log(format!(
                    "Failed to delete temporary directory, error: {e:?}"
                ));
            });
        }
        Ok(())
    }

    fn delete_temporary_directory(&self) -> Result<(), Error> {
        std::fs::remove_dir_all(self.dir_path.clone())?;
        Ok(())
    }
}
