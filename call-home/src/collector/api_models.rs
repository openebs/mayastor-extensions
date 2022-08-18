use serde::Serialize;
use serde::Deserialize;

/// Volumes contains the volumes model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Volumes
{
    pub entries : Vec<VolumeStats>,
}

/// VolumeStats contains the volume stats model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeStats
{
    pub spec : VolumeSpec
}

/// VolumeSpec contains the volume spec model
#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct VolumeSpec {
    pub num_replicas : u64,
    pub size :	u64,
}

/// Pools contains the pools model
#[derive(Serialize, Deserialize, Debug)]
pub struct Pools
{
    pub state : PoolsState
}

/// PoolsState contains the pools state model
#[derive(Serialize, Deserialize, Debug)]
pub struct PoolsState
{
    pub capacity: u64,
}

/// Nodes contains the nodes model
#[derive(Serialize, Deserialize, Debug)]
pub struct Nodes
{
    pub id : String,
    pub spec: Option<NodeSpec>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
}

/// Replicas contains the replica model
#[derive(Serialize, Deserialize, Debug)]
pub struct Replicas
{
    pub node: String,
    pub pool: String,
    pub size: u64,
    pub thin: bool,
    pub uri: String,
}