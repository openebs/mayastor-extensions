use std::{env, fmt::Debug, ops::DerefMut};

use prometheus::{
    core::{Collector, Desc},
    GaugeVec, Opts,
};

use crate::{cache::Cache, client::pool::Pool};

/// PoolsCollector contains the list of custom metrics that has to be exposed by exporter.
#[derive(Clone, Debug)]
pub struct PoolsCollector {
    pool_total_size: GaugeVec,
    pool_used_size: GaugeVec,
    pool_status: GaugeVec,
    descs: Vec<Desc>,
}

impl Default for PoolsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolsCollector {
    /// Initialize all the metrics to be defined for pools collector
    pub fn new() -> Self {
        let pool_total_size_opts = Opts::new("total_size_bytes", "Total size of the pool in bytes")
            .subsystem("disk_pool")
            .variable_labels(vec!["node".to_string(), "name".to_string()]);
        let pool_used_size_opts = Opts::new("used_size_bytes", "Used size of the pool in bytes")
            .subsystem("disk_pool")
            .variable_labels(vec!["node".to_string(), "name".to_string()]);
        let pool_status_opts = Opts::new("status", "Status of the pool")
            .subsystem("disk_pool")
            .variable_labels(vec!["node".to_string(), "name".to_string()]);
        let mut descs = Vec::new();

        let pool_total_size = GaugeVec::new(pool_total_size_opts, &["node", "name"])
            .expect("Unable to create gauge metric type for pool_total_size");
        let pool_used_size = GaugeVec::new(pool_used_size_opts, &["node", "name"])
            .expect("Unable to create gauge metric type for pool_used_size");
        let pool_status = GaugeVec::new(pool_status_opts, &["node", "name"])
            .expect("Unable to create gauge metric type for pool_status");
        // Descriptors for the custom metrics
        descs.extend(pool_total_size.desc().into_iter().cloned());
        descs.extend(pool_used_size.desc().into_iter().cloned());
        descs.extend(pool_status.desc().into_iter().cloned());

        Self {
            pool_total_size,
            pool_used_size,
            pool_status,
            descs,
        }
    }
}

/// prometheus collector implementation
impl Collector for PoolsCollector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut c = match Cache::get_cache().lock() {
            Ok(c) => c,
            Err(err) => {
                println!("Error while getting cache resource:{}", err);
                return Vec::new();
            }
        };
        let cp = c.deref_mut();
        let mut metric_family = Vec::with_capacity(3 * cp.data_mut().pools().pools.capacity());
        let node_name = match get_node_name() {
            Ok(name) => name,
            Err(_) => {
                println!("Unable to get node name");
                return metric_family;
            }
        };

        for i in &cp.data_mut().pools().pools {
            let p: &Pool = i;

            let pool_total_size = match self
                .pool_total_size
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_total_size) => pool_total_size,
                Err(_) => {
                    println!("Error while creating metrics(pool_total_size) with label values:");
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
                Err(_) => {
                    println!("Error while creating metrics(pool_used_size) with label values:");
                    return metric_family;
                }
            };
            pool_used_size.set(p.used() as f64);
            let mut x = pool_used_size.collect();
            metric_family.extend(x.pop());

            let pool_status = match self
                .pool_status
                .get_metric_with_label_values(&[node_name.clone().as_str(), p.name().as_str()])
            {
                Ok(pool_status) => pool_status,
                Err(_) => {
                    println!("Error while creating metrics(pool_status) with label values:");
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

/// get node name from pod spec
pub fn get_node_name() -> Result<String, String> {
    env::var("MY_NODE_NAME").map_err(|_| "Unable to get node name".to_string())
}
