use serde::{Deserialize, Serialize};

/// This stores Capacity and state information of a pool.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct PoolInfo {
    name: String,
    used: u64,
    capacity: u64,
    state: u64,
    committed: u64,
}

impl PoolInfo {
    /// Get name of the pool.
    pub(crate) fn name(&self) -> &String {
        &self.name
    }

    /// Get used capacity of the pool.
    pub(crate) fn used(&self) -> u64 {
        self.used
    }

    /// Get total capacity of the pool.
    pub(crate) fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Get the pool commitment in bytes.
    pub(crate) fn committed(&self) -> u64 {
        self.committed
    }

    /// Get pool of the io_engine.
    pub(crate) fn state(&self) -> u64 {
        self.state
    }
}

/// Array of PoolInfo objects.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Pools {
    pub(crate) pools: Vec<PoolInfo>,
}

impl From<rpc::v1::pool::Pool> for PoolInfo {
    fn from(value: rpc::v1::pool::Pool) -> Self {
        Self {
            name: value.name,
            used: value.used,
            capacity: value.capacity,
            state: value.state as u64,
            committed: value.committed,
        }
    }
}
