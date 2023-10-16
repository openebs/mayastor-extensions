#[derive(Debug)]
/// Error used in exporters
pub enum ExporterError {
    GrpcResponseError(String),
    GetNodeError(String),
    InvalidURI(String),
    PodIPError(String),
    GrpcClientError(String),
    HttpServerError(String),
    HttpBindError(String),
}
