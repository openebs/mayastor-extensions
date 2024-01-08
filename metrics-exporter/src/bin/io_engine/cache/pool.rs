use super::{Cache, ResourceOps};
use crate::client::{
    grpc_client::GrpcClient,
    pool::{PoolInfo, PoolOperations, Pools},
};
use std::ops::DerefMut;
use tracing::error;

impl ResourceOps for Pools {
    type ResourceVec = Vec<PoolInfo>;

    fn set(&mut self, val: Self::ResourceVec) {
        self.pools = val
    }

    fn invalidate(&mut self) {
        self.pools = vec![]
    }
}

/// To store pools state and capacity data in cache.
pub(crate) async fn store_pool_info_data(client: GrpcClient) -> Result<(), ()> {
    let pools = client.list_pools().await;
    let mut cache = match Cache::get_cache().lock() {
        Ok(cache) => cache,
        Err(error) => {
            error!(%error, "Error while getting cache resource");
            return Err(());
        }
    };
    let pools_cache = cache.deref_mut();
    match pools {
        // set pools in the cache
        Ok(pools) => {
            pools_cache.pool_mut().set(pools.pools);
        }
        // invalidate cache in case of error
        Err(error) => {
            error!(?error, "Error getting pools data, invalidating pools cache");
            pools_cache.pool_mut().invalidate();
            return Err(());
        }
    };
    Ok(())
}
