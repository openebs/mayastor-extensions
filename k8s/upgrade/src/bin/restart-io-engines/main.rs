use clap::Parser;
use openapi::clients::tower::ApiClient;
use opts::CliArgs;
use std::{path::PathBuf, time::Duration};
use tracing::error;
use upgrade::{
    constants::job_constants::PRODUCT, data_plane_upgrade::upgrade_data_plane,
    error::job_error::Result, rest_client::RestClientSet,
};
use utils::{
    print_package_info, raw_version_str,
    tracing_telemetry::{default_tracing_tags, flush_traces, TracingTelemetry},
};

mod opts;

#[tokio::main]
async fn main() -> Result<()> {
    print_package_info!();

    let opts = CliArgs::parse();

    init_logging(&opts);

    let rest_client = rest_client(opts.namespace(), opts.kube_config()).await?;

    upgrade_data_plane(
        opts.namespace(),
        RestClientSet::new_from_api_client(rest_client),
        "i_am_not_a_version".to_string(),
        true,
    )
    .await
    .map_err(|error| {
        error!(%error, "Failed to perpetually restart {PRODUCT} io-engines");
        flush_traces();
        error
    })
}

fn init_logging(opts: &CliArgs) {
    let tags = default_tracing_tags(raw_version_str(), env!("CARGO_PKG_VERSION"));

    TracingTelemetry::builder()
        .with_tracing_tags(tags)
        .with_style(opts.fmt_style())
        .with_colours(opts.ansi_colours())
        .init("restart-io-engines");
}

async fn rest_client(namespace: String, kube_config: Option<PathBuf>) -> Result<ApiClient> {
    let config = kube_proxy::ConfigBuilder::default_api_rest()
        .with_kube_config(kube_config)
        .with_timeout(Duration::from_secs(10))
        .with_target_mod(|t| t.with_namespace(namespace))
        .build()
        .await
        .expect("failed to create REST client config");

    Ok(ApiClient::new(config))
}
