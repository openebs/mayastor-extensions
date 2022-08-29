#[derive(Debug, Clone)]
pub enum HelmError {
    HelmNotInstalled(String),
    HelmStdError(String),
    HelmVersionNotFound(String, String),
    HelmChartNotFound(String),
    Utf8Error(String),
    HelmGetValuesError(String),
    SerdeDeserializationError(String),
}
