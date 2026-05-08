use crate::{
    collect::{
        logs::create_directory_if_not_exist, resources, resources::traits, rest_wrapper::RestClient,
    },
    log,
};
use openapi::models::{BlockDevice, Node};
use resources::ResourceError;
use traits::{Resourcer, Topologer};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, path::PathBuf};

/// NodeTopology represents information about
/// mayastor node and devices attached to node
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct NodeTopology {
    node: Node,
    devices: Option<Vec<BlockDevice>>,
}

/// Topologer Contains methods to build topology information for generic Object
/// which interns supports inspecting topology resources
#[async_trait(?Send)]
impl Topologer for NodeTopology {
    /// Convert node topology into JSON structure
    fn get_printable_topology(&self) -> Result<(String, String), ResourceError> {
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        let file_path = format!("node-{}-topology.json", self.node.id);
        Ok((file_path, topology_as_pretty))
    }

    /// Writes topology information into a file in specified directory
    fn dump_topology_info(&self, dir_path: PathBuf) -> Result<(), ResourceError> {
        create_directory_if_not_exist(dir_path.clone())?;
        let file_path = dir_path.join(format!("node-{}-topology.json", self.node.id));
        let mut topo_file = File::create(file_path)?;
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        topo_file.write_all(topology_as_pretty.as_bytes())?;
        topo_file.flush()?;
        Ok(())
    }
}

// Wrapper around mayastor REST client
#[derive(Debug)]
pub(crate) struct NodeClientWrapper {
    pub rest_client: RestClient,
}

impl NodeClientWrapper {
    /// Creates new instance of NodeClientWrapper
    pub(crate) fn new(rest_client: RestClient) -> Self {
        NodeClientWrapper { rest_client }
    }

    // TODO: Add pagination support when REST service supports it
    async fn list_nodes(&self) -> Result<Vec<Node>, ResourceError> {
        let nodes = self
            .rest_client
            .nodes_api()
            .get_nodes(None)
            .await?
            .into_body();
        Ok(nodes)
    }

    async fn get_node(&self, id: &str) -> Result<Node, ResourceError> {
        let node = self.rest_client.nodes_api().get_node(id).await?.into_body();
        Ok(node)
    }

    async fn list_node_block_devices(&self, id: &str) -> Result<Vec<BlockDevice>, ResourceError> {
        let devices = match self
            .rest_client
            .block_devices_api()
            .get_node_block_devices(id, Some(true))
            .await
        {
            Ok(response) => response.into_body(),
            Err(err) => {
                let _is_not_found =
                    ResourceError::RestJsonError(err).not_found_rest_json_error()?;
                Vec::new()
            }
        };
        Ok(devices)
    }
}

#[async_trait(?Send)]
impl Resourcer for NodeClientWrapper {
    type ID = String;

    async fn get_topologer(
        &self,
        id: Option<Self::ID>,
    ) -> Result<Box<dyn Topologer>, ResourceError> {
        // When ID is provided then caller needs topologer for given node name
        if let Some(node_id) = id {
            let node = self.get_node(&node_id).await?;
            let mut devices: Option<Vec<BlockDevice>> = None;
            if let Some(node_state) = node.clone().state {
                if matches!(node_state.status, openapi::models::NodeStatus::Online) {
                    devices = Some(self.list_node_block_devices(&node_id).await?);
                }
            }
            let node_topology = NodeTopology { node, devices };
            return Ok(Box::new(node_topology));
        }
        // When ID is not provided then caller needs topology information to build for all
        // available nodes
        let mut nodes_topology: Vec<NodeTopology> = Vec::new();
        let mayastor_nodes = self.list_nodes().await?;
        for node in mayastor_nodes.iter() {
            let mut devices: Option<Vec<BlockDevice>> = None;
            if let Some(node_state) = &node.state {
                if matches!(node_state.status, openapi::models::NodeStatus::Online) {
                    devices = Some(self.list_node_block_devices(&node.id).await?);
                }
            }
            let node_topology = NodeTopology {
                node: node.clone(),
                devices,
            };
            nodes_topology.push(node_topology);
        }
        if nodes_topology.is_empty() {
            log("No Node resources, Are daemonset pods in Running State?!!");
            return Err(ResourceError::CustomError("No Node resources".to_string()));
        }
        Ok(Box::new(nodes_topology))
    }
}
