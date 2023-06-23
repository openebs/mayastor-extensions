use chrono::prelude::*;
use k8s_openapi::chrono;
use obs::common::{constants::*, errors::ReceiverError};
use reqwest::Response;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};

/// Struct used to make calls to the receiver API.
pub struct Receiver {
    cluster_id: String,
    client: ClientWithMiddleware,
    url: String,
}

impl Receiver {
    /// 'Receiver::new()' creates a new instance of Receiver
    /// which is initialized with sane default values.
    pub(crate) async fn new<T>(cluster_id: T) -> Result<Self, ReceiverError>
    where
        T: ToString,
    {
        // Retry up to 3 times with increasing intervals between attempts.
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

        let client_config = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        Ok(Self {
            cluster_id: cluster_id.to_string(),
            client: ClientBuilder::new(client_config)
                .with(RetryTransientMiddleware::new_with_policy(retry_policy))
                .build(),
            url: RECEIVER_ENDPOINT.to_string(),
        })
    }

    /// 'post()' method attempts an HTTP POST with some headers
    pub(crate) async fn post(&self, body: Vec<u8>) -> Result<Response, ReceiverError> {
        Ok(self
            .client
            .post(&self.url)
            .header("CAStor-Cluster-Id", &self.cluster_id)
            .header("CAStor-Version", release_version())
            .header("CAStor-Report-Type", "health_report")
            .header("CAStor-Product", PRODUCT)
            .header("CAStor-Time", Utc::now().to_string())
            .header("Content-Type", "text/PGP; charset=binary")
            .body(body)
            .send()
            .await?)
    }
}
