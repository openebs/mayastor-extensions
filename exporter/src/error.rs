#[derive(Debug)]
/// Error used in exporters
pub enum ExporterError {
    GrpcContextError(String),
    GrpcConnectTimeout {
        endpoint: String,
        timeout: std::time::Duration,
    },
    GrpcResponseError(String),
    GetNodeError(String),
    InvalidURI(String),
    DeserializationError(String),
    PodIPError(String),
}
