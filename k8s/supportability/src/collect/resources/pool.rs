use crate::{
    collect::{
        logs::create_directory_if_not_exist, resources, resources::traits, rest_wrapper::RestClient,
    },
    log,
};
use openapi::models::{BlockDevice, Node, Pool};
use resources::ResourceError;
use traits::{Resourcer, Topologer};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs::File, io::Write, iter::FromIterator, path::PathBuf};

/// PoolTopology represents information about
/// mayastor pool and all it's descendants
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct PoolTopology {
    pool: Pool,
    node_info: Option<Node>,
    device_info: Option<Vec<BlockDevice>>,
}

/// Topologer Contains methods to inspect topology information of pool resource
#[async_trait(?Send)]
impl Topologer for PoolTopology {
    fn get_printable_topology(&self) -> Result<(String, String), ResourceError> {
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        let file_path = format!("pool-{}-topology.json", self.pool.id);
        Ok((file_path, topology_as_pretty))
    }

    fn dump_topology_info(&self, dir_path: PathBuf) -> Result<(), ResourceError> {
        create_directory_if_not_exist(&dir_path)?;
        let file_path = dir_path.join(format!("pool-{}-topology.json", self.pool.id));
        let mut topo_file = File::create(file_path)?;
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        topo_file.write_all(topology_as_pretty.as_bytes())?;
        topo_file.flush()?;
        Ok(())
    }
}

/// Wrapper around mayastor REST client which interns used to interact with REST client
#[derive(Debug)]
pub struct PoolClientWrapper {
    rest_client: RestClient,
}

impl PoolClientWrapper {
    /// Creates new instance of PoolClientWrapper
    pub fn new(client: RestClient) -> Self {
        PoolClientWrapper {
            rest_client: client,
        }
    }

    // TODO: Add pagination support when REST service supports it
    async fn list_pools(&self) -> Result<Vec<Pool>, ResourceError> {
        let pools = self.rest_client.pools_api().get_pools(None).await?;
        Ok(pools.into_body())
    }

    async fn get_pool(&self, id: &str) -> Result<Pool, ResourceError> {
        let pool = self.rest_client.pools_api().get_pool(id).await?.into_body();
        Ok(pool)
    }

    async fn get_pool_node_info(&self, pool: &Pool) -> Result<Option<Node>, ResourceError> {
        if let Some(pool_spec) = &pool.spec {
            let node = self
                .rest_client
                .nodes_api()
                .get_node(&pool_spec.node)
                .await?
                .into_body();
            return Ok(Some(node));
        }
        Ok(None)
    }

    async fn get_pool_disks_info(
        &self,
        pool: &Pool,
    ) -> Result<Option<Vec<BlockDevice>>, ResourceError> {
        if let Some(pool_spec) = &pool.spec {
            let devices = self
                .rest_client
                .block_devices_api()
                .get_node_block_devices(&pool_spec.node, Some(true))
                .await?
                .into_body();

            let filtered_devices: Vec<BlockDevice> = devices
                .into_iter()
                .filter(|device| is_it_pool_device(pool_spec, device))
                .collect::<Vec<BlockDevice>>();
            return Ok(Some(filtered_devices));
        }
        Ok(None)
    }
}

fn is_it_pool_device(pool_spec: &openapi::models::PoolSpec, device: &BlockDevice) -> bool {
    let mut device_links: HashSet<String> = HashSet::from_iter(device.devlinks.iter().cloned());
    device_links.insert(device.devname.clone());
    device_links.insert(device.devpath.clone());
    for pool_device_name in &pool_spec.disks {
        if device_links.contains(pool_device_name) {
            return true;
        }
    }
    false
}

#[async_trait(?Send)]
impl Resourcer for PoolClientWrapper {
    type ID = String;

    async fn get_topologer(
        &self,
        id: Option<Self::ID>,
    ) -> Result<Box<dyn Topologer>, ResourceError> {
        // When ID is provided then caller needs topologer for given pool id
        if let Some(pool_id) = id {
            let pool = self.get_pool(&pool_id).await?;
            let node_info = match self.get_pool_node_info(&pool).await {
                Ok(node_info) => node_info,
                Err(e) => {
                    // TODO: Collect errors and return to caller at end
                    log(format!(
                        "Failed to get node information for pool: {pool_id}, error: {e:?}"
                    ));
                    None
                }
            };
            let device_info = match self.get_pool_disks_info(&pool).await {
                Ok(d_info) => d_info,
                Err(e) => {
                    // TODO: Collect errors and return to caller at end
                    log(format!(
                        "Failed to get device information for pool: {pool_id}, error: {e:?}"
                    ));
                    None
                }
            };
            let pool_topology = PoolTopology {
                pool,
                node_info,
                device_info,
            };
            return Ok(Box::new(pool_topology));
        }

        // When ID is not provided then caller needs topology information to build for all
        // available pools
        let mut pools_topology: Vec<PoolTopology> = Vec::new();
        let pools = self.list_pools().await?;
        for pool in pools.into_iter() {
            let node_info = match self.get_pool_node_info(&pool).await {
                Ok(node_info) => node_info,
                Err(e) => {
                    // TODO: Collect errors and return to caller at end
                    log(format!(
                        "Failed to get node information for pools, error: {e:?}"
                    ));
                    None
                }
            };
            let device_info = match self.get_pool_disks_info(&pool).await {
                Ok(d_info) => d_info,
                Err(e) => {
                    // TODO: Collect errors and return to caller at end
                    log(format!(
                        "Failed to get device information for pools, error: {e:?}"
                    ));
                    None
                }
            };

            let pool_topology = PoolTopology {
                pool,
                node_info,
                device_info,
            };
            pools_topology.push(pool_topology);
        }
        Ok(Box::new(pools_topology))
    }
}
