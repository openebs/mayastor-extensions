use prometheus::{
    core::{Collector, Desc},
    GaugeVec, Opts,
};

pub(crate) mod nexus_stat;
pub(crate) mod pool;
pub(crate) mod pool_stat;

/// Initializes a GaugeVec metric for diskpool with the provided metric name, description and
/// descriptors.
fn init_diskpool_gauge_vec(
    metric_name: &str,
    metric_desc: &str,
    descs: &mut Vec<Desc>,
) -> GaugeVec {
    let opts = Opts::new(metric_name, metric_desc)
        .subsystem("diskpool")
        .variable_labels(vec!["node".to_string(), "name".to_string()]);
    let gauge_vec = GaugeVec::new(opts, &["node", "name"])
        .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {}", metric_name));
    descs.extend(gauge_vec.desc().into_iter().cloned());
    gauge_vec
}

/// Initializes a GaugeVec metric for volume with the provided metric name, description and
/// descriptors.
fn init_volume_gauge_vec(metric_name: &str, metric_desc: &str, descs: &mut Vec<Desc>) -> GaugeVec {
    let opts = Opts::new(metric_name, metric_desc)
        .subsystem("volume")
        .variable_labels(vec!["node".to_string(), "pv_name".to_string()]);
    let gauge_vec = GaugeVec::new(opts, &["node", "pv_name"])
        .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {}", metric_name));
    descs.extend(gauge_vec.desc().into_iter().cloned());
    gauge_vec
}
