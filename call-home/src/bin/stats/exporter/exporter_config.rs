use once_cell::sync::OnceCell;
use std::net::SocketAddr;
static CONFIG: OnceCell<ExporterConfig> = OnceCell::new();

/// Exporter config that can be passed through arguments.
pub struct ExporterConfig {
    /// Network address where the prometheus metrics endpoint will listen (example: 9090).
    metrics_endpoint: SocketAddr,
}

impl ExporterConfig {
    /// Initialize exporter configs.
    pub fn initialize(addr: SocketAddr) {
        CONFIG.get_or_init(|| Self {
            metrics_endpoint: addr,
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
}
