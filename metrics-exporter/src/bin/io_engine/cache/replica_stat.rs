use super::{Cache, ResourceOps};
use crate::client::{
    grpc_client::GrpcClient,
    replica_stat::{ReplicaIoStat, ReplicaIoStats},
};
use std::ops::DerefMut;
use tracing::error;

impl ResourceOps for ReplicaIoStats {
    type ResourceVec = Vec<ReplicaIoStat>;

    fn set(&mut self, val: Self::ResourceVec) {
        self.replica_stats = val
    }

    fn invalidate(&mut self) {
        self.replica_stats = vec![]
    }
}

/// To store replica iostat data in cache.
pub(crate) async fn store_replica_stats_data(client: &GrpcClient) -> Result<(), ()> {
    let replica_stats = client.get_replica_iostat().await;
    let mut cache = match Cache::get_cache().lock() {
        Ok(cache) => cache,
        Err(error) => {
            error!(%error, "Error while getting cache resource");
            return Err(());
        }
    };
    let replica_cache = cache.deref_mut();
    match replica_stats {
        Ok(replicas) => {
            replica_cache
                .replica_iostat_mut()
                .set(replicas.replica_stats);
        }
        // invalidate cache in case of error
        Err(error) => {
            error!(
                ?error,
                "Error getting replica stats data, invalidating replica cache"
            );
            replica_cache.replica_iostat_mut().invalidate();
            return Err(());
        }
    };
    Ok(())
}
