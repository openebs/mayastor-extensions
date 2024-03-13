use crate::{
    client::grpc_client::{init_client, GrpcClient},
    error::ExporterError,
    serve::metric_route,
};
use actix_web::{middleware, HttpServer};
use clap::Parser;
use once_cell::sync::OnceCell;
use std::{env, net::SocketAddr};
use utils::tracing_telemetry::{FmtLayer, FmtStyle};

/// Cache module for exporter.
pub(crate) mod cache;
/// Grpc client module.
pub(crate) mod client;
/// Collector module.
pub(crate) mod collector;
/// Error module.
pub(crate) mod error;
/// Prometheus metrics handler module.
pub(crate) mod serve;

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
pub(crate) struct Cli {
    /// TCP address where prometheus endpoint will listen to
    #[clap(long, short, default_value = "0.0.0.0:9502")]
    metrics_endpoint: SocketAddr,

    /// Formatting style to be used while logging.
    #[clap(default_value = FmtStyle::Pretty.as_ref(), short, long)]
    fmt_style: FmtStyle,

    /// Use ANSI colors for the logs.
    #[clap(long, default_value_t = true, action = clap::ArgAction::Set)]
    ansi_colors: bool,
}

impl Cli {
    fn args() -> Self {
        Cli::parse()
    }
}

static GRPC_CLIENT: OnceCell<GrpcClient> = OnceCell::new();

/// Get IO engine gRPC Client.
pub(crate) fn grpc_client<'a>() -> &'a GrpcClient {
    GRPC_CLIENT
        .get()
        .expect("gRPC Client should have been initialised")
}

#[tokio::main]
async fn main() -> Result<(), ExporterError> {
    let args = Cli::args();
    utils::print_package_info!();

    utils::tracing_telemetry::TracingTelemetry::builder()
        .with_writer(FmtLayer::Stdout)
        .with_style(args.fmt_style)
        .with_colours(args.ansi_colors)
        .init("metrics-exporter-io_engine");

    initialize_cache().await;
    let client = init_client().await?;
    // Initialize io engine gRPC client.
    GRPC_CLIENT
        .set(client)
        .expect("Expect to be initialised only once");
    let app = move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .configure(metric_route)
    };
    HttpServer::new(app)
        .bind(args.metrics_endpoint)
        .map_err(|_| {
            ExporterError::HttpBindError("Failed to bind endpoint to http server".to_string())
        })?
        .workers(1)
        .run()
        .await
        .map_err(|_| ExporterError::HttpServerError("Failed to start http Service".to_string()))?;
    Ok(())
}
