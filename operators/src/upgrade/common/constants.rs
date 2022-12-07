use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

/// Upgrade operator.
pub(crate) const UPGRADE_OPERATOR: &str = "upgrade-operator";

/// Service internal port constant for upgrade operator.
pub const UPGRADE_OPERATOR_INTERNAL_PORT: u16 = 8080;

/// Label used with Mayastor storage nodes
pub(crate) const NODE_LABEL: &str = "openebs.io/engine=mayastor";

/// IO_ENGINE_POD_LABEL is the Kubernetes Pod label set on mayastor-io-engine Pods.
pub(crate) const IO_ENGINE_POD_LABEL: &str = "app=io-engine";

/// DEFAULT_VALUES_PATH is the default path to the values.yaml file that will will be generated from
/// the source with `helm get values`.
pub(crate) const DEFAULT_VALUES_PATH: &str = "./";

/// This is the finalizer that will be set on UpgradeAction resources.
pub(crate) const UPGRADE_ACTION_FINALIZER: &str = "openebs.io/upgrade-protection";

/// DEFAULT_CHART_DIR_PATH is the default path to the target helm chart.
const DEFAULT_CHART_DIR_PATH: &str = "./chart";
pub(crate) fn chart_dir_path() -> PathBuf {
    const KEY: &str = "CHART_DIR";
    match env::var(KEY) {
        Ok(input) => {
            let path = Path::new(&input);
            // Validate path.
            match path.exists() && path.is_dir() {
                true => path.to_path_buf(),
                false => {
                    panic!("validation failed for {} value \"{}\": path must exist and must be that of a directory", KEY, &input);
                }
            }
        }
        Err(_) => Path::new(DEFAULT_CHART_DIR_PATH).to_path_buf(),
    }
}

/// This contains the list of components which will be upgraded.
pub(crate) fn components() -> HashMap<String, Vec<String>> {
    let core_components: Vec<&str> = vec![
        "core_agent",
        "rest_api",
        "csi_controller",
        "csi_node",
        "ha_node",
        "io_engine",
        "metrics_exporter_pool",
        "disk_pool_operator",
    ];
    let supportability_components: Vec<&str> = vec!["loki", "promtail"];
    let tracing_components: Vec<&str> = vec!["jaegar"];
    let mut components: HashMap<String, Vec<String>> = HashMap::new();
    components.insert(
        "core_components".to_string(),
        core_components
            .into_iter()
            .map(ToString::to_string)
            .collect(),
    );
    components.insert(
        "supportability_components".to_string(),
        supportability_components
            .into_iter()
            .map(ToString::to_string)
            .collect(),
    );
    components.insert(
        "tracing_components".to_string(),
        tracing_components
            .into_iter()
            .map(ToString::to_string)
            .collect(),
    );
    components
}
