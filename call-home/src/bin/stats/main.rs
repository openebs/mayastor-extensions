use clap::Parser;

use crate::{
    cache::events_cache::{Cache, EventSet},
    exporter::{events_collector::StatsCollector, exporter_config::ExporterConfig},
    store::events_store::initialize,
};
use actix_web::{http::header, middleware, web, HttpResponse, HttpServer, Responder};
use k8s_openapi::api::core::v1::ConfigMap;
use mbus_api::{
    mbus_nats::{message_bus_init, BusSubscription},
    message::EventMessage,
    Bus,
};
use obs::common::errors;
use prometheus::{Encoder, Registry};
use snafu::ResultExt;
use std::net::SocketAddr;
use tracing::{error, info, trace};
use utils::{
    raw_version_str,
    tracing_telemetry::{default_tracing_tags, flush_traces, init_tracing},
};
mod cache;
mod exporter;
mod store;

#[derive(Parser, Debug)]
#[clap(name = utils::package_description!(), version = utils::version_info_str!())]
struct Cli {
    /// The url for mbus.
    #[clap(long, default_value = "nats://mayastor-nats:4222")]
    mbus_url: String,

    /// The namespace we are supposed to operate in.
    #[arg(short, long, default_value = "mayastor")]
    namespace: String,

    /// TCP address where events stats are exposed.
    #[clap(long, short, default_value = "0.0.0.0:9090")]
    metrics_endpoint: SocketAddr,

    /// Interval to update the config map.
    #[clap(short, long, default_value = "300s")]
    update_period: humantime::Duration,

    /// Sends opentelemetry spans to the Jaeger endpoint agent.
    #[clap(long, short)]
    jaeger: Option<String>,
}

impl Cli {
    fn args() -> Self {
        Cli::parse()
    }
}

/// Intilize mbus.
pub(crate) async fn mbus_init(mbus_url: &str) -> BusSubscription<EventMessage> {
    let mut bus = message_bus_init(mbus_url).await;
    bus.subscribe::<EventMessage>()
        .await
        .map_err(|error| trace!("Error subscribing to jetstream: {error:?}"))
        .unwrap()
}

/// Initialize events store cache from config map.
pub(crate) async fn initialize_events_cache(init_data: ConfigMap) -> errors::Result<()> {
    let events = EventSet::from_event_store(init_data).unwrap();
    Cache::initialize(events);
    Ok(())
}

/// Initialize events store.
pub(crate) async fn initialize_events_store(namespace: &str) -> errors::Result<ConfigMap> {
    let event_store: ConfigMap = initialize(namespace).await?;
    Ok(event_store)
}

/// Initialize exporter config that are passed through arguments.
fn initialize_exporter(args: &Cli) {
    ExporterConfig::initialize(args.metrics_endpoint);
}

#[tokio::main]
async fn main() -> errors::Result<()> {
    let args = Cli::args();
    utils::print_package_info!();
    info!(?args, "stats aggregation started");

    init_logging(&args);

    let bus_sub = mbus_init(args.mbus_url.as_str()).await;
    info!("mbus initialized successfully!");

    let init_data = initialize_events_store(&args.namespace).await?;
    info!("event store initialized successfully!");

    initialize_events_cache(init_data).await?;
    info!("event cache initialized successfully!");

    initialize_exporter(&args);
    info!("exporter initialized successfully!");

    // spawn a new task to store the data in cache.
    tokio::spawn(async move {
        cache::events_cache::store_events(bus_sub)
            .await
            .map_err(|error| {
                error!(%error, "Error while storing the events to cahce");
                flush_traces();
                error
            })
    });

    // spawn a new task to update the config map from cache.
    tokio::spawn(async move {
        store::events_store::update_config_map_data(&args.namespace, args.update_period.into())
            .await
            .map_err(|error| {
                error!(%error, "Error while persisting the stats to config map from cache");
                flush_traces();
                error
            })
    });

    let app = move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .configure(stats_route)
    };

    HttpServer::new(app)
        .bind(exporter::exporter_config::ExporterConfig::get_config().metrics_endpoint())
        .context(errors::SocketBindingFailure)?
        .run()
        .await
        .expect("Port should be free to expose the stats");

    Ok(())
}

/// Initialize logging components -- tracing.
pub(crate) fn init_logging(args: &Cli) {
    let tags = default_tracing_tags(raw_version_str(), env!("CARGO_PKG_VERSION"));

    init_tracing("stats", tags, args.jaeger.clone());
}

fn stats_route(cfg: &mut web::ServiceConfig) {
    info!(" configuted at /stats");
    cfg.route("/stats", web::get().to(metrics_handlers));
}

async fn metrics_handlers() -> impl Responder {
    // Initialize stats collector
    let stats_collector = StatsCollector::default();
    // Create a new registry for prometheus
    let registry = Registry::default();

    // Register stats collector in the registry
    if let Err(error) = Registry::register(&registry, Box::new(stats_collector)) {
        tracing::warn!(%error, "Stats collector already registered");
    }
    let mut buffer = Vec::new();

    let encoder = prometheus::TextEncoder::new();

    _ = encoder
        .encode(&registry.gather(), &mut buffer)
        .context(errors::CustomMetricsEndodeFailure);

    let res_custom = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(error) => {
            error!(%error, "Prometheus metrics could not be parsed from_utf8'd");
            String::default()
        }
    };
    HttpResponse::Ok()
        .insert_header(header::ContentType(mime::TEXT_PLAIN))
        .body(res_custom)
}
