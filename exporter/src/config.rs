use std::{net::SocketAddr, time::Duration};

use clap::ArgMatches;
use once_cell::sync::OnceCell;

static CONFIG: OnceCell<ExporterConfig> = OnceCell::new();

/// Exporter config that can be passed through arguments
pub struct ExporterConfig {
    /// Network address where the prometheus metrics endpoint will listen (example: 9502)
    metrics_endpoint: SocketAddr,

    /// polling time to do grpc calls to get data from the server.(Default: 30s)
    polling_time: Duration,
}

impl ExporterConfig {
    /// Initialize exporter configs
    pub fn initialize(args: &ArgMatches) {
        let metrics_endpoint = args
            .value_of("metrics-endpoint")
            .expect("metrics listen address must be specified to enable exporter");

        let addr: SocketAddr = format!("0.0.0.0:{}", metrics_endpoint)
            .parse()
            .expect("Error while parsing address");

        let polling_time = args
            .value_of("polling-time")
            .expect("Polling time for gRPC calls to get data")
            .parse::<humantime::Duration>()
            .expect("Invalid polling time value");

        CONFIG.get_or_init(|| Self {
            metrics_endpoint: addr,
            polling_time: polling_time.into(),
        });
    }

    /// Get exporter config
    pub fn get_config() -> &'static ExporterConfig {
        CONFIG.get().expect("Exporter config is not initialized")
    }

    /// Get metrics endpoint
    pub fn metrics_endpoint(&self) -> &SocketAddr {
        &self.metrics_endpoint
    }

    /// Get polling time
    pub fn polling_time(&self) -> Duration {
        self.polling_time
    }
}
