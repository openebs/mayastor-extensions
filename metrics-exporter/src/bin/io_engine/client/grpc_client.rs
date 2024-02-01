use crate::{error::ExporterError, get_node_name, get_pod_ip};

use crate::client::{
    nexus_stat::{NexusIoStat, NexusIoStats},
    pool::{PoolInfo, Pools},
    pool_stat::{PoolIoStat, PoolIoStats},
};
use actix_web::http::Uri;
use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::Channel;
use tracing::error;

/// Timeout for gRPC connection.
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
pub(crate) struct GrpcContext {
    endpoint: tonic::transport::Endpoint,
}

impl GrpcContext {
    pub fn new(endpoint: Uri, timeouts: Timeouts) -> Self {
        let endpoint = tonic::transport::Endpoint::from(endpoint)
            .connect_timeout(timeouts.connect())
            .timeout(timeouts.request());
        Self { endpoint }
    }
}

/// The V1 PoolClient.
type PoolClient = rpc::v1::pool::pool_rpc_client::PoolRpcClient<Channel>;
type StatsClient = rpc::v1::stats::StatsRpcClient<Channel>;

/// A wrapper for client for the V1 dataplane interface.
#[derive(Clone, Debug)]
pub(crate) struct MayaClientV1 {
    pub(crate) pool: PoolClient,
    pub(crate) stats: StatsClient,
}

/// Dataplane grpc client.
#[derive(Debug, Clone)]
pub(crate) struct GrpcClient {
    client: Option<MayaClientV1>,
}

/// Number of grpc connect retries without error logging.
const SILENT_RETRIES: i32 = 3;

impl GrpcClient {
    /// Initialize v1 io engine gRPC client.
    pub(crate) async fn new(context: GrpcContext) -> Result<Self, ExporterError> {
        let sleep_duration_sec = 10;
        let mut num_retires = 0;
        loop {
            if let Ok(channel) = context.endpoint.connect().await {
                let pool = PoolClient::new(channel.clone());
                let stats = StatsClient::new(channel.clone());
                return Ok(Self {
                    client: Some(MayaClientV1 { pool, stats }),
                });
            } else {
                if num_retires > SILENT_RETRIES {
                    error!(
                        "Grpc connection timeout, retrying after {}s",
                        sleep_duration_sec
                    );
                }
                num_retires += 1;
            }
            sleep(Duration::from_secs(sleep_duration_sec)).await;
        }
    }

    /// Get the v1 api client.
    pub(crate) fn client_v1(&self) -> Result<MayaClientV1, ExporterError> {
        match self.client.clone() {
            Some(client) => Ok(client),
            None => Err(ExporterError::GrpcClientError(
                "Could not get v1 client".to_string(),
            )),
        }
    }
}

/// Initialize mayastor grpc client.
pub(crate) async fn init_client() -> Result<GrpcClient, ExporterError> {
    let timeout = Timeouts::new(Duration::from_secs(1), Duration::from_secs(5));
    let pod_ip = get_pod_ip()?;
    let _ = get_node_name()?;
    let endpoint = Uri::builder()
        .scheme("https")
        .authority(format!("{pod_ip}:10124"))
        .path_and_query("")
        .build()
        .map_err(|error| ExporterError::InvalidURI(error.to_string()))?;
    let ctx = GrpcContext::new(endpoint, timeout);
    let client = GrpcClient::new(ctx).await?;
    Ok(client)
}

impl GrpcClient {
    /// Gets Capacity statistics of all pool on the io engine.
    /// Maps the response to PoolInfo struct.
    pub(crate) async fn list_pools(&self) -> Result<Pools, ExporterError> {
        let pools = match self
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
        };

        Ok(Pools { pools })
    }

    /// Gets Io Statistics of all pool on the io engine. Maps the response to PoolIoStat struct.
    pub(crate) async fn get_pool_iostat(&self) -> Result<PoolIoStats, ExporterError> {
        let pool_stats = match self
            .client_v1()?
            .stats
            .get_pool_io_stats(rpc::v1::stats::ListStatsOption { name: None })
            .await
        {
            Ok(response) => Ok(response
                .into_inner()
                .stats
                .into_iter()
                .map(PoolIoStat::from)
                .collect::<Vec<_>>()),
            Err(error) => Err(ExporterError::GrpcResponseError(error.to_string())),
        }?;
        Ok(PoolIoStats { pool_stats })
    }

    /// Gets Io Statistics of all nexus on the io engine. Maps the response to NexusIoStat struct.
    pub(crate) async fn get_nexus_iostat(&self) -> Result<NexusIoStats, ExporterError> {
        let nexus_stats = match self
            .client_v1()?
            .stats
            .get_nexus_io_stats(rpc::v1::stats::ListStatsOption { name: None })
            .await
        {
            Ok(response) => Ok(response
                .into_inner()
                .stats
                .into_iter()
                .map(NexusIoStat::from)
                .collect::<Vec<_>>()),
            Err(error) => Err(ExporterError::GrpcResponseError(error.to_string())),
        }?;
        Ok(NexusIoStats { nexus_stats })
    }
}
