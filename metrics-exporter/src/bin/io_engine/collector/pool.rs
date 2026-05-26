use crate::{
    cache::Cache,
    client::pool::Pools,
    collector::{init_diskpool_alert_reason_gauge_vec, init_diskpool_gauge_vec},
    get_node_name,
};
use prometheus::{
    core::{Collector, Desc},
    GaugeVec,
};
use rpc::v1::pb::PoolAlert;
use std::{fmt::Debug, ops::Deref};
use tracing::error;

/// Collects Pool capacity metrics from cache.
#[derive(Clone, Debug)]
pub(crate) struct PoolCapacityCollector {
    cache: Pools,
    pool_total_size: GaugeVec,
    pool_used_size: GaugeVec,
    pool_committed_size: GaugeVec,
    pool_disk_capacity: GaugeVec,
    pool_max_expandable_size: GaugeVec,
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
        let pool_total_size = init_diskpool_gauge_vec(
            "total_size_bytes",
            "Total size of the pool in bytes",
            &mut descs,
        );
        let pool_used_size = init_diskpool_gauge_vec(
            "used_size_bytes",
            "Used size of the pool in bytes",
            &mut descs,
        );
        let pool_committed_size = init_diskpool_gauge_vec(
            "committed_size_bytes",
            "Committed size of the pool in bytes",
            &mut descs,
        );
        let pool_disk_capacity = init_diskpool_gauge_vec(
            "disk_capacity_bytes",
            "Capacity of the Pool's underlying device",
            &mut descs,
        );
        let pool_max_expandable_size = init_diskpool_gauge_vec(
            "max_expandable_size",
            "Maximum capacity upto which this pool can be expanded, in bytes",
            &mut descs,
        );
        let cache = match Cache::get_cache().lock() {
            Ok(cache) => cache,
            Err(error) => {
                error!(%error,"Error while getting cache resource");
                panic!("panic");
            }
        };
        let pools = cache.deref().pool();

        Self {
            cache: pools.clone(),
            pool_total_size,
            pool_used_size,
            pool_committed_size,
            pool_disk_capacity,
            pool_max_expandable_size,
            descs,
        }
    }
}

impl Collector for PoolCapacityCollector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::with_capacity(5 * self.cache.pools.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };

        for pool in self.cache.pools.iter() {
            let pool_total_size = match self
                .pool_total_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), pool.name().as_str()])
            {
                Ok(pool_total_size) => pool_total_size,
                Err(error) => {
                    error!(%error, "Error while creating pool_total_size counter with label values");
                    return metric_family;
                }
            };
            pool_total_size.set(pool.capacity() as f64);
            let mut metric_vec = pool_total_size.collect();
            metric_family.extend(metric_vec.pop());

            let pool_used_size = match self
                .pool_used_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), pool.name().as_str()])
            {
                Ok(pool_used_size) => pool_used_size,
                Err(error) => {
                    error!(%error, "Error while creating pool_used_size counter with label values");
                    return metric_family;
                }
            };
            pool_used_size.set(pool.used() as f64);
            let mut metric_vec = pool_used_size.collect();
            metric_family.extend(metric_vec.pop());

            let pool_committed_size = match self
                .pool_committed_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), pool.name().as_str()])
            {
                Ok(pool_committed_size) => pool_committed_size,
                Err(error) => {
                    error!(%error, "Error while creating pool_committed_size counter with label values");
                    return metric_family;
                }
            };
            pool_committed_size.set(pool.committed() as f64);
            let mut metric_vec = pool_committed_size.collect();
            metric_family.extend(metric_vec.pop());

            let pool_disk_capacity = match self
                .pool_disk_capacity
                .get_metric_with_label_values(&[node_name.clone().as_str(), pool.name().as_str()])
            {
                Ok(pool_disk_capacity) => pool_disk_capacity,
                Err(error) => {
                    error!(%error, "Error while creating pool_disk_capacity counter with label values");
                    return metric_family;
                }
            };
            pool_disk_capacity.set(pool.disk_capacity() as f64);
            let mut metric_vec = pool_disk_capacity.collect();
            metric_family.extend(metric_vec.pop());

            let pool_max_expandable_size = match self
                .pool_max_expandable_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), pool.name().as_str()])
            {
                Ok(pool_max_expandable_size) => pool_max_expandable_size,
                Err(error) => {
                    error!(%error, "Error while creating pool_max_expandable_size counter with label values");
                    return metric_family;
                }
            };
            pool_max_expandable_size.set(pool.max_expandable_size() as f64);
            let mut metric_vec = pool_max_expandable_size.collect();
            metric_family.extend(metric_vec.pop());
        }
        metric_family
    }
}

/// Collects pool status info from cache.
#[derive(Clone, Debug)]
pub(crate) struct PoolStatusCollector {
    cache: Pools,
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
        let pool_status = init_diskpool_gauge_vec("status", "Status of the pool", &mut descs);
        let cache = match Cache::get_cache().lock() {
            Ok(cache) => cache,
            Err(error) => {
                error!(%error,"Error while getting cache resource");
                panic!("panic");
            }
        };
        let pools = cache.deref().pool();
        Self {
            cache: pools.clone(),
            pool_status,
            descs,
        }
    }
}

impl Collector for PoolStatusCollector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        self.descs.iter().collect()
    }
    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::with_capacity(3 * self.cache.pools.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                return metric_family;
            }
        };
        for pool in self.cache.pools.clone() {
            let pool_status = match self
                .pool_status
                .get_metric_with_label_values(&[node_name.clone().as_str(), pool.name().as_str()])
            {
                Ok(pool_status) => pool_status,
                Err(error) => {
                    error!(%error, "Error while creating pool_status counter with label values");
                    return metric_family;
                }
            };
            pool_status.set(pool.state() as f64);
            let mut metric_vec = pool_status.collect();
            metric_family.extend(metric_vec.pop());
        }
        metric_family
    }
}

/// Collects pool alerts info from cache.
#[derive(Clone, Debug)]
pub(crate) struct PoolAlertCollector {
    cache: Pools,
    io_error_count: GaugeVec,
    io_error_threshold: GaugeVec,
    io_stalled: GaugeVec,
    io_stall_transition_count: GaugeVec,
    io_stall_transition_threshold: GaugeVec,
    io_alert_status: GaugeVec,
    io_alert_notice_reason: GaugeVec,
    io_alert_attention_reason: GaugeVec,
    io_alert_warning_reason: GaugeVec,
    io_alert_critical_reason: GaugeVec,
    node_name: String,
    descs: Vec<Desc>,
}

impl Default for PoolAlertCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolAlertCollector {
    /// Initialize all the metrics to be defined for pools alerts collector.
    pub fn new() -> Self {
        let mut descs = Vec::new();
        let io_error_count = init_diskpool_gauge_vec(
            "io_error_count",
            "Count of I/O errors for the pool",
            &mut descs,
        );
        let io_error_threshold = init_diskpool_gauge_vec(
            "io_error_threshold",
            "Threshold for I/O errors in the pool",
            &mut descs,
        );
        let io_stalled = init_diskpool_gauge_vec(
            "io_stalled",
            "Stalled I/O operations in the pool",
            &mut descs,
        );
        let io_stall_transition_count = init_diskpool_gauge_vec(
            "io_stall_transition_count",
            "Count of I/O stall transitions in the pool",
            &mut descs,
        );
        let io_stall_transition_threshold = init_diskpool_gauge_vec(
            "io_stall_transition_threshold",
            "Threshold for I/O stall transitions in the pool",
            &mut descs,
        );
        let io_alert_status =
            init_diskpool_gauge_vec("io_alert_status", "DiskPool alert status", &mut descs);
        let io_alert_notice_reason = init_diskpool_alert_reason_gauge_vec(
            "notice_reason",
            "Collection of reason for notice alert",
            &mut descs,
        );
        let io_alert_attention_reason = init_diskpool_alert_reason_gauge_vec(
            "attention_reason",
            "Collection of reason for attention alert",
            &mut descs,
        );
        let io_alert_warning_reason = init_diskpool_alert_reason_gauge_vec(
            "warning_reason",
            "Collection of reason for warning alert",
            &mut descs,
        );
        let io_alert_critical_reason = init_diskpool_alert_reason_gauge_vec(
            "critical_reason",
            "Collection of reason for critical alert",
            &mut descs,
        );
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(error) => {
                error!(?error, "Unable to get node name");
                String::new()
            }
        };
        let cache = match Cache::get_cache().lock() {
            Ok(cache) => cache,
            Err(error) => {
                error!(%error,"Error while getting cache resource");
                panic!("panic");
            }
        };
        let pool = cache.deref().pool();
        Self {
            cache: pool.clone(),
            io_error_count,
            io_error_threshold,
            io_stalled,
            io_stall_transition_count,
            io_stall_transition_threshold,
            io_alert_status,
            io_alert_notice_reason,
            io_alert_attention_reason,
            io_alert_warning_reason,
            io_alert_critical_reason,
            node_name,
            descs,
        }
    }
}

impl Collector for PoolAlertCollector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        self.descs.iter().collect()
    }
    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::with_capacity(10 * self.cache.pools.capacity());
        for pool in self.cache.pools.clone() {
            let io_error_count = match self
                .io_error_count
                .get_metric_with_label_values(&[self.node_name.as_str(), pool.name().as_str()])
            {
                Ok(io_error_count) => io_error_count,
                Err(error) => {
                    error!(%error, "Error while creating io_error_count gauge with label values");
                    return metric_family;
                }
            };
            io_error_count.set(pool.io_error_count() as f64);
            let mut metric_vec = io_error_count.collect();
            metric_family.extend(metric_vec.pop());

            let io_error_threshold = match self
                .io_error_threshold
                .get_metric_with_label_values(&[self.node_name.as_str(), pool.name().as_str()])
            {
                Ok(io_error_threshold) => io_error_threshold,
                Err(error) => {
                    error!(%error, "Error while creating io_error_threshold gauge with label values");
                    return metric_family;
                }
            };
            io_error_threshold.set(pool.io_error_threshold() as f64);
            let mut metric_vec = io_error_threshold.collect();
            metric_family.extend(metric_vec.pop());

            let io_stalled = match self
                .io_stalled
                .get_metric_with_label_values(&[self.node_name.as_str(), pool.name().as_str()])
            {
                Ok(io_stalled) => io_stalled,
                Err(error) => {
                    error!(%error, "Error while creating io_stalled gauge with label values");
                    return metric_family;
                }
            };
            let io_stalled_metrics: f64 = if pool.io_stalled() { 1_f64 } else { 0_f64 };
            io_stalled.set(io_stalled_metrics);
            let mut metric_vec = io_stalled.collect();
            metric_family.extend(metric_vec.pop());

            let io_stall_transition_count = match self
                .io_stall_transition_count
                .get_metric_with_label_values(&[self.node_name.as_str(), pool.name().as_str()])
            {
                Ok(io_stall_transition_count) => io_stall_transition_count,
                Err(error) => {
                    error!(%error, "Error while creating io_stall_transition_count gauge with label values");
                    return metric_family;
                }
            };
            io_stall_transition_count.set(pool.io_stall_transition_count() as f64);
            let mut metric_vec = io_stall_transition_count.collect();
            metric_family.extend(metric_vec.pop());

            let io_stall_transition_threshold = match self
                .io_stall_transition_threshold
                .get_metric_with_label_values(&[self.node_name.as_str(), pool.name().as_str()])
            {
                Ok(io_stall_transition_threshold) => io_stall_transition_threshold,
                Err(error) => {
                    error!(%error, "Error while creating io_stall_transition_threshold gauge with label values");
                    return metric_family;
                }
            };
            io_stall_transition_threshold.set(pool.io_stall_transition_threshold() as f64);
            let mut metric_vec = io_stall_transition_threshold.collect();
            metric_family.extend(metric_vec.pop());

            let io_alert_status = match self
                .io_alert_status
                .get_metric_with_label_values(&[self.node_name.as_str(), pool.name().as_str()])
            {
                Ok(io_alert_status) => io_alert_status,
                Err(error) => {
                    error!(%error, "Error while creating io_alert_status gauge with label values");
                    return metric_family;
                }
            };
            io_alert_status.set(pool.alert_status() as i32 as f64);
            let mut metric_vec = io_alert_status.collect();
            metric_family.extend(metric_vec.pop());

            let mut notice_reason_set = AlertReasons::default();
            notice_reason_set.update_alerts(pool.notice());
            let io_alert_notice_reason = match self
                .io_alert_notice_reason
                .get_metric_with_label_values(&[
                    self.node_name.as_str(),
                    pool.name().as_str(),
                    &notice_reason_set.unknown.to_string(),
                    &notice_reason_set.io_stalled.to_string(),
                    &notice_reason_set.io_stall_intermittent.to_string(),
                    &notice_reason_set.io_stall_intermittent_exc.to_string(),
                    &notice_reason_set.io_error.to_string(),
                    &notice_reason_set.io_error_exc.to_string(),
                ]) {
                Ok(io_alert_notice_reason) => io_alert_notice_reason,
                Err(error) => {
                    error!(%error, "Error while creating io_alert_notice_reason gauge with label values");
                    return metric_family;
                }
            };
            io_alert_notice_reason.set(1_f64);
            let mut metric_vec = io_alert_notice_reason.collect();
            metric_family.extend(metric_vec.pop());

            let mut attention_reason_set = AlertReasons::default();
            attention_reason_set.update_alerts(pool.attention());
            let io_alert_attention_reason = match self
                .io_alert_attention_reason
                .get_metric_with_label_values(&[
                    self.node_name.as_str(),
                    pool.name().as_str(),
                    &attention_reason_set.unknown.to_string(),
                    &attention_reason_set.io_stalled.to_string(),
                    &attention_reason_set.io_stall_intermittent.to_string(),
                    &attention_reason_set.io_stall_intermittent_exc.to_string(),
                    &attention_reason_set.io_error.to_string(),
                    &attention_reason_set.io_error_exc.to_string(),
                ]) {
                Ok(io_alert_attention_reason) => io_alert_attention_reason,
                Err(error) => {
                    error!(%error, "Error while creating io_alert_attention_reason gauge with label values");
                    return metric_family;
                }
            };
            io_alert_attention_reason.set(1_f64);
            let mut metric_vec = io_alert_attention_reason.collect();
            metric_family.extend(metric_vec.pop());

            let mut warning_reason_set = AlertReasons::default();
            warning_reason_set.update_alerts(pool.warning());
            let io_alert_warning_reason = match self
                .io_alert_warning_reason
                .get_metric_with_label_values(&[
                    self.node_name.as_str(),
                    pool.name().as_str(),
                    &warning_reason_set.unknown.to_string(),
                    &warning_reason_set.io_stalled.to_string(),
                    &warning_reason_set.io_stall_intermittent.to_string(),
                    &warning_reason_set.io_stall_intermittent_exc.to_string(),
                    &warning_reason_set.io_error.to_string(),
                    &warning_reason_set.io_error_exc.to_string(),
                ]) {
                Ok(io_alert_warning_reason) => io_alert_warning_reason,
                Err(error) => {
                    error!(%error, "Error while creating io_alert_warning_reason gauge with label values");
                    return metric_family;
                }
            };
            io_alert_warning_reason.set(1_f64);
            let mut metric_vec = io_alert_warning_reason.collect();
            metric_family.extend(metric_vec.pop());

            let mut critical_reason_set = AlertReasons::default();
            critical_reason_set.update_alerts(pool.critical());
            let io_alert_critical_reason = match self
                .io_alert_critical_reason
                .get_metric_with_label_values(&[
                    self.node_name.as_str(),
                    pool.name().as_str(),
                    &critical_reason_set.unknown.to_string(),
                    &critical_reason_set.io_stalled.to_string(),
                    &critical_reason_set.io_stall_intermittent.to_string(),
                    &critical_reason_set.io_stall_intermittent_exc.to_string(),
                    &critical_reason_set.io_error.to_string(),
                    &critical_reason_set.io_error_exc.to_string(),
                ]) {
                Ok(io_alert_critical_reason) => io_alert_critical_reason,
                Err(error) => {
                    error!(%error, "Error while creating io_alert_critical_reason gauge with label values");
                    return metric_family;
                }
            };
            io_alert_critical_reason.set(1_f64);
            let mut metric_vec = io_alert_critical_reason.collect();
            metric_family.extend(metric_vec.pop());
        }

        metric_family
    }
}

#[derive(Clone, Debug, Default)]
/// Struct to hold the reasons for each alert type, with default values set to 0 (indicating no reason).
struct AlertReasons {
    unknown: i32,
    io_stalled: i32,
    io_stall_intermittent: i32,
    io_stall_intermittent_exc: i32,
    io_error: i32,
    io_error_exc: i32,
}

impl AlertReasons {
    fn update_alerts(&mut self, pool_alerts: &Vec<PoolAlert>) {
        for alert in pool_alerts {
            match alert {
                PoolAlert::Unknown => self.unknown = 1,
                PoolAlert::IoStalled => self.io_stalled = 1,
                PoolAlert::IoStallIntermittent => self.io_stall_intermittent = 1,
                PoolAlert::IoStallIntermittentExc => self.io_stall_intermittent_exc = 1,
                PoolAlert::IoError => self.io_error = 1,
                PoolAlert::IoErrorExc => self.io_error_exc = 1,
            }
        }
    }
}
