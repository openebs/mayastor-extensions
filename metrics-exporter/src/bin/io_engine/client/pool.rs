use crate::{client::grpc_client::GrpcClient, error::ExporterError, ApiVersion};

use serde::{Deserialize, Serialize};

/// This stores Capacity and state information of a pool.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct PoolInfo {
    name: String,
    used: u64,
    capacity: u64,
    state: u64,
    committed: u64,
}

impl PoolInfo {
    /// Get name of the pool.
    pub(crate) fn name(&self) -> &String {
        &self.name
    }

    /// Get used capacity of the pool.
    pub(crate) fn used(&self) -> u64 {
        self.used
    }

    /// Get total capacity of the pool.
    pub(crate) fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get the pool commitment in bytes.
    pub(crate) fn committed(&self) -> u64 {
        self.committed
    }

    /// Get pool of the io_engine.
    pub(crate) fn state(&self) -> u64 {
        self.state
    }
}

/// Array of PoolInfo objects.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Pools {
    pub(crate) pools: Vec<PoolInfo>,
}

/// Trait to be implemented by grpc client to call pool rpc.
#[tonic::async_trait]
pub(crate) trait PoolOperations: Send + Sync + Sized {
    async fn list_pools(&self) -> Result<Pools, ExporterError>;
}

impl From<rpc::io_engine::Pool> for PoolInfo {
    fn from(value: rpc::io_engine::Pool) -> Self {
        Self {
            name: value.name,
            used: value.used,
            capacity: value.capacity,
            state: value.state as u64,
            committed: value.used,
        }
    }
}
impl From<rpc::v1::pool::Pool> for PoolInfo {
    fn from(value: rpc::v1::pool::Pool) -> Self {
        Self {
            name: value.name,
            used: value.used,
            capacity: value.capacity,
            state: value.state as u64,
            committed: value.committed,
        }
    }
}

#[tonic::async_trait]
impl PoolOperations for GrpcClient {
    async fn list_pools(&self) -> Result<Pools, ExporterError> {
        let pools = match self.api_version() {
            ApiVersion::V0 => match self.client_v0()?.list_pools(rpc::io_engine::Null {}).await {
                Ok(response) => response
                    .into_inner()
                    .pools
                    .into_iter()
                    .map(PoolInfo::from)
                    .collect::<Vec<_>>(),
                Err(error) => return Err(ExporterError::GrpcResponseError(error.to_string())),
            },
            ApiVersion::V1 => match self
                .client_v1()?
                .pool
                .list_pools(rpc::v1::pool::ListPoolOptions::default())
                .await
            {
                Ok(response) => response
                    .into_inner()
                    .pools
                    .into_iter()
                    .map(PoolInfo::from)
                    .collect::<Vec<_>>(),
                Err(error) => return Err(ExporterError::GrpcResponseError(error.to_string())),
            },
        };

        Ok(Pools { pools })
    }
}
