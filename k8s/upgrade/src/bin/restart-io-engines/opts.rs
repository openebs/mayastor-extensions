use clap::Parser;
use std::path::PathBuf;
use upgrade::constants::job_constants::PRODUCT;
use utils::{package_description, tracing_telemetry::FmtStyle, version_info_str};

/// These are the supported cli configuration options for upgrade.
#[derive(Parser)]
#[command(name = package_description!(), version = version_info_str!())]
#[command(about = format!("Restarts all of the {} io-engine Pods", PRODUCT), long_about = None)]
pub(crate) struct CliArgs {
    /// This is the Kubernetes Namespace for the Helm release.
    #[clap(short, long)]
    namespace: String,

    /// Set the path to the kubeconfig file.
    #[clap(long = "kubeconfig", env = "KUBECONFIG")]
    kube_config: Option<PathBuf>,

    /// Formatting style to be used while logging.
    #[clap(default_value = FmtStyle::Pretty.as_ref(), short, long)]
    fmt_style: FmtStyle,

    /// Use ANSI colors for the logs.
    #[clap(long)]
    ansi_colors: bool,
}

impl CliArgs {
    /// This returns the Kubernetes Namespace for the Helm chart release.
    pub(crate) fn namespace(&self) -> String {
        self.namespace.clone()
    }

    /// This returns path to the kubeconfig file
    pub(crate) fn kube_config(&self) -> Option<PathBuf> {
        self.kube_config.clone()
    }

    /// This returns formatting style to be used.
    pub(crate) fn fmt_style(&self) -> FmtStyle {
        self.fmt_style
    }

    /// This returns ansi_colours arg.
    pub(crate) fn ansi_colours(&self) -> bool {
        self.ansi_colors
    }
}
