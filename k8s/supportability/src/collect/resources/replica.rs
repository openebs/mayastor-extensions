use crate::collect::{
    resources,
    resources::{
        pool::{PoolClientWrapper, PoolTopology},
        Resourcer,
    },
    rest_wrapper::RestClient,
};
use openapi::models::Replica;
use resources::ResourceError;

use serde::{Deserialize, Serialize};

/// ReplicaTopology represents information about
/// mayastor volume replica and all it's descendants
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ReplicaTopology {
    replica: Replica,
    pool_topology: PoolTopology,
}

// Wrapper around mayastor REST client
#[derive(Debug)]
pub struct ReplicaClientWrapper {
    rest_client: RestClient,
    pool_client: PoolClientWrapper,
}

impl ReplicaClientWrapper {
    /// Creates new instance of ReplicaClientWrapper
    pub fn new(client: RestClient) -> Self {
        ReplicaClientWrapper {
            rest_client: client.clone(),
            pool_client: PoolClientWrapper::new(client),
        }
    }

    // TODO: Add pagination support when REST service supports it
    #[allow(dead_code)]
    async fn list_replicas(&self) -> Result<Vec<Replica>, ResourceError> {
        let replicas = self
            .rest_client
            .replicas_api()
            .get_replicas()
            .await?
            .into_body();
        Ok(replicas)
    }

    async fn get_replica(&self, id: openapi::apis::Uuid) -> Result<Replica, ResourceError> {
        let replicas = self
            .rest_client
            .replicas_api()
            .get_replica(&id)
            .await?
            .into_body();
        Ok(replicas)
    }

    /// Fetch topological information of replica and all it's descendants(pool, node)
    pub(crate) async fn get_replica_topology(
        &self,
        id: openapi::apis::Uuid,
    ) -> Result<ReplicaTopology, ResourceError> {
        let replica = self.get_replica(id).await?;
        let topologer = self
            .pool_client
            .get_topologer(Some(replica.pool.clone()))
            .await?;
        let pool_topology = match topologer.downcast_ref::<PoolTopology>() {
            Some(val) => val.clone(),
            None => {
                panic!("Not a PoolTopology type");
            }
        };

        Ok(ReplicaTopology {
            replica,
            pool_topology,
        })
    }
}
