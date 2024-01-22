use crate::{cache::Cache, client::pool::PoolInfo, collector::init_gauge_vec, get_node_name};
use prometheus::{
    core::{Collector, Desc},
    GaugeVec,
};
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use tracing::error;

/// Collects Pool capacity metrics from cache.
#[derive(Clone, Debug)]
pub(crate) struct PoolCapacityCollector {
    pool_total_size: GaugeVec,
    pool_used_size: GaugeVec,
    pool_committed_size: GaugeVec,
    descs: Vec<Desc>,
}

impl Default for PoolCapacityCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolCapacityCollector {
    /// Initialize all the metrics to be defined for pools capacity collector.
    pub fn new() -> Self {
        let mut descs = Vec::new();
        let pool_total_size = init_gauge_vec(
            "total_size_bytes",
            "Total size of the pool in bytes",
            &mut descs,
        );
        let pool_used_size = init_gauge_vec(
            "total_used_bytes",
            "Used size of the pool in bytes",
            &mut descs,
        );
        let pool_committed_size = init_gauge_vec(
            "committed_size_bytes",
            "Committed size of the pool in bytes",
            &mut descs,
        );

        Self {
            pool_total_size,
            pool_used_size,
            pool_committed_size,
            descs,
        }
    }
}

impl Collector for PoolCapacityCollector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
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
        let mut metric_family = Vec::with_capacity(3 * cp.pool().pools.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };

        for i in &cp.pool().pools {
            let p: &PoolInfo = i;

            let pool_total_size = match self
                .pool_total_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_total_size) => pool_total_size,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_total_size) with label values");
                    return metric_family;
                }
            };
            pool_total_size.set(p.capacity() as f64);
            let mut x = pool_total_size.collect();
            metric_family.extend(x.pop());

            let pool_used_size = match self
                .pool_used_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_used_size) => pool_used_size,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_used_size) with label values");
                    return metric_family;
                }
            };
            pool_used_size.set(p.used() as f64);
            let mut x = pool_used_size.collect();
            metric_family.extend(x.pop());

            let pool_committed_size = match self
                .pool_committed_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_committed_size) => pool_committed_size,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_committed_size) with label values");
                    return metric_family;
                }
            };
            pool_committed_size.set(p.committed() as f64);
            let mut x = pool_committed_size.collect();
            metric_family.extend(x.pop());
        }
        metric_family
    }
}

/// Collects pool status info from cache.
#[derive(Clone, Debug)]
pub(crate) struct PoolStatusCollector {
    pool_status: GaugeVec,
    descs: Vec<Desc>,
}

impl Default for PoolStatusCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolStatusCollector {
    /// Initialize all the metrics to be defined for pools status collector.
    pub fn new() -> Self {
        let mut descs = Vec::new();
        let pool_status = init_gauge_vec("status", "Status of the pool", &mut descs);
        Self { pool_status, descs }
    }
}

impl Collector for PoolStatusCollector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        self.descs.iter().collect()
    }
    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut c = match Cache::get_cache().lock() {
            Ok(c) => c,
            Err(error) => {
                error!(%error,"Error while getting cache resource");
                return Vec::new();
            }
        };
        let cp = c.deref_mut();
        let mut metric_family = Vec::with_capacity(3 * cp.pool_mut().pools.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };
        for i in &cp.pool_mut().pools {
            let p: &PoolInfo = i;
            let pool_status = match self
                .pool_status
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_status) => pool_status,
                Err(error) => {
                    error!(%error, "Error while creating metrics(pool_status) with label values");
                    return metric_family;
                }
            };
            pool_status.set(p.state() as f64);
            let mut x = pool_status.collect();
            metric_family.extend(x.pop());
        }
        metric_family
    }
}
