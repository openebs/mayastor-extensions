use crate::{common::error, events::store::events_store::initialize};
use k8s_openapi::api::core::v1::ConfigMap;
mod common;
mod events;
use clap::Parser;
use events::cache::events_cache::{Cache, EventSet};
use mbus_api::mbus_nats::{message_bus_init, NatsMessageBus};
use tracing::{error, info};
use utils::{
    raw_version_str,
    tracing_telemetry::{default_tracing_tags, flush_traces, init_tracing},
};

async fn mbus_init(mbus_url: &str) -> error::Result<NatsMessageBus> {
    let message_bus = message_bus_init(mbus_url).await;
    Ok(message_bus)
}

/// Initialize events store cache from config map.
async fn initialize_events_cache(init_data: ConfigMap) {
    let events = EventSet::from_event_store(init_data).unwrap();
    Cache::initialize(events);
}

/// Initialize events store.
async fn initialize_events_store(namespace: &str) -> error::Result<ConfigMap> {
    let event_store = initialize(namespace).await?;
    Ok(event_store)
}

#[derive(Parser, Debug)]
#[clap(name = utils::package_description!(), version = utils::version_info_str!())]
struct Cli {
    /// TCP address where prometheus endpoint will listen to
    #[clap(long, short, default_value = "nats://mayastor-nats:4222")]
    mbus_url: String,

    /// Kubernetes namespace of mayastor service
    #[clap(global = true, long, short = 'n', default_value = "mayastor")]
    namespace: String,
}

impl Cli {
    fn args() -> Self {
        Cli::parse()
    }
}

#[tokio::main]
async fn main() -> error::Result<()> {
    init_logging();
    info!("stats aggregation started!");

    let args = Cli::args();

    utils::print_package_info!();

    let m_bus = mbus_init(args.mbus_url.as_str()).await?;
    info!("mbus initialized successfully!");

    let init_data = initialize_events_store(&args.namespace).await?;
    info!("event store initialized successfully!");

    initialize_events_cache(init_data).await;
    info!("event cache initialized successfully!");

    // spawn a new task to store the data in cache.
    tokio::spawn(async move {
        events::cache::events_cache::store_events(m_bus)
            .await
            .map_err(|error| {
                error!(%error, "Error while storing the events to cahce");
                flush_traces();
                error
            })
    });

    // spawn a new task to update the config map from cache.
    tokio::spawn(async move {
        events::store::events_store::update_config_map_data("mayastor")
            .await
            .map_err(|error| {
                error!(%error, "Error while persisting the stats to config map from cache");
                flush_traces();
                error
            })
    });

    for n in 1 .. 100 {
        info!("Inside for loop {}th time!", n);
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }

    Ok(())
}

/// Initialize logging components -- tracing.
fn init_logging() {
    let tags = default_tracing_tags(raw_version_str(), env!("CARGO_PKG_VERSION"));

    init_tracing("stats", tags, None);
}
