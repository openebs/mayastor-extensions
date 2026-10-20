use crate::common::{
    error::{
        RestClientConfiguration, RestClientSecurity, RestClientSecurityDiscovery, RestUrlParse,
        Result, WaitForRest,
    },
    kube::client as KubeClient,
};
use openapi::tower::client::{
    configuration::{ClientSecurity, TlsMode},
    ApiClient, Configuration as RestConfig,
};
use snafu::ResultExt;
use std::{path::PathBuf, time::Duration};
use tracing::{info, warn};
use url::Url;

/// The api-rest container ports, as set by the helm chart.
const API_REST_HTTPS_PORT: i32 = 8080;
const API_REST_HTTP_PORT: i32 = 8081;

/// How long to wait for the REST endpoint to become reachable after discovery.
/// The api-rest Service endpoints are only programmed asynchronously after the pods become ready.
const REST_REACHABLE_TIMEOUT: Duration = Duration::from_secs(120);
const REST_REACHABLE_RETRY: Duration = Duration::from_secs(2);

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
    /// Build the REST client for the given release, discovering the TLS/JWT configuration
    /// from the api-rest pods at call time (via the `openebs.io/rest-*` pod annotations).
    /// Explicitly provided file-based TLS/JWT settings take precedence over discovery.
    /// If `rest_endpoint` is not given, it's derived from the release name and the
    /// discovered TLS mode.
    pub async fn discover(
        namespace: &str,
        release_name: &str,
        rest_endpoint: Option<String>,
        jwt: &Option<PathBuf>,
        tls_ca_file: Option<&PathBuf>,
        tls_cert_file: Option<&PathBuf>,
        tls_key_file: Option<&PathBuf>,
    ) -> Result<Self> {
        let tls =
            TlsMode::new(tls_ca_file, tls_cert_file, tls_key_file).context(RestClientSecurity)?;
        let security = ClientSecurity::try_new(jwt, tls).context(RestClientSecurity)?;

        let discovered = security.discover();
        let security = if discovered {
            // Make sure we discover from the current api-rest pod template, and not from pods
            // which are being replaced by an ongoing rollout.
            KubeClient::wait_for_deployments_rollout(namespace, utils::API_REST_LABEL).await?;
            let target = kube_forward::Target::new(
                kube_forward::TargetSelector::PodLabel(utils::API_REST_LABEL.to_string()),
                API_REST_HTTPS_PORT,
                namespace,
            );
            kube_forward::PortForward::new(target, None, KubeClient::client().await?)
                .discover_client_security(&security)
                .await
                .context(RestClientSecurityDiscovery)?
        } else {
            security
        };

        let rest_url = match rest_endpoint {
            None => {
                let (scheme, port) = Self::scheme_and_port(&security);
                let rest_endpoint = format!("{scheme}://{release_name}-api-rest:{port}");
                Url::try_from(rest_endpoint.as_str()).context(RestUrlParse { rest_endpoint })?
            }
            Some(rest_endpoint) => {
                let mut rest_url = Url::try_from(rest_endpoint.as_str())
                    .context(RestUrlParse { rest_endpoint })?;
                // The given endpoint (eg: `http://<release>-api-rest:8081`) may be stale when the
                // TLS configuration was discovered, as it may have changed during the upgrade.
                // Keep the host, but reconcile the scheme (and the chart default port) with the
                // discovered TLS mode.
                if discovered {
                    let (scheme, port) = Self::scheme_and_port(&security);
                    if rest_url.scheme() != scheme {
                        let _ = rest_url.set_scheme(scheme);
                        if matches!(
                            rest_url.port().map(i32::from),
                            Some(API_REST_HTTP_PORT) | Some(API_REST_HTTPS_PORT)
                        ) {
                            let _ = rest_url.set_port(Some(port as u16));
                        }
                    }
                }
                rest_url
            }
        };

        let client = Self::new_with_rest_url(rest_url, security)?;
        client.wait_reachable().await?;
        Ok(client)
    }

    /// Wait until the REST endpoint answers. Only transport-level failures (eg: connection
    /// refused while the Service endpoints are not yet programmed) are retried, any response
    /// from the server, even an error, means the endpoint is reachable.
    async fn wait_reachable(&self) -> Result<()> {
        let deadline = tokio::time::Instant::now() + REST_REACHABLE_TIMEOUT;
        let mut log = true;
        loop {
            match self.nodes_api().get_nodes(None).await {
                Ok(_) => return Ok(()),
                Err(error)
                    if error.request().is_some() && tokio::time::Instant::now() < deadline =>
                {
                    if log {
                        warn!(%error, "REST API endpoint not reachable yet, retrying...");
                        log = false;
                    }
                    tokio::time::sleep(REST_REACHABLE_RETRY).await;
                }
                Err(error) => return Err(error).context(WaitForRest),
            }
        }
    }

    /// The REST scheme and chart default port for the given security configuration.
    fn scheme_and_port(security: &ClientSecurity) -> (&'static str, i32) {
        if security.tls.is_none() {
            ("http", API_REST_HTTP_PORT)
        } else {
            ("https", API_REST_HTTPS_PORT)
        }
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
