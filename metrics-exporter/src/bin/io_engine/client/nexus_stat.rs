use super::ticks_to_time;
use serde::{Deserialize, Serialize};

/// This stores IoStat information of a nexus.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct NexusIoStat {
    name: String,
    bytes_read: u64,
    num_read_ops: u64,
    bytes_written: u64,
    num_write_ops: u64,
    read_latency_us: u64,
    write_latency_us: u64,
}

impl NexusIoStat {
    /// Get name of the nexus.
    pub(crate) fn name(&self) -> &String {
        &self.name
    }

    /// Get used bytes read of the nexus.
    pub(crate) fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Get total number of read ops performed by the nexus.
    pub(crate) fn num_read_ops(&self) -> u64 {
        self.num_read_ops
    }

    /// Get the total bytes written in bytes.
    pub(crate) fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Get total number of write ops performed by the nexus.
    pub(crate) fn num_write_ops(&self) -> u64 {
        self.num_write_ops
    }

    /// Get total read latency in usec on the nexus.
    pub(crate) fn read_latency_us(&self) -> u64 {
        self.read_latency_us
    }

    /// Get total write latency in usec on the nexus.
    pub(crate) fn write_latency_us(&self) -> u64 {
        self.write_latency_us
    }
}

/// Array of NexusIoStat objects.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct NexusIoStats {
    pub(crate) nexus_stats: Vec<NexusIoStat>,
}

impl From<rpc::v1::stats::IoStats> for NexusIoStat {
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
