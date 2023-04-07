use openapi::{clients, models::RestJsonError};
use thiserror::Error;

/// Contains Errors that may generate while execution of kubectl upgrade client.
#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    /// K8s client error.
    #[error("K8Client Error: {}", source)]
    K8sClientError { source: kube::Error },

    /// Failed in creating service account.
    #[error("Service account creation failed Error: {}", source)]
    ServiceAccountCreateError { source: kube::Error },

    /// Failed in creating cluster role.
    #[error("Cluster role creation failed Error: {}", source)]
    ClusterRoleCreateError { source: kube::Error },

    /// Failed in creating cluster role binding.
    #[error("Cluster role binding creation failed Error: {}", source)]
    ClusterRoleBindingCreateError { source: kube::Error },

    /// Failed in creating upgrade job.
    #[error("Upgrade Job creation failed Error: {}", source)]
    UpgradeJobCreateError { source: kube::Error },

    /// Failed in deleting upgrade job.
    #[error("Upgrade Job deleting failed Error: {}", source)]
    UpgradeJobDeleteError { source: kube::Error },

    /// Failed in deletion service account.
    #[error("Service account deletion failed Error: {}", source)]
    ServiceAccountDeleteError { source: kube::Error },

    /// Failed in deletion cluster role.
    #[error("Cluster role creation deletion Error: {}", source)]
    ClusterRoleDeleteError { source: kube::Error },

    /// Failed in deletion cluster role binding.
    #[error("Cluster role binding deletion failed Error: {}", source)]
    ClusterRoleBindingDeleteError { source: kube::Error },

    /// Openapi configuration error.
    #[error("openapi configuration Error: {}", source)]
    OpenapiClientConfigurationErr { source: anyhow::Error },

    /// HTTP request error.
    #[error("HTTP request error: {}", source)]
    Request {
        source: clients::tower::RequestError,
    },

    /// HTTP response error.
    #[error("HTTP response error: {}", source)]
    Response {
        source: clients::tower::ResponseError<RestJsonError>,
    },

    /// Node spec not present error.
    #[error("Node spec not present, node: {}", node)]
    NodeSpecNotPresent { node: String },

    /// Pod Name not present error.
    #[error("Pod name not present: {}", source)]
    PodNameNotPresent { source: kube::Error },

    /// Deserialization error for event.
    #[error("Error in desiaralizing upgrade event.")]
    EventSerdeDeserializationError,

    /// No message in upgrade event.
    #[error("No Message present in event.")]
    MessageInEventNotPresent,

    /// No upgrade event present.
    #[error("No upgrade event present.")]
    UpgradeEventNotPresent,
}

impl From<anyhow::Error> for Error {
    fn from(source: anyhow::Error) -> Self {
        Self::OpenapiClientConfigurationErr { source }
    }
}

impl From<clients::tower::Error<RestJsonError>> for Error {
    fn from(source: clients::tower::Error<RestJsonError>) -> Self {
        match source {
            clients::tower::Error::Request(source) => Self::Request { source },
            clients::tower::Error::Response(source) => Self::Response { source },
        }
    }
}

impl From<kube::Error> for Error {
    fn from(source: kube::Error) -> Self {
        Self::K8sClientError { source }
    }
}
