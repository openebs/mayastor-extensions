use crate::upgrade::common::error::K8sResourceError;
use clap::ArgMatches;
use core::time;
use kube::Client;
use once_cell::sync::OnceCell;
use openapi::tower::client::{ApiClient, Configuration};
use url::Url;

static CONFIG: OnceCell<UpgradeOperatorConfig> = OnceCell::new();

/// Exporter config that can be passed through arguments
pub struct UpgradeOperatorConfig {
    k8s_client: kube::Client,

    mayastor_rest_client: ApiClient,

    namespace: String,

    chart_name: String,
}

impl UpgradeOperatorConfig {
    /// Initialize exporter configs
    pub async fn initialize(args: &ArgMatches) -> Result<(), K8sResourceError> {
        let k8s_client = Client::try_default().await?;

        let mayastor_rest_endpoint = args
            .value_of("mayastor-endpoint")
            .expect("mayastor rest endpoint");
        let url = Url::parse(mayastor_rest_endpoint).expect("Unbale to parse string to url");
        let config_rest = Configuration::new(url, time::Duration::from_secs(30), None, None, true)
            .map_err(|error| {
                anyhow::anyhow!(
                    "Failed to create openapi configuration, Error: '{:?}'",
                    error
                )
            })
            .expect("Unable to create openapi configuration");
        let client = ApiClient::new(config_rest);
        let namespace = args.value_of("namespace").expect("mayastor rest endpoint");
        let chart_name = args.value_of("chart-name").expect("mayastor rest endpoint");
        CONFIG.get_or_init(|| Self {
            k8s_client: k8s_client.clone(),
            mayastor_rest_client: client.clone(),
            namespace: namespace.to_string(),
            chart_name: chart_name.to_string(),
        });
        Ok(())
    }

    /// Get exporter config
    pub(crate) fn get_config() -> &'static UpgradeOperatorConfig {
        CONFIG
            .get()
            .expect("Upgrade operator config is not initialized")
    }
}
