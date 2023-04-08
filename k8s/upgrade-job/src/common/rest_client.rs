use crate::common::error::{RestClientConfiguration, RestUrlParse, Result};
use openapi::tower::client::{ApiClient, Configuration as RestConfig};
use snafu::ResultExt;
use std::time::Duration;
use url::Url;

/// This is a wrapper for the openapi::tower::client::ApiClient.
pub(crate) struct RestClientSet {
    client: ApiClient,
}

impl RestClientSet {
    /// Build the RestConfig, and the eventually the ApiClient. Fails if configuration is invalid.
    pub(crate) fn new_with_url(rest_endpoint: String) -> Result<Self> {
        let rest_url =
            Url::try_from(rest_endpoint.as_str()).context(RestUrlParse { rest_endpoint })?;

        let config = RestConfig::builder()
            .with_timeout(Duration::from_secs(30))
            .with_tracing(true)
            .build_url(rest_url.clone())
            .map_err(|e| {
                RestClientConfiguration {
                    source: e,
                    rest_endpoint: rest_url,
                }
                .build()
            })?;
        let client = ApiClient::new(config);

        Ok(RestClientSet { client })
    }

    pub(crate) fn nodes_api(&self) -> &dyn openapi::apis::nodes_api::tower::client::Nodes {
        self.client.nodes_api()
    }

    pub(crate) fn volumes_api(&self) -> &dyn openapi::apis::volumes_api::tower::client::Volumes {
        self.client.volumes_api()
    }
}
