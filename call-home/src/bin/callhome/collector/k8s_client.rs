use k8s_openapi::api::core::v1::{Namespace, Node};
use kube::{
    api::{Api, ListParams},
    Client,
};
use obs::common::errors::K8sResourceError;

const KUBE_API_PAGE_SIZE: u32 = 500;

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

    /// Get number of nodes present in the cluster.
    pub(crate) async fn get_node_len(&self) -> Result<usize, K8sResourceError> {
        let mut nodes_count: usize = 0;

        let nodes: Api<Node> = Api::all(self.client.clone());
        let mut list_params = ListParams::default().limit(KUBE_API_PAGE_SIZE);

        loop {
            let node_list = nodes.list_metadata(&list_params).await?;

            let continue_ = node_list.metadata.continue_.clone();

            nodes_count += node_list.items.len();

            match continue_ {
                Some(token) => {
                    list_params = list_params.continue_token(token.as_str());
                }
                None => break,
            }
        }

        Ok(nodes_count)
    }

    /// Get kube-system namespace uid.
    pub(crate) async fn get_cluster_id(&self) -> Result<String, K8sResourceError> {
        let namespace_api: Api<Namespace> = Api::all(self.client.clone());
        let kube_system_namespace = namespace_api.get_metadata("kube-system").await?;
        Ok(kube_system_namespace.metadata.uid.unwrap_or_default())
    }
}
