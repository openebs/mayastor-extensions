use obs::common::{constants::ACTION, errors};
use openapi::models::Volume;
use prometheus_parse::{Sample, Value};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::convert::TryFrom;
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
}
impl Volumes {
    /// Receives a openapi::models::Volumes object and returns a new report_models::volume object by
    /// using the data provided.
    pub(crate) fn new(volumes: openapi::models::Volumes, event_data: EventData) -> Self {
        let volumes_size_vector = get_volumes_size_vector(volumes.entries);
        Self {
            count: volumes_size_vector.len() as u64,
            max_size_in_bytes: get_max_value(volumes_size_vector.clone()),
            min_size_in_bytes: get_min_value(volumes_size_vector.clone()),
            mean_size_in_bytes: get_mean_value(volumes_size_vector.clone()),
            capacity_percentiles_in_bytes: Percentiles::new(volumes_size_vector),
            created: event_data.volume_created.value(),
            deleted: event_data.volume_deleted.value(),
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
}
impl Pools {
    /// Receives a vector of openapi::models::Pool and returns a new report_models::Pools object by
    /// using the data provided.
    pub(crate) fn new(pools: Vec<openapi::models::Pool>, event_data: EventData) -> Self {
        let pools_size_vector = get_pools_size_vector(pools);
        Self {
            count: pools_size_vector.len() as u64,
            max_size_in_bytes: get_max_value(pools_size_vector.clone()),
            min_size_in_bytes: get_min_value(pools_size_vector.clone()),
            mean_size_in_bytes: get_mean_value(pools_size_vector.clone()),
            capacity_percentiles_in_bytes: Percentiles::new(pools_size_vector),
            created: event_data.pool_created.value(),
            deleted: event_data.pool_deleted.value(),
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
fn get_volumes_size_vector(volumes: Vec<Volume>) -> Vec<u64> {
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
fn get_pools_size_vector(pools: Vec<openapi::models::Pool>) -> Vec<u64> {
    let mut pools_size_vector = Vec::with_capacity(pools.len());
    for pool in pools.iter() {
        match &pool.state {
            Some(pool_state) => pools_size_vector.push(pool_state.capacity),
            None => {}
        };
    }
    pools_size_vector
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
