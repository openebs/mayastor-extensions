use snafu::Snafu;

/// Contains Errors that may generate while execution of k8s_client.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub), context(suffix(false)))]
#[allow(clippy::enum_variant_names)]
pub(crate) enum K8sResourceError {
    #[snafu(display("Json Parse Error : {}", source))]
    SerdeError { source: serde_json::Error },

    #[snafu(display("K8Client Error: {}", source))]
    ClientError { source: kube::Error },
}

impl From<kube::Error> for K8sResourceError {
    fn from(source: kube::Error) -> Self {
        Self::ClientError { source }
    }
}

impl From<serde_json::Error> for K8sResourceError {
    fn from(source: serde_json::Error) -> Self {
        Self::SerdeError { source }
    }
}

/// ReceiverError is a custom error enum which is returned when building
/// an instance of crate::transmitter::client::Receiver.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub), context(suffix(false)))]
#[allow(clippy::enum_variant_names)]
pub(crate) enum ReceiverError {
    #[snafu(display("HTTP client error: {}", source))]
    HttpClientError { source: reqwest::Error },

    #[snafu(display("HTTP client (with middleware) error: {}", source))]
    HttpClientWithMiddlewareError { source: reqwest_middleware::Error },
}

impl From<reqwest::Error> for ReceiverError {
    fn from(source: reqwest::Error) -> Self {
        Self::HttpClientError { source }
    }
}

impl From<reqwest_middleware::Error> for ReceiverError {
    fn from(source: reqwest_middleware::Error) -> Self {
        Self::HttpClientWithMiddlewareError { source }
    }
}
