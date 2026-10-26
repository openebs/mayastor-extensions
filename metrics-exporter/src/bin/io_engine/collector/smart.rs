use crate::{cache::Cache, client::smart::PoolsSmart, get_node_name};
use prometheus::{
    core::{Collector, Desc},
    proto::MetricFamily,
    GaugeVec, Opts,
};
use rpc::v1::pool::{smart_data::Protocol, AtaSmart, DiskSmart, NvmeSmart, SmartData};
use std::{collections::BTreeMap, ops::Deref};
use tracing::error;

/// The labels that identify a disk.
const DISK_LABELS: [&str; 3] = ["node", "name", "disk"];

/// The set of SMART gauges, looked up by metric name.
#[derive(Clone, Debug)]
struct SmartGauges {
    gauges: BTreeMap<&'static str, GaugeVec>,
    descs: Vec<Desc>,
}

impl SmartGauges {
    /// Initialize all the metrics defined for the SMART collector.
    fn new() -> Self {
        let mut gauges = Self {
            gauges: BTreeMap::new(),
            descs: Vec::new(),
        };
        gauges.add_disk_gauges();
        gauges.add_nvme_gauges();
        gauges.add_ata_gauges();
        gauges
    }

    /// Initializes a GaugeVec metric for the disk SMART data with the provided metric name,
    /// description and labels.
    fn add(&mut self, metric_name: &'static str, metric_desc: &str, label_refs: &[&str]) {
        let labels: Vec<String> = label_refs.iter().map(|s| s.to_string()).collect();
        let opts = Opts::new(metric_name, metric_desc)
            .subsystem("diskpool_disk_smart")
            .variable_labels(labels);
        let gauge_vec = GaugeVec::new(opts, label_refs)
            .unwrap_or_else(|_| panic!("Unable to create gauge metric type for {metric_name}"));
        self.descs.extend(gauge_vec.desc().into_iter().cloned());
        self.gauges.insert(metric_name, gauge_vec);
    }

    /// Initializes a GaugeVec metric with the labels that identify a disk, plus the extra ones.
    fn add_disk(&mut self, metric_name: &'static str, metric_desc: &str, extra: &[&str]) {
        let labels = [DISK_LABELS.as_slice(), extra].concat();
        self.add(metric_name, metric_desc, &labels);
    }

    /// The metrics that every device type reports with the same meaning.
    fn add_disk_gauges(&mut self) {
        self.add_disk(
            "supported",
            "Indicates if SMART data can be read from the disk (0/1)",
            &[],
        );
        self.add_disk(
            "status_passed",
            "Overall SMART health self-assessment of the disk, 1 if passed and 0 if failed",
            &[],
        );
        self.add_disk(
            "temperature_celsius",
            "Temperature of the disk in degrees Celsius",
            &[],
        );
        self.add_disk(
            "power_on_hours",
            "Cumulative power-on hours of the disk",
            &[],
        );
        self.add_disk("power_cycles", "Number of power cycles of the disk", &[]);
        self.add_disk(
            "available_spare_percent",
            "Remaining spare capacity of the disk as a percentage",
            &[],
        );
        self.add_disk(
            "available_spare_threshold_percent",
            "Spare capacity percentage below which the disk flags a problem",
            &[],
        );
        self.add_disk(
            "percentage_used",
            "Estimated percentage of the rated write endurance of the disk used, may exceed 100",
            &[],
        );
        self.add_disk(
            "info",
            "Identity of the disk, always 1",
            &["model", "firmware", "transport", "type"],
        );
    }

    /// The metrics of the NVMe SMART / health log, which only NVMe disks report.
    fn add_nvme_gauges(&mut self) {
        self.add_disk(
            "nvme_temperature_celsius",
            "Composite temperature of the NVMe disk in degrees Celsius",
            &[],
        );
        self.add_disk(
            "nvme_power_on_hours",
            "Cumulative power-on hours of the NVMe disk",
            &[],
        );
        self.add_disk(
            "nvme_power_cycles",
            "Number of power cycles of the NVMe disk",
            &[],
        );
        self.add_disk(
            "nvme_available_spare_percent",
            "Remaining spare capacity of the NVMe disk as a percentage",
            &[],
        );
        self.add_disk(
            "nvme_available_spare_threshold_percent",
            "Spare capacity percentage below which the NVMe disk flags a problem",
            &[],
        );
        self.add_disk(
            "nvme_percentage_used",
            "Estimated percentage of the life of the NVMe disk used, may exceed 100",
            &[],
        );
        self.add_disk(
            "nvme_critical_warning",
            "Raw NVMe critical warning flags of the disk, 0 if there is none",
            &[],
        );
        self.add_disk(
            "nvme_media_errors",
            "Number of media and data integrity errors of the NVMe disk",
            &[],
        );
        self.add_disk(
            "nvme_unsafe_shutdowns",
            "Number of unsafe shutdowns of the NVMe disk",
            &[],
        );
        self.add_disk(
            "nvme_error_log_entries",
            "Number of error information log entries of the NVMe disk",
            &[],
        );
        self.add_disk(
            "nvme_warning_temp_time_minutes",
            "Minutes the NVMe disk has been at or above its warning composite temperature",
            &[],
        );
        self.add_disk(
            "nvme_critical_comp_time_minutes",
            "Minutes the NVMe disk has been at or above its critical composite temperature",
            &[],
        );
        self.add_disk(
            "nvme_data_units_read",
            "Number of data units read from the NVMe disk, a data unit is 512000 bytes",
            &[],
        );
        self.add_disk(
            "nvme_data_units_written",
            "Number of data units written to the NVMe disk, a data unit is 512000 bytes",
            &[],
        );
        self.add_disk(
            "nvme_host_reads",
            "Number of host read commands completed by the NVMe disk",
            &[],
        );
        self.add_disk(
            "nvme_host_writes",
            "Number of host write commands completed by the NVMe disk",
            &[],
        );
        self.add_disk(
            "nvme_controller_busy_minutes",
            "Minutes the controller of the NVMe disk was busy with I/O",
            &[],
        );
    }

    /// The metrics of the ATA SMART attribute table, which only SATA/ATA disks report.
    fn add_ata_gauges(&mut self) {
        let labels = ["attribute_id", "attribute_name"];
        self.add_disk(
            "ata_attribute_value",
            "Current normalized value of the ATA SMART attribute",
            &labels,
        );
        self.add_disk(
            "ata_attribute_worst",
            "Worst normalized value recorded for the ATA SMART attribute",
            &labels,
        );
        self.add_disk(
            "ata_attribute_threshold",
            "Failure threshold of the ATA SMART attribute",
            &labels,
        );
        self.add_disk(
            "ata_attribute_raw_value",
            "Raw value of the ATA SMART attribute",
            &labels,
        );
    }

    /// Set the gauge with the given name for the given label values.
    fn set(&self, metric_name: &str, labels: &[&str], value: f64) {
        let Some(gauge) = self.gauges.get(metric_name) else {
            error!(metric_name, "SMART gauge is not defined");
            return;
        };
        match gauge.get_metric_with_label_values(labels) {
            Ok(gauge) => gauge.set(value),
            Err(error) => {
                error!(%error, metric_name, "Error while creating SMART gauge with label values");
            }
        }
    }

    /// Set the gauge with the given name, only if the value is set.
    fn set_opt(&self, metric_name: &str, labels: &[&str], value: Option<f64>) {
        if let Some(value) = value {
            self.set(metric_name, labels, value);
        }
    }

    /// Collect the metric families of all the gauges that have a value.
    fn collect(&self) -> Vec<MetricFamily> {
        self.gauges
            .values()
            .flat_map(|gauge| gauge.collect())
            .filter(|family| !family.get_metric().is_empty())
            .collect()
    }
}

/// Collects the disk SMART metrics from cache.
#[derive(Clone, Debug)]
pub(crate) struct SmartCollector {
    cache: PoolsSmart,
    node: Option<String>,
    gauges: SmartGauges,
}

impl Default for SmartCollector {
    fn default() -> Self {
        let cache = if let Ok(cache) = Cache::get_cache().lock() {
            cache.deref().pools_smart().clone()
        } else {
            PoolsSmart::default()
        };
        let node = match get_node_name() {
            Ok(name) => Some(name),
            Err(error) => {
                error!(?error, "Unable to get node name");
                None
            }
        };
        Self::new(cache, node)
    }
}

impl SmartCollector {
    /// Initialize all the metrics to be defined for the SMART collector.
    pub(crate) fn new(cache: PoolsSmart, node: Option<String>) -> Self {
        Self {
            cache,
            node,
            gauges: SmartGauges::new(),
        }
    }

    /// Set the metrics of a disk.
    fn set_disk(&self, node: &str, pool: &str, disk: &DiskSmart) {
        let labels = [node, pool, disk.disk_uri.as_str()];
        self.gauges
            .set("supported", &labels, f64::from(u8::from(disk.supported)));
        let Some(smart) = disk.smart.as_ref() else {
            return;
        };
        self.set_smart(&labels, smart);
        match smart.protocol.as_ref() {
            Some(Protocol::Nvme(nvme)) => self.set_nvme(&labels, nvme),
            Some(Protocol::Ata(ata)) => self.set_ata(&labels, ata),
            None => {}
        }
    }

    /// Set the metrics that every device type reports with the same meaning.
    fn set_smart(&self, labels: &[&str; 3], smart: &SmartData) {
        let gauges = &self.gauges;
        gauges.set_opt(
            "status_passed",
            labels,
            smart
                .smart_status_passed
                .map(|passed| f64::from(u8::from(passed))),
        );
        gauges.set_opt(
            "temperature_celsius",
            labels,
            smart.temperature_celsius.map(f64::from),
        );
        gauges.set_opt(
            "power_on_hours",
            labels,
            smart.power_on_hours.map(|hours| hours as f64),
        );
        gauges.set_opt(
            "power_cycles",
            labels,
            smart.power_cycles.map(|cycles| cycles as f64),
        );
        gauges.set_opt(
            "available_spare_percent",
            labels,
            smart.available_spare_percent.map(f64::from),
        );
        gauges.set_opt(
            "available_spare_threshold_percent",
            labels,
            smart.available_spare_threshold_percent.map(f64::from),
        );
        gauges.set_opt(
            "percentage_used",
            labels,
            smart.percentage_used.map(f64::from),
        );

        let identity = smart.identity.as_ref();
        let device_type = match smart.protocol {
            Some(Protocol::Nvme(_)) => "nvme",
            Some(Protocol::Ata(_)) => "ata",
            None => "",
        };
        let info_labels = [
            labels[0],
            labels[1],
            labels[2],
            identity
                .and_then(|identity| identity.model.as_deref())
                .unwrap_or_default(),
            identity
                .and_then(|identity| identity.firmware_revision.as_deref())
                .unwrap_or_default(),
            identity
                .and_then(|identity| identity.transport.as_deref())
                .unwrap_or_default(),
            device_type,
        ];
        gauges.set("info", &info_labels, 1.0);
    }

    /// Set the metrics of an NVMe disk.
    fn set_nvme(&self, labels: &[&str; 3], nvme: &NvmeSmart) {
        let gauges = &self.gauges;
        gauges.set_opt(
            "nvme_temperature_celsius",
            labels,
            nvme.temperature_celsius.map(f64::from),
        );
        gauges.set_opt(
            "nvme_power_on_hours",
            labels,
            nvme.power_on_hours.map(|hours| hours as f64),
        );
        gauges.set_opt(
            "nvme_power_cycles",
            labels,
            nvme.power_cycles.map(|cycles| cycles as f64),
        );
        gauges.set_opt(
            "nvme_available_spare_percent",
            labels,
            nvme.available_spare_percent.map(f64::from),
        );
        gauges.set_opt(
            "nvme_available_spare_threshold_percent",
            labels,
            nvme.available_spare_threshold_percent.map(f64::from),
        );
        gauges.set_opt(
            "nvme_percentage_used",
            labels,
            nvme.percentage_used.map(f64::from),
        );
        gauges.set(
            "nvme_critical_warning",
            labels,
            f64::from(nvme.critical_warning),
        );
        gauges.set_opt(
            "nvme_media_errors",
            labels,
            nvme.media_errors.map(|errors| errors as f64),
        );
        gauges.set_opt(
            "nvme_unsafe_shutdowns",
            labels,
            nvme.unsafe_shutdowns.map(|shutdowns| shutdowns as f64),
        );
        gauges.set_opt(
            "nvme_error_log_entries",
            labels,
            nvme.num_error_log_entries.map(|entries| entries as f64),
        );
        gauges.set_opt(
            "nvme_warning_temp_time_minutes",
            labels,
            nvme.warning_temp_time.map(f64::from),
        );
        gauges.set_opt(
            "nvme_critical_comp_time_minutes",
            labels,
            nvme.critical_comp_time.map(f64::from),
        );
        gauges.set_opt(
            "nvme_data_units_read",
            labels,
            nvme.data_units_read.map(|units| units as f64),
        );
        gauges.set_opt(
            "nvme_data_units_written",
            labels,
            nvme.data_units_written.map(|units| units as f64),
        );
        gauges.set_opt(
            "nvme_host_reads",
            labels,
            nvme.host_reads.map(|reads| reads as f64),
        );
        gauges.set_opt(
            "nvme_host_writes",
            labels,
            nvme.host_writes.map(|writes| writes as f64),
        );
        gauges.set_opt(
            "nvme_controller_busy_minutes",
            labels,
            nvme.controller_busy_minutes.map(|minutes| minutes as f64),
        );
    }

    /// Set the metrics of the attributes of a SATA/ATA disk.
    fn set_ata(&self, labels: &[&str; 3], ata: &AtaSmart) {
        for attribute in &ata.attributes {
            let id = attribute.id.to_string();
            let labels = [
                labels[0],
                labels[1],
                labels[2],
                id.as_str(),
                attribute.name.as_str(),
            ];
            let gauges = &self.gauges;
            gauges.set("ata_attribute_value", &labels, f64::from(attribute.value));
            gauges.set("ata_attribute_worst", &labels, f64::from(attribute.worst));
            gauges.set(
                "ata_attribute_threshold",
                &labels,
                f64::from(attribute.threshold),
            );
            gauges.set(
                "ata_attribute_raw_value",
                &labels,
                attribute.raw_value as f64,
            );
        }
    }
}

impl Collector for SmartCollector {
    fn desc(&self) -> Vec<&Desc> {
        self.gauges.descs.iter().collect()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let Some(node) = self.node.as_deref() else {
            return Vec::new();
        };

        for pool in &self.cache.pools {
            for disk in &pool.disks {
                self.set_disk(node, &pool.name, disk);
            }
        }
        self.gauges.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpc::v1::pool::{DeviceIdentity, PoolSmart, SmartAttribute};

    fn nvme_disk() -> DiskSmart {
        DiskSmart {
            disk_uri: "aio:///dev/disk/by-id/nvme-disk".into(),
            supported: true,
            smart: Some(SmartData {
                smart_status_passed: Some(true),
                temperature_celsius: Some(42),
                power_on_hours: Some(6950),
                power_cycles: Some(1526),
                identity: Some(DeviceIdentity {
                    model: Some("Samsung SSD 970 EVO Plus 500GB".into()),
                    firmware_revision: Some("2B2QEXM7".into()),
                    transport: Some("NVMe".into()),
                    ..Default::default()
                }),
                available_spare_percent: Some(100),
                available_spare_threshold_percent: Some(10),
                percentage_used: Some(43),
                protocol: Some(Protocol::Nvme(NvmeSmart {
                    critical_warning: 0,
                    temperature_celsius: Some(42),
                    power_on_hours: Some(6950),
                    media_errors: Some(0),
                    unsafe_shutdowns: Some(212),
                    ..Default::default()
                })),
            }),
            error: None,
        }
    }

    fn sata_disk() -> DiskSmart {
        DiskSmart {
            disk_uri: "aio:///dev/disk/by-id/ata-disk".into(),
            supported: true,
            smart: Some(SmartData {
                smart_status_passed: Some(false),
                protocol: Some(Protocol::Ata(AtaSmart {
                    attributes: vec![SmartAttribute {
                        id: 5,
                        name: "Reallocated_Sector_Ct".into(),
                        value: 100,
                        worst: 100,
                        threshold: 10,
                        raw_value: 8,
                    }],
                })),
                ..Default::default()
            }),
            error: None,
        }
    }

    fn unsupported_disk() -> DiskSmart {
        DiskSmart {
            disk_uri: "aio:///var/local/blob".into(),
            supported: false,
            smart: None,
            error: Some("the operation is invalid for this bdev".into()),
        }
    }

    fn cache(disks: Vec<DiskSmart>) -> PoolsSmart {
        PoolsSmart {
            pools: vec![PoolSmart {
                name: "pool-1".into(),
                uuid: "uuid-1".into(),
                disks,
            }],
        }
    }

    /// Returns the value of the series of the metric with the given labels.
    fn value(families: &[MetricFamily], name: &str, labels: &[(&str, &str)]) -> Option<f64> {
        families
            .iter()
            .find(|family| family.get_name() == name)?
            .get_metric()
            .iter()
            .find(|metric| {
                labels.iter().all(|(key, value)| {
                    metric
                        .get_label()
                        .iter()
                        .any(|label| label.get_name() == *key && label.get_value() == *value)
                })
            })
            .map(|metric| metric.get_gauge().get_value())
    }

    #[test]
    fn nothing_is_exposed_without_smart_data() {
        let collector = SmartCollector::new(PoolsSmart::default(), Some("node-1".into()));
        assert!(collector.collect().is_empty());
    }

    #[test]
    fn nvme_disk_metrics() {
        let collector = SmartCollector::new(
            cache(vec![nvme_disk(), unsupported_disk()]),
            Some("node-1".into()),
        );
        let families = collector.collect();
        let disk = [
            ("node", "node-1"),
            ("name", "pool-1"),
            ("disk", "aio:///dev/disk/by-id/nvme-disk"),
        ];

        let expected = [
            ("diskpool_disk_smart_status_passed", 1.0),
            ("diskpool_disk_smart_temperature_celsius", 42.0),
            ("diskpool_disk_smart_power_on_hours", 6950.0),
            ("diskpool_disk_smart_power_cycles", 1526.0),
            ("diskpool_disk_smart_available_spare_percent", 100.0),
            (
                "diskpool_disk_smart_available_spare_threshold_percent",
                10.0,
            ),
            ("diskpool_disk_smart_percentage_used", 43.0),
            ("diskpool_disk_smart_nvme_temperature_celsius", 42.0),
            ("diskpool_disk_smart_nvme_critical_warning", 0.0),
            ("diskpool_disk_smart_nvme_media_errors", 0.0),
            ("diskpool_disk_smart_nvme_unsafe_shutdowns", 212.0),
        ];
        for (name, expected) in expected {
            assert_eq!(value(&families, name, &disk), Some(expected), "{name}");
        }
        assert_eq!(
            value(
                &families,
                "diskpool_disk_smart_info",
                &[
                    ("model", "Samsung SSD 970 EVO Plus 500GB"),
                    ("firmware", "2B2QEXM7"),
                    ("transport", "NVMe"),
                    ("type", "nvme")
                ],
            ),
            Some(1.0)
        );
        // The fields that the device did not report are absent, not zero.
        for name in [
            "diskpool_disk_smart_nvme_power_cycles",
            "diskpool_disk_smart_nvme_data_units_read",
            "diskpool_disk_smart_nvme_warning_temp_time_minutes",
        ] {
            assert_eq!(value(&families, name, &disk), None, "{name}");
        }
        // No ATA attributes for an NVMe disk.
        assert!(!families
            .iter()
            .any(|family| family.get_name().contains("ata_attribute")));
    }

    #[test]
    fn unsupported_disk_is_only_reported_as_unsupported() {
        let collector = SmartCollector::new(cache(vec![unsupported_disk()]), Some("node-1".into()));
        let families = collector.collect();
        let disk = [("disk", "aio:///var/local/blob")];
        assert_eq!(
            value(&families, "diskpool_disk_smart_supported", &disk),
            Some(0.0)
        );
        for family in &families {
            let name = family.get_name();
            if name != "diskpool_disk_smart_supported" {
                assert!(
                    value(&families, name, &disk).is_none(),
                    "{name} must not be set for an unsupported disk"
                );
            }
        }
    }

    #[test]
    fn sata_disk_metrics() {
        let collector = SmartCollector::new(cache(vec![sata_disk()]), Some("node-1".into()));
        let families = collector.collect();
        let attribute = [
            ("disk", "aio:///dev/disk/by-id/ata-disk"),
            ("attribute_id", "5"),
            ("attribute_name", "Reallocated_Sector_Ct"),
        ];
        assert_eq!(
            value(
                &families,
                "diskpool_disk_smart_status_passed",
                &attribute[..1]
            ),
            Some(0.0)
        );
        for (name, expected) in [
            ("diskpool_disk_smart_ata_attribute_value", 100.0),
            ("diskpool_disk_smart_ata_attribute_worst", 100.0),
            ("diskpool_disk_smart_ata_attribute_threshold", 10.0),
            ("diskpool_disk_smart_ata_attribute_raw_value", 8.0),
        ] {
            assert_eq!(value(&families, name, &attribute), Some(expected), "{name}");
        }
        // No NVMe metrics for a SATA disk.
        assert!(!families
            .iter()
            .any(|family| family.get_name().contains("_nvme_")));
    }
}
