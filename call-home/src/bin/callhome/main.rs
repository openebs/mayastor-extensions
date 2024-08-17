mod collector;
mod transmitter;

use crate::{
    collector::{
        k8s_client::K8sClient,
        report_models::{
            event_stats, EventData, NexusCreated, NexusDeleted, PoolCreated, PoolDeleted, Pools,
            RebuildEnded, RebuildStarted, Replicas, Report, VolumeCreated, VolumeDeleted, Volumes,
        },
        storage_rest::list_all_volumes,
    },
    transmitter::*,
};
use clap::Parser;
use collector::report_models::{MayastorManagedDisks, Nexus, StorageMedia, StorageNodes};
use obs::common::constants::*;
use openapi::tower::client::{ApiClient, Configuration};
use sha256::digest;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
use tokio_retry::{
    strategy::{jitter, ExponentialBackoff},
    Retry,
};
use tracing::{error, info, Event, Level, Subscriber};
use tracing_subscriber::{
    layer::Context, prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer,
};
use url::Url;
use utils::{package_description, version_info_str};

#[derive(Parser)]
#[command(name = package_description!(), version = version_info_str!())]
struct CliArgs {
    /// An URL endpoint to the control plane's rest endpoint.
    #[arg(short, long, default_value = "http://mayastor-api-rest:8081")]
    endpoint: Url,

    /// The namespace we are supposed to operate in.
    #[arg(short, long, default_value = DEFAULT_NAMESPACE)]
    namespace: String,

    /// Sends the report to the remote collection endpoint.
    #[clap(long, short)]
    send_report: bool,

    /// The endpoint to fetch events stats.
    #[clap(long, short)]
    aggregator_url: Option<Url>,
}
impl CliArgs {
    fn args() -> Self {
        CliArgs::parse()
    }
}

const ERR_LOG_BUF_CAPACITY: usize = 100;

#[tokio::main]
async fn main() {
    let logs = Arc::new(Mutex::new(VecDeque::with_capacity(ERR_LOG_BUF_CAPACITY)));
    let vec_layer = LogsLayer::new(logs.clone());

    let subscriber = tracing_subscriber::Registry::default()
        .with(vec_layer)
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::from_default_env());

    tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");

    if let Err(error) = run(logs).await {
        error!(?error, "failed call-home");
        std::process::exit(1);
    }
}

async fn run(logs: Arc<Mutex<VecDeque<LogEntry>>>) -> anyhow::Result<()> {
    let args = CliArgs::args();
    let version = release_version();
    let endpoint = args.endpoint;
    let aggregator_url = args.aggregator_url;
    let send_report = args.send_report;
    let namespace = digest(args.namespace);
    let sleep_duration = call_home_frequency();
    let encryption_dir = encryption_dir();
    let key_filepath = key_filepath();

    // Generate kubernetes client.
    let k8s_client = K8sClient::new()
        .await
        .map_err(|error| anyhow::anyhow!("failed to generate kubernetes client: {:?}", error))?;

    // Generate SHA256 hash of kube-system namespace UID.
    let k8s_cluster_id = digest(k8s_client.get_cluster_id().await.map_err(|error| {
        anyhow::anyhow!("failed to generate kubernetes cluster ID: {:?}", error)
    })?);

    // Generate receiver API client.
    let receiver = client::Receiver::new(&k8s_cluster_id)
        .await
        .map_err(|error| {
            anyhow::anyhow!("failed to generate metrics receiver client: {:?}", error)
        })?;

    // Generate Mayastor REST client.
    let config = Configuration::builder()
        .with_timeout(Duration::from_secs(30))
        .with_tracing(true)
        .build_url(endpoint)
        .map_err(|error| anyhow::anyhow!("failed to create openapi configuration: {:?}", error))?;
    let client = openapi::clients::tower::ApiClient::new(config);

    loop {
        // Generate report.
        let report = generate_report(
            k8s_client.clone(),
            client.clone(),
            k8s_cluster_id.clone(),
            namespace.clone(),
            version.clone(),
            aggregator_url.clone(),
            logs.clone(),
        )
        .await;

        // Encrypt data.
        let encryption_dir = encryption_dir.clone();
        let key_filepath = key_filepath.clone();
        let output = tokio::task::spawn_blocking(move || {
            encryption::encrypt(&report, &encryption_dir, &key_filepath)
        })
        .await?;
        let output = output.map_err(|error| anyhow::anyhow!("encryption failed: {:?}", error))?;

        // POST data to receiver API.
        if send_report {
            match receiver.post(output).await {
                Ok(response) => info!(?response, "Success"),
                Err(e) => error!(?e, "failed HTTP POST request"),
            }
        }

        // Block until next transmission window.
        sleep(sleep_duration).await;
    }
}

async fn generate_report(
    k8s_client: K8sClient,
    http_client: ApiClient,
    k8s_cluster_id: String,
    deploy_namespace: String,
    product_version: String,
    aggregator_url: Option<Url>,
    logs: Arc<Mutex<VecDeque<LogEntry>>>,
) -> Report {
    let retry_strategy = ExponentialBackoff::from_millis(100)
        .map(jitter) // add jitter to delays
        .take(5); // retry up to 5 times

    let mut report = Report {
        product_name: product(),
        k8s_cluster_id,
        deploy_namespace,
        product_version,
        ..Default::default()
    };

    let mut event_data = EventData::default();

    if let Some(url) = aggregator_url {
        match event_stats(url).await {
            Ok(data) => {
                event_data.volume_created =
                    Option::<VolumeCreated>::from(&data).unwrap_or_default();
                event_data.volume_deleted =
                    Option::<VolumeDeleted>::from(&data).unwrap_or_default();
                event_data.pool_created = Option::<PoolCreated>::from(&data).unwrap_or_default();
                event_data.pool_deleted = Option::<PoolDeleted>::from(&data).unwrap_or_default();
                event_data.nexus_created = Option::<NexusCreated>::from(&data).unwrap_or_default();
                event_data.nexus_deleted = Option::<NexusDeleted>::from(&data).unwrap_or_default();
                event_data.rebuild_started =
                    Option::<RebuildStarted>::from(&data).unwrap_or_default();
                event_data.rebuild_ended = Option::<RebuildEnded>::from(&data).unwrap_or_default();
            }
            Err(err) => {
                error!("{:?}", err);
            }
        };
    }

    let k8s_node_count = k8s_client.get_node_len().await;
    match k8s_node_count {
        Ok(k8s_node_count) => report.k8s_node_count = Some(k8s_node_count as u8),
        Err(err) => {
            error!("{:?}", err);
        }
    };

    // List of disks on each node.
    let mut node_disks = HashMap::new();
    let nodes = Retry::spawn(retry_strategy.clone(), || async {
        http_client.nodes_api().get_nodes(None).await
    })
    .await;
    match nodes {
        Ok(nodes) => {
            let nodes = nodes.into_body();
            for node in &nodes {
                let b_devs_result = Retry::spawn(retry_strategy.clone(), || async {
                    http_client
                        .block_devices_api()
                        .get_node_block_devices(&node.id, Some(true))
                        .await
                })
                .await;
                if let Ok(b_devs) = b_devs_result {
                    node_disks
                        .entry(node.id.to_string())
                        .or_insert_with(Vec::new)
                        .extend(b_devs.into_body());
                };
            }
            report.storage_node_count = Some(nodes.len() as u8)
        }
        Err(err) => {
            error!("{:?}", err);
        }
    };

    let pools = Retry::spawn(retry_strategy.clone(), || async {
        http_client.pools_api().get_pools(None).await
    })
    .await;
    match pools {
        Ok(ref pools) => {
            report.pools = Some(Pools::new(pools.clone().into_body(), event_data.clone()))
        }
        Err(ref err) => {
            error!("{:?}", err);
        }
    };

    let volumes = Retry::spawn(retry_strategy.clone(), || async {
        list_all_volumes(&http_client).await
    })
    .await
    .map_err(|error| error!("Failed to list all volumes: {error:?}"))
    .ok();

    if let Some(volumes) = &volumes {
        report.volumes = Some(Volumes::new(volumes.clone(), event_data.clone()));
    }

    let replicas = Retry::spawn(retry_strategy, || async {
        http_client.replicas_api().get_replicas().await
    })
    .await;
    match replicas {
        Ok(ref replicas) => {
            report.replicas = Some(Replicas::new(replicas.clone().into_body().len(), volumes))
        }
        Err(ref err) => {
            error!("{:?}", err);
        }
    };

    if let Ok(pools) = pools {
        report.mayastor_managed_disks =
            MayastorManagedDisks::new(pools.clone().into_body(), node_disks.clone());
        if let Ok(replicas) = replicas {
            report.storage_nodes = StorageNodes::new(
                replicas.clone().into_body(),
                pools.into_body(),
                node_disks.clone(),
            )
        }
    }

    // find valid disks to calculate storage media metrics
    let valid_disks = node_disks
        .values()
        .flat_map(|disks| disks.iter().cloned())
        .filter(|device| {
            device.size > 0
                && device.devtype != "partition"
                && !device.devpath.starts_with("/devices/virtual/")
        })
        .collect();

    report.storage_media = StorageMedia::new(valid_disks);

    report.nexus = Nexus::new(event_data);

    let mut logs = logs.lock().unwrap();
    for log in logs.iter() {
        let log_string = format!(
            "[{} {} {}:{}] {}",
            log.timestamp, log.level, log.file, log.line, log.message
        );
        report.logs.push(log_string);
    }
    // Clear logs, as these entries are no longer needed after updating the report.
    logs.clear();
    report
}

// Define the LogsLayer
struct LogsLayer {
    logs: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogsLayer {
    fn new(logs: Arc<Mutex<VecDeque<LogEntry>>>) -> Self {
        LogsLayer { logs }
    }
}

impl<S> Layer<S> for LogsLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event, _ctx: Context<S>) {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let level = event.metadata().level();
        let file = event.metadata().file().unwrap_or("").to_string();
        let line = event.metadata().line().unwrap_or(0);

        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);

        let log_entry = LogEntry {
            timestamp,
            level: level.to_string(),
            file,
            line,
            message: visitor.log,
        };

        // Only capture logs of level ERROR
        if level == &Level::ERROR {
            let mut logs = self.logs.lock().unwrap();
            log_with_limit(&mut logs, log_entry, ERR_LOG_BUF_CAPACITY);
        }
    }
}

struct LogVisitor {
    log: String,
}

impl LogVisitor {
    fn new() -> Self {
        LogVisitor { log: String::new() }
    }
}

impl tracing::field::Visit for LogVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.log
            .push_str(&format!("{}: {:?};", field.name(), value));
    }
}

#[derive(Debug, serde::Serialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    file: String,
    line: u32,
    message: String,
}

/// Ensures the length of a vector is no more than the input limit, and removes elements from
/// the front if it goes over the limit.
fn log_with_limit<T>(logs: &mut VecDeque<T>, message: T, limit: usize) {
    while logs.len().ge(&limit) {
        logs.pop_front();
    }
    logs.push_back(message);
}
