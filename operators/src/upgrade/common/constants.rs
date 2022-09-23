use std::collections::HashMap;

/// macros to define group used in the upgrade crd.
#[macro_export]
macro_rules! upgrade_group {
    () => {
        "openebs.io"
    };
    ($s:literal) => {
        concat!(upgrade_group!(), "/", $s)
    };
}

/// macros to define labels for upgrade operator.
#[macro_export]
macro_rules! upgrade_labels {
    ($s:expr) => {
        btreemap! {
            APP => $s,
            LABEL => $s,
        }
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    };
}

/// label used for upgrade operator.
pub(crate) const APP: &str = "app.kubernetes.io/component";
/// label used for upgrade operator.
pub(crate) const LABEL: &str = upgrade_group!("component");

/// Upgrade operator.
pub(crate) const UPGRADE_OPERATOR: &str = "upgrade-operator";
/// Service account name for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_SERVICE_ACCOUNT: &str = "upgrade-operator-service-account";
/// Role constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE: &str = "upgrade-operator-role";
/// Role binding constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING: &str = "upgrade-operator-role-binding";
/// Deployment constant for upgrade operator.
pub(crate) const UPGRADE_CONTROLLER_DEPLOYMENT: &str = "upgrade-operator-deployment";
/// Service name constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_SERVICE: &str = "upgrade-operator-service";
/// Service port constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_SERVICE_PORT: i32 = 8080;
/// Service internal port constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_INTERNAL_PORT: i32 = 8080;

pub(crate) const NODE_LABEL: &str = "openebs.io/engine=mayastor";

pub(crate) const DEFAULT_VALUES_PATH: &str = "./";

pub(crate) fn components() -> HashMap<String, Vec<String>> {
    let core_components: Vec<String> = vec![
        "core_agent".to_string(),
        "rest_api".to_string(),
        "csi_controller".to_string(),
        "csi_node".to_string(),
        "io_agent".to_string(),
        "metrics_exporter_pool".to_string(),
        "disk_pool_operator".to_string(),
    ];
    let supportability_components: Vec<String> = vec!["loki".to_string(), "promtail".to_string()];
    let tracing_components: Vec<String> = vec!["jaegar".to_string()];
    let mut components: HashMap<String, Vec<String>> = HashMap::new();
    components.insert("core_components".to_string(), core_components);
    components.insert(
        "supportability_components".to_string(),
        supportability_components,
    );
    components.insert("tracing_components".to_string(), tracing_components);
    components
}
