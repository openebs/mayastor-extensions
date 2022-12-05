use std::time::Duration;

use crate::pool::{client::ApiVersion, error::ExporterError};
use actix_web::http::Uri;
use rpc::io_engine::IoEngineClientV0;
use tokio::time::sleep;
use tonic::transport::Channel;
use tracing::error;

/// Timeout for gRPC.
#[derive(Debug, Clone)]
pub struct Timeouts {
    connect: Duration,
    request: Duration,
}

impl Timeouts {
    /// Return a new `Self` with the connect and request timeouts.
    pub fn new(connect: Duration, request: Duration) -> Self {
        Self { connect, request }
    }
    /// Timeout to establish connection to the node.
    pub fn connect(&self) -> Duration {
        self.connect
    }
    /// Timeout for the request itself.
    pub fn request(&self) -> Duration {
        self.request
    }
}

/// Context for Grpc client.
#[derive(Debug, Clone)]
pub struct GrpcContext {
    endpoint: tonic::transport::Endpoint,
    timeouts: Timeouts,
    api_version: ApiVersion,
}

impl GrpcContext {
    /// initialize context
    pub fn new(endpoint: Uri, timeouts: Timeouts, api_version: ApiVersion) -> Self {
        let endpoint = tonic::transport::Endpoint::from(endpoint)
            .connect_timeout(timeouts.connect())
            .timeout(timeouts.request());
        Self {
            endpoint,
            timeouts,
            api_version,
        }
    }
}
/// The V0 Mayastor client;
pub type MayaClientV0 = IoEngineClientV0<Channel>;

/// The V1 PoolClient.
pub type PoolClient = rpc::v1::pool::pool_rpc_client::PoolRpcClient<Channel>;

/// A wrapper for client for the V1 dataplane interface.
#[derive(Clone, Debug)]
pub struct MayaClientV1 {
    pub pool: PoolClient,
}

/// Grpc client
#[derive(Debug, Clone)]
pub struct GrpcClient {
    ctx: GrpcContext,
    v0_client: Option<MayaClientV0>,
    v1_client: Option<MayaClientV1>,
}

impl GrpcClient {
    /// Initialize gRPC client.
    pub async fn new(context: GrpcContext) -> Result<Self, ExporterError> {
        let sleep_duration_sec = 10;
        loop {
            match context.api_version {
                ApiVersion::V0 => {
                    match tokio::time::timeout(
                        context.timeouts.connect(),
                        MayaClientV0::connect(context.endpoint.clone()),
                    )
                    .await
                    {
                        Err(error) => {
                            error!(error=%error, "Grpc connection timeout, retrying after {}s",sleep_duration_sec);
                        }
                        Ok(result) => match result {
                            Ok(v0_client) => {
                                return Ok(Self {
                                    ctx: context.clone(),
                                    v0_client: Some(v0_client),
                                    v1_client: None,
                                })
                            }
                            Err(error) => {
                                error!(error=%error, "Grpc client connection error, retrying after {}s",sleep_duration_sec);
                            }
                        },
                    }
                }
                ApiVersion::V1 => {
                    match tokio::time::timeout(
                        context.timeouts.connect(),
                        PoolClient::connect(context.endpoint.clone()),
                    )
                    .await
                    {
                        Err(error) => {
                            error!(error=%error, "Grpc connection timeout, retrying after {}s",sleep_duration_sec);
                        }
                        Ok(result) => match result {
                            Ok(pool) => {
                                return Ok(Self {
                                    ctx: context.clone(),
                                    v0_client: None,
                                    v1_client: Some(MayaClientV1 { pool }),
                                })
                            }
                            Err(error) => {
                                error!(error=%error, "Grpc client connection error, retrying after {}s",sleep_duration_sec);
                            }
                        },
                    }
                }
            }
            sleep(Duration::from_secs(sleep_duration_sec)).await;
        }
    }

    /// Get the v0 api client.
    pub fn client_v0(&self) -> Result<MayaClientV0, ExporterError> {
        match self.v0_client.clone() {
            Some(client) => Ok(client),
            None => Err(ExporterError::GrpcClientError(
                "Could not get v0 client".to_string(),
            )),
        }
    }

    /// Get the v1 api client.
    pub fn client_v1(&self) -> Result<MayaClientV1, ExporterError> {
        match self.v1_client.clone() {
            Some(client) => Ok(client),
            None => Err(ExporterError::GrpcClientError(
                "Could not get v1 client".to_string(),
            )),
        }
    }

    /// Get the api version.
    pub fn api_version(&self) -> ApiVersion {
        self.ctx.api_version.clone()
    }
}
