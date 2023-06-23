use openapi::models::Volume;
use serde::{Deserialize, Serialize};

/// Volumes contains volume count, min, max, mean and capacity percentiles.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Volumes {
    count: u64,
    min_size_in_bytes: u64,
    mean_size_in_bytes: u64,
    max_size_in_bytes: u64,
    capacity_percentiles_in_bytes: Percentiles,
}
impl Volumes {
    /// Receives a openapi::models::Volumes object and returns a new report_models::volume object by
    /// using the data provided.
    pub(crate) fn new(volumes: openapi::models::Volumes) -> Self {
        let volumes_size_vector = get_volumes_size_vector(volumes.entries);
        Self {
            count: volumes_size_vector.len() as u64,
            max_size_in_bytes: get_max_value(volumes_size_vector.clone()),
            min_size_in_bytes: get_min_value(volumes_size_vector.clone()),
            mean_size_in_bytes: get_mean_value(volumes_size_vector.clone()),
            capacity_percentiles_in_bytes: Percentiles::new(volumes_size_vector),
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
}
impl Pools {
    /// Receives a vector of openapi::models::Pool and returns a new report_models::Pools object by
    /// using the data provided.
    pub(crate) fn new(pools: Vec<openapi::models::Pool>) -> Self {
        let pools_size_vector = get_pools_size_vector(pools);
        Self {
            count: pools_size_vector.len() as u64,
            max_size_in_bytes: get_max_value(pools_size_vector.clone()),
            min_size_in_bytes: get_min_value(pools_size_vector.clone()),
            mean_size_in_bytes: get_mean_value(pools_size_vector.clone()),
            capacity_percentiles_in_bytes: Percentiles::new(pools_size_vector),
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
