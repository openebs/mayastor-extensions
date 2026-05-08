use openapi::{
    clients::tower::{ApiClient, Configuration, Error, Url},
    models::{Node, RestJsonError},
};
use std::{path::Path, time::Duration};
use tracing::trace;

/// REST client for fetching node status from the control-plane.
#[derive(Clone, Debug)]
pub(crate) struct NodeStatusClient {
    client: ApiClient,
}

impl NodeStatusClient {
    /// Create a new NodeStatusClient.
    pub(crate) fn new(
        endpoint: &str,
        timeout: Duration,
        tls_client_ca_path: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let url = Url::parse(endpoint)
            .map_err(|e| anyhow::anyhow!("Invalid REST endpoint URL '{endpoint}': {e}"))?;
        if url.scheme() != "https" && tls_client_ca_path.is_some() {
            anyhow::bail!("CA certificate path is only supported for HTTPS REST endpoints");
        }

        let ca_certificate = match tls_client_ca_path {
            Some(path) => Some(std::fs::read(path).map_err(|error| {
                anyhow::anyhow!("Failed to read TLS CA certificate '{path:?}': {error}")
            })?),
            None => None,
        };
        let config = Configuration::new(url, timeout, None, ca_certificate.as_deref(), false, None)
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
