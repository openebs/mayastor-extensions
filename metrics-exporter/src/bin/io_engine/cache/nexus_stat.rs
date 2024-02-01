use super::{Cache, ResourceOps};
use crate::client::{
    grpc_client::GrpcClient,
    nexus_stat::{NexusIoStat, NexusIoStats},
};
use std::ops::DerefMut;
use tracing::error;

impl ResourceOps for NexusIoStats {
    type ResourceVec = Vec<NexusIoStat>;

    fn set(&mut self, val: Self::ResourceVec) {
        self.nexus_stats = val
    }

    fn invalidate(&mut self) {
        self.nexus_stats = vec![]
    }
}

pub(crate) async fn store_nexus_stats_data(client: &GrpcClient) -> Result<(), ()> {
    let nexus_stats = client.get_nexus_iostat().await;
    let mut cache = match Cache::get_cache().lock() {
        Ok(cache) => cache,
        Err(error) => {
            error!(%error, "Error while getting cache resource");
            return Err(());
        }
    };
    let nexus_cache = cache.deref_mut();
    match nexus_stats {
        Ok(nexus) => {
            nexus_cache.nexus_iostat_mut().set(nexus.nexus_stats);
        }
        // invalidate cache in case of error
        Err(error) => {
            error!(
                ?error,
                "Error getting nexus stats data, invalidating nexus stats cache"
            );
            nexus_cache.nexus_iostat_mut().invalidate();
            return Err(());
        }
    };
    Ok(())
}
