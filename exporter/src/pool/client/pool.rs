use serde::{Deserialize, Serialize};
use tracing::error;

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

#[tonic::async_trait]
impl PoolOperations for GrpcClient {
    // wrapper over list_pools rpc call
    async fn list_pools(&self) -> Result<Pools, ExporterError> {
        let response = match self.api_version() {
            ApiVersion::V0 => match self.client_v0()?.list_pools(rpc::io_engine::Null {}).await {
                Ok(response) => serde_json::to_string_pretty(&response.into_inner()),
                Err(error) => return Err(ExporterError::GrpcResponseError(error.to_string())),
            },
            ApiVersion::V1 => match self
                .client_v1()?
                .pool
                .list_pools(rpc::v1::pool::ListPoolOptions {
                    name: None,
                    pooltype: None,
                })
                .await
            {
                Ok(response) => serde_json::to_string_pretty(&response.into_inner()),
                Err(error) => return Err(ExporterError::GrpcResponseError(error.to_string())),
            },
        };

        let json_string = match response {
            Ok(json_string) => json_string,
            Err(error) => {
                error!(error=%error, "Error while deserializing response");
                return Err(ExporterError::DeserializationError(
                    "Error while deserializing response to string".to_string(),
                ));
            }
        };

        let pools: Pools = match serde_json::from_str(json_string.as_str()) {
            Ok(pools) => pools,
            Err(error) => {
                error!(error=%error, "Error while deserializing string to Pools resource");
                return Err(ExporterError::DeserializationError(
                    "Error while deserializing string data to Pools resource".to_string(),
                ));
            }
        };
        Ok(pools)
    }
}
