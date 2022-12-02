#[derive(Debug)]
/// Error used in exporters
pub enum ExporterError {
    GrpcResponseError(String),
    GetNodeError(String),
    InvalidURI(String),
    DeserializationError(String),
    PodIPError(String),
    GrpcClientError(String),
}
