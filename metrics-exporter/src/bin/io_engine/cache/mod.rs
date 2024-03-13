mod nexus_stat;
mod pool;
mod pool_stat;
mod replica_stat;

use crate::client::{
    grpc_client::GrpcClient, nexus_stat::NexusIoStats, pool::Pools, pool_stat::PoolIoStats,
    replica_stat::ReplicaIoStats,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

static CACHE: OnceCell<Mutex<Cache>> = OnceCell::new();

/// Trait to be implemented by all Resource structs stored in Cache.
trait ResourceOps {
    type ResourceVec;
    fn set(&mut self, val: Self::ResourceVec);
    fn invalidate(&mut self);
}

/// Cache to store data that has to be exposed though metrics-exporter.
pub(crate) struct Cache {
    data: Data,
}

/// Wrapper over all the data that has to be stored in cache.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Data {
    /// Contains Pool Capacity and state data.
    pools: Pools,
    /// Contains Pool IOStats data.
    pool_stats: PoolIoStats,
    /// Contains Nexus IOStats data.
    nexus_stats: NexusIoStats,
    /// Contains Replica IOStats data.
    replica_stats: ReplicaIoStats,
}

impl Cache {
    /// Initialize the cache with default value.
    pub(crate) fn initialize(data: Data) {
        CACHE.get_or_init(|| Mutex::new(Self { data }));
    }

    /// Returns cache.
    pub(crate) fn get_cache() -> &'static Mutex<Cache> {
        CACHE.get().expect("Cache is not initialized")
    }

    /// Get pool mutably stored in struct.
    pub(crate) fn pool_mut(&mut self) -> &mut Pools {
        &mut self.data.pools
    }

    /// Get mutable reference to PoolIOStats.
    pub(crate) fn pool_iostat_mut(&mut self) -> &mut PoolIoStats {
        &mut self.data.pool_stats
    }

    /// Get mutable reference to NexusIOStats.
    pub(crate) fn nexus_iostat_mut(&mut self) -> &mut NexusIoStats {
        &mut self.data.nexus_stats
    }

    /// Get a reference to NexusIoStats.
    pub(crate) fn nexus_iostat(&self) -> &NexusIoStats {
        &self.data.nexus_stats
    }

    /// Get a reference to Pool.
    pub(crate) fn pool(&self) -> &Pools {
        &self.data.pools
    }

    /// Get a reference to PoolIoStats.
    pub(crate) fn pool_iostat(&self) -> &PoolIoStats {
        &self.data.pool_stats
    }

    /// Get a reference to ReplicaIoStats.
    pub(crate) fn replica_iostat(&self) -> &ReplicaIoStats {
        &self.data.replica_stats
    }

    /// Get mutable reference to ReplicaIOStats.
    pub(crate) fn replica_iostat_mut(&mut self) -> &mut ReplicaIoStats {
        &mut self.data.replica_stats
    }
}

impl Default for Data {
    fn default() -> Self {
        Self::new()
    }
}

impl Data {
    /// Constructor for Cache data.
    fn new() -> Self {
        Self {
            pools: Pools { pools: vec![] },
            pool_stats: PoolIoStats { pool_stats: vec![] },
            nexus_stats: NexusIoStats {
                nexus_stats: vec![],
            },
            replica_stats: ReplicaIoStats {
                replica_stats: vec![],
            },
        }
    }
}

/// Populates Resource cache struct.
pub(crate) async fn store_resource_data(client: &GrpcClient) {
    let _ = pool::store_pool_info_data(client).await;
    let _ = pool_stat::store_pool_stats_data(client).await;
    let _ = nexus_stat::store_nexus_stats_data(client).await;
    let _ = replica_stat::store_replica_stats_data(client).await;
}
