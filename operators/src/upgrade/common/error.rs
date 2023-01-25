use openapi::{clients, models::RestJsonError};
use serde::Serialize;
use std::string::FromUtf8Error;
use thiserror::Error;

/// Contains Errors that may generate while execution of k8s_client.
#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    /// Helm not installed error.
    #[error("Error: {0}")]
    HelmNotInstalled(String),

    /// Error while running helm command.
    #[error("Error: {0}")]
    HelmStd(String),

    /// Error when helm commands throws io error.
    #[error("Not able to execute helm command:{}", source)]
    HelmCommandNotExecutable { source: std::io::Error },

    /// Error when specific helm version not found.
    #[error("{error:?} {version:?}")]
    HelmVersionNotFound { error: String, version: String },

    /// Error when helm chart is not present in the cluster.
    #[error("{0}")]
    HelmChartNotFound(String),

    /// Error when a thread failed to synchronize successfully
    #[error("JoinError: {}", source)]
    JoinError { source: tokio::task::JoinError },

    /// Error when converting utf8 to string.
    #[error("{}", source)]
    Utf8 { source: FromUtf8Error },

    /// Error while running helm get values command.
    #[error("{0}")]
    HelmGetValues(String),

    /// Deserialization error for helm client.
    #[error("{}", source)]
    SerdeDeserialization { source: serde_json::Error },

    /// K8s client error.
    #[error("K8Client Error: {}", source)]
    K8sClientError { source: kube::Error },

    /// Url parse error.
    #[error("Url parse Error: {}", source)]
    UrlParse { source: url::ParseError },

    /// Openapi configuration error.
    #[error("openapi configuration Error: {}", source)]
    OpenapiClientConfigurationErr { source: anyhow::Error },

    /// Error generated when the loop stops processing
    #[error(
        "Failed to reconcile '{}' CRD within set limits, aborting operation",
        name
    )]
    ReconcileError { name: String },

    /// Error for when we get an unexpected response from the Kubernetes cluster API server
    #[error(
        "Unexpected behaviour from Kubernetes API server: resource_name: {}, reason: {}",
        name,
        reason
    )]
    K8sApiError { name: String, reason: String },

    /// Generated when we have a duplicate resource version for a given resource
    #[error("Duplicate: {}", timeout)]
    Duplicate { timeout: u32 },

    /// Spec error
    #[error(
        "Failed to reconcile '{}' CRD within set limits, aborting operation",
        value
    )]
    SpecError { value: String, timeout: u32 },

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

    /// Node status not present error.
    #[error("Node status not present, node: {}", node)]
    NodeStatusNotPresent { node: String },

    /// Node Condition error.
    #[error("Node condition not present, node: {}", node)]
    NodeConditionNotPresent { node: String },

    /// Volume response error.
    #[error("Volume response error, source: {}", source)]
    VolumeResponse {
        source: clients::tower::Error<RestJsonError>,
    },

    /// Io error.
    #[error("file io error: {}", source)]
    IoError { source: std::io::Error },

    /// Error for when all volumes are not unpublished.
    #[error("All volumes must be unpublished before upgrade: {}", reason)]
    VolumesNotUnpublishedError { reason: String },
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Self::IoError { source }
    }
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

impl From<FromUtf8Error> for Error {
    fn from(source: FromUtf8Error) -> Self {
        Self::Utf8 { source }
    }
}

impl From<serde_json::Error> for Error {
    fn from(source: serde_json::Error) -> Self {
        Self::SerdeDeserialization { source }
    }
}

impl From<kube::Error> for Error {
    fn from(source: kube::Error) -> Self {
        Self::K8sClientError { source }
    }
}

impl From<url::ParseError> for Error {
    fn from(source: url::ParseError) -> Self {
        Self::UrlParse { source }
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(source: tokio::task::JoinError) -> Self {
        Self::JoinError { source }
    }
}

/// Error to be used for api calls.
#[derive(Debug, Serialize, Default)]
pub struct RestError {
    id: u32,
    error: String,
}

impl RestError {
    /// Changes the error ID, for use with a default RestError.
    pub(crate) fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }

    /// Changes the error String, for use with a default RestError.
    pub(crate) fn with_error(mut self, error: String) -> Self {
        self.error = error;
        self
    }
}
