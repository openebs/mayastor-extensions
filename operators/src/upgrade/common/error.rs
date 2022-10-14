use std::string::FromUtf8Error;

use serde::Serialize;
/// Helm error to be used for client.
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
    ClientError { source: kube::Error },

    /// Url parse error.
    #[error("Url parse Error: {}", source)]
    UrlParse { source: url::ParseError },

    /// Openapi configuration error.
    #[error("openapi configuration Error: {}", source)]
    OpenapiClientConfigurationErr { source: anyhow::Error },
}

impl From<anyhow::Error> for Error {
    fn from(source: anyhow::Error) -> Self {
        Self::OpenapiClientConfigurationErr { source }
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
        Self::ClientError { source }
    }
}

impl From<url::ParseError> for Error {
    fn from(source: url::ParseError) -> Self {
        Self::UrlParse { source }
    }
}

/// Error to be used for api calls.
#[derive(Debug, Serialize)]
pub struct RestError {
    id: u32,
    error: String,
}
