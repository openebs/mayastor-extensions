use crate::common::errors::K8sResourceError;
use k8s_openapi::api::core::v1::{Namespace, Node};
use kube::{Api, Client};

/// K8sClient contains k8s client.
#[derive(Clone)]
pub(crate) struct K8sClient {
    client: kube::Client,
}

impl K8sClient {
    /// Create a new K8sClient from default configuration.
    pub(crate) async fn new() -> Result<Self, K8sResourceError> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }

    /// Get a clone of the inner `kube::Client`.
    pub(crate) fn kube_client(&self) -> kube::Client {
        self.client.clone()
    }

    /// Get number of nodes present in the cluster.
    pub(crate) async fn get_node_len(&self) -> Result<usize, K8sResourceError> {
        let nodes: Api<Node> = Api::all(self.client.clone());
        let list = nodes.list(&Default::default()).await?;
        Ok(list.items.len())
    }

    /// Get kube-system namespace uuid.
    pub(crate) async fn get_cluster_id(&self) -> Result<String, K8sResourceError> {
        let namespace_api: Api<Namespace> = Api::all(self.client.clone());
        let kube_system_namespace = namespace_api.get("kube-system").await?;
        Ok(kube_system_namespace.metadata.uid.unwrap_or_default())
    }
}
