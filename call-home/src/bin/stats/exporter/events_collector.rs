use crate::cache::events_cache::{Cache, EventSet};
use obs::common::constants::{ACTION, CREATED, DELETED, POOL_STATS, VOLUME_STATS};
use prometheus::{
    core::{Collector, Desc},
    GaugeVec, Opts,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, ops::DerefMut};
use tracing::error;

/// StatsCollector contains the list of custom metrics that has to be exposed by exporter.
#[derive(Clone, Debug)]
pub struct StatsCollector {
    volumes: GaugeVec,
    pools: GaugeVec,
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
    Unknown,
}

impl ToString for Metrics {
    fn to_string(&self) -> String {
        match self {
            Metrics::Pool => "pool".to_string(),
            Metrics::Volume => "volume".to_string(),
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
        let mut descs = Vec::new();

        let volumes = GaugeVec::new(volume_opts, &[ACTION])
            .expect("Unable to create gauge metric type for volume stats");
        let pools = GaugeVec::new(pool_opts, &[ACTION])
            .expect("Unable to create gauge metric type for pool stats");
        descs.extend(volumes.desc().into_iter().cloned());
        descs.extend(pools.desc().into_iter().cloned());

        Self {
            volumes,
            pools,
            descs,
        }
    }

    fn volume_metrics(&self, events: &mut EventSet) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::new();
        let volumes_created = match self.volumes.get_metric_with_label_values(&[CREATED]) {
            Ok(volumes) => volumes,
            Err(error) => {
                error!(%error,"Error while creating metrics(volumes created) with label values: {}", CREATED);
                return metric_family;
            }
        };
        volumes_created.set(events.volume.volume_created as f64);
        let volumes_deleted = match self.volumes.get_metric_with_label_values(&[DELETED]) {
            Ok(volumes) => volumes,
            Err(error) => {
                error!(%error,"Error while creating metrics(volumes deleted) with label values: {}", DELETED);
                return metric_family;
            }
        };
        volumes_deleted.set(events.volume.volume_deleted as f64);
        metric_family.extend(volumes_created.collect().pop());
        metric_family.extend(volumes_deleted.collect().pop());
        metric_family
    }

    fn pool_metrics(&self, events: &mut EventSet) -> Vec<prometheus::proto::MetricFamily> {
        let mut metric_family = Vec::new();
        let pools_created = match self.pools.get_metric_with_label_values(&[CREATED]) {
            Ok(pools) => pools,
            Err(error) => {
                error!(%error,"Error while creating metrics(pools created) with label values: {}", CREATED );
                return metric_family;
            }
        };
        pools_created.set(events.pool.pool_created as f64);
        let pools_deleted = match self.pools.get_metric_with_label_values(&[DELETED]) {
            Ok(pools) => pools,
            Err(error) => {
                error!(%error,"Error while creating metrics(pools deleted) with label values: {}", DELETED);
                return metric_family;
            }
        };
        pools_deleted.set(events.pool.pool_deleted as f64);
        metric_family.extend(pools_created.collect().pop());
        metric_family.extend(pools_deleted.collect().pop());
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
        metric_family
    }
}
