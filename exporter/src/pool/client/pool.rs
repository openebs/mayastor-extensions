use serde::{Deserialize, Serialize};

use crate::pool::{
    client::{grpc_client::GrpcClient, ApiVersion},
    error::ExporterError,
};

/// Pool resource.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Pool {
    name: String,
    disks: Vec<String>,
    used: u64,
    capacity: u64,
    state: u64,
}

impl Pool {
    /// Get name of the pool.
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Get used capacity of the pool.
    pub fn used(&self) -> u64 {
        self.used
    }

    /// Get total capacity of the pool.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get state of the pool.
    pub fn state(&self) -> u64 {
        self.state
    }
}

/// Pools resource.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Pools {
    pub pools: Vec<Pool>,
}

/// Pool operations i.e wrapper over rpc calls to get pools data.
#[tonic::async_trait]
pub trait PoolOperations: Send + Sync + Sized {
    async fn list_pools(&self) -> Result<Pools, ExporterError>;
}

impl From<rpc::io_engine::Pool> for Pool {
    fn from(value: rpc::io_engine::Pool) -> Self {
        Self {
            name: value.name,
            disks: value.disks,
            used: value.used,
            capacity: value.capacity,
            state: value.state as u64,
        }
    }
}
impl From<rpc::v1::pool::Pool> for Pool {
    fn from(value: rpc::v1::pool::Pool) -> Self {
        Self {
            name: value.name,
            disks: value.disks,
            used: value.used,
            capacity: value.capacity,
            state: value.state as u64,
        }
    }
}

#[tonic::async_trait]
impl PoolOperations for GrpcClient {
    // wrapper over list_pools rpc call
    async fn list_pools(&self) -> Result<Pools, ExporterError> {
        let pools = match self.api_version() {
            ApiVersion::V0 => match self.client_v0()?.list_pools(rpc::io_engine::Null {}).await {
                Ok(response) => response
                    .into_inner()
                    .pools
                    .into_iter()
                    .map(Pool::from)
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
                    .map(Pool::from)
                    .collect::<Vec<_>>(),
                Err(error) => return Err(ExporterError::GrpcResponseError(error.to_string())),
            },
        };

        Ok(Pools { pools })
    }
}
