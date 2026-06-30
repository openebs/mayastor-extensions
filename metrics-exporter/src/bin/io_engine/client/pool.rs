use rpc::v1::pb::{PoolAlert, PoolAlertStatus};

/// This stores Capacity and state information of a pool.
#[derive(Debug, Clone)]
pub(crate) struct PoolInfo {
    name: String,
    used: u64,
    capacity: u64,
    state: u64,
    committed: u64,
    disk_capacity: u64,
    max_expandable_size: u64,
    io_error_count: u64,
    io_error_threshold: u64,
    io_stalled: bool,
    io_stall_transition_count: u64,
    io_stall_transition_threshold: u64,
    alert_status: PoolAlertStatus,
    notice: Vec<PoolAlert>,
    attention: Vec<PoolAlert>,
    warning: Vec<PoolAlert>,
    critical: Vec<PoolAlert>,
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

    /// Get the underlying disk capacity in bytes.
    pub(crate) fn disk_capacity(&self) -> u64 {
        self.disk_capacity
    }

    /// Get the max expandable size in bytes.
    pub(crate) fn max_expandable_size(&self) -> u64 {
        self.max_expandable_size
    }

    /// Get state of the Pool.
    pub(crate) fn state(&self) -> u64 {
        self.state
    }

    /// Get the count of IO errors on the pool.
    pub(crate) fn io_error_count(&self) -> u64 {
        self.io_error_count
    }

    /// Get the IO error threshold for the pool.
    pub(crate) fn io_error_threshold(&self) -> u64 {
        self.io_error_threshold
    }

    /// Get whether the pool is currently in stalled state.
    pub(crate) fn io_stalled(&self) -> bool {
        self.io_stalled
    }

    /// Get the count of IO stall transitions on the pool.
    pub(crate) fn io_stall_transition_count(&self) -> u64 {
        self.io_stall_transition_count
    }

    /// Get the IO stall transition threshold for the pool.
    pub(crate) fn io_stall_transition_threshold(&self) -> u64 {
        self.io_stall_transition_threshold
    }

    /// Get the alert status for the pool.
    pub(crate) fn alert_status(&self) -> PoolAlertStatus {
        self.alert_status
    }

    /// Get the collection of notice alert reasons for the pool.
    pub(crate) fn notice(&self) -> &Vec<PoolAlert> {
        &self.notice
    }

    /// Get the collection of attention alert reasons for the pool.
    pub(crate) fn attention(&self) -> &Vec<PoolAlert> {
        &self.attention
    }

    /// Get the collection of warning alert reasons for the pool.
    pub(crate) fn warning(&self) -> &Vec<PoolAlert> {
        &self.warning
    }

    /// Get the collection of critical alert reasons for the pool.
    pub(crate) fn critical(&self) -> &Vec<PoolAlert> {
        &self.critical
    }
}

/// Array of PoolInfo objects.
#[derive(Debug, Clone, Default)]
pub(crate) struct Pools {
    pub(crate) pools: Vec<PoolInfo>,
}

impl From<rpc::v1::pool::Pool> for PoolInfo {
    fn from(value: rpc::v1::pool::Pool) -> Self {
        let mut io_error_count: u64 = 0;
        let mut io_error_threshold: u64 = 0;
        let mut io_stalled: bool = false;
        let mut io_stall_transition_count: u64 = 0;
        let mut io_stall_transition_threshold: u64 = 0;
        let mut alert_status: PoolAlertStatus = PoolAlertStatus::Healthy;
        let mut notice: Vec<PoolAlert> = Vec::new();
        let mut attention: Vec<PoolAlert> = Vec::new();
        let mut warning: Vec<PoolAlert> = Vec::new();
        let mut critical: Vec<PoolAlert> = Vec::new();
        if let Some(errors) = value.errors.clone() {
            io_error_count = errors.io_error_count;
            io_error_threshold = errors.io_error_threshold;
            io_stalled = errors.io_stalled;
            io_stall_transition_count = errors.io_stall_transition_count;
            io_stall_transition_threshold = errors.io_stall_transition_threshold;
            if let Some(alerts) = errors.alerts {
                alert_status = alerts.status();
                notice = alerts.notice().collect();
                attention = alerts.attention().collect();
                warning = alerts.warning().collect();
                critical = alerts.critical().collect();
            }
        }
        Self {
            name: value.name,
            used: value.used,
            capacity: value.capacity,
            state: value.state as u64,
            committed: value.committed,
            disk_capacity: value.disk_capacity,
            max_expandable_size: value.max_expandable_size.unwrap_or_default(),
            io_error_count,
            io_error_threshold,
            io_stalled,
            io_stall_transition_count,
            io_stall_transition_threshold,
            alert_status,
            notice,
            attention,
            warning,
            critical,
        }
    }
}
