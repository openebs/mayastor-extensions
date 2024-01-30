use super::init_volume_gauge_vec;
use crate::{cache::Cache, get_node_name};
use prometheus::{
    core::{Collector, Desc},
    GaugeVec,
};
use std::{fmt::Debug, ops::Deref};
use tracing::error;

/// Collects Nexus IoStat metrics from cache.
#[derive(Clone, Debug)]
pub(crate) struct NexusIoStatsCollector {
    nexus_bytes_read: GaugeVec,
    nexus_num_read_ops: GaugeVec,
    nexus_bytes_written: GaugeVec,
    nexus_num_write_ops: GaugeVec,
    nexus_read_latency_us: GaugeVec,
    nexus_write_latency_us: GaugeVec,
    descs: Vec<Desc>,
}

impl Default for NexusIoStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl NexusIoStatsCollector {
    /// Initialize all the metrics to be defined for nexus iostat collector.
    pub fn new() -> Self {
        let mut descs = Vec::new();

        let nexus_bytes_read =
            init_volume_gauge_vec("bytes_read", "Total bytes read from the volume", &mut descs);
        let nexus_num_read_ops = init_volume_gauge_vec(
            "num_read_ops",
            "Number of read operations on the volume",
            &mut descs,
        );
        let nexus_bytes_written = init_volume_gauge_vec(
            "bytes_written",
            "Total bytes written on the volume",
            &mut descs,
        );
        let nexus_num_write_ops = init_volume_gauge_vec(
            "num_write_ops",
            "Number of write operations on the volume",
            &mut descs,
        );
        let nexus_read_latency_us = init_volume_gauge_vec(
            "read_latency_us",
            "Total read latency on the volume in usec",
            &mut descs,
        );
        let nexus_write_latency_us = init_volume_gauge_vec(
            "write_latency_us",
            "Total write latency on the volume in usec",
            &mut descs,
        );

        Self {
            nexus_bytes_read,
            nexus_num_read_ops,
            nexus_bytes_written,
            nexus_num_write_ops,
            nexus_read_latency_us,
            nexus_write_latency_us,
            descs,
        }
    }
}

impl Collector for NexusIoStatsCollector {
    fn desc(&self) -> Vec<&Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let cache = match Cache::get_cache().lock() {
            Ok(cache) => cache,
            Err(error) => {
                error!(%error,"Error while getting cache resource");
                return Vec::new();
            }
        };
        let cache_deref = cache.deref();
        let mut metric_family =
            Vec::with_capacity(6 * cache_deref.nexus_iostat().nexus_stats.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };

        for nexus_stat in &cache_deref.nexus_iostat().nexus_stats {
            let pv_name = "pvc-".to_string() + nexus_stat.name();
            let nexus_bytes_read = match self
                .nexus_bytes_read
                .get_metric_with_label_values(&[node_name.clone().as_str(), pv_name.as_str()])
            {
                Ok(nexus_bytes_read) => nexus_bytes_read,
                Err(error) => {
                    error!(%error, "Error while creating nexus_bytes_read counter with label values");
                    return metric_family;
                }
            };
            nexus_bytes_read.set(nexus_stat.bytes_read() as f64);
            let mut metric_vec = nexus_bytes_read.collect();
            metric_family.extend(metric_vec.pop());

            let nexus_num_read_ops = match self
                .nexus_num_read_ops
                .get_metric_with_label_values(&[node_name.clone().as_str(), pv_name.as_str()])
            {
                Ok(nexus_num_read_ops) => nexus_num_read_ops,
                Err(error) => {
                    error!(%error, "Error while creating nexus_num_read_ops counter with label values");
                    return metric_family;
                }
            };
            nexus_num_read_ops.set(nexus_stat.num_read_ops() as f64);
            let mut metric_vec = nexus_num_read_ops.collect();
            metric_family.extend(metric_vec.pop());

            let nexus_bytes_written = match self
                .nexus_bytes_written
                .get_metric_with_label_values(&[node_name.clone().as_str(), pv_name.as_str()])
            {
                Ok(nexus_bytes_written) => nexus_bytes_written,
                Err(error) => {
                    error!(%error, "Error while creating nexus_bytes_written counter with label values");
                    return metric_family;
                }
            };
            nexus_bytes_written.set(nexus_stat.bytes_written() as f64);
            let mut metric_vec = nexus_bytes_written.collect();
            metric_family.extend(metric_vec.pop());

            let nexus_num_write_ops = match self
                .nexus_num_write_ops
                .get_metric_with_label_values(&[node_name.clone().as_str(), pv_name.as_str()])
            {
                Ok(nexus_num_write_ops) => nexus_num_write_ops,
                Err(error) => {
                    error!(%error, "Error while creating nexus_num_write_ops counter with label values");
                    return metric_family;
                }
            };
            nexus_num_write_ops.set(nexus_stat.num_write_ops() as f64);
            let mut metric_vec = nexus_num_write_ops.collect();
            metric_family.extend(metric_vec.pop());

            let nexus_read_latency_us = match self
                .nexus_read_latency_us
                .get_metric_with_label_values(&[node_name.clone().as_str(), pv_name.as_str()])
            {
                Ok(nexus_read_latency_us) => nexus_read_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating nexus_read_latency counter with label values");
                    return metric_family;
                }
            };
            nexus_read_latency_us.set(nexus_stat.read_latency_us() as f64);
            let mut metric_vec = nexus_read_latency_us.collect();
            metric_family.extend(metric_vec.pop());

            let nexus_write_latency_us = match self
                .nexus_write_latency_us
                .get_metric_with_label_values(&[node_name.clone().as_str(), pv_name.as_str()])
            {
                Ok(nexus_write_latency_us) => nexus_write_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating nexus_write_latency counter with label values");
                    return metric_family;
                }
            };
            nexus_write_latency_us.set(nexus_stat.write_latency_us() as f64);
            let mut metric_vec = nexus_write_latency_us.collect();
            metric_family.extend(metric_vec.pop());
        }
        metric_family
    }
}
