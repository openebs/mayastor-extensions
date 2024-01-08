mod pool;

use crate::client::{grpc_client::GrpcClient, pool::Pools};

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

impl Cache {
    /// Initialize the cache with default value.
    pub fn initialize(data: Data) {
        CACHE.get_or_init(|| Mutex::new(Self { data }));
    }

    /// Returns cache.
    pub fn get_cache() -> &'static Mutex<Cache> {
        CACHE.get().expect("Cache is not initialized")
    }

    /// Get pool mutably stored in struct.
    pub fn pool_mut(&mut self) -> &mut Pools {
        &mut self.data.pools
    }
}

/// Wrapper over all the data that has to be stored in cache.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Data {
    /// Contains Pool Capacity and state data.
    pools: Pools,
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
        }
    }
}

/// Populates Resource cache struct.
pub(crate) async fn store_resource_data(client: &GrpcClient) {
    let _ = pool::store_pool_info_data(client.clone()).await;
}
