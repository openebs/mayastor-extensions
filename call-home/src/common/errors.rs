use serde_json::Error;
use snafu::Snafu;

/// Contains Errors that may generate while execution of k8s_client
#[derive(Debug, Snafu)]
#[snafu(visibility(pub), context(suffix(false)))]
#[allow(clippy::enum_variant_names)]
pub enum K8sResourceError {
    #[snafu(display("Json Parse Error : {}", source))]
    SerdeError { source: serde_json::Error },
    #[snafu(display("K8Client Error: {}", source))]
    ClientError { source: kube::Error },
}

impl From<kube::Error> for K8sResourceError {
    fn from(source: kube::Error) -> Self {
        Self::ClientError { source }
    }
}

impl From<Error> for K8sResourceError {
    fn from(source: Error) -> Self {
        Self::SerdeError { source }
    }
}
