use lazy_static::lazy_static;
use std::collections::HashMap;

/// Defines the name of the core-agent service
pub(crate) const CORE_AGENT_SERVICE: &str = "agent-core";

/// Defines the name of the csi-controller service
pub(crate) const CSI_CONTROLLER_SERVICE: &str = "csi-controller";

/// Defines the name of the jaeger-operator service
pub(crate) const JAEGER_OPERATOR_SERVICE: &str = "jaeger-operator";

/// Defines the name of the jaeger service
pub(crate) const JAEGER_SERVICE: &str = "jaeger";

/// Defines the name of the pool-operator service
pub(crate) const POOL_OPERATOR_SERVICE: &str = "operator-diskpool";

/// Defines the name of the rest service
pub(crate) const REST_SERVICE: &str = "api-rest";

/// Defines the name of agent-ha-node
pub(crate) const AGENT_HA_NODE_SERVICE: &str = "agent-ha-node";

/// Defines the name of the csi node daemon service
pub(crate) const CSI_NODE_SERVICE: &str = "csi-node";

/// Defines the name of the etcd service
pub(crate) const ETCD_SERVICE: &str = "etcd";

/// Defines the name of the etcd service
pub(crate) const ETCD_PAGED_LIMIT: i64 = 1000;

/// Defines the name of mayastor service
pub(crate) const MAYASTOR_SERVICE: &str = "io-engine";

/// Defines the name of mayastor-io container(dataplane container)
pub(crate) const DATA_PLANE_CONTAINER_NAME: &str = "io-engine";

/// Defines the logging label(key-value pair) on services.
pub(crate) fn logging_label_selector() -> String {
    format!("{}=true", ::constants::loki_logging_key())
}

/// Defines the name of upgrade service
pub(crate) const UPGRADE_SERVICE: &str = "upgrade";

/// Defines the name of callhome service
pub(crate) const CALLHOME_SERVICE: &str = "obs-callhome";

/// Defines the name of nats services
pub(crate) const NATS_SERVICE: &str = "nats";

lazy_static! {
    /// List of resources fall under control plane services
    pub(crate) static ref CONTROL_PLANE_SERVICES: HashMap<&'static str, bool> =
        HashMap::from([
            (CORE_AGENT_SERVICE, true),
            (CSI_CONTROLLER_SERVICE, true),
            (JAEGER_OPERATOR_SERVICE, true),
            (JAEGER_SERVICE, true),
            (POOL_OPERATOR_SERVICE, true),
            (REST_SERVICE, true),
            (CSI_NODE_SERVICE, true),
            (ETCD_SERVICE, true),
            (AGENT_HA_NODE_SERVICE, true),
        ]);

    /// List of resources fall under data plane services
    pub(crate) static ref DATA_PLANE_SERVICES: HashMap<&'static str, bool> =
        HashMap::from([
            (MAYASTOR_SERVICE, true),
        ]);

    /// List of resources fall under upgrade services
    pub(crate) static ref UPGRADE_JOB_SERVICE: HashMap<&'static str, bool> =
    HashMap::from([
        (UPGRADE_SERVICE, true),
    ]);

    /// List of resources fall under callhome services
    pub(crate) static ref CALLHOME_JOB_SERVICE: HashMap<&'static str, bool> =
    HashMap::from([
        (CALLHOME_SERVICE, true),
    ]);

    /// List of resources fall under nats services
    pub(crate) static ref NATS_JOB_SERVICE: HashMap<&'static str, bool> =
    HashMap::from([
        (NATS_SERVICE, true),
    ]);

    /// Represents the list of services that requires hostname to collect logs
    pub(crate) static ref HOST_NAME_REQUIRED_SERVICES: HashMap<&'static str, bool> =
        HashMap::from([
            (MAYASTOR_SERVICE, true),
            (ETCD_SERVICE, true),
            (CSI_NODE_SERVICE, true),
            (AGENT_HA_NODE_SERVICE, true),
            (NATS_SERVICE, true),
        ]);
}
