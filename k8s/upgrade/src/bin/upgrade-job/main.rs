use crate::{
    common::{constants::product_pascal, error::Result},
    opts::validators::{
        validate_helm_chart_dir, validate_helm_release, validate_helmv3_in_path,
        validate_namespace, validate_rest_endpoint,
    },
    upgrade::upgrade,
};
use clap::Parser;
use opts::CliArgs;
use tracing::{error, info};
use utils::{
    print_package_info, raw_version_str,
    tracing_telemetry::{default_tracing_tags, flush_traces, TracingTelemetry},
};

mod common;
mod events;
mod helm;
mod opts;
mod upgrade;

#[tokio::main]
async fn main() -> Result<()> {
    print_package_info!();
    init_logging();

    let opts = parse_cli_args().await.map_err(|error| {
        error!(%error, "Failed to upgrade {}", product_pascal());
        error
    })?;

    upgrade(&opts).await.map_err(|error| {
        error!(%error, "Failed to upgrade {}", product_pascal());
        flush_traces();
        error
    })
}

/// Initialize logging components -- tracing.
fn init_logging() {
    let tags = default_tracing_tags(raw_version_str(), env!("CARGO_PKG_VERSION"));

    let opts = CliArgs::parse();
    TracingTelemetry::builder()
        .with_tracing_tags(tags)
        .with_style(opts.fmt_style())
        .with_colours(opts.ansi_colours())
        .init("upgrade-job");
}

/// This function handles the following tasks -- 1. Argument parsing, 2. Validating arguments whose
/// validation depends on other arguments.
pub(crate) async fn parse_cli_args() -> Result<CliArgs> {
    let opts = CliArgs::parse();

    validate_namespace(opts.namespace()).await?;
    validate_rest_endpoint(opts.rest_endpoint()).await?;

    validate_helmv3_in_path()?;
    validate_helm_release(opts.release_name(), opts.namespace())?;
    validate_helm_chart_dir(opts.core_chart_dir())?;

    info!("Validated all inputs");

    Ok(opts)
}
