use super::{Cache, ResourceOps};
use crate::client::{grpc_client::GrpcClient, smart::PoolsSmart};
use rpc::v1::pool::PoolSmart;
use std::ops::DerefMut;
use tracing::error;

impl ResourceOps for PoolsSmart {
    type ResourceVec = Vec<PoolSmart>;

    fn set(&mut self, val: Self::ResourceVec) {
        self.pools = val
    }

    fn invalidate(&mut self) {
        self.pools = vec![]
    }
}

/// To store pools SMART data in cache.
pub(crate) async fn store_pools_smart_data(client: &GrpcClient) -> Result<(), ()> {
    let pools = client.list_pools_smart().await;
    let mut cache = match Cache::get_cache().lock() {
        Ok(cache) => cache,
        Err(error) => {
            error!(%error, "Error while getting cache resource");
            return Err(());
        }
    };
    let pools_cache = cache.deref_mut();
    match pools {
        Ok(pools) => {
            pools_cache.pools_smart_mut().set(pools);
        }
        // invalidate cache in case of error
        Err(error) => {
            error!(
                ?error,
                "Error getting pools SMART data, invalidating SMART cache"
            );
            pools_cache.pools_smart_mut().invalidate();
            return Err(());
        }
    };
    Ok(())
}
