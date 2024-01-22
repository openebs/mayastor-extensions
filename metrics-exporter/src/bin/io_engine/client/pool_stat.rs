use serde::{Deserialize, Serialize};

/// This stores IoStat information of a pool.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct PoolIoStat {
    name: String,
    bytes_read: u64,
    num_read_ops: u64,
    bytes_written: u64,
    num_write_ops: u64,
    read_latency_us: u64,
    write_latency_us: u64,
}

impl PoolIoStat {
    /// Get name of the pool.
    pub(crate) fn name(&self) -> &String {
        &self.name
    }

    /// Get used bytes read of the pool.
    pub(crate) fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Get total number of read ops of the pool.
    pub(crate) fn num_read_ops(&self) -> u64 {
        self.num_read_ops
    }

    /// Get the total bytes written in bytes.
    pub(crate) fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Get total number of write ops of the pool.
    pub(crate) fn num_write_ops(&self) -> u64 {
        self.num_write_ops
    }

    /// Get total read latency in usec of the pool.
    pub(crate) fn read_latency(&self) -> u64 {
        self.read_latency_us
    }

    /// Get total write latency in usec of the pool.
    pub(crate) fn write_latency(&self) -> u64 {
        self.write_latency_us
    }
}

/// Array of PoolIoStat objects.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct PoolIoStats {
    pub(crate) pool_stats: Vec<PoolIoStat>,
}

impl From<rpc::v1::stats::IoStats> for PoolIoStat {
    fn from(value: rpc::v1::stats::IoStats) -> Self {
        Self {
            name: value.name,
            bytes_read: value.bytes_read,
            num_read_ops: value.num_read_ops,
            bytes_written: value.bytes_written,
            num_write_ops: value.num_write_ops,
            read_latency_us: ticks_to_time(value.read_latency_ticks, value.tick_rate),
            write_latency_us: ticks_to_time(value.write_latency_ticks, value.tick_rate),
        }
    }
}

fn ticks_to_time(tick: u64, tick_rate: u64) -> u64 {
    ((tick as u128 * 1000000) / tick_rate as u128) as u64
}
