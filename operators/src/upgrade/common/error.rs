/// K8sResourceError holds errors that can obtain while fetching
/// information of Kubernetes Objects
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum K8sResourceError {
    ClientConfigError(kube::config::KubeconfigError),
    InferConfigError(kube::config::InferConfigError),
    ClientError(kube::Error),
    ResourceError(Box<dyn std::error::Error>),
    CustomError(String),
}

impl From<kube::config::KubeconfigError> for K8sResourceError {
    fn from(e: kube::config::KubeconfigError) -> K8sResourceError {
        K8sResourceError::ClientConfigError(e)
    }
}

impl From<kube::config::InferConfigError> for K8sResourceError {
    fn from(e: kube::config::InferConfigError) -> K8sResourceError {
        K8sResourceError::InferConfigError(e)
    }
}

impl From<kube::Error> for K8sResourceError {
    fn from(e: kube::Error) -> K8sResourceError {
        K8sResourceError::ClientError(e)
    }
}

impl From<Box<dyn std::error::Error>> for K8sResourceError {
    fn from(e: Box<dyn std::error::Error>) -> K8sResourceError {
        K8sResourceError::ResourceError(e)
    }
}

impl From<String> for K8sResourceError {
    fn from(e: String) -> K8sResourceError {
        K8sResourceError::CustomError(e)
    }
}

impl K8sResourceError {
    /// Returns K8sResourceError from provided message
    pub fn invalid_k8s_resource_value(err: String) -> Self {
        Self::CustomError(err)
    }
}
