use prometheus::{
    core::{Collector, Desc},
    GaugeVec, Opts,
};

pub(crate) mod nexus_stat;
pub(crate) mod node_status;
pub(crate) mod pool;
pub(crate) mod pool_stat;
pub(crate) mod replica_stat;

/// Initializes a GaugeVec metric for diskpool with the provided metric name, description and
/// descriptors.
fn init_diskpool_gauge_vec(
    metric_name: &str,
    metric_desc: &str,
    descs: &mut Vec<Desc>,
) -> GaugeVec {
    let label_refs = vec!["node", "name"];
    let labels: Vec<String> = label_refs.iter().map(|s| s.to_string()).collect();
    let opts = Opts::new(metric_name, metric_desc)
        .subsystem("diskpool")
        .variable_labels(labels);
    let gauge_vec = GaugeVec::new(opts, &label_refs)
        .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {metric_name}"));
    descs.extend(gauge_vec.desc().into_iter().cloned());
    gauge_vec
}

/// Initializes a GaugeVec metric for diskpool alert reason with the provided metric name, description and
/// descriptors.
fn init_diskpool_alert_reason_gauge_vec(
    metric_name: &str,
    metric_desc: &str,
    descs: &mut Vec<Desc>,
) -> GaugeVec {
    let label_refs = vec![
        "node",
        "name",
        "unknown",
        "io_stalled",
        "io_stall_intermittent",
        "io_stall_intermittent_exc",
        "io_error",
        "io_error_exc",
    ];

    let labels: Vec<String> = label_refs.iter().map(|s| s.to_string()).collect();

    let opts = Opts::new(metric_name, metric_desc)
        .subsystem("diskpool_alert")
        .variable_labels(labels);

    let gauge_vec = GaugeVec::new(opts, &label_refs)
        .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {metric_name}"));

    descs.extend(gauge_vec.desc().into_iter().cloned());
    gauge_vec
}

/// Initializes a GaugeVec metric for volume with the provided metric name, description and
/// descriptors.
fn init_volume_gauge_vec(metric_name: &str, metric_desc: &str, descs: &mut Vec<Desc>) -> GaugeVec {
    let label_refs = vec!["node", "pv_name"];
    let labels: Vec<String> = label_refs.iter().map(|s| s.to_string()).collect();
    let opts = Opts::new(metric_name, metric_desc)
        .subsystem("volume")
        .variable_labels(labels);
    let gauge_vec = GaugeVec::new(opts, &label_refs)
        .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {metric_name}"));
    descs.extend(gauge_vec.desc().into_iter().cloned());
    gauge_vec
}

/// Initializes a GaugeVec metric for replica with the provided metric name, description and
/// descriptors.
fn init_replica_gauge_vec(metric_name: &str, metric_desc: &str, descs: &mut Vec<Desc>) -> GaugeVec {
    let label_refs = vec!["node", "name", "pv_name", "pool_name"];
    let labels: Vec<String> = label_refs.iter().map(|s| s.to_string()).collect();
    let opts = Opts::new(metric_name, metric_desc)
        .subsystem("replica")
        .variable_labels(labels);
    let gauge_vec = GaugeVec::new(opts, &label_refs)
        .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {metric_name}"));
    descs.extend(gauge_vec.desc().into_iter().cloned());
    gauge_vec
}
