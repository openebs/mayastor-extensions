use crate::collect::resources::{
    traits::{ResourceInformation, Topologer},
    ResourceError,
};
use serde::Serialize;
use std::collections::HashSet;

/// Defines maximum entries REST service can fetch at one network call
pub(crate) const MAX_RESOURCE_ENTRIES: isize = 200;

impl<T> Topologer for Vec<T>
where
    T: Topologer + Serialize,
{
    fn get_printable_topology(&self) -> Result<(String, String), ResourceError> {
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        Ok(("all-topology.json".to_string(), topology_as_pretty))
    }

    fn dump_topology_info(&self, dir_path: String) -> Result<(), ResourceError> {
        for obj in self.iter() {
            obj.dump_topology_info(dir_path.clone())?;
        }
        Ok(())
    }

    fn get_unhealthy_resource_info(&self) -> HashSet<ResourceInformation> {
        let mut resources = HashSet::new();
        for topo in self.iter() {
            resources.extend(topo.get_unhealthy_resource_info());
        }
        resources
    }

    fn get_all_resource_info(&self) -> HashSet<ResourceInformation> {
        let mut resources = HashSet::new();
        for topo in self.iter() {
            resources.extend(topo.get_all_resource_info());
        }
        resources
    }

    fn get_k8s_resource_names(&self) -> Vec<String> {
        self.iter()
            .flat_map(|t| t.get_k8s_resource_names())
            .collect::<Vec<String>>()
    }
}
