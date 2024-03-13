use super::init_replica_gauge_vec;
use crate::{cache::Cache, get_node_name};
use prometheus::{
    core::{Collector, Desc},
    GaugeVec,
};
use std::{fmt::Debug, ops::Deref};
use tracing::error;

/// Collects Replica IoStat metrics from cache.
#[derive(Clone, Debug)]
pub(crate) struct ReplicaIoStatsCollector {
    replica_bytes_read: GaugeVec,
    replica_num_read_ops: GaugeVec,
    replica_bytes_written: GaugeVec,
    replica_num_write_ops: GaugeVec,
    replica_read_latency_us: GaugeVec,
    replica_write_latency_us: GaugeVec,
    descs: Vec<Desc>,
}

impl Default for ReplicaIoStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}
/// Initialize all the metrics to be defined for replicas iostat collector.
impl ReplicaIoStatsCollector {
    /// Initialize all the metrics to be defined for replicas iostat collector.
    pub fn new() -> Self {
        let mut descs = Vec::new();

        let replica_bytes_read =
            init_replica_gauge_vec("bytes_read", "Total bytes read on the replica", &mut descs);
        let replica_num_read_ops = init_replica_gauge_vec(
            "num_read_ops",
            "Number of read operations on the replica",
            &mut descs,
        );
        let replica_bytes_written = init_replica_gauge_vec(
            "bytes_written",
            "Total bytes written on the replica",
            &mut descs,
        );
        let replica_num_write_ops = init_replica_gauge_vec(
            "num_write_ops",
            "Number of write operations on the replica",
            &mut descs,
        );
        let replica_read_latency_us = init_replica_gauge_vec(
            "read_latency_us",
            "Total read latency on the replica in usec",
            &mut descs,
        );
        let replica_write_latency_us = init_replica_gauge_vec(
            "write_latency_us",
            "Total write latency on the replica in usec",
            &mut descs,
        );

        Self {
            replica_bytes_read,
            replica_num_read_ops,
            replica_bytes_written,
            replica_num_write_ops,
            replica_read_latency_us,
            replica_write_latency_us,
            descs,
        }
    }
}

impl Collector for ReplicaIoStatsCollector {
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
            Vec::with_capacity(6 * cache_deref.replica_iostat().replica_stats.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };

        for replica_stat in &cache_deref.replica_iostat().replica_stats {
            let pv_name = format!("pvc-{}", replica_stat.entity_id());
            let replica_bytes_read = match self.replica_bytes_read.get_metric_with_label_values(&[
                node_name.as_str(),
                replica_stat.name().as_str(),
                pv_name.as_str(),
            ]) {
                Ok(replica_bytes_read) => replica_bytes_read,
                Err(error) => {
                    error!(%error, "Error while creating replica_bytes_read counter with label values");
                    return metric_family;
                }
            };
            replica_bytes_read.set(replica_stat.bytes_read() as f64);
            let mut metric_vec = replica_bytes_read.collect();
            metric_family.extend(metric_vec.pop());

            let replica_num_read_ops = match self.replica_num_read_ops.get_metric_with_label_values(
                &[
                    node_name.as_str(),
                    replica_stat.name().as_str(),
                    pv_name.as_str(),
                ],
            ) {
                Ok(replica_num_read_ops) => replica_num_read_ops,
                Err(error) => {
                    error!(%error, "Error while creating replica_num_read_ops counter with label values");
                    return metric_family;
                }
            };
            replica_num_read_ops.set(replica_stat.num_read_ops() as f64);
            let mut metric_vec = replica_num_read_ops.collect();
            metric_family.extend(metric_vec.pop());

            let replica_bytes_written = match self
                .replica_bytes_written
                .get_metric_with_label_values(&[
                    node_name.as_str(),
                    replica_stat.name().as_str(),
                    pv_name.as_str(),
                ]) {
                Ok(replica_bytes_written) => replica_bytes_written,
                Err(error) => {
                    error!(%error, "Error while creating replica_bytes_written counter with label values");
                    return metric_family;
                }
            };
            replica_bytes_written.set(replica_stat.bytes_written() as f64);
            let mut metric_vec = replica_bytes_written.collect();
            metric_family.extend(metric_vec.pop());

            let replica_num_write_ops = match self
                .replica_num_write_ops
                .get_metric_with_label_values(&[
                    node_name.as_str(),
                    replica_stat.name().as_str(),
                    pv_name.as_str(),
                ]) {
                Ok(replica_num_write_ops) => replica_num_write_ops,
                Err(error) => {
                    error!(%error, "Error while creating replica_num_write_ops counter with label values");
                    return metric_family;
                }
            };
            replica_num_write_ops.set(replica_stat.num_write_ops() as f64);
            let mut metric_vec = replica_num_write_ops.collect();
            metric_family.extend(metric_vec.pop());

            let replica_read_latency_us = match self
                .replica_read_latency_us
                .get_metric_with_label_values(&[
                    node_name.as_str(),
                    replica_stat.name().as_str(),
                    pv_name.as_str(),
                ]) {
                Ok(replica_read_latency_us) => replica_read_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating replica_read_latency counter with label values");
                    return metric_family;
                }
            };
            replica_read_latency_us.set(replica_stat.read_latency_us() as f64);
            let mut metric_vec = replica_read_latency_us.collect();
            metric_family.extend(metric_vec.pop());

            let replica_write_latency_us = match self
                .replica_write_latency_us
                .get_metric_with_label_values(&[
                    node_name.as_str(),
                    replica_stat.name().as_str(),
                    pv_name.as_str(),
                ]) {
                Ok(replica_write_latency_us) => replica_write_latency_us,
                Err(error) => {
                    error!(%error, "Error while creating replica_write_latency counter with label values");
                    return metric_family;
                }
            };
            replica_write_latency_us.set(replica_stat.write_latency_us() as f64);
            let mut metric_vec = replica_write_latency_us.collect();
            metric_family.extend(metric_vec.pop());
        }
        metric_family
    }
}
