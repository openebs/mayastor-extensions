use prometheus::{
    core::{Collector, Desc},
    GaugeVec, Opts,
};

/// Module for pools collector.
pub mod pool;
pub mod pool_stat;

/// Initializes a GaugeVec metric with the provided metric name, description and descriptors.
fn init_gauge_vec(metric_name: &str, metric_desc: &str, descs: &mut Vec<Desc>) -> GaugeVec {
    let opts = Opts::new(metric_name, metric_desc)
        .subsystem("diskpool")
        .variable_labels(vec!["node".to_string(), "name".to_string()]);
    let gauge_vec = GaugeVec::new(opts, &["node", "name"])
        .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {}", metric_name));
    descs.extend(gauge_vec.desc().into_iter().cloned());
    gauge_vec
}
