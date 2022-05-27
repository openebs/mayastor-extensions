use serde::{Deserialize, Serialize};

use crate::{client::grpc_client::GrpcClient, error::ExporterError};

/// Pool resource
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Pool {
    name: String,
    disks: Vec<String>,
    used: u64,
    capacity: u64,
    state: u64,
}

impl Pool {
    /// get name of the pool
    pub fn name(&self) -> &String {
        &self.name
    }

    /// get disks
    pub fn disks(&self) -> &Vec<String> {
        &self.disks
    }

    /// get used capacity of the pool
    pub fn used(&self) -> u64 {
        self.used
    }

    /// get total capacity of the pool
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// get state of the pool
    pub fn state(&self) -> u64 {
        self.state
    }
}

/// Pools resource
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Pools {
    pub(crate) pools: Vec<Pool>,
}

/// Pool operations i.e wrapper over rpc calls to get pools data
#[tonic::async_trait]
pub trait PoolOperations: Send + Sync + Sized {
    async fn list_pools(client: GrpcClient) -> Result<Pools, ExporterError>;
}

#[tonic::async_trait]
impl PoolOperations for GrpcClient {
    // wrapper over list_pools rpc call
    async fn list_pools(mut client: GrpcClient) -> Result<Pools, ExporterError> {
        let response = match client
            .clients_mut()
            .mayastor_client_mut()
            .list_pools(rpc::mayastor::Null {})
            .await
        {
            Ok(response) => response,
            Err(error) => return Err(ExporterError::GrpcResponseError(error.to_string())),
        };

        let x = match serde_json::to_string_pretty(&response.get_ref()) {
            Ok(x) => x,
            Err(err) => {
                println!("Error while deserializing response: {}", err);
                return Err(ExporterError::DeserializationError(
                    "Error while deserializing response to string".to_string(),
                ));
            }
        };

        let p: Pools = match serde_json::from_str(&*x) {
            Ok(p) => p,
            Err(err) => {
                println!("Error while deserializing string to Pools resource{}", err);
                return Err(ExporterError::DeserializationError(
                    "Error while deserializing string data to Pools resource".to_string(),
                ));
            }
        };
        Ok(p)
    }
}
