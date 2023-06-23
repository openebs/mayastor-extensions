use snafu::Snafu;

/// Contains Errors that may generate while execution of k8s_client.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub), context(suffix(false)))]
#[allow(clippy::enum_variant_names)]
pub enum K8sResourceError {
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
pub enum ReceiverError {
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

/// EncryptError is a custom error enum which is returned by the
/// crate::transmitter::encryption::encrypt() function.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub), context(suffix(false)))]
#[allow(clippy::enum_variant_names)]
pub enum EncryptError {
    #[snafu(display("error during JSON marshalling: {}", source))]
    SerdeSerializeError { source: serde_json::Error },

    #[snafu(display("file io error: {}", source))]
    IoError { source: std::io::Error },
}

impl From<serde_json::Error> for EncryptError {
    fn from(source: serde_json::Error) -> Self {
        Self::SerdeSerializeError { source }
    }
}

impl From<std::io::Error> for EncryptError {
    fn from(source: std::io::Error) -> Self {
        Self::IoError { source }
    }
}

/// A wrapper type to remove repeated Result<T, Error> returns.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// For use with multiple fallible operations which may fail for different reasons, but are
/// defined withing the same scope and must return to the outer scope (calling scope) using
/// the try operator -- '?'.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
#[snafu(context(suffix(false)))]
pub enum Error {
    /// Error subscribing to jetstream
    #[snafu(display("Error subscribing to nats jetstream."))]
    NatsSubscriptionFailure,

    /// K8s client error.
    #[snafu(display("K8Client Error: {}", source))]
    K8sClient { source: kube::Error },

    /// Error for when a Kubernetes API request for GET-ing a list of Deployments filtered by
    /// label(s) fails.
    #[snafu(display(
        "Failed to list Deployments with label {} in namespace {}: {}",
        label,
        namespace,
        source
    ))]
    ListDeploymentsWithLabel {
        source: kube::Error,
        label: String,
        namespace: String,
    },

    /// Error for no Deployment present.
    #[snafu(display("No deployment present."))]
    NoDeploymentPresent,

    /// Error updating the config map.
    #[snafu(display(
        "Failed to update configmap {} in namespace {}: {}",
        name,
        namespace,
        source
    ))]
    UpdatingConfigmap {
        source: kube::Error,
        name: String,
        namespace: String,
    },

    /// Error when a Get Config map fails.
    #[snafu(display("Failed to get the event store config map {}: {}", name, source))]
    GetEventStoreConfigMap { source: kube::Error, name: String },

    /// Failed in creating config map.
    #[snafu(display("Config map for event store: {} does not exist.", name,))]
    ConfigMapNotPresent { name: String },

    /// Error in serializing event struct.
    #[snafu(display("Failed to serialize event struct: {}", source))]
    SerializeEvent { source: serde_json::Error },

    /// Error for when .data is None for the reference ConfigMap.
    #[snafu(display("No .data found for the reference config map"))]
    ReferenceConfigMapNoData,

    /// Reference Key not present.
    #[snafu(display("Referenced key not present in config map: {}", key))]
    ReferencedKeyNotPresent { key: String },

    /// Deserialization error for event.
    #[snafu(display("Error in deserializing event {} Error {}", event, source))]
    EventSerdeDeserialization {
        event: String,
        source: serde_json::Error,
    },

    /// Failed in creating config map.
    #[snafu(display(
        "Config map for event store: {} creation failed Error: {}",
        name,
        source
    ))]
    ServiceAccountCreate { name: String, source: kube::Error },

    /// Could not encode custom metrics
    #[snafu(display("Error encoding custom metrics {} ", source))]
    CustomMetricsEndodeFailure { source: prometheus::Error },

    /// Could not encode custom metrics
    #[snafu(display("Error while binding socket {} ", source))]
    SocketBindingFailure { source: std::io::Error },
}
