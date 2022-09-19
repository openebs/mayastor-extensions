use openapi::{clients, models::RestJsonError};
use snafu::Snafu;

/// Errors generated during the reconciliation loop
#[derive(Debug, Snafu)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Error {
    #[snafu(display(
        "Failed to reconcile '{}' CRD within set limits, aborting operation",
        name
    ))]
    /// Error generated when the loop stops processing
    ReconcileError {
        name: String,
    },
    /// Generated when we have a duplicate resource version for a given resource
    Duplicate {
        timeout: u32,
    },
    /// Spec error
    SpecError {
        value: String,
        timeout: u32,
    },
    #[snafu(display("Kubernetes client error: {}", source))]
    /// k8s client error
    Kube {
        source: kube::Error,
    },
    #[snafu(display("HTTP request error: {}", source))]
    Request {
        source: clients::tower::RequestError,
    },
    #[snafu(display("HTTP response error: {}", source))]
    Response {
        source: clients::tower::ResponseError<RestJsonError>,
    },
    Noun {},
}


impl From<clients::tower::Error<RestJsonError>> for Error {
    fn from(source: clients::tower::Error<RestJsonError>) -> Self {
        match source {
            clients::tower::Error::Request(source) => Error::Request { source },
            clients::tower::Error::Response(source) => Self::Response { source },
        }
    }
}

