use crate::{error::ExporterError, get_node_name, get_pod_ip};

use crate::client::{
    nexus_stat::{NexusIoStat, NexusIoStats},
    pool::{PoolInfo, Pools},
    pool_stat::{PoolIoStat, PoolIoStats},
    replica_stat::{ReplicaIoStat, ReplicaIoStats},
};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tokio::time::sleep;
use tonic::transport::{Channel, Uri};
use tracing::error;

/// The TLS policy for the gRPC connection to the local io-engine.
///
/// The io-engine advertises its transport via the control-plane, but this
/// sidecar always dials the io-engine on the same pod, so the mode is taken
/// from this exporter's own flags rather than from registration.
#[derive(Debug, Clone)]
pub(crate) enum GrpcTls {
    /// Plaintext, no TLS.
    None,
    /// Auto-TLS: encryption without certificate verification, matching the
    /// io-engine's `--grpc-auto-tls`.
    Auto,
    /// File-backed TLS, verifying the io-engine against the configured CA.
    Files(grpc_tls::TlsConfig),
}

impl GrpcTls {
    /// Build the policy from the exporter's gRPC TLS CLI arguments.
    ///
    /// `--grpc-auto-tls` takes precedence; otherwise file-backed TLS is used
    /// when any certificate file is given, and plaintext when none is.
    pub(crate) fn from_args(
        auto_tls: bool,
        ca_file: Option<PathBuf>,
        cert_file: Option<PathBuf>,
        key_file: Option<PathBuf>,
    ) -> Result<Self, ExporterError> {
        if auto_tls {
            return Ok(Self::Auto);
        }
        let tls = grpc_tls::TlsConfig::new(ca_file, cert_file, key_file)
            .map_err(|error| ExporterError::GrpcClientError(error.to_string()))?;
        Ok(if tls.enabled() {
            Self::Files(tls)
        } else {
            Self::None
        })
    }
}

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
    tls: GrpcTls,
}

impl GrpcContext {
    pub fn new(endpoint: Uri, timeouts: Timeouts, tls: GrpcTls) -> Self {
        let endpoint = tonic::transport::Endpoint::from(endpoint)
            .connect_timeout(timeouts.connect())
            .timeout(timeouts.request());
        Self { endpoint, tls }
    }

    /// Establish a channel to the io-engine using the configured TLS policy.
    ///
    /// For both TLS modes the endpoint keeps its `http` scheme: the handshake
    /// is driven by a custom connector, so tonic's own TLS logic is bypassed.
    async fn connect(&self) -> Result<Channel, ExporterError> {
        match &self.tls {
            GrpcTls::None => self
                .endpoint
                .connect()
                .await
                .map_err(|error| ExporterError::GrpcClientError(error.to_string())),
            GrpcTls::Auto => grpc_tls::auto_tls_connect(&self.endpoint)
                .await
                .map_err(|error| ExporterError::GrpcClientError(error.to_string())),
            GrpcTls::Files(tls) => grpc_tls::file_tls_connect(&self.endpoint, tls)
                .await
                .map_err(|error| ExporterError::GrpcClientError(error.to_string())),
        }
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
const SILENT_RETRIES: u32 = 3;

impl GrpcClient {
    /// Initialize v1 io engine gRPC client.
    pub(crate) async fn new(context: GrpcContext) -> Result<Self, ExporterError> {
        let backoff = std::time::Duration::from_secs(10);
        let mut num_retries = 0;
        loop {
            let result = context.connect().await;
            let error = match result {
                Ok(channel) => {
                    let pool = PoolClient::new(channel.clone());
                    let stats = StatsClient::new(channel.clone());
                    return Ok(Self {
                        client: Some(MayaClientV1 { pool, stats }),
                    });
                }
                Err(error) => error,
            };
            if num_retries > SILENT_RETRIES {
                error!(?error, "Grpc connection failed, retrying after {backoff:?}");
            }
            num_retries += 1;
            sleep(backoff).await;
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
pub(crate) async fn init_client(grpc_port: u16, tls: GrpcTls) -> Result<GrpcClient, ExporterError> {
    let timeout = Timeouts::new(Duration::from_secs(1), Duration::from_secs(5));
    let pod_ip = get_pod_ip()?;
    let _ = get_node_name()?;

    // The endpoint keeps the `http` scheme even under TLS: the TLS handshake is
    // performed by a custom connector, so tonic must not apply its own.
    let endpoint = Uri::builder()
        .scheme("http")
        .authority(SocketAddr::new(pod_ip, grpc_port).to_string())
        .path_and_query("")
        .build()
        .map_err(|error| ExporterError::InvalidURI(error.to_string()))?;
    let ctx = GrpcContext::new(endpoint, timeout, tls);
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

    /// Gets Io Statistics of all replica on the io engine. Maps the response to ReplicaIoStat
    /// struct.
    pub(crate) async fn get_replica_iostat(&self) -> Result<ReplicaIoStats, ExporterError> {
        let replica_stats = match self
            .client_v1()?
            .stats
            .get_replica_io_stats(rpc::v1::stats::ListStatsOption { name: None })
            .await
        {
            Ok(response) => Ok(response
                .into_inner()
                .stats
                .into_iter()
                .filter_map(|replica| ReplicaIoStat::try_from(replica).ok())
                .collect::<Vec<_>>()),
            Err(error) => Err(ExporterError::GrpcResponseError(error.to_string())),
        }?;
        Ok(ReplicaIoStats { replica_stats })
    }

    /// Gets SMART data of all the pools on the io engine.
    pub(crate) async fn list_pools_smart(
        &self,
    ) -> Result<Vec<rpc::v1::pool::PoolSmart>, ExporterError> {
        match self
            .client_v1()?
            .pool
            .list_pools_smart(rpc::v1::pool::ListPoolsSmartOptions::default())
            .await
        {
            Ok(response) => Ok(response.into_inner().pools),
            Err(error) => Err(ExporterError::GrpcResponseError(error.to_string())),
        }
    }
}
