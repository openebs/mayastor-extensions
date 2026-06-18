use crate::{
    client::grpc_client::{init_client, GrpcClient},
    error::ExporterError,
    node_status::NodeStatusClient,
    serve::metric_route,
};
use actix_web::{middleware, HttpServer};
use clap::Parser;
use once_cell::sync::OnceCell;
use openapi::tower::client::configuration::{ClientSecurity, TlsMode};
use std::{
    env,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};
use tracing::{info, warn};
use utils::tracing_telemetry::{FmtLayer, FmtStyle};

/// Cache module for exporter.
pub(crate) mod cache;
/// gRPC client module.
pub(crate) mod client;
/// Collector module.
pub(crate) mod collector;
/// Error module.
pub(crate) mod error;
/// Node status module for REST-based metrics.
pub(crate) mod node_status;
/// Prometheus metrics handler module.
pub(crate) mod serve;

/// Initialize cache.
async fn initialize_cache() {
    cache::Cache::initialize(cache::Data::default());
}

/// Get pod ip from env.
fn get_pod_ip() -> Result<IpAddr, ExporterError> {
    let ip = env::var("MY_POD_IP")
        .map_err(|_| ExporterError::PodIPError("Unable to get pod ip".to_string()))?;
    ip.parse::<IpAddr>()
        .map_err(|_| ExporterError::PodIPError("Invalid pod ip".to_string()))
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
    #[clap(long, short, default_value = "[::]:9502")]
    metrics_endpoint: SocketAddr,

    /// Port for the io-engine gRPC server running on the same pod.
    #[clap(long, default_value_t = 10124)]
    grpc_port: u16,

    /// Formatting style to be used while logging.
    #[clap(default_value = FmtStyle::Pretty.as_ref(), short, long)]
    fmt_style: FmtStyle,

    /// Use ANSI colors for the logs.
    #[clap(long, default_value_t = true, action = clap::ArgAction::Set)]
    ansi_colors: bool,

    /// REST endpoint for control-plane API (for node status metrics).
    #[clap(long, env = "MAYASTOR_REST_ENDPOINT")]
    rest_endpoint: Option<tonic::transport::Uri>,

    /// Timeout for node status REST requests.
    #[clap(long, default_value = "10s", env = "MAYASTOR_SCRAPE_TIMEOUT")]
    scrape_timeout: humantime::Duration,

    /// Path to the TLS CA certificate bundle used to validate REST server certificates.
    #[clap(long)]
    tls_ca_file: Option<PathBuf>,

    /// Path to the client TLS certificate chain file (PEM) for mTLS.
    #[clap(long, requires = "tls_key_file")]
    tls_cert_file: Option<PathBuf>,

    /// Path to the client TLS private key file (PEM) for mTLS.
    #[clap(long, requires = "tls_cert_file")]
    tls_key_file: Option<PathBuf>,

    /// Path to a file containing the JWT bearer token for REST authentication.
    #[clap(long)]
    jwt: Option<PathBuf>,
}

static GRPC_CLIENT: OnceCell<GrpcClient> = OnceCell::new();
static NODE_STATUS_CLIENT: OnceCell<NodeStatusClient> = OnceCell::new();
static NODE_NAME: OnceCell<String> = OnceCell::new();

/// Get IO engine gRPC Client.
pub(crate) fn grpc_client<'a>() -> &'a GrpcClient {
    GRPC_CLIENT
        .get()
        .expect("gRPC Client should have been initialised")
}

/// Get node status REST client, if configured.
pub(crate) fn node_status_client() -> Option<&'static NodeStatusClient> {
    NODE_STATUS_CLIENT.get()
}

/// Get the name of the node this exporter is running on.
pub(crate) fn node_name() -> &'static str {
    NODE_NAME
        .get()
        .expect("Node name should have been initialised")
}

#[tokio::main]
async fn main() -> Result<(), ExporterError> {
    let args = Cli::parse();
    utils::print_package_info!();

    utils::tracing_telemetry::TracingTelemetry::builder()
        .with_writer(FmtLayer::Stdout)
        .with_style(args.fmt_style)
        .with_colours(args.ansi_colors)
        .init("metrics-exporter-io_engine");

    initialize_cache().await;
    let client = init_client(args.grpc_port).await?;
    // Initialize io engine gRPC client.
    GRPC_CLIENT
        .set(client)
        .expect("Expect to be initialised only once");

    // Store node name for use by the metrics handler on every scrape.
    NODE_NAME
        .set(get_node_name()?)
        .expect("Node name should be initialised only once");

    // Initialize node status REST client if endpoint is configured.
    if let Some(ref endpoint) = args.rest_endpoint {
        info!("Initializing node status REST client with endpoint: {endpoint}");
        let tls = TlsMode::new(
            args.tls_ca_file.as_ref(),
            args.tls_cert_file.as_ref(),
            args.tls_key_file.as_ref(),
        )
        .map_err(|error| {
            ExporterError::HttpServerError(format!("Failed to create TLS config: {error}"))
        })?;
        let security = ClientSecurity::try_new(&args.jwt, tls).map_err(|error| {
            ExporterError::HttpServerError(format!("Failed to read JWT file: {error}"))
        })?;
        let node_client = NodeStatusClient::new(
            &endpoint.to_string(),
            *args.scrape_timeout,
            security,
        )
        .map_err(|error| {
            ExporterError::HttpServerError(format!("Failed to create node status client: {error}"))
        })?;
        NODE_STATUS_CLIENT
            .set(node_client)
            .expect("Node status client should be initialised only once");
    } else {
        warn!("REST endpoint not configured, node status metrics will not be available");
    }

    let app = move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .configure(metric_route)
    };
    HttpServer::new(app)
        .bind(args.metrics_endpoint)
        .map_err(|error| {
            ExporterError::HttpBindError(format!("Failed to bind endpoint to http server: {error}"))
        })?
        .workers(1)
        .run()
        .await
        .map_err(|error| {
            ExporterError::HttpServerError(format!("Failed to start http Service: {error}"))
        })?;
    Ok(())
}
