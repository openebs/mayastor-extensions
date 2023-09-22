use crate::cache::events_cache::{Cache, EventSet};
use obs::common::constants::{
    ACTION, CREATED, DELETED, NEXUS_STATS, POOL_STATS, REBUILD_ENDED, REBUILD_STARTED, VOLUME_STATS,
};
use prometheus::{
    core::{Collector, Desc},
    CounterVec, Opts,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, ops::DerefMut};
use tracing::error;

/// StatsCollector contains the list of custom metrics that has to be exposed by exporter.
#[derive(Clone, Debug)]
pub struct StatsCollector {
    volumes: CounterVec,
    pools: CounterVec,
    nexus: CounterVec,
    descs: Vec<Desc>,
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics category.
#[derive(Serialize, Deserialize, Debug)]
pub enum Metrics {
    Pool,
    Volume,
    Nexus,
    Unknown,
}

impl ToString for Metrics {
    fn to_string(&self) -> String {
        match self {
            Metrics::Pool => "pool".to_string(),
            Metrics::Volume => "volume".to_string(),
            Metrics::Nexus => "nexus".to_string(),
            Metrics::Unknown => "".to_string(),
        }
    }
}

impl StatsCollector {
    /// Initialize all the metrics to be defined for stats collector.
    pub fn new() -> Self {
        let volume_opts = Opts::new(Metrics::Volume.to_string(), VOLUME_STATS)
            .variable_labels(vec![ACTION.to_string()]);
        let pool_opts = Opts::new(Metrics::Pool.to_string(), POOL_STATS)
            .variable_labels(vec![ACTION.to_string()]);
        let nexus_opts = Opts::new(Metrics::Nexus.to_string(), NEXUS_STATS)
            .variable_labels(vec![ACTION.to_string()]);
        let mut descs = Vec::new();

        let volumes = CounterVec::new(volume_opts, &[ACTION])
            .expect("Unable to create counter metric type for volume stats");
        let pools = CounterVec::new(pool_opts, &[ACTION])
            .expect("Unable to create counter metric type for pool stats");
        let nexus = CounterVec::new(nexus_opts, &[ACTION])
            .expect("Unable to create counter metric type for nexus stats");
        descs.extend(volumes.desc().into_iter().cloned());
        descs.extend(pools.desc().into_iter().cloned());
        descs.extend(nexus.desc().into_iter().cloned());

        Self {
            volumes,
            pools,
            nexus,
            descs,
        }
    }

    fn volume_metrics(&self, events: &EventSet) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::new();
        let volumes_created = match self.volumes.get_metric_with_label_values(&[CREATED]) {
            Ok(volumes) => volumes,
            Err(error) => {
                error!(%error,"Error while creating metrics(volumes created) with label values: {CREATED}");
                return metric_family;
            }
        };
        volumes_created.inc_by(events.volume.volume_created as f64);
        let volumes_deleted = match self.volumes.get_metric_with_label_values(&[DELETED]) {
            Ok(volumes) => volumes,
            Err(error) => {
                error!(%error,"Error while creating metrics(volumes deleted) with label values: {DELETED}");
                return metric_family;
            }
        };
        volumes_deleted.inc_by(events.volume.volume_deleted as f64);
        metric_family.extend(volumes_created.collect());
        metric_family.extend(volumes_deleted.collect());
        metric_family
    }

    fn pool_metrics(&self, events: &EventSet) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::new();
        let pools_created = match self.pools.get_metric_with_label_values(&[CREATED]) {
            Ok(pools) => pools,
            Err(error) => {
                error!(%error,"Error while creating metrics(pools created) with label values: {CREATED}");
                return metric_family;
            }
        };
        pools_created.inc_by(events.pool.pool_created as f64);
        let pools_deleted = match self.pools.get_metric_with_label_values(&[DELETED]) {
            Ok(pools) => pools,
            Err(error) => {
                error!(%error,"Error while creating metrics(pools deleted) with label values: {DELETED}");
                return metric_family;
            }
        };
        pools_deleted.inc_by(events.pool.pool_deleted as f64);
        metric_family.extend(pools_created.collect());
        metric_family.extend(pools_deleted.collect());
        metric_family
    }

    fn nexus_metrics(&self, events: &EventSet) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::new();
        let nexus_created = match self.nexus.get_metric_with_label_values(&[CREATED]) {
            Ok(nexus) => nexus,
            Err(error) => {
                error!(%error,"Error while creating metrics(nexus created) with label values: {CREATED}");
                return metric_family;
            }
        };
        nexus_created.inc_by(events.nexus.nexus_created as f64);
        let nexus_deleted = match self.nexus.get_metric_with_label_values(&[DELETED]) {
            Ok(nexus) => nexus,
            Err(error) => {
                error!(%error,"Error while creating metrics(nexus deleted) with label values: {DELETED}");
                return metric_family;
            }
        };
        nexus_deleted.inc_by(events.nexus.nexus_deleted as f64);
        let rebuild_started = match self.nexus.get_metric_with_label_values(&[REBUILD_STARTED]) {
            Ok(nexus) => nexus,
            Err(error) => {
                error!(%error,"Error while creating metrics(rebuild started) with label values: {REBUILD_STARTED}");
                return metric_family;
            }
        };
        rebuild_started.inc_by(events.nexus.rebuild_started as f64);
        let rebuild_ended = match self.nexus.get_metric_with_label_values(&[REBUILD_ENDED]) {
            Ok(nexus) => nexus,
            Err(error) => {
                error!(%error,"Error while creating metrics(rebuild ended) with label values: {REBUILD_ENDED}");
                return metric_family;
            }
        };
        rebuild_ended.inc_by(events.nexus.rebuild_ended as f64);
        metric_family.extend(nexus_created.collect());
        metric_family.extend(nexus_deleted.collect());
        metric_family.extend(rebuild_started.collect());
        metric_family.extend(rebuild_ended.collect());
        metric_family
    }
}

/// Prometheus collector implementation
impl Collector for StatsCollector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut c = match Cache::cache_init().lock() {
            Ok(c) => c,
            Err(error) => {
                error!(%error,"Error while getting stats cache resource");
                return Vec::new();
            }
        };
        let cp = c.deref_mut();
        let mut metric_family = Vec::new();
        metric_family.extend(self.volume_metrics(cp.data_mut().deref_mut()));
        metric_family.extend(self.pool_metrics(cp.data_mut().deref_mut()));
        metric_family.extend(self.nexus_metrics(cp.data_mut().deref_mut()));
        metric_family
    }
}
