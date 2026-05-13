use crate::collect::resources::{traits::Topologer, ResourceError};

use serde::Serialize;
use std::path::PathBuf;

/// Defines maximum entries REST service can fetch at one network call
pub(crate) const MAX_RESOURCE_ENTRIES: isize = 200;

/// Defines maximum entries REST service can fetch at one network call, for small resources.
pub(crate) const MAX_SMALL_RESOURCE_ENTRIES: isize = 500;

impl<T> Topologer for Vec<T>
where
    T: Topologer + Serialize,
{
    fn get_printable_topology(&self) -> Result<(String, String), ResourceError> {
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        Ok(("all-topology.json".to_string(), topology_as_pretty))
    }

    fn dump_topology_info(&self, dir_path: PathBuf) -> Result<(), ResourceError> {
        for obj in self.iter() {
            obj.dump_topology_info(dir_path.clone())?;
        }
        Ok(())
    }
}
