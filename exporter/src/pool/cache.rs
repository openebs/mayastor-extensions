use std::{ops::DerefMut, sync::Mutex};

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{debug, error};

use crate::pool::{
    client::{
        grpc_client::GrpcClient,
        pool::{PoolOperations, Pools},
    },
    config::ExporterConfig,
};

static CACHE: OnceCell<Mutex<Cache>> = OnceCell::new();

/// Wrapper over all the data that has to be stored in cache.
#[derive(Serialize, Deserialize, Debug)]
pub struct Data {
    pools: Pools,
}

impl Default for Data {
    fn default() -> Self {
        Self::new()
    }
}

impl Data {
    // initialize Data
    fn new() -> Self {
        Self {
            pools: Pools { pools: vec![] },
        }
    }

    /// Get pools data.
    pub fn pools(&self) -> &Pools {
        &self.pools
    }

    // Set pools data.
    fn set_pools(&mut self, pools: Pools) {
        self.pools = pools;
    }

    // Invalidate pools for cache.
    fn invalidate_pools(&mut self) {
        self.pools = Pools { pools: vec![] };
    }
}

/// Cache to store data that has to be exposed though exporter.
pub struct Cache {
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

    /// Get data field in cache.
    pub fn data_mut(&mut self) -> &mut Data {
        &mut self.data
    }
}

/// To store pools related data in cache.
pub async fn store_pool_data(client: GrpcClient) {
    loop {
        let pools = client.list_pools().await;
        {
            let mut cache = match Cache::get_cache().lock() {
                Ok(cache) => cache,
                Err(error) => {
                    error!(%error, "Error while getting cache resource");
                    continue;
                }
            };
            let pools_cache = cache.deref_mut();
            match pools {
                // set pools in the cache
                Ok(pools) => {
                    debug!("Updated pool cache with latest metrics");
                    pools_cache.data_mut().set_pools(pools);
                }
                // invalidate cache in case of error
                Err(error) => {
                    error!(?error, "Error getting pools data, invalidating pools cache");
                    pools_cache.data_mut().invalidate_pools();
                }
            };
        }
        sleep(ExporterConfig::get_config().polling_time()).await;
    }
}

/// To store data in shared variable i.e cache.
pub async fn store_data(client: GrpcClient) -> Result<(), String> {
    // Store pools data
    store_pool_data(client).await;
    Ok(())
}
