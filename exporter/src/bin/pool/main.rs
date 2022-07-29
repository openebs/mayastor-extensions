use std::env;
use std::time::Duration;

use actix_web::http::header;
use actix_web::{middleware, web, HttpResponse, HttpServer, Responder};
use clap::{App, Arg, ArgMatches};
use prometheus::{Encoder, Registry};

use exporter::{
    cache,
    cache::{Cache, Data},
    client::grpc_client::{GrpcClient, GrpcContext, Timeouts},
    collector::pools_collector::PoolsCollector,
    config::ExporterConfig,
    error::ExporterError,
};

/// Initialize exporter config that are passed through arguments
fn initialize_exporter(args: &ArgMatches) {
    ExporterConfig::initialize(args);
}

/// Initialize mayastor grpc client
async fn initialize_client() -> Result<GrpcClient, ExporterError> {
    let t = Timeouts::new(Duration::from_millis(500), Duration::from_millis(500));
    let pod_ip = get_pod_ip()?;
    let endpoint = format!("{}:10124", pod_ip);
    let ctx = match GrpcContext::new(endpoint.as_str(), t) {
        Ok(ctx) => ctx,
        Err(_) => {
            return Err(ExporterError::GrpcContextError(
                "gRPC context error {}".to_string(),
            ));
        }
    };
    let client = GrpcClient::new(ctx).await?;
    Ok(client)
}

/// Initialize cache
async fn initialize_cache() {
    Cache::initialize(Data::default());
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = App::new("Pool metrics exporter")
        .author(clap::crate_authors!())
        .version(clap::crate_version!())
        .settings(&[
            clap::AppSettings::ColoredHelp,
            clap::AppSettings::ColorAlways,
        ])
        .arg(
            Arg::with_name("metrics-endpoint")
                .long("metrics-endpoint")
                .short("-m")
                .default_value("9502")
                .help("TCP address where prometheus endpoint will listen to"),
        )
        .arg(
            Arg::with_name("polling-time")
                .long("polling-time")
                .short("-p")
                .default_value("300s")
                .help("Polling time in seconds to get pools data through gRPC calls"),
        )
        .get_matches();

    initialize_exporter(&args);

    initialize_cache().await;

    let client = initialize_client()
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
    if let Err(_e) = Registry::register(&registry, Box::new(pools_collector)) {
        println!("Pools collector already registered");
    }
    let mut buffer = Vec::new();

    let encoder = prometheus::TextEncoder::new();
    // Starts collecting metrics via calling gatherers
    if let Err(error) = encoder.encode(&registry.gather(), &mut buffer) {
        println!("could not encode custom metrics: {}", error);
    };

    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            println!("prometheus metrics could not be parsed from_utf8'd: {}", e);
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
