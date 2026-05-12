use crate::{
    collect::{
        logs::create_directory_if_not_exist,
        resources,
        resources::{traits, utils},
        rest_wrapper::RestClient,
    },
    log,
};
use openapi::models::AppNode;
use resources::ResourceError;
use traits::{Resourcer, Topologer};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, path::PathBuf};

/// NodeTopology represents information about mayastor application/csi nodes.
/// todo: add nvmf devices and their subsystems.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AppNodeTopology {
    node: AppNode,
}

/// Topologer Contains methods to build topology information for generic Object
/// which interns supports inspecting topology resources
#[async_trait(?Send)]
impl Topologer for AppNodeTopology {
    /// Convert node topology into JSON structure
    fn get_printable_topology(&self) -> Result<(String, String), ResourceError> {
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        let file_path = format!("app-node-{}-topology.json", self.node.id);
        Ok((file_path, topology_as_pretty))
    }

    /// Writes topology information into a file in specified directory
    fn dump_topology_info(&self, dir_path: PathBuf) -> Result<(), ResourceError> {
        create_directory_if_not_exist(&dir_path)?;
        let file_path = dir_path.join(format!("app-node-{}-topology.json", self.node.id));
        let mut topo_file = File::create(file_path)?;
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        topo_file.write_all(topology_as_pretty.as_bytes())?;
        topo_file.flush()?;
        Ok(())
    }
}

// Wrapper around mayastor REST client
#[derive(Debug)]
pub(crate) struct AppNodeClientWrapper {
    pub rest_client: RestClient,
}

impl AppNodeClientWrapper {
    /// Creates new instance of AppNodeClientWrapper
    pub(crate) fn new(rest_client: RestClient) -> Self {
        Self { rest_client }
    }

    async fn list_nodes(&self) -> Result<Vec<AppNode>, ResourceError> {
        let mut app_nodes: Vec<AppNode> = Vec::new();
        let mut next_token: Option<isize> = Some(0);
        let max_entries: isize = utils::MAX_SMALL_RESOURCE_ENTRIES;
        let client = self.rest_client.app_nodes_api();
        loop {
            let api_resp = client.get_app_nodes(max_entries, next_token).await?;
            let content = api_resp.into_body();

            app_nodes.extend(content.entries);
            if content.next_token.is_none() {
                break;
            }
            next_token = content.next_token;
        }
        Ok(app_nodes)
    }

    async fn get_node(&self, id: &str) -> Result<AppNode, ResourceError> {
        let node = self.rest_client.app_nodes_api().get_app_node(id).await?;
        Ok(node.into_body())
    }
}

#[async_trait(?Send)]
impl Resourcer for AppNodeClientWrapper {
    type ID = String;

    async fn get_topologer(
        &self,
        id: Option<Self::ID>,
    ) -> Result<Box<dyn Topologer>, ResourceError> {
        // When ID is provided then caller needs topologer for given node name
        if let Some(node_id) = id {
            let node = self.get_node(&node_id).await?;
            // todo: fetch connected nvmf devices and subsystems
            let node_topology = AppNodeTopology { node };
            return Ok(Box::new(node_topology));
        }
        // When ID is not provided then caller needs topology information to build for all
        // available nodes
        let mut nodes_topology: Vec<AppNodeTopology> = Vec::new();
        let nodes = self.list_nodes().await?;
        for node in nodes {
            let node_topology = AppNodeTopology { node };
            nodes_topology.push(node_topology);
        }
        if nodes_topology.is_empty() {
            log("No AppNode resources, are the csi-node daemonset pods in Running State?!!");
            return Err(ResourceError::CustomError(
                "No AppNode resources".to_string(),
            ));
        }
        Ok(Box::new(nodes_topology))
    }
}
