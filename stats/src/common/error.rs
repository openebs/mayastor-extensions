use crate::EventSet;
use snafu::Snafu;
/// A wrapper type to remove repeated Result<T, Error> returns.
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/// For use with multiple fallible operations which may fail for different reasons, but are
/// defined withing the same scope and must return to the outer scope (calling scope) using
/// the try operator -- '?'.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
#[snafu(context(suffix(false)))]
pub enum Error {
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
    ListDeploymantsWithLabel {
        source: kube::Error,
        label: String,
        namespace: String,
    },

    /// Error for no Deployment present.
    #[snafu(display("No deployment present."))]
    NoDeploymentPresent,

    /// Error when a Get Config map fails.
    #[snafu(display("Failed to get the event store config map {}: {}", name, source))]
    GetEventStoreConfigMap { source: kube::Error, name: String },

    /// Failed in creating config map.
    #[snafu(display(
        "Config map for event store: {} creation failed Error: {}",
        name,
        source
    ))]
    ServiceAccountCreate { name: String, source: kube::Error },

    /// Error in serializing event struct.
    #[snafu(display("Failed to serialize event struct {:?}: {}", note, source))]
    SerializeEvent {
        source: serde_json::Error,
        note: EventSet,
    },

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
}
