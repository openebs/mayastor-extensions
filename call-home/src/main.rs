use std::time;
pub mod collector;
pub mod common;
use crate::collector::k8s_client::K8sClient;
use crate::collector::report_models::{Pools, Replicas, Report, Volumes};
use clap::Parser;
use openapi::tower::client::{ApiClient, Configuration};
use sha256::digest;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use url::Url;

const PRODUCT: &str = common::constants::PRODUCT;

#[derive(Parser)]
#[clap(author, version, about)]
pub struct CliArgs {
    /// An URL endpoint to the control plane's rest endpoint.
    #[clap(short, long,default_value = "http://mayastor-api-rest:8081")]
    endpoint: String,

    /// The namespace we are supposed to operate in.
    #[clap(short, long, value_parser, default_value = "mayastor")]
    namespace: String,
}
impl CliArgs {
    fn args() -> Self {
        CliArgs::parse()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .init();

    let args = CliArgs::args();
    let version = clap::crate_version!().to_string();
    let endpoint = args.endpoint;
    let namespace = args.namespace;

    let k8s_client = K8sClient::new().await.unwrap();

    let url = Url::parse(endpoint.trim()).expect("endpoint is not a valid URL");

    let config = Configuration::new(url, time::Duration::from_secs(30), None, None, true).map_err(
        |error| {
            anyhow::anyhow!(
                "Failed to create openapi configuration, Error: '{:?}'",
                error
            )
        },
    )?;
    let client = openapi::clients::tower::ApiClient::new(config);

    let mut report = generate_report(k8s_client.clone(), client.clone()).await;
    report.deploy_namespace = namespace.clone();
    report.product_version = version.clone();

    println!("{:?}", report);

    loop {
        // TODO: For now it loops every 60 sec. Need to change this to 24hr and set the value in constants
        sleep(Duration::from_secs(60)).await;
        let mut report = generate_report(k8s_client.clone(), client.clone()).await;
        report.deploy_namespace = namespace.clone();
        report.product_version = version.clone();
        println!("{:?}", report);
    }
}

// TODO: For now this will only log the generated report. Needs a Transmitter
pub async fn generate_report(k8s_client: K8sClient, http_client: ApiClient) -> Report {
    let mut report = Report::new();
    report.product_name = PRODUCT.to_string();
    let k8s_node_count = k8s_client.get_nodes().await;
    match k8s_node_count {
        Ok(k8s_node_count) => report.k8s_node_count = k8s_node_count as u8,
        Err(err) => {
            error!("{:?}", err);
        }
    };
    let k8s_cluster_id = k8s_client.get_cluster_id().await;
    match k8s_cluster_id {
        Ok(k8s_cluster_id) => report.k8s_cluster_id = digest(k8s_cluster_id),
        Err(err) => {
            error!("{:?}", err);
        }
    };

    let nodes = http_client.nodes_api().get_nodes().await;
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

    let volumes = http_client.volumes_api().get_volumes(0, None).await;
    let volumes = match volumes {
        Ok(volumes) => Some(volumes.into_body()),
        Err(err) => {
            error!("{:?}", err);
            None
        }
    };

    match volumes.clone() {
        Some(volumes) => report.volumes = Volumes::new(volumes),
        None => {}
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
