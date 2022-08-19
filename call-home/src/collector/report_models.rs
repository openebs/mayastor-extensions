use openapi::models::Volume;
use serde::Deserialize;
use serde::Serialize;

/// Volumes contains volume count, min,max,mean and capacity percentiles
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Volumes {
    pub count: u64,
    pub min_size_in_bytes: u64,
    pub mean_size_in_bytes: u64,
    pub max_size_in_bytes: u64,
    pub capacity_percentiles_in_bytes: Percentiles,
}
impl Volumes {
    /// Return a volume object with default values
    pub(crate) fn default() -> Self {
        Self {
            count: 0,
            mean_size_in_bytes: 0,
            min_size_in_bytes: 0,
            max_size_in_bytes: 0,
            capacity_percentiles_in_bytes: Percentiles::default(),
        }
    }
    /// Receives a api_models::Volumes object and returns a new report_models::volume object by using the data provided
    pub(crate) fn new(volumes: openapi::models::Volumes) -> Self {
        let volumes_size_vector = get_volumes_size_vector(volumes.entries);
        if volumes_size_vector.len() > 0 {
            return Self {
                count: volumes_size_vector.len() as u64,
                max_size_in_bytes: get_max_value(volumes_size_vector.clone()),
                min_size_in_bytes: get_min_value(volumes_size_vector.clone()),
                mean_size_in_bytes: get_mean_value(volumes_size_vector.clone()),
                capacity_percentiles_in_bytes: Percentiles::new(volumes_size_vector),
            };
        }
        Self::default()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Pools {
    pub count: u64,
    pub max_size_in_bytes: u64,
    pub min_size_in_bytes: u64,
    pub mean_size_in_bytes: u64,
    pub capacity_percentiles_in_bytes: Percentiles,
}
impl Pools {
    /// Returns pools object with default values
    pub(crate) fn default() -> Self {
        Self {
            count: 0,
            max_size_in_bytes: 0,
            min_size_in_bytes: 0,
            mean_size_in_bytes: 0,
            capacity_percentiles_in_bytes: Percentiles::default(),
        }
    }
    /// Receives a vector of api_models::Pools and returns a new Reports::Pools object by using the data provided
    pub(crate) fn new(pools: Vec<openapi::models::Pool>) -> Self {
        let pools_size_vector = get_pools_size_vector(pools);
        if pools_size_vector.len() > 0 {
            return Self {
                count: pools_size_vector.len() as u64,
                max_size_in_bytes: get_max_value(pools_size_vector.clone()),
                min_size_in_bytes: get_min_value(pools_size_vector.clone()),
                mean_size_in_bytes: get_mean_value(pools_size_vector.clone()),
                capacity_percentiles_in_bytes: Percentiles::new(pools_size_vector),
            };
        }
        Self::default()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Replicas {
    count: u64,
    count_per_volume_percentiles: Percentiles,
}
impl Replicas {
    /// Returns a replica object with default values
    pub fn default() -> Self {
        Self {
            count: 0,
            count_per_volume_percentiles: Percentiles::default(),
        }
    }

    /// Receives a Option<api_models::Volumes> and replica_count and returns a new report_models::replica object by using the data provided
    pub fn new(replica_count: usize, volumes: Option<openapi::models::Volumes>) -> Self {
        let mut replicas = Self::default();
        match volumes {
            Some(volumes) => {
                let replicas_size_vector = get_replicas_size_vector(volumes.entries);
                if replicas_size_vector.len() > 0 {
                    replicas.count_per_volume_percentiles =
                        Percentiles::new(replicas_size_vector.clone());
                } else {
                    replicas.count_per_volume_percentiles = Percentiles::default();
                }
            }
            None => {}
        };
        replicas.count = replica_count as u64;
        replicas
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    control_plane_version: String,
}
impl Versions {
    pub(crate) fn default() -> Self {
        Self {
            control_plane_version: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Percentiles {
    #[serde(rename = "50%")]
    pub percentile_50: u64,
    #[serde(rename = "75%")]
    pub percentile_75: u64,
    #[serde(rename = "90%")]
    pub percentile_90: u64,
}

impl Percentiles {
    /// Returns Percentiles with default values
    pub(crate) fn default() -> Self {
        Self {
            percentile_50: 0,
            percentile_75: 0,
            percentile_90: 0,
        }
    }
    /// Receives  a Vec<u64> and returns Percentiles
    pub(crate) fn new(values: Vec<u64>) -> Self {
        Self {
            percentile_50: get_percentile(values.clone(), 50),
            percentile_75: get_percentile(values.clone(), 75),
            percentile_90: get_percentile(values, 90),
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub k8s_cluster_id: String,
    pub k8s_node_count: u8,
    pub product_name: String,
    pub product_version: String,
    pub deploy_namespace: String,
    pub storage_node_count: u8,
    pub pools: Pools,
    pub volumes: Volumes,
    pub replicas: Replicas,
    pub versions: Versions,
}
impl Report {
    pub(crate) fn new() -> Self {
        Self {
            k8s_cluster_id: String::new(),
            k8s_node_count: 0,
            product_name: String::new(),
            product_version: String::new(),
            deploy_namespace: String::new(),
            storage_node_count: 0,
            pools: Pools::default(),
            volumes: Volumes::default(),
            replicas: Replicas::default(),
            versions: Versions::default(),
        }
    }
}

/// Get maximum value from a vector
fn get_max_value(values: Vec<u64>) -> u64 {
    *values.iter().max().unwrap()
}
/// Get minimum value from a vector
fn get_min_value(values: Vec<u64>) -> u64 {
    *values.iter().min().unwrap()
}
/// Get mean of all values from a vector
fn get_mean_value(values: Vec<u64>) -> u64 {
    let mut sum = 0.0;
    for value in values.iter() {
        sum += *value as f64 / (values.len() as f64);
    }
    sum as u64
}

/// Get percentile value from a vector
fn get_percentile(mut values: Vec<u64>, percentile: usize) -> u64 {
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
}

/// Gets a vector containing volume sizes from Vec<VolumeStats>
fn get_volumes_size_vector(volumes: Vec<Volume>) -> Vec<u64> {
    let mut volume_size_vector = Vec::new();
    for volume in volumes.iter() {
        volume_size_vector.push(volume.spec.size);
    }
    volume_size_vector
}
/// Gets a vector containing replica sizes from Vec<api_models::VolumeStats>
fn get_replicas_size_vector(volumes: Vec<Volume>) -> Vec<u64> {
    let mut replicas_size_vector = Vec::new();
    for volume in volumes.iter() {
        replicas_size_vector.push(volume.spec.num_replicas as u64);
    }
    replicas_size_vector
}

/// Gets a vector containing pool sizes from Vec<api_models::Pools>
fn get_pools_size_vector(pools: Vec<openapi::models::Pool>) -> Vec<u64> {
    let mut pools_size_vector = Vec::new();
    for pool in pools.iter() {
        match &pool.state {
            Some(pool_state) => pools_size_vector.push(pool_state.capacity),
            None => {}
        };
    }
    pools_size_vector
}
