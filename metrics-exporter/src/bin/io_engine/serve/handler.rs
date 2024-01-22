use crate::{
    cache::store_resource_data,
    collector::{
        pool::{PoolCapacityCollector, PoolStatusCollector},
        pool_stat::PoolIoStatsCollector,
    },
    grpc_client,
};
use actix_web::{http::header, HttpResponse, Responder};
use prometheus::{Encoder, Registry};
use tracing::{error, warn};

/// Handler for metrics. Initializes all collector and serves data over Http.
pub(crate) async fn metrics_handler() -> impl Responder {
    store_resource_data(grpc_client()).await;
    let pools_collector = PoolCapacityCollector::default();
    let pool_status_collector = PoolStatusCollector::default();
    let pool_iostat_collector = PoolIoStatsCollector::default();
    // Create a new registry for prometheus
    let registry = Registry::default();
    // Register pools collector in the registry
    if let Err(error) = Registry::register(&registry, Box::new(pools_collector)) {
        warn!(%error, "Pools collector already registered");
    }
    if let Err(error) = Registry::register(&registry, Box::new(pool_status_collector)) {
        warn!(%error, "Pools status collector already registered");
    }
    if let Err(error) = Registry::register(&registry, Box::new(pool_iostat_collector)) {
        warn!(%error, "Pools status collector already registered");
    }

    let mut buffer = Vec::new();

    let encoder = prometheus::TextEncoder::new();
    // Starts collecting metrics via calling gatherers
    if let Err(error) = encoder.encode(&registry.gather(), &mut buffer) {
        error!(%error, "Could not encode custom metrics");
    };

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
