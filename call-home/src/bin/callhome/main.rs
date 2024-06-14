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
use std::{collections::HashMap, time};
use tokio::time::sleep;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if let Err(error) = run().await {
        error!(?error, "failed call-home");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
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
        .with_timeout(time::Duration::from_secs(30))
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
) -> Report {
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
        Ok(k8s_node_count) => report.k8s_node_count = k8s_node_count as u8,
        Err(err) => {
            error!("{:?}", err);
        }
    };

    // List of disks on each node.
    let mut node_disks = HashMap::new();
    let nodes = http_client.nodes_api().get_nodes(None).await;
    match nodes {
        Ok(nodes) => {
            let nodes = nodes.into_body();
            for node in &nodes {
                if let Ok(b_devs_result) = http_client
                    .block_devices_api()
                    .get_node_block_devices(&node.id, Some(true))
                    .await
                {
                    node_disks
                        .entry(node.id.to_string())
                        .or_insert_with(Vec::new)
                        .extend(b_devs_result.into_body());
                };
            }
            report.storage_node_count = nodes.len() as u8
        }
        Err(err) => {
            error!("{:?}", err);
        }
    };

    let pools = http_client.pools_api().get_pools(None).await;
    match pools {
        Ok(ref pools) => report.pools = Pools::new(pools.clone().into_body(), event_data.clone()),
        Err(ref err) => {
            error!("{:?}", err);
        }
    };

    let volumes = list_all_volumes(&http_client)
        .await
        .map_err(|error| error!("Failed to list all volumes: {error:?}"))
        .ok();

    if let Some(volumes) = &volumes {
        report.volumes = Volumes::new(volumes.clone(), event_data.clone());
    }

    let replicas = http_client.replicas_api().get_replicas().await;
    match replicas {
        Ok(ref replicas) => {
            report.replicas = Replicas::new(replicas.clone().into_body().len(), volumes)
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
    report
}
