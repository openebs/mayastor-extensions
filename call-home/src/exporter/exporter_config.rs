use std::time::Duration;
use once_cell::sync::OnceCell;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

static CONFIG: OnceCell<ExporterConfig> = OnceCell::new();

/// Exporter config that can be passed through arguments.
pub struct ExporterConfig {
    /// Network address where the prometheus metrics endpoint will listen (example: 9502).
    metrics_endpoint: SocketAddr,

    /// Polling time to do grpc calls to get data from the server.(Default: 30s).
    polling_time: Duration,
}

impl ExporterConfig {
    /// Initialize exporter configs.
    pub fn initialize() {
        CONFIG.get_or_init(|| Self {
            metrics_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9505),
            polling_time: Duration::from_secs(30),
        });
    }

    /// Get exporter config.
    pub fn get_config() -> &'static ExporterConfig {
        CONFIG.get().expect("Exporter config is not initialized")
    }

    /// Get metrics endpoint.
    pub fn metrics_endpoint(&self) -> &SocketAddr {
        &self.metrics_endpoint
    }

    /// Get polling time.
    pub fn polling_time(&self) -> Duration {
        self.polling_time
    }
}
