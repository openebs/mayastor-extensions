use crate::constant::UPGRADE_OPERATOR_END_POINT;
use anyhow::Error;
use http::Method;
use hyper::body::Buf;
use serde::{Deserialize, Serialize};
/// Client to interact with upgrade operator.
#[derive(Debug)]
pub(crate) struct UpgradeOperatorClient {
    /// Address of UpgradeOperatorClient service.
    pub uri: Uri,
    /// UpgradeOperatorClient client.
    pub inner_client: kube_proxy::UpgradeOperatorClient,
    /// Endpoint of upgrade operator service.
    pub service_endpoint: Uri,
}
use http::Uri;
use tower::{util::BoxService, Service, ServiceExt};

impl UpgradeOperatorClient {
    /// Instantiate new instance of Http UpgradeOperatorClient client
    pub(crate) async fn new(
        uri: Option<Uri>,
        namespace: String,
        kube_config_path: Option<std::path::PathBuf>,
        timeout: humantime::Duration,
    ) -> Result<Self, Error> {
        let (uri, client) = match uri {
            None => {
                let (uri, svc) = match kube_proxy::ConfigBuilder::default_upgrade()
                    .with_kube_config(kube_config_path)
                    .with_target_mod(|t| t.with_namespace(namespace))
                    .build()
                    .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(error);
                    }
                };
                (uri, svc)
            }
            Some(uri) => {
                let mut connector = hyper::client::HttpConnector::new();
                connector.set_connect_timeout(Some(*timeout));
                let client = hyper::Client::builder()
                    .http2_keep_alive_timeout(*timeout)
                    .http2_keep_alive_interval(*timeout / 2)
                    .build(connector);
                let service = tower::ServiceBuilder::new()
                    .timeout(*timeout)
                    .service(client);
                (uri, BoxService::new(service))
            }
        };

        Ok(UpgradeOperatorClient {
            uri,
            inner_client: client,
            service_endpoint: (UPGRADE_OPERATOR_END_POINT.to_string())
                .parse::<Uri>()
                .unwrap(),
        })
    }

    /// Function to appply the upgrade.
    pub async fn apply_upgrade(&mut self) -> Result<Option<Vec<String>>, UpgradeClientError> {
        self.upgrade_actions(Method::PUT).await
    }

    /// Function to get the status of upgrade.
    pub async fn get_upgrade(&mut self) -> Result<Option<Vec<String>>, UpgradeClientError> {
        self.upgrade_actions(Method::GET).await
    }

    async fn upgrade_actions(
        &mut self,
        method_type: Method,
    ) -> Result<Option<Vec<String>>, UpgradeClientError> {
        let request_str = format!("{}{}", self.uri.clone(), self.service_endpoint);
        let request = match http::Request::builder()
            .method(method_type)
            .uri(&request_str)
            .body(hyper::body::Body::empty())
        {
            Ok(req) => req,
            Err(error) => {
                return Err(UpgradeClientError::Request(error));
            }
        };
        let response = match self.inner_client.ready().await?.call(request).await {
            Ok(res) => res,
            Err(error) => {
                return Err(UpgradeClientError::Response(error.to_string()));
            }
        };
        if !response.status().is_success() {
            let body_bytes = hyper::body::to_bytes(response.into_body()).await?;
            let text = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();
            return Err(UpgradeClientError::Response(text));
        } else {
            let body = hyper::body::aggregate(response.into_body()).await?;
            let ug_response: UpgradeClientResponse = serde_json::from_reader(body.reader())?;
            println!("Upgrade From : {}", ug_response.current_version);
            println!("Upgrade To : {}", ug_response.target_version);
            println!("Upgrade Status : {}", ug_response.state);
        }
        Ok(None)
    }
}

/// Possible errors can occur while interacting with Upgrade Operator.
#[derive(Debug)]
pub(crate) enum UpgradeClientError {
    Request(http::Error),
    Response(String),
    Tower(tower::BoxError),
    Serde(serde_json::Error),
    Hyper(hyper::Error),
    IOError(std::io::Error),
}

impl From<http::Error> for UpgradeClientError {
    fn from(e: http::Error) -> UpgradeClientError {
        UpgradeClientError::Request(e)
    }
}
impl From<tower::BoxError> for UpgradeClientError {
    fn from(e: tower::BoxError) -> UpgradeClientError {
        UpgradeClientError::Tower(e)
    }
}
impl From<serde_json::Error> for UpgradeClientError {
    fn from(e: serde_json::Error) -> UpgradeClientError {
        UpgradeClientError::Serde(e)
    }
}
impl From<hyper::Error> for UpgradeClientError {
    fn from(e: hyper::Error) -> UpgradeClientError {
        UpgradeClientError::Hyper(e)
    }
}
impl From<std::io::Error> for UpgradeClientError {
    fn from(e: std::io::Error) -> UpgradeClientError {
        UpgradeClientError::IOError(e)
    }
}

/// Response from upgrade operator client upon http requests.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct UpgradeClientResponse {
    current_version: String,
    target_version: String,
    state: String,
}
