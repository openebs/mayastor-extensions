use crate::{
    cache::store_data,
    client::{grpc_client::init_client, ApiVersion},
    config::ExporterConfig,
    error::ExporterError,
    serve::metric_route,
};
use actix_web::{middleware, HttpServer};
use clap::Parser;
use std::{env, net::SocketAddr};

/// Cache module for exporter.
pub(crate) mod cache;
/// Grpc client module.
pub(crate) mod client;
/// Collector module.
pub(crate) mod collector;
/// Config module for metrics-exporter.
pub(crate) mod config;
/// Error module.
pub(crate) mod error;
/// Prometheus metrics handler module.
pub(crate) mod serve;

/// Initialize metrics-exporter config that are passed through arguments.
fn initialize_exporter(args: &Cli) {
    ExporterConfig::initialize(args.metrics_endpoint, args.polling_time.into());
}

/// Initialize cache.
async fn initialize_cache() {
    cache::Cache::initialize(cache::Data::default());
}

/// Get pod ip from env.
fn get_pod_ip() -> Result<String, ExporterError> {
    env::var("MY_POD_IP").map_err(|_| ExporterError::PodIPError("Unable to get pod ip".to_string()))
}

/// Get node name from env.
fn get_node_name() -> Result<String, ExporterError> {
    env::var("MY_NODE_NAME")
        .map_err(|_| ExporterError::GetNodeError("Unable to get node name".to_string()))
}

#[derive(Parser, Debug)]
#[clap(name = utils::package_description!(), version = utils::version_info_str!())]
struct Cli {
    /// TCP address where prometheus endpoint will listen to
    #[clap(long, short, default_value = "0.0.0.0:9502")]
    metrics_endpoint: SocketAddr,

    /// Polling time in seconds to get pools data through gRPC calls
    #[clap(short, long, default_value = "300s")]
    polling_time: humantime::Duration,

    /// Io engine api versions
    #[clap(short, long, value_delimiter = ',', required = true)]
    api_versions: Vec<ApiVersion>,
}

impl Cli {
    fn args() -> Self {
        Cli::parse()
    }
}

#[tokio::main]
async fn main() -> Result<(), ExporterError> {
    let args = Cli::args();

    utils::print_package_info!();

    utils::tracing_telemetry::init_tracing("metrics-exporter-io_engine", vec![], None);

    initialize_exporter(&args);

    initialize_cache().await;

    // sort to get the latest api version
    let mut api_versions = args.api_versions;
    api_versions.sort_by(|a, b| b.cmp(a));

    let client = init_client(api_versions.get(0).unwrap_or(&ApiVersion::V0).clone()).await?;

    store_data(client).await;
    let app = move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .configure(metric_route)
    };
    HttpServer::new(app)
        .bind(ExporterConfig::get_config().metrics_endpoint())
        .map_err(|_| {
            ExporterError::HttpBindError("Failed to bind endpoint to http server".to_string())
        })?
        .workers(1)
        .run()
        .await
        .map_err(|_| ExporterError::HttpServerError("Failed to start http Service".to_string()))?;
    Ok(())
}
