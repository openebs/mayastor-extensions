/// Helm error to be used for client.
#[derive(Debug, Clone)]
pub(crate) enum Error {
    /// Helm not installed error.
    HelmNotInstalled(String),
    /// Error while running helm command.
    HelmStd(String),
    /// Error when specific helm version not found.
    HelmVersionNotFound(String, String),
    /// Error when helm chart is not present in the cluster.
    HelmChartNotFound(String),
    /// Error when converting utf8 to string.
    Utf8(String),
    /// Error while running helm get values command.
    HelmGetValues(String),
    /// Deserialization error for helm client.
    SerdeDeserialization(String),
}
