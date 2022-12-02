use actix_web::{http::header, middleware, web, HttpResponse, HttpServer, Responder};

use actix_web::http::Uri;
use clap::Parser;
use exporter::pool::{
    cache,
    cache::{Cache, Data},
    client::{
        grpc_client::{GrpcClient, GrpcContext, Timeouts},
        ApiVersion,
    },
    collector::pools_collector::PoolsCollector,
    config::ExporterConfig,
    error::ExporterError,
};
use prometheus::{Encoder, Registry};
use std::{env, net::SocketAddr, time::Duration};
use tracing::{error, warn};

/// Initialize exporter config that are passed through arguments
fn initialize_exporter(args: &Cli) {
    ExporterConfig::initialize(args.metrics_endpoint, args.polling_time.into());
}

/// Initialize mayastor grpc client
async fn initialize_client(api_version: ApiVersion) -> Result<GrpcClient, ExporterError> {
    let timeout = Timeouts::new(Duration::from_secs(1), Duration::from_secs(5));
    let pod_ip = get_pod_ip()?;
    let endpoint = Uri::builder()
        .scheme("https")
        .authority(format!("{}:10124", pod_ip))
        .path_and_query("")
        .build()
        .map_err(|error| ExporterError::InvalidURI(error.to_string()))?;
    let ctx = GrpcContext::new(endpoint, timeout, api_version);
    let client = GrpcClient::new(ctx).await?;
    Ok(client)
}

/// Initialize cache
async fn initialize_cache() {
    Cache::initialize(Data::default());
}

#[derive(Parser, Debug)]
#[clap(name = utils::package_description ! (), version = utils::version_info_str ! ())]
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
async fn main() -> Result<(), String> {
    let args = Cli::args();

    utils::print_package_info!();

    utils::tracing_telemetry::init_tracing("metrics-exporter-pool", vec![], None);

    initialize_exporter(&args);

    initialize_cache().await;

    // sort to get the latest api version
    let mut api_versions = args.api_versions;
    api_versions.sort_by(|a, b| b.cmp(a));

    let client = initialize_client(api_versions.get(0).unwrap_or(&ApiVersion::V0).clone())
        .await
        .expect("gRPC client not initialized");

    tokio::spawn(async move {
        cache::store_data(client)
            .await
            .expect("Unable to store data in cache");
    });

    let app = move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .configure(metric_route)
    };

    HttpServer::new(app)
        .bind(ExporterConfig::get_config().metrics_endpoint())
        .unwrap()
        .run()
        .await
        .expect("Port should be free to expose the metrics");
    Ok(())
}

fn metric_route(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics", web::get().to(metrics_handlers));
}

/// Handler for prometheus
async fn metrics_handlers() -> impl Responder {
    // Initialize pools collector
    let pools_collector = PoolsCollector::default();
    // Create a new registry for prometheus
    let registry = Registry::default();
    // Register pools collector in the registry
    if let Err(error) = Registry::register(&registry, Box::new(pools_collector)) {
        warn!(error=%error, "Pools collector already registered");
    }
    let mut buffer = Vec::new();

    let encoder = prometheus::TextEncoder::new();
    // Starts collecting metrics via calling gatherers
    if let Err(error) = encoder.encode(&registry.gather(), &mut buffer) {
        error!(eror=%error, "Could not encode custom metrics");
    };

    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(error) => {
            error!(error=%error, "Prometheus metrics could not be parsed from_utf8'd");
            String::default()
        }
    };
    HttpResponse::Ok()
        .insert_header(header::ContentType(mime::TEXT_PLAIN))
        .body(res_custom)
}

// get pod ip
fn get_pod_ip() -> Result<String, ExporterError> {
    env::var("MY_POD_IP").map_err(|_| ExporterError::PodIPError("Unable to get pod ip".to_string()))
}
