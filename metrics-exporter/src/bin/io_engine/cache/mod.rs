mod nexus_stat;
mod pool;
mod pool_stat;
mod replica_stat;

use crate::client::{
    grpc_client::GrpcClient, nexus_stat::NexusIoStats, pool::Pools, pool_stat::PoolIoStats,
    replica_stat::ReplicaIoStats,
};
use once_cell::sync::OnceCell;
use std::{collections::HashMap, sync::Mutex};

/// NOTE: try to reference cache from the Collector.
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
#[derive(Debug)]
pub(crate) struct Data {
    /// Contains Pool Capacity and state data.
    pools: Pools,
    /// Contains Pool IOStats data.
    pool_stats: PoolIoStats,
    /// Contains Nexus IOStats data.
    nexus_stats: NexusIoStats,
    /// Contains Replica IOStats data.
    replica_stats: ReplicaIoStats,
    /// Maps replica name to pool_name.
    replica_pool_map: HashMap<String, String>,
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

    /// Get a reference to replica_pool_map.
    pub(crate) fn replica_pool_map(&self) -> &HashMap<String, String> {
        &self.data.replica_pool_map
    }

    /// Get mutable reference to replica_pool_map.
    pub(crate) fn replica_pool_map_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.data.replica_pool_map
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
            replica_pool_map: HashMap::new(),
        }
    }
}

/// Populates Resource cache struct.
pub(crate) async fn store_resource_data(client: &GrpcClient) {
    let _ = pool::store_pool_info_data(client).await;
    let _ = pool_stat::store_pool_stats_data(client).await;
    let _ = nexus_stat::store_nexus_stats_data(client).await;
    let _ = replica_stat::store_replica_stats_data(client).await;
    store_replica_pool_map(client).await;
}

/// Fetches replica list and stores replica name → pool_name mapping in cache.
/// Only refreshes when new replicas appear that aren't in the existing map.
async fn store_replica_pool_map(client: &GrpcClient) {
    let needs_refresh = {
        let cache = match Cache::get_cache().lock() {
            Ok(cache) => cache,
            Err(_) => return,
        };
        let map = cache.replica_pool_map();
        cache
            .replica_iostat()
            .replica_stats
            .iter()
            .any(|r| !map.contains_key(r.name()))
    };

    if !needs_refresh {
        return;
    }

    match client.list_replicas().await {
        Ok(new_map) => {
            if let Ok(mut cache) = Cache::get_cache().lock() {
                *cache.replica_pool_map_mut() = new_map;
            }
        }
        Err(error) => {
            tracing::error!(?error, "Error fetching replica list for pool mapping");
        }
    }
}
