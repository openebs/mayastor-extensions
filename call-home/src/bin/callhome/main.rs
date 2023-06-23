mod collector;
mod transmitter;

use crate::{
    collector::{
        k8s_client::K8sClient,
        report_models::{Pools, Replicas, Report, Volumes},
    },
    transmitter::*,
};
use clap::Parser;
use obs::common::constants::*;
use openapi::tower::client::{ApiClient, Configuration};
use sha256::digest;
use std::time;
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
    #[arg(short, long, default_value = "mayastor")]
    namespace: String,
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
        match receiver.post(output).await {
            Ok(response) => info!(?response, "Success"),
            Err(e) => error!(?e, "failed HTTP POST request"),
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
) -> Report {
    let mut report = Report {
        product_name: PRODUCT.to_string(),
        k8s_cluster_id,
        deploy_namespace,
        product_version,
        ..Default::default()
    };

    let k8s_node_count = k8s_client.get_node_len().await;
    match k8s_node_count {
        Ok(k8s_node_count) => report.k8s_node_count = k8s_node_count as u8,
        Err(err) => {
            error!("{:?}", err);
        }
    };

    let nodes = http_client.nodes_api().get_nodes(None).await;
    match nodes {
        Ok(nodes) => report.storage_node_count = nodes.into_body().len() as u8,
        Err(err) => {
            error!("{:?}", err);
        }
    };

    let pools = http_client.pools_api().get_pools().await;
    match pools {
        Ok(pools) => report.pools = Pools::new(pools.into_body()),
        Err(err) => {
            error!("{:?}", err);
        }
    };

    let volumes = http_client.volumes_api().get_volumes(0, None, None).await;
    let volumes = match volumes {
        Ok(volumes) => Some(volumes.into_body()),
        Err(err) => {
            error!("{:?}", err);
            None
        }
    };

    if let Some(volumes) = &volumes {
        report.volumes = Volumes::new(volumes.clone());
    }

    let replicas = http_client.replicas_api().get_replicas().await;
    match replicas {
        Ok(replicas) => report.replicas = Replicas::new(replicas.into_body().len(), volumes),
        Err(err) => {
            error!("{:?}", err);
        }
    };
    report
}
