use crate::common::constants::PRODUCT;
use clap::Parser;
use std::path::PathBuf;
use utils::{package_description, version_info_str};

/// Validate input whose validation depends on other inputs.
pub(crate) mod validators;

/// These are the supported cli configuration options for upgrade.
#[derive(Parser)]
#[command(name = package_description!(), version = version_info_str!())]
#[command(about = format!("Upgrades {}", PRODUCT), long_about = None)]
pub(crate) struct CliArgs {
    /// This is the URL for the storage REST API server.
    #[arg(short = 'e', long)]
    rest_endpoint: String,

    /// This is the Kubernetes Namespace for the Helm release.
    #[arg(short, long)]
    namespace: String,

    /// This is the release name of the installed Helm chart.
    #[arg(long)]
    release_name: String,

    /// This is the Helm chart directory filepath for the core Helm chart variant.
    #[arg(long, env = "CORE_CHART_DIR", value_name = "DIR PATH")]
    core_chart_dir: PathBuf,

    /// If not set, this skips the Kubernetes Pod restarts for the io-engine DaemonSet.
    #[arg(long, default_value_t = false)]
    skip_data_plane_restart: bool,

    /// If set then this skips the upgrade path validation.
    #[arg(long, default_value_t = false)]
    skip_upgrade_path_validation: bool,

    /// The name of the Kubernetes Job Pod. The Job object will be used to post upgrade event.
    #[arg(env = "POD_NAME")]
    pod_name: String,
}

impl CliArgs {
    /// This returns the URL to the storage REST API.
    pub(crate) fn rest_endpoint(&self) -> String {
        self.rest_endpoint.clone()
    }

    /// This returns the Kubernetes Namespace for the Helm chart release.
    pub(crate) fn namespace(&self) -> String {
        self.namespace.clone()
    }

    /// This returns the Helm release name for the installed Helm chart.
    pub(crate) fn release_name(&self) -> String {
        self.release_name.clone()
    }

    /// This returns the Helm chart directory filepath for a crate::helm::upgrade::HelmChart::Core.
    pub(crate) fn core_chart_dir(&self) -> PathBuf {
        self.core_chart_dir.clone()
    }

    /// This is a predicate to decide if <release-name>-io-engine Kubernetes DaemonSet Pods should
    /// be restarted as a part of the data-plane upgrade.
    pub(crate) fn skip_data_plane_restart(&self) -> bool {
        self.skip_data_plane_restart
    }

    /// This decides to skip upgrade path validation or not.
    pub(crate) fn skip_upgrade_path_validation(&self) -> bool {
        self.skip_upgrade_path_validation
    }

    /// This returns the name of the Kubernetes Pod where this binary will be running.
    pub(crate) fn pod_name(&self) -> String {
        self.pod_name.clone()
    }
}
