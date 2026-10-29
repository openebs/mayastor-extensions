use rpc::v1::pool::PoolSmart;

/// SMART data of all the pools on the io-engine.
#[derive(Debug, Clone, Default)]
pub(crate) struct PoolsSmart {
    /// SMART data of each pool, one entry per disk backing the pool.
    pub(crate) pools: Vec<PoolSmart>,
}
