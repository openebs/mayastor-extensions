use openapi::{
    clients::tower::Error,
    models::{Node, RestJsonError},
    tower::client::{ApiClient, Configuration},
};
use std::time::Duration;
use tracing::trace;

/// REST client for fetching node status from the control-plane.
#[derive(Clone, Debug)]
pub(crate) struct NodeStatusClient {
    client: ApiClient,
}

impl NodeStatusClient {
    /// Create a new NodeStatusClient.
    pub(crate) fn new(endpoint: &str, timeout: Duration) -> anyhow::Result<Self> {
        let url = url::Url::parse(endpoint)
            .map_err(|e| anyhow::anyhow!("Invalid REST endpoint URL '{endpoint}': {e}"))?;
        let config = Configuration::builder()
            .with_timeout(timeout)
            .with_tracing(false)
            .build_url(url)
            .map_err(|e| anyhow::anyhow!("Failed to create openapi configuration: {e:?}"))?;
        let client = ApiClient::new(config);
        Ok(Self { client })
    }

    /// Fetch a single node from the control-plane REST API.
    pub(crate) async fn fetch_node(&self, node_id: &str) -> Result<Node, Error<RestJsonError>> {
        trace!("Fetching node {node_id} from control-plane");
        let node = self.client.nodes_api().get_node(node_id).await?.into_body();
        trace!("Successfully fetched node {node_id}");
        Ok(node)
    }
}
