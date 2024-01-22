use super::{Cache, ResourceOps};
use crate::client::{
    grpc_client::GrpcClient,
    pool_stat::{PoolIoStat, PoolIoStats},
};
use std::ops::DerefMut;
use tracing::error;

impl ResourceOps for PoolIoStats {
    type ResourceVec = Vec<PoolIoStat>;

    fn set(&mut self, val: Self::ResourceVec) {
        self.pool_stats = val
    }

    fn invalidate(&mut self) {
        self.pool_stats = vec![]
    }
}

/// To store pool iostat data in cache.
pub(crate) async fn store_pool_stats_data(client: &GrpcClient) -> Result<(), ()> {
    let pool_stats = client.get_pool_iostat().await;
    let mut cache = match Cache::get_cache().lock() {
        Ok(cache) => cache,
        Err(error) => {
            error!(%error, "Error while getting cache resource");
            return Err(());
        }
    };
    let pools_cache = cache.deref_mut();
    match pool_stats {
        // set pools in the cache
        Ok(pools) => {
            pools_cache.pool_iostat_mut().set(pools.pool_stats);
        }
        // invalidate cache in case of error
        Err(error) => {
            error!(?error, "Error getting pools data, invalidating pools cache");
            pools_cache.pool_iostat_mut().invalidate();
            return Err(());
        }
    };
    Ok(())
}
