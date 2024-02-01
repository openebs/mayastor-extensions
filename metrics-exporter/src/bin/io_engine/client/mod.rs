/// Grpc client module.
pub(crate) mod grpc_client;
/// NexusIoStats module.
pub(crate) mod nexus_stat;
/// PoolInfo module.
pub(crate) mod pool;
/// PoolIoStats module
pub(crate) mod pool_stat;

/// Convert ticks to time in microseconds.
fn ticks_to_time(tick: u64, tick_rate: u64) -> u64 {
    ((tick as u128 * 1000000) / tick_rate as u128) as u64
}
