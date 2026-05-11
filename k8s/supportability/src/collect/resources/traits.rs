use crate::collect::resources::error::ResourceError;

use async_trait::async_trait;
use downcast_rs::{impl_downcast, Downcast};
use std::{fmt::Debug, path::PathBuf};

/// Implements functionality to inspect topology information
pub(crate) trait Topologer: Downcast + Debug {
    #[allow(unused)]
    fn get_printable_topology(&self) -> Result<(String, String), ResourceError>;
    fn dump_topology_info(&self, dir_path: PathBuf) -> Result<(), ResourceError>;
}
impl_downcast!(Topologer);

/// Resourcer adds functionality to read inputs and build topology information
#[async_trait(?Send)]
pub(crate) trait Resourcer {
    type ID;
    async fn get_topologer(
        &self,
        _id: Option<Self::ID>,
    ) -> Result<Box<dyn Topologer>, ResourceError> {
        panic!("get_topologer is unimplemented");
    }
}
