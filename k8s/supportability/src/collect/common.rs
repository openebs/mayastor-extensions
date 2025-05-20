use crate::collect::error::Error;

use chrono::Local;

/// DumpConfig helps to create new instance of Dumper
#[derive(Debug)]
pub struct DumpConfig {
    /// directory path to create archive files
    output_directory: String,
    /// namespace of mayastor system
    namespace: String,
    /// Address of Loki service endpoint
    loki_uri: Option<String>,
    /// Address of etcd service endpoint
    etcd_uri: Option<String>,
    /// Period states to collect logs from specified duration
    since: humantime::Duration,
    /// Path to kubeconfig file, which requires to interact with Kube-Apiserver
    kube_config_path: Option<std::path::PathBuf>,
    /// Specifies the timeout value to interact with other systems
    timeout: humantime::Duration,
    /// Specfies the output format, i.e tar, stdout.
    output_format: OutputFormat,
    /// Tenant ID that needs to be passed while querying.
    tenant_id: String,
    /// Logging label selectors.
    logging_label_selectors: String,
}

impl DumpConfig {
    /// Creates a new instance of `DumpConfig`.
    ///
    /// # Arguments
    ///
    /// * `output_directory` - Directory path to create archive files.
    /// * `namespace` - Namespace of the Mayastor system.
    /// * `loki_uri` - Optional address of the Loki service endpoint.
    /// * `etcd_uri` - Optional address of the etcd service endpoint.
    /// * `since` - Duration from which to collect logs.
    /// * `kube_config_path` - Optional path to the kubeconfig file.
    /// * `timeout` - Timeout duration for interacting with external systems.
    /// * `output_format` - Output format (e.g., tar, stdout).
    /// * `tenant_id` - Tenant ID used while querying.
    /// * `logging_label_selectors` - Label selectors used for filtering logs.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        output_directory: String,
        namespace: String,
        loki_uri: Option<String>,
        etcd_uri: Option<String>,
        since: humantime::Duration,
        kube_config_path: Option<std::path::PathBuf>,
        timeout: humantime::Duration,
        output_format: OutputFormat,
        tenant_id: String,
        logging_label_selectors: String,
    ) -> Self {
        Self {
            output_directory,
            namespace,
            loki_uri,
            etcd_uri,
            since,
            kube_config_path,
            timeout,
            output_format,
            tenant_id,
            logging_label_selectors,
        }
    }

    /// Returns the directory path where archive files will be created.
    pub fn output_directory(&self) -> &str {
        &self.output_directory
    }

    /// Returns the namespace of the Mayastor system.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the optional address of the Loki service endpoint.
    pub fn loki_uri(&self) -> Option<&String> {
        self.loki_uri.as_ref()
    }

    /// Returns the optional address of the etcd service endpoint.
    pub fn etcd_uri(&self) -> Option<&String> {
        self.etcd_uri.as_ref()
    }

    /// Returns the duration from which logs should be collected.
    pub fn since(&self) -> &humantime::Duration {
        &self.since
    }

    /// Returns the optional path to the kubeconfig file used to interact with the Kube-Apiserver.
    pub fn kube_config_path(&self) -> Option<&std::path::PathBuf> {
        self.kube_config_path.as_ref()
    }

    /// Returns the timeout duration used to interact with external systems.
    pub fn timeout(&self) -> &humantime::Duration {
        &self.timeout
    }

    /// Returns the specified output format (e.g., tar, stdout).
    pub fn output_format(&self) -> &OutputFormat {
        &self.output_format
    }

    /// Returns the tenant ID used while querying.
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Returns the logging label selectors used to filter logs.
    pub fn logging_label_selectors(&self) -> &str {
        &self.logging_label_selectors
    }

    /// Sets the output format (e.g., tar, stdout).
    ///
    /// # Arguments
    ///
    /// * `format` - The desired `OutputFormat`.
    pub fn set_output_format(&mut self, format: OutputFormat) {
        self.output_format = format;
    }
}

/// The output format.
#[derive(Debug, Clone)]
pub enum OutputFormat {
    /// A tar file.
    Tar,
    /// The STDOUT.
    Stdout,
}

/// Defines prefix name of temporary directory to create dump files
pub(crate) const DUMP_TMP_PREFIX: &str = "tmp-";

/// Creates new temporary directory in given path to store dump artifacts
pub(crate) fn create_and_get_tmp_directory(
    dir_path: String,
    archive_prefix: &str,
) -> Result<String, Error> {
    let date = Local::now();
    let suffix_dir_name = format!(
        "{}{}-{}",
        DUMP_TMP_PREFIX,
        archive_prefix,
        date.format("%Y-%m-%d-%H-%M-%S")
    );
    let new_dir_path = std::path::Path::new(&dir_path).join(suffix_dir_name);
    std::fs::create_dir_all(new_dir_path.clone())?;
    Ok(new_dir_path.into_os_string().into_string()?)
}
