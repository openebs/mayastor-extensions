use super::init_diskpool_gauge_vec;
use crate::{cache::Cache, get_node_name};
use prometheus::{
    core::{Collector, Desc},
    GaugeVec,
};
use std::{fmt::Debug, ops::Deref};
use tracing::error;

/// Collects Pool IoStat metrics from cache.
#[derive(Clone, Debug)]
pub(crate) struct PoolIoStatsCollector {
    pool_bytes_read: GaugeVec,
    pool_num_read_ops: GaugeVec,
    pool_bytes_written: GaugeVec,
    pool_num_write_ops: GaugeVec,
    pool_read_latency_us: GaugeVec,
    pool_write_latency_us: GaugeVec,
    descs: Vec<Desc>,
}

impl Default for PoolIoStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}
/// Initialize all the metrics to be defined for pools iostat collector.
impl PoolIoStatsCollector {
    /// Initialize all the metrics to be defined for pools iostat collector.
    pub fn new() -> Self {
        let mut descs = Vec::new();

        let pool_bytes_read =
            init_diskpool_gauge_vec("bytes_read", "Total bytes read on the pool", &mut descs);
        let pool_num_read_ops = init_diskpool_gauge_vec(
            "num_read_ops",
            "Number of read operations on the pool",
            &mut descs,
        );
        let pool_bytes_written = init_diskpool_gauge_vec(
            "bytes_written",
            "Total bytes written on the pool",
            &mut descs,
        );
        let pool_num_write_ops = init_diskpool_gauge_vec(
            "num_write_ops",
            "Number of write operations on the pool",
            &mut descs,
        );
        let pool_read_latency_us = init_diskpool_gauge_vec(
            "read_latency_us",
            "Total read latency on the pool in usec",
            &mut descs,
        );
        let pool_write_latency_us = init_diskpool_gauge_vec(
            "write_latency_us",
            "Total write latency on the pool in usec",
            &mut descs,
        );

        Self {
            pool_bytes_read,
            pool_num_read_ops,
            pool_bytes_written,
            pool_num_write_ops,
            pool_read_latency_us,
            pool_write_latency_us,
            descs,
        }
    }
}

impl Collector for PoolIoStatsCollector {
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
            Vec::with_capacity(6 * cache_deref.pool_iostat().pool_stats.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };

        for pool_stat in &cache_deref.pool_iostat().pool_stats {
            let pool_bytes_read = match self.pool_bytes_read.get_metric_with_label_values(&[
                node_name.clone().as_str(),
                pool_stat.name().as_str(),
            ]) {
                Ok(pool_bytes_read) => pool_bytes_read,
                Err(error) => {
                    error!(%error, "Error while creating pool_bytes_read counter with label values");
                    return metric_family;
                }
            };
            pool_bytes_read.set(pool_stat.bytes_read() as f64);
            let mut metric_vec = pool_bytes_read.collect();
            metric_family.extend(metric_vec.pop());

            let pool_num_read_ops = match self.pool_num_read_ops.get_metric_with_label_values(&[
                node_name.clone().as_str(),
                pool_stat.name().as_str(),
            ]) {
                Ok(pool_num_read_ops) => pool_num_read_ops,
                Err(error) => {
                    error!(%error, "Error while creating pool_num_read_ops counter with label values");
                    return metric_family;
                }
            };
            pool_num_read_ops.set(pool_stat.num_read_ops() as f64);
            let mut metric_vec = pool_num_read_ops.collect();
            metric_family.extend(metric_vec.pop());

            let pool_bytes_written = match self.pool_bytes_written.get_metric_with_label_values(&[
                node_name.clone().as_str(),
                pool_stat.name().as_str(),
            ]) {
                Ok(pool_bytes_written) => pool_bytes_written,
                Err(error) => {
                    error!(%error, "Error while creating pool_bytes_written counter with label values");
                    return metric_family;
                }
            };
            pool_bytes_written.set(pool_stat.bytes_written() as f64);
            let mut metric_vec = pool_bytes_written.collect();
            metric_family.extend(metric_vec.pop());

            let pool_num_write_ops = match self.pool_num_write_ops.get_metric_with_label_values(&[
                node_name.clone().as_str(),
                pool_stat.name().as_str(),
            ]) {
                Ok(pool_num_write_ops) => pool_num_write_ops,
                Err(error) => {
                    error!(%error, "Error while creating pool_num_write_ops counter with label values");
                    return metric_family;
                }
            };
            pool_num_write_ops.set(pool_stat.num_write_ops() as f64);
            let mut metric_vec = pool_num_write_ops.collect();
            metric_family.extend(metric_vec.pop());

            let pool_read_latency_us = match self.pool_read_latency_us.get_metric_with_label_values(
                &[node_name.clone().as_str(), pool_stat.name().as_str()],
            ) {
                Ok(pool_read_latency_us) => pool_read_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating pool_read_latency counter with label values");
                    return metric_family;
                }
            };
            pool_read_latency_us.set(pool_stat.read_latency_us() as f64);
            let mut metric_vec = pool_read_latency_us.collect();
            metric_family.extend(metric_vec.pop());

            let pool_write_latency_us = match self
                .pool_write_latency_us
                .get_metric_with_label_values(&[
                    node_name.clone().as_str(),
                    pool_stat.name().as_str(),
                ]) {
                Ok(pool_write_latency_us) => pool_write_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating pool_write_latency counter with label values");
                    return metric_family;
                }
            };
            pool_write_latency_us.set(pool_stat.write_latency_us() as f64);
            let mut metric_vec = pool_write_latency_us.collect();
            metric_family.extend(metric_vec.pop());
        }
        metric_family
    }
}
