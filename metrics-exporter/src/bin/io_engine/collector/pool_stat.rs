use super::init_gauge_vec;
use crate::{cache::Cache, client::pool_stat::PoolIoStat, get_node_name};
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
            init_gauge_vec("bytes_read", "Total bytes read on the pool", &mut descs);
        let pool_num_read_ops = init_gauge_vec(
            "num_read_ops",
            "Number of read operations on the pool",
            &mut descs,
        );
        let pool_bytes_written = init_gauge_vec(
            "bytes_written",
            "Total bytes written on the pool",
            &mut descs,
        );
        let pool_num_write_ops = init_gauge_vec(
            "num_write_ops",
            "Number of write operations on the pool",
            &mut descs,
        );
        let pool_read_latency_us = init_gauge_vec(
            "read_latency_us",
            "Total read latency on the pool in usec",
            &mut descs,
        );
        let pool_write_latency_us = init_gauge_vec(
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
        let c = match Cache::get_cache().lock() {
            Ok(c) => c,
            Err(error) => {
                error!(%error,"Error while getting cache resource");
                return Vec::new();
            }
        };
        let cp = c.deref();
        let mut metric_family = Vec::with_capacity(6 * cp.pool_iostat().pool_stats.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };

        for i in &cp.pool_iostat().pool_stats {
            let p: &PoolIoStat = i;

            let pool_bytes_read = match self
                .pool_bytes_read
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_bytes_read) => pool_bytes_read,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_bytes_read) with label values");
                    return metric_family;
                }
            };
            pool_bytes_read.set(p.bytes_read() as f64);
            let mut x = pool_bytes_read.collect();
            metric_family.extend(x.pop());

            let pool_num_read_ops = match self
                .pool_num_read_ops
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_num_read_ops) => pool_num_read_ops,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_num_read_ops) with label values");
                    return metric_family;
                }
            };
            pool_num_read_ops.set(p.num_read_ops() as f64);
            let mut x = pool_num_read_ops.collect();
            metric_family.extend(x.pop());

            let pool_bytes_written = match self
                .pool_bytes_written
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_bytes_written) => pool_bytes_written,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_bytes_written) with label values");
                    return metric_family;
                }
            };
            pool_bytes_written.set(p.bytes_written() as f64);
            let mut x = pool_bytes_written.collect();
            metric_family.extend(x.pop());

            let pool_num_write_ops = match self
                .pool_num_write_ops
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_num_write_ops) => pool_num_write_ops,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_num_write_ops) with label values");
                    return metric_family;
                }
            };
            pool_num_write_ops.set(p.num_write_ops() as f64);
            let mut x = pool_num_write_ops.collect();
            metric_family.extend(x.pop());

            let pool_read_latency_us = match self
                .pool_read_latency_us
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_read_latency_us) => pool_read_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_read_latency) with label values");
                    return metric_family;
                }
            };
            pool_read_latency_us.set(p.read_latency() as f64);
            let mut x = pool_read_latency_us.collect();
            metric_family.extend(x.pop());

            let pool_write_latency_us = match self
                .pool_write_latency_us
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_write_latency_us) => pool_write_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_write_latency) with label values");
                    return metric_family;
                }
            };
            pool_write_latency_us.set(p.write_latency() as f64);
            let mut x = pool_write_latency_us.collect();
            metric_family.extend(x.pop());
        }
        metric_family
    }
}
