use crate::common::error::{RestClientConfiguration, RestClientSecurity, RestUrlParse, Result};
use openapi::tower::client::{
    configuration::{ClientSecurity, TlsMode},
    ApiClient, Configuration as RestConfig,
};
use snafu::ResultExt;
use std::{path::PathBuf, time::Duration};
use tracing::info;
use url::Url;

/// The TLS mode variant name only. The derived `Debug` of `TlsMode` includes the raw
/// certificate and private key bytes, which must not be logged.
fn tls_mode_name(tls: &TlsMode) -> &'static str {
    match tls {
        TlsMode::None => "none",
        TlsMode::Auto => "auto",
        TlsMode::ServerVerify { .. } => "server-verify",
        TlsMode::ClientVerify { .. } => "client-verify",
        TlsMode::Mtls { .. } => "mtls",
    }
}

/// This is a wrapper for the openapi::tower::client::ApiClient.
pub struct RestClientSet {
    client: ApiClient,
}

impl RestClientSet {
    /// Build the REST client for the given endpoint, with the TLS/JWT client security read
    /// from the given files.
    pub fn new(
        rest_endpoint: String,
        jwt: &Option<PathBuf>,
        tls_ca_file: Option<&PathBuf>,
        tls_cert_file: Option<&PathBuf>,
        tls_key_file: Option<&PathBuf>,
    ) -> Result<Self> {
        let tls =
            TlsMode::new(tls_ca_file, tls_cert_file, tls_key_file).context(RestClientSecurity)?;
        let security = ClientSecurity::try_new(jwt, tls).context(RestClientSecurity)?;
        Self::new_with_url(rest_endpoint, security)
    }

    /// Build the RestConfig, and the eventually the ApiClient. Fails if configuration is invalid.
    pub fn new_with_url(rest_endpoint: String, security: ClientSecurity) -> Result<Self> {
        let rest_url =
            Url::try_from(rest_endpoint.as_str()).context(RestUrlParse { rest_endpoint })?;
        Self::new_with_rest_url(rest_url, security)
    }

    fn new_with_rest_url(rest_url: Url, security: ClientSecurity) -> Result<Self> {
        info!(
            rest_endpoint = %rest_url,
            tls = tls_mode_name(&security.tls),
            jwt = security.jwt.is_some(),
            "REST API client"
        );
        let config = RestConfig::builder()
            .with_timeout(Duration::from_secs(30))
            .with_tracing(true)
            .with_client_security(Some(security))
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

    pub fn nodes_api(&self) -> &dyn openapi::apis::nodes_api::tower::client::Nodes {
        self.client.nodes_api()
    }

    pub fn volumes_api(&self) -> &dyn openapi::apis::volumes_api::tower::client::Volumes {
        self.client.volumes_api()
    }
}
