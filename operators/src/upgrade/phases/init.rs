use std::collections::HashMap;

use crate::upgrade::k8s::crd::v0::UpgradePhase;

/// Core components state.
pub struct ComponentsState {
    core: CoreComponentsState,

    supportability: SupportabilityComponentsState,

    tracing: TracingComponentsState,
}

impl ComponentsState {
    /// Core components state.
    pub(crate) fn core(&self) -> CoreComponentsState {
        self.core.clone()
    }

    /// Supportability components state.
    pub(crate) fn supportability(&self) -> SupportabilityComponentsState {
        self.supportability.clone()
    }

    /// tracing components state.
    pub(crate) fn tracing(&self) -> TracingComponentsState {
        self.tracing.clone()
    }

    /// Initialize state.
    pub(crate) fn with_state(upgrade_phase: UpgradePhase) -> Self {
        Self {
            core: CoreComponentsState::with_state(upgrade_phase.clone()),
            supportability: SupportabilityComponentsState::with_state(upgrade_phase.clone()),
            tracing: TracingComponentsState::with_state(upgrade_phase),
        }
    }

    /// Covert into hashmaps.
    pub(crate) fn convert_into_hash(&self) -> HashMap<String, HashMap<String, UpgradePhase>> {
        let mut h: HashMap<String, HashMap<String, UpgradePhase>> = HashMap::new();
        h.insert("core".to_string(), self.core().convert_into_hash());
        h.insert(
            "supportability".to_string(),
            self.supportability().convert_into_hash(),
        );
        h.insert("tracing".to_string(), self.tracing().convert_into_hash());
        h
    }
}

/// Core components state.
#[derive(Clone)]
pub struct CoreComponentsState {
    core_agent: UpgradePhase,
    rest_api: UpgradePhase,
    csi_controller: UpgradePhase,
    csi_node: UpgradePhase,
    io_agent: UpgradePhase,
    metrics_exporter_pool: UpgradePhase,
    disk_pool_operator: UpgradePhase,
}

impl CoreComponentsState {
    /// Core agent state.
    pub(crate) fn core_agent(&self) -> UpgradePhase {
        self.core_agent.clone()
    }

    /// Rest api state.
    pub(crate) fn rest_api(&self) -> UpgradePhase {
        self.rest_api.clone()
    }

    /// Csi controller state.
    pub(crate) fn csi_controller(&self) -> UpgradePhase {
        self.csi_controller.clone()
    }

    /// Csi node state.
    pub(crate) fn csi_node(&self) -> UpgradePhase {
        self.csi_node.clone()
    }

    /// Io agent state.
    pub(crate) fn io_agent(&self) -> UpgradePhase {
        self.io_agent.clone()
    }

    /// Metrics exporter state.
    pub(crate) fn metrics_exporter_pool(&self) -> UpgradePhase {
        self.metrics_exporter_pool.clone()
    }

    /// Disk pool operator state.
    pub(crate) fn disk_pool_operator(&self) -> UpgradePhase {
        self.disk_pool_operator.clone()
    }

    /// Initialize core agent state.
    pub(crate) fn with_core_agent_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.core_agent = upgrade_phase;
        self
    }

    /// Initialize rest api state.
    pub(crate) fn with_rest_api_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.rest_api = upgrade_phase;
        self
    }

    /// Initialize csi controller state.
    pub(crate) fn with_csi_controller_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.csi_controller = upgrade_phase;
        self
    }

    /// Initialize csi node state.
    pub(crate) fn with_csi_node_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.csi_node = upgrade_phase;
        self
    }

    /// Initialize io agent state.
    pub(crate) fn with_io_agent_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.io_agent = upgrade_phase;
        self
    }

    /// Initialize metrics exporter state.
    pub(crate) fn with_metrics_exporter_pool_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.metrics_exporter_pool = upgrade_phase;
        self
    }

    /// Initialize disk pool operator state.
    pub(crate) fn with_disk_pool_operator_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.disk_pool_operator = upgrade_phase;
        self
    }

    /// Initialize core componentss state.
    pub(crate) fn with_state(upgrade_phase: UpgradePhase) -> Self {
        Self {
            core_agent: upgrade_phase.clone(),
            rest_api: upgrade_phase.clone(),
            csi_controller: upgrade_phase.clone(),
            csi_node: upgrade_phase.clone(),
            io_agent: upgrade_phase.clone(),
            metrics_exporter_pool: upgrade_phase.clone(),
            disk_pool_operator: upgrade_phase,
        }
    }

    /// Converts into hashmap.
    pub(crate) fn convert_into_hash(&self) -> HashMap<String, UpgradePhase> {
        let mut h: HashMap<String, UpgradePhase> = HashMap::new();
        h.insert("core_agent".to_string(), self.core_agent());
        h.insert("rest_api".to_string(), self.rest_api());
        h.insert("csi_controller".to_string(), self.csi_controller());
        h.insert("csi_node".to_string(), self.csi_node());
        h.insert(
            "metrics_exporter_pool".to_string(),
            self.metrics_exporter_pool(),
        );
        h.insert("disk_pool_operator".to_string(), self.disk_pool_operator());
        h
    }
}

/// Supportability components state.
#[derive(Clone)]
pub struct SupportabilityComponentsState {
    loki: UpgradePhase,
    promtail: UpgradePhase,
}

impl SupportabilityComponentsState {
    /// Loki state.
    pub(crate) fn loki(&self) -> UpgradePhase {
        self.loki.clone()
    }

    /// Promtail state.
    pub(crate) fn promtail(&self) -> UpgradePhase {
        self.promtail.clone()
    }

    /// Initialize loki state.
    pub(crate) fn with_loki_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.loki = upgrade_phase;
        self
    }

    /// Initialize promtail state
    pub(crate) fn with_promtail_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.promtail = upgrade_phase;
        self
    }

    /// Initialize supportability state.
    pub(crate) fn with_state(upgrade_phase: UpgradePhase) -> Self {
        Self {
            loki: upgrade_phase.clone(),
            promtail: upgrade_phase,
        }
    }

    /// Converts into hashmap.
    pub(crate) fn convert_into_hash(&self) -> HashMap<String, UpgradePhase> {
        let mut h: HashMap<String, UpgradePhase> = HashMap::new();
        h.insert("loki".to_string(), self.loki());
        h.insert("promtail".to_string(), self.promtail());
        h
    }
}

// Tracing components state.
#[derive(Clone)]
pub struct TracingComponentsState {
    jaeger: UpgradePhase,
}

impl TracingComponentsState {
    /// Initialize jaegar state.
    pub(crate) fn with_jaeger_phase(mut self, upgrade_phase: UpgradePhase) -> Self {
        self.jaeger = upgrade_phase;
        self
    }

    /// Jaegar state.
    pub(crate) fn jaeger(&self) -> UpgradePhase {
        self.jaeger.clone()
    }

    /// Initialize state.
    pub(crate) fn with_state(upgrade_phase: UpgradePhase) -> Self {
        Self {
            jaeger: upgrade_phase,
        }
    }

    /// Converts into hashmap.
    pub(crate) fn convert_into_hash(&self) -> HashMap<String, UpgradePhase> {
        let mut h: HashMap<String, UpgradePhase> = HashMap::new();
        h.insert("jaeger".to_string(), self.jaeger());
        h
    }
}
