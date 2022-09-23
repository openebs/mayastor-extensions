use kube::Client;

use crate::upgrade::common::error::Error;

#[derive(Clone)]
pub(crate) struct K8sClient {
    client: kube::Client,
}

impl K8sClient {
    /// Create a new K8sClient from default configuration.
    pub(crate) async fn new() -> Result<Self, Error> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }
}
