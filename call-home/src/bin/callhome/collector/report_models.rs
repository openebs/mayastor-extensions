use obs::common::{
    constants::{ACTION, BYTES_PER_SECTOR},
    errors,
};
use openapi::models::{BlockDevice, Volume, VolumeStatus};
use prometheus_parse::{Sample, Value};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
};
use url::Url;

/// Volumes contains volume count, min, max, mean and capacity percentiles.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Volumes {
    count: u64,
    min_size_in_bytes: u64,
    mean_size_in_bytes: u64,
    max_size_in_bytes: u64,
    capacity_percentiles_in_bytes: Percentiles,
    #[serde(skip_serializing_if = "is_zero")]
    created: u32,
    #[serde(skip_serializing_if = "is_zero")]
    deleted: u32,
    volume_replica_counts: VolumeReplicaCounts,
    volume_state_counts: VolumeStateCounts,
}
impl Volumes {
    /// Receives a openapi::models::Volumes object and returns a new report_models::volume object by
    /// using the data provided.
    pub(crate) fn new(volumes: openapi::models::Volumes, event_data: EventData) -> Self {
        let volumes_size_vector = get_volumes_size_vector(&volumes.entries);
        Self {
            count: volumes_size_vector.len() as u64,
            max_size_in_bytes: get_max_value(volumes_size_vector.clone()),
            min_size_in_bytes: get_min_value(volumes_size_vector.clone()),
            mean_size_in_bytes: get_mean_value(volumes_size_vector.clone()),
            capacity_percentiles_in_bytes: Percentiles::new(volumes_size_vector),
            created: event_data.volume_created.value(),
            deleted: event_data.volume_deleted.value(),
            volume_replica_counts: VolumeReplicaCounts::new(&volumes.entries),
            volume_state_counts: VolumeStateCounts::new(&volumes.entries),
        }
    }
}

// The count of volumes with a specific number of replicas.
#[derive(Serialize, Deserialize, Debug, Default)]
struct VolumeReplicaCounts {
    one_replica: u32,
    two_replicas: u32,
    three_replicas: u32,
    four_replicas: u32,
    five_or_more_replicas: u32,
}

impl VolumeReplicaCounts {
    // Receives a openapi::models::Volumes object and returns number of volumes with specific number
    // of replicas.
    fn new(volumes: &[Volume]) -> Self {
        Self {
            one_replica: volumes
                .iter()
                .filter(|vol| vol.spec.num_replicas == 1)
                .count() as u32,
            two_replicas: volumes
                .iter()
                .filter(|vol| vol.spec.num_replicas == 2)
                .count() as u32,
            three_replicas: volumes
                .iter()
                .filter(|vol| vol.spec.num_replicas == 3)
                .count() as u32,
            four_replicas: volumes
                .iter()
                .filter(|vol| vol.spec.num_replicas == 4)
                .count() as u32,
            five_or_more_replicas: volumes
                .iter()
                .filter(|vol| vol.spec.num_replicas >= 5)
                .count() as u32,
        }
    }
}

// The count of volumes with a specific state of the volume.
#[derive(Serialize, Deserialize, Debug, Default)]
struct VolumeStateCounts {
    unknown: u64,
    online: u64,
    degraded: u64,
    faulted: u64,
    shutdown: u64,
}

impl VolumeStateCounts {
    // Receives a openapi::models::Volumes object and returns number of volumes with specific state
    // of the volume.
    fn new(volumes: &[Volume]) -> Self {
        Self {
            unknown: volumes
                .iter()
                .filter(|vol| vol.state.status == VolumeStatus::Unknown)
                .count() as u64,
            online: volumes
                .iter()
                .filter(|vol| vol.state.status == VolumeStatus::Online)
                .count() as u64,
            degraded: volumes
                .iter()
                .filter(|vol| vol.state.status == VolumeStatus::Degraded)
                .count() as u64,
            faulted: volumes
                .iter()
                .filter(|vol| vol.state.status == VolumeStatus::Faulted)
                .count() as u64,
            shutdown: volumes
                .iter()
                .filter(|vol| vol.state.status == VolumeStatus::Shutdown)
                .count() as u64,
        }
    }
}

/// Pools contains pool count, min, max, mean and capacity percentiles.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Pools {
    count: u64,
    max_size_in_bytes: u64,
    min_size_in_bytes: u64,
    mean_size_in_bytes: u64,
    capacity_percentiles_in_bytes: Percentiles,
    #[serde(skip_serializing_if = "is_zero")]
    created: u32,
    #[serde(skip_serializing_if = "is_zero")]
    deleted: u32,
    total_capacity_in_bytes: u64,
}
impl Pools {
    /// Receives a vector of openapi::models::Pool and returns a new report_models::Pools object by
    /// using the data provided.
    pub(crate) fn new(pools: Vec<openapi::models::Pool>, event_data: EventData) -> Self {
        let pools_size_vector = get_pools_size_vector(&pools);
        Self {
            count: pools_size_vector.len() as u64,
            max_size_in_bytes: get_max_value(pools_size_vector.clone()),
            min_size_in_bytes: get_min_value(pools_size_vector.clone()),
            mean_size_in_bytes: get_mean_value(pools_size_vector.clone()),
            capacity_percentiles_in_bytes: Percentiles::new(pools_size_vector.clone()),
            created: event_data.pool_created.value(),
            deleted: event_data.pool_deleted.value(),
            total_capacity_in_bytes: pools_size_vector.iter().sum(),
        }
    }
}

/// Replicas contains replica count and count per volume percentiles.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Replicas {
    count: u64,
    count_per_volume_percentiles: Percentiles,
}
impl Replicas {
    /// Receives a Option<openapi::models::Volumes> and replica_count and returns a new
    /// report_models::replica object by using the data provided.
    pub(crate) fn new(replica_count: usize, volumes: Option<openapi::models::Volumes>) -> Self {
        let mut replicas = Self::default();
        if let Some(volumes) = volumes {
            let replicas_size_vector = get_replicas_size_vector(volumes.entries);
            replicas.count_per_volume_percentiles = Percentiles::new(replicas_size_vector);
        }
        replicas.count = replica_count as u64;
        replicas
    }
}

/// Nexus contains nexus created, deleted counts, and rebuild started and rebuild ended counts.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Nexus {
    #[serde(skip_serializing_if = "is_zero")]
    created: u32,
    #[serde(skip_serializing_if = "is_zero")]
    deleted: u32,
    #[serde(skip_serializing_if = "is_zero")]
    rebuild_started: u32,
    #[serde(skip_serializing_if = "is_zero")]
    rebuild_ended: u32,
}
impl Nexus {
    /// Returns nexus object using the event_data.
    pub(crate) fn new(event_data: EventData) -> Self {
        Self {
            created: event_data.nexus_created.value(),
            deleted: event_data.nexus_deleted.value(),
            rebuild_started: event_data.rebuild_started.value(),
            rebuild_ended: event_data.rebuild_ended.value(),
        }
    }
}

/// StorageMedia contains storage media devices count, total capacity of all the storage media
/// devices and disk_types which contains capacity and count of storage media for each disk_type.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageMedia {
    count: u64,
    total_capacity_in_bytes: u64,
    disk_types: HashMap<String, DiskTypeInfo>,
}

impl StorageMedia {
    /// Receives a vector of openapi::models::BlockDevice and returns a new
    /// report_models::StorageMedia object by using the data provided.
    pub(crate) fn new(disks: Vec<BlockDevice>) -> Self {
        let mut disk_types = HashMap::new();

        // obtain the details of each disk using the Block Device API.
        for device in disks.clone() {
            let disk_type = get_disk_type(Some(&device), None);

            // Get or insert DiskTypeInfo for the disk type
            let disk_type_info: &mut DiskTypeInfo =
                disk_types.entry(disk_type.clone()).or_default();

            // Increase capacity by the size of the device in bytes
            disk_type_info.capacity_in_bytes += device.size * BYTES_PER_SECTOR;

            // Increment the count of disks for the disk type
            disk_type_info.count += 1;
        }

        let total_capacity: u64 = disks.iter().map(|disk| disk.size).sum();
        Self {
            count: disks.len() as u64,
            total_capacity_in_bytes: total_capacity * BYTES_PER_SECTOR,
            disk_types,
        }
    }
}

/// StorageNodes contains per node data.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageNodes {
    nodes: HashMap<String, NodeInfo>,
}

impl StorageNodes {
    /// Receives a vector of openapi::models::Replica, a vector of openapi::models::Pool and returns
    /// a new report_models::StorageNodes object by using the data provided.
    pub(crate) fn new(
        replicas: Vec<openapi::models::Replica>,
        pools: Vec<openapi::models::Pool>,
        node_disks: HashMap<String, Vec<BlockDevice>>,
    ) -> Self {
        let mut storage_nodes = Self::default();

        // HashMap to keep track of counted node disks.
        let mut counted_node_disks = HashMap::new();

        // Iterate over each pool specification.
        for pool_spec in pools.into_iter().filter_map(|pool| pool.spec) {
            // Get or insert the node entry in storage_nodes.nodes for the pool's node
            let node_entry = storage_nodes
                .nodes
                .entry(pool_spec.node.clone())
                .or_insert(NodeInfo::default());

            // Create a new PoolInfo instance with pool id and replicas, and push it to the node's
            // pools
            let pool_info = PoolInfo::new(&pool_spec.id, &replicas);
            node_entry.pools.push(pool_info);

            // Retrieve the disks associated with the current pool's node.
            if let Some(disks) = node_disks.get(&pool_spec.node.clone()) {
                for pool_disk in pool_spec.clone().disks {
                    // Retrieve the block device associated with the pool disk.
                    if let Some(bdev) = get_bdev(disks, &pool_disk) {
                        // Determine the parent block device, if partition exists.
                        let parent_bdev_name = bdev.partition.map(|partition| partition.name);

                        // Count the disk if not already counted for the node.
                        if counted_node_disks
                            .entry(pool_spec.node.clone())
                            .or_insert(HashSet::new())
                            .insert(parent_bdev_name.unwrap_or(bdev.devname))
                        {
                            node_entry.mayastor_managed_disks_count += 1;
                        }
                    }
                }
            }
        }

        storage_nodes
    }
}

/// NodeInfo contains PoolInfo (replicas capacity and replicas count) for each pool in a node and
/// mayastor managed disks count for the node.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeInfo {
    pools: Vec<PoolInfo>,
    mayastor_managed_disks_count: u64,
}

/// PoolInfo contains total replicas capacity and replicas count in a pool.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PoolInfo {
    replicas: u64,
    replicas_capacity_in_bytes: u64,
}

impl PoolInfo {
    /// Receives pool_id and a vector of openapi::models::Replica and returns a new
    /// report_models::PoolInfo object by using the data provided.
    pub(crate) fn new(pool_id: &str, replicas: &[openapi::models::Replica]) -> Self {
        let pool_replicas = replicas.iter().filter(|replica| replica.pool == pool_id);
        Self {
            replicas: pool_replicas.clone().count() as u64,
            replicas_capacity_in_bytes: pool_replicas.map(|replica| replica.size).sum(),
        }
    }
}

/// MayastorManagedDisks contains capacity and count of mayastor managed disks for each disk_type.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MayastorManagedDisks {
    disk_types: HashMap<String, DiskTypeInfo>,
}

impl MayastorManagedDisks {
    /// Creates a new instance of `MayastorManagedDisks` based on the provided pools and node disks.
    pub(crate) fn new(
        pools: Vec<openapi::models::Pool>,
        node_disks: HashMap<String, Vec<BlockDevice>>,
    ) -> Self {
        let mut mayastor_disks = Self::default();

        // HashMap to keep track of counted node disks.
        let mut counted_node_disks = HashMap::new();

        // Iterate over each pool specification.
        for pool_state in pools.into_iter().filter_map(|pool| pool.state) {
            // Retrieve the disks associated with the current pool's node.
            if let Some(disks) = node_disks.get(&pool_state.node) {
                if let Some(pool_disk) = &pool_state.disks.get(0) {
                    // Retrieve the block device associated with the pool disk.
                    match get_bdev(disks, pool_disk) {
                        Some(bdev) => {
                            // Determine the parent block device, if partition exists.
                            let parent_bdev = bdev.partition.as_ref().and_then(|parent| {
                                disks
                                    .iter()
                                    .find(|bd| bd.devname.contains(&parent.name))
                                    .cloned()
                            });

                            // Determine the disk type based on the parent block device or current
                            // block device.
                            let disk_type = parent_bdev.as_ref().map_or_else(
                                || get_disk_type(Some(&bdev), None),
                                |bdev| get_disk_type(Some(bdev), None),
                            );

                            // Get or insert DiskTypeInfo for the disk type
                            let disk_type_info =
                                mayastor_disks.disk_types.entry(disk_type).or_default();

                            // Update the capacity and count for the disk type.
                            disk_type_info.capacity_in_bytes += pool_state.capacity;

                            // Count the disk if not already counted for the node.
                            if counted_node_disks
                                .entry(pool_state.node.clone())
                                .or_insert(HashSet::new())
                                .insert(parent_bdev.unwrap_or(bdev).devname)
                            {
                                disk_type_info.count += 1;
                            }
                        }
                        None => {
                            if let Some(scheme) = get_scheme(pool_disk) {
                                let disk_type = get_disk_type(None, Some(scheme));
                                // Get or insert DiskTypeInfo for the disk type
                                let disk_type_info =
                                    mayastor_disks.disk_types.entry(disk_type).or_default();

                                // Update the capacity and count for the disk type.
                                disk_type_info.capacity_in_bytes += pool_state.capacity;
                                disk_type_info.count += 1;
                            }
                        }
                    };
                }
            }
        }

        mayastor_disks
    }
}

// Retrieves the block device associated with the given pool disk from the provided list of block
// devices.
fn get_bdev(disks: &[BlockDevice], pool_disk: &str) -> Option<BlockDevice> {
    // Extracts the device path from the pool disk.
    let device_path = get_device_path(pool_disk);

    // Finds the block device that matches the device path or device name.
    disks
        .iter()
        .find(|bd| {
            bd.devlinks.contains(&device_path.to_string()) || bd.devname.contains(device_path)
        })
        .cloned()
}

// Extracts the device path from the given entry, stripping any protocol prefix and UUID suffix.
fn get_device_path(entry: &str) -> &str {
    // Removes the protocol prefix (if present) and splits at the first occurrence of '?' to discard
    // UUIDs.
    let without_protocol = entry.split("://").last().unwrap_or(entry);
    let without_uuid = without_protocol
        .split('?')
        .next()
        .unwrap_or(without_protocol);
    without_uuid
}

// Get the scheme from the disk uri.
fn get_scheme(disk: &str) -> Option<&str> {
    disk.split("://").next()
}

/// DiskTypeInfo contains total capacity and count for a disk type.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiskTypeInfo {
    count: u64,
    capacity_in_bytes: u64,
}

/// Versions will contain versions of different mayastor components.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Versions {
    control_plane_version: String,
}

/// Percentiles contains percentile value at 50%, 75% and 90%.
#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct Percentiles {
    #[serde(rename = "50%")]
    percentile_50: u64,
    #[serde(rename = "75%")]
    percentile_75: u64,
    #[serde(rename = "90%")]
    percentile_90: u64,
}

impl Percentiles {
    /// Receives a Vec<u64> and returns Percentiles object.
    pub(crate) fn new(values: Vec<u64>) -> Self {
        Self {
            percentile_50: get_percentile(values.clone(), 50),
            percentile_75: get_percentile(values.clone(), 75),
            percentile_90: get_percentile(values, 90),
        }
    }
}

/// Report contains all the values and objects that we want to include in JSON payload.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    pub(crate) k8s_cluster_id: String,
    pub(crate) k8s_node_count: u8,
    pub(crate) product_name: String,
    pub(crate) product_version: String,
    pub(crate) deploy_namespace: String,
    pub(crate) storage_node_count: u8,
    pub(crate) pools: Pools,
    pub(crate) volumes: Volumes,
    pub(crate) replicas: Replicas,
    pub(crate) nexus: Nexus,
    pub(crate) versions: Versions,
    pub(crate) storage_nodes: StorageNodes,
    pub(crate) mayastor_managed_disks: MayastorManagedDisks,
    pub(crate) storage_media: StorageMedia,
}

/// Get maximum value from a vector.
fn get_max_value(values: Vec<u64>) -> u64 {
    values.into_iter().max().unwrap_or_default()
}

/// Get minimum value from a vector.
fn get_min_value(values: Vec<u64>) -> u64 {
    values.into_iter().min().unwrap_or_default()
}

/// Get mean of all values from a vector.
fn get_mean_value(values: Vec<u64>) -> u64 {
    let mut sum = 0.0;
    for value in values.iter() {
        sum += *value as f64 / (values.len() as f64);
    }
    sum as u64
}

/// Get percentile value from a vector.
fn get_percentile(mut values: Vec<u64>, percentile: usize) -> u64 {
    if !values.is_empty() {
        values.sort();
        let index_as_f64 = (percentile as f64) * (values.len() - 1) as f64 / 100.0;
        let index = (percentile * (values.len() - 1)) / 100;

        if index_as_f64 - index as f64 > 0.0 {
            (values[index] as f64
                + (index_as_f64 - index as f64) * (values[index + 1] - values[index]) as f64)
                as u64
        } else {
            values[index]
        }
    } else {
        0
    }
}

/// Gets a vector containing volume sizes from Vec<Volume>.
fn get_volumes_size_vector(volumes: &[Volume]) -> Vec<u64> {
    let mut volume_size_vector = Vec::with_capacity(volumes.len());
    for volume in volumes.iter() {
        volume_size_vector.push(volume.spec.size);
    }
    volume_size_vector
}

/// Gets a vector containing replica sizes from Vec<Volume>.
fn get_replicas_size_vector(volumes: Vec<Volume>) -> Vec<u64> {
    let mut replicas_size_vector = Vec::with_capacity(volumes.len());
    for volume in volumes.iter() {
        replicas_size_vector.push(volume.spec.num_replicas as u64);
    }
    replicas_size_vector
}

/// Gets a vector containing pool sizes from Vec<openapi::models::Pool>.
fn get_pools_size_vector(pools: &Vec<openapi::models::Pool>) -> Vec<u64> {
    let mut pools_size_vector = Vec::with_capacity(pools.len());
    for pool in pools.iter() {
        match &pool.state {
            Some(pool_state) => pools_size_vector.push(pool_state.capacity),
            None => {}
        };
    }
    pools_size_vector
}

// Gets disk type based on BlockDevice data or a given scheme.
fn get_disk_type(device_option: Option<&BlockDevice>, scheme_option: Option<&str>) -> String {
    if let Some(device) = device_option {
        // Get device name
        let devname = &device.devname;

        // Determine disk type based on device name patterns
        let disk_type = if devname.starts_with("/dev/sr") {
            "Optical Drive"
        } else if devname.starts_with("/dev/fd") {
            "Floppy Disk"
        } else if devname.starts_with("/dev/md") {
            "RAID Device"
        } else if devname.starts_with("/dev/dm") || devname.starts_with("/dev/mapper") {
            "Device Mapper"
        } else if devname.starts_with("/dev/xvd") {
            "Amazon Disk"
        } else if devname.starts_with("/dev/mmcblk") {
            "Memory Card"
        } else if devname.starts_with("/dev/mtd") {
            "Embedded Storage"
        } else if devname.starts_with("/dev/tape") {
            "Magnetic Tape"
        } else if devname.starts_with("/dev/loop") {
            "Loop Device"
        } else if devname.starts_with("/dev/nbd") {
            "Network Block Device"
        } else if devname.starts_with("/dev/ram") {
            "RAM Disk"
        } else {
            // Determine disk type based on connection type and rotational property
            match (device.connection_type.as_str(), device.is_rotational) {
                ("usb", _) => "USB Disk",
                ("nvme", _) => "NVMe SSD",
                ("scsi", Some(true)) => "HDD",
                ("scsi", Some(false)) => "SSD",
                ("ata", Some(true)) => "HDD",
                ("ata", Some(false)) => "SSD",
                _ => "Unknown",
            }
        };
        return disk_type.to_string();
    }
    // Check if scheme information is provided
    if let Some(scheme) = scheme_option {
        if scheme == "pcie" {
            return "NVMe SSD".to_string();
        }
    }

    // Default to "Unknown" if no sufficient information is provided
    "Unknown".to_string()
}

/// EventData consists of pool and volume events.
#[derive(Debug, Default, Clone)]
pub(crate) struct EventData {
    pub(crate) volume_created: VolumeCreated,
    pub(crate) volume_deleted: VolumeDeleted,
    pub(crate) pool_created: PoolCreated,
    pub(crate) pool_deleted: PoolDeleted,
    pub(crate) nexus_created: NexusCreated,
    pub(crate) nexus_deleted: NexusDeleted,
    pub(crate) rebuild_started: RebuildStarted,
    pub(crate) rebuild_ended: RebuildEnded,
}

/// Record of events populated from prometheus.
#[derive(Debug, Default, Clone)]
pub struct EventsRecord {
    record: Vec<Sample>,
}

/// Implements methods for wrapper.
impl EventsRecord {
    pub(crate) fn new(data: Vec<Sample>) -> Self {
        Self { record: data }
    }
}

/// Fetch the events stats.
pub async fn event_stats(url: Url) -> errors::Result<EventsRecord> {
    match fetch_stats_with_timeout(url.clone()).await {
        Ok(response) => {
            let body = match response.text().await {
                Ok(text) => text,
                Err(_) => {
                    return errors::GetRsponseBodyFailure.fail();
                }
            };
            let lines: Vec<_> = body.lines().map(|s| Ok(s.to_owned())).collect();
            let metrics = prometheus_parse::Scrape::parse(lines.into_iter())
                .context(errors::PrometheusOutPutParseFailure)?;
            Ok(EventsRecord::new(metrics.samples))
        }
        Err(_) => errors::StatsFetchFailure.fail(),
    }
}

async fn fetch_stats_with_timeout(
    url: Url,
) -> Result<reqwest::Response, reqwest_middleware::Error> {
    new_client()
        .get(url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
}

fn new_client() -> ClientWithMiddleware {
    // Retry up to 20 times with increasing intervals between attempts.
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(20);
    let client_config = reqwest::Client::new();
    ClientBuilder::new(client_config)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

struct Record<'a>(&'a Sample);

impl Record<'_> {
    fn action(&self) -> Action {
        Action::from(self.0.labels.get(ACTION).unwrap_or(""))
    }
    fn resource(&self) -> Resource {
        Resource::from(self.0.metric.as_str())
    }
    fn counter_value(&self) -> u32 {
        match self.0.value {
            Value::Counter(v) => v as u32,
            _ => 0,
        }
    }
    fn try_counter(&self, resource: Resource, action: Action) -> Result<CounterValue, ()> {
        if self.resource() == resource && self.action() == action {
            return Ok(CounterValue(self.counter_value()));
        }
        Err(())
    }
}

/// Metrics Resource.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Resource {
    #[default]
    Unknown,
    Pool,
    Volume,
    Nexus,
}
/// Metrics action.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub(crate) enum Action {
    #[default]
    Unknown,
    Created,
    Deleted,
    RebuildStarted,
    RebuildEnded,
}

impl From<&str> for Resource {
    fn from(s: &str) -> Self {
        match s {
            "pool" => Resource::Pool,
            "volume" => Resource::Volume,
            _ => Resource::Unknown,
        }
    }
}
impl From<&str> for Action {
    fn from(s: &str) -> Self {
        match s {
            "created" => Action::Created,
            "deleted" => Action::Deleted,
            "rebuild_started" => Action::RebuildStarted,
            "rebuild_ended" => Action::RebuildEnded,
            _ => Action::Unknown,
        }
    }
}

macro_rules! make_counter {
    ($name:ident, $resource:expr, $action:expr) => {
        #[derive(Debug, Default, Clone)]
        pub struct $name(CounterValue);
        impl std::ops::Deref for $name {
            type Target = CounterValue;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl TryFrom<Record<'_>> for $name {
            type Error = ();
            fn try_from(record: Record<'_>) -> Result<Self, ()> {
                record.try_counter($resource, $action).map($name)
            }
        }
    };
}

/// A Counter represents a single numerical value that only increases.
#[derive(Debug, Default, Clone)]
pub struct CounterValue(u32);
impl CounterValue {
    /// Get the inner value.
    pub(crate) fn value(&self) -> u32 {
        self.0
    }
}

make_counter!(VolumeCreated, Resource::Volume, Action::Created);
make_counter!(VolumeDeleted, Resource::Volume, Action::Deleted);
make_counter!(PoolCreated, Resource::Pool, Action::Created);
make_counter!(PoolDeleted, Resource::Pool, Action::Deleted);
make_counter!(NexusCreated, Resource::Nexus, Action::Created);
make_counter!(NexusDeleted, Resource::Nexus, Action::Deleted);
make_counter!(RebuildStarted, Resource::Nexus, Action::RebuildStarted);
make_counter!(RebuildEnded, Resource::Nexus, Action::RebuildEnded);

impl<'a, T: TryFrom<Record<'a>>> From<&'a EventsRecord> for Option<T> {
    fn from(src: &'a EventsRecord) -> Option<T> {
        src.record.iter().find_map(|s| T::try_from(Record(s)).ok())
    }
}

// Define the `is_zero` function to determine if the field should be serialized.
fn is_zero(value: &u32) -> bool {
    value == &0
}
