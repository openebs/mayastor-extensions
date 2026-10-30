# Monitoring Disk pools

## Metrics supported by exporter as of the current release are

| Metric name                | Metric type | Labels/tags | Metric unit | Description                                                                    |
|----------------------------| ----------- | ----------- | ----------- |--------------------------------------------------------------------------------|
| diskpool_total_size_bytes | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Total size of the pool                                                         |
| diskpool_used_size_bytes  | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Used size of the pool                                                          |
| diskpool_status           | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Status of the pool (0, 1, 2, 3, 4) = {"Unknown", "Online", "Degraded", "Faulted", "Suspected"} |

### Example of the above-mentioned metrics:

```
# HELP diskpool_status Status of the pool
# TYPE diskpool_status gauge
diskpool_status{name="pool-on-node-2-477115",node="node-2-477115"} 1
# HELP diskpool_total_size_bytes Total size of the pool in bytes
# TYPE diskpool_total_size_bytes gauge
diskpool_total_size_bytes{name="pool-on-node-2-477115",node="node-2-477115"} 10724835328
# HELP diskpool_used_size_bytes Used size of the pool in bytes
# TYPE diskpool_used_size_bytes gauge
diskpool_used_size_bytes{name="pool-on-node-2-477115",node="node-2-477115"} 1073741824
```

## Disk SMART metrics

The exporter exposes the SMART data of the disks that back the pools.

The SMART data is read from the io-engine on every scrape, like the other pool metrics.

All the metrics are gauges, with the labels `node`=&lt;pool_node&gt;, `name`=&lt;pool_id&gt; and `disk`=&lt;disk_uri&gt;. Some metrics have extra labels, as stated below. A metric is only exposed when the disk reports it, so a missing value is never exposed as `0`. Disks that SMART cannot be read from, for example file and malloc bdevs, only have `diskpool_disk_smart_supported` set to `0`.

### All device types

These metrics mean the same on NVMe, SATA and SAS disks, so use them for anything that has to work across device types.

| Metric name | Description |
| ----------- | ----------- |
| diskpool_disk_smart_supported | `1` if SMART can be read from the disk, `0` if not |
| diskpool_disk_smart_status_passed | Overall SMART self-assessment of the disk, `1` if passed and `0` if failed. Not exposed if the disk gives no verdict |
| diskpool_disk_smart_temperature_celsius | Temperature in degrees Celsius |
| diskpool_disk_smart_power_on_hours | Cumulative power-on hours |
| diskpool_disk_smart_power_cycles | Number of power cycles. SAS disks do not report it |
| diskpool_disk_smart_available_spare_percent | Remaining spare capacity as a percentage. NVMe disks, and SATA SSDs that report it |
| diskpool_disk_smart_available_spare_threshold_percent | Spare capacity percentage below which the disk flags a problem |
| diskpool_disk_smart_percentage_used | Estimated percentage of the rated write endurance used, may exceed 100 |
| diskpool_disk_smart_info | Always `1`. Extra labels: `model`, `firmware`, `transport` and `type` (`nvme`, `ata`, or empty) |

### NVMe disks

The first six metrics repeat the value of the metric with the same name above, without the `nvme_`. They are read together, so they are always equal. Alert on one of them, not both.

| Metric name | Description |
| ----------- | ----------- |
| diskpool_disk_smart_nvme_temperature_celsius | Composite temperature in degrees Celsius |
| diskpool_disk_smart_nvme_power_on_hours | Cumulative power-on hours |
| diskpool_disk_smart_nvme_power_cycles | Number of power cycles |
| diskpool_disk_smart_nvme_available_spare_percent | Remaining spare capacity as a percentage |
| diskpool_disk_smart_nvme_available_spare_threshold_percent | Spare capacity percentage below which the disk flags a problem |
| diskpool_disk_smart_nvme_percentage_used | Estimated percentage of the life of the disk used, may exceed 100 |
| diskpool_disk_smart_nvme_critical_warning | Raw NVMe critical warning flags, `0` if there is none |
| diskpool_disk_smart_nvme_media_errors | Number of media and data integrity errors |
| diskpool_disk_smart_nvme_unsafe_shutdowns | Number of unsafe shutdowns |
| diskpool_disk_smart_nvme_error_log_entries | Number of error information log entries |
| diskpool_disk_smart_nvme_warning_temp_time_minutes | Minutes at or above the warning composite temperature. Needs smartctl 7.5 or newer |
| diskpool_disk_smart_nvme_critical_comp_time_minutes | Minutes at or above the critical composite temperature. Needs smartctl 7.5 or newer |
| diskpool_disk_smart_nvme_data_units_read | Number of data units read, a data unit is 512000 bytes |
| diskpool_disk_smart_nvme_data_units_written | Number of data units written, a data unit is 512000 bytes |
| diskpool_disk_smart_nvme_host_reads | Number of host read commands completed |
| diskpool_disk_smart_nvme_host_writes | Number of host write commands completed |
| diskpool_disk_smart_nvme_controller_busy_minutes | Minutes the controller was busy with I/O |

### SATA disks

There is one series per attribute in the SMART attribute table of the disk. These metrics have the extra labels `attribute_id` and `attribute_name`. An attribute is failing when its value is at or below its threshold. The raw value holds the counters, for example attribute 5 (reallocated sectors), and 187 and 198 (uncorrectable errors).

| Metric name | Description |
| ----------- | ----------- |
| diskpool_disk_smart_ata_attribute_value | Current normalized value of the attribute |
| diskpool_disk_smart_ata_attribute_worst | Worst normalized value recorded for the attribute |
| diskpool_disk_smart_ata_attribute_threshold | Failure threshold of the attribute |
| diskpool_disk_smart_ata_attribute_raw_value | Raw value of the attribute |

### Example of the above-mentioned metrics:

```
# HELP diskpool_disk_smart_temperature_celsius Temperature of the disk in degrees Celsius
# TYPE diskpool_disk_smart_temperature_celsius gauge
diskpool_disk_smart_temperature_celsius{disk="aio:///dev/disk/by-id/nvme-Samsung_SSD_970_EVO_Plus_500GB_S58SNM0000000X",name="pool-1",node="node-1"} 42
# HELP diskpool_disk_smart_info Identity of the disk, always 1
# TYPE diskpool_disk_smart_info gauge
diskpool_disk_smart_info{disk="aio:///dev/disk/by-id/nvme-Samsung_SSD_970_EVO_Plus_500GB_S58SNM0000000X",firmware="2B2QEXM7",model="Samsung SSD 970 EVO Plus 500GB",name="pool-1",node="node-1",transport="NVMe",type="nvme"} 1
# HELP diskpool_disk_smart_supported Indicates if SMART data can be read from the disk (0/1)
# TYPE diskpool_disk_smart_supported gauge
diskpool_disk_smart_supported{disk="aio:///var/local/openebs/blob?size=100GiB",name="pool-1",node="node-1"} 0
```
