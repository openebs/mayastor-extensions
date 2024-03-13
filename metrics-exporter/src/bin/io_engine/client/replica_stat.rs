use super::ticks_to_time;
use crate::error::ExporterError;
use serde::{Deserialize, Serialize};

/// This stores IoStat information of a replica.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct ReplicaIoStat {
    name: String,
    entity_id: String,
    bytes_read: u64,
    num_read_ops: u64,
    bytes_written: u64,
    num_write_ops: u64,
    read_latency_us: u64,
    write_latency_us: u64,
}

impl ReplicaIoStat {
    /// Get name of the replica.
    pub(crate) fn name(&self) -> &String {
        &self.name
    }

    /// Get used bytes read of the replica.
    pub(crate) fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Get total number of read ops performed by the replica.
    pub(crate) fn num_read_ops(&self) -> u64 {
        self.num_read_ops
    }

    /// Get the total bytes written in bytes.
    pub(crate) fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Get total number of write ops performed by the replica.
    pub(crate) fn num_write_ops(&self) -> u64 {
        self.num_write_ops
    }

    /// Get total read latency in usec on the replica.
    pub(crate) fn read_latency_us(&self) -> u64 {
        self.read_latency_us
    }

    /// Get total write latency in usec on the replica.
    pub(crate) fn write_latency_us(&self) -> u64 {
        self.write_latency_us
    }

    /// Get entity_id of the replica.
    pub(crate) fn entity_id(&self) -> String {
        self.entity_id.clone()
    }
}

/// Array of NexusIoStat objects.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct ReplicaIoStats {
    pub(crate) replica_stats: Vec<ReplicaIoStat>,
}

impl TryFrom<rpc::v1::stats::ReplicaIoStats> for ReplicaIoStat {
    type Error = ExporterError;

    fn try_from(value: rpc::v1::stats::ReplicaIoStats) -> Result<Self, Self::Error> {
        let stats = match value.stats {
            Some(stats) => stats,
            None => {
                return Err(ExporterError::GrpcResponseError(
                    "Stats is None for replica stats".to_string(),
                ))
            }
        };
        Ok(Self {
            name: stats.name,
            entity_id: match value.entity_id {
                Some(id) => id,
                None => {
                    return Err(ExporterError::GrpcResponseError(
                        "entity_id is not set for replica stat".to_string(),
                    ))
                }
            },
            bytes_read: stats.bytes_read,
            num_read_ops: stats.num_read_ops,
            bytes_written: stats.bytes_written,
            num_write_ops: stats.num_write_ops,
            read_latency_us: ticks_to_time(stats.read_latency_ticks, stats.tick_rate),
            write_latency_us: ticks_to_time(stats.write_latency_ticks, stats.tick_rate),
        })
    }
}
