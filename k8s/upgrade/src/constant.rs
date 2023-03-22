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

/// Append the release name to k8s objects.
pub(crate) fn upgrade_group(release_name: &str, name: &str) -> String {
    format!("{release_name}-{name}")
}

/// label used for upgrade operator.
pub(crate) const APP: &str = "app.kubernetes.io/component";
/// label used for upgrade operator.
pub(crate) const LABEL: &str = "app";
/// Upgrade operator.
pub(crate) const UPGRADE_OPERATOR: &str = "operator-upgrade";

/// Upgrade Job.
pub(crate) const UPGRADE_JOB: &str = "upgrade-job";
/// Service account name for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_SERVICE_ACCOUNT: &str = "operator-upgrade-service-account";
/// Role constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE: &str = "operator-upgrade-role";
/// Role binding constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING: &str = "operator-upgrade-role-binding";
///Job constant for upgrade.
pub(crate) const UPGRADE_CONTROLLER_JOB: &str = "upgrade-job";
/// Pod constant for upgrade job.
pub(crate) const UPGRADE_CONTROLLER_JOB_POD: &str = "upgrade-pod";
/// This is the upgrade-operator container image.
pub(crate) const UPGRADE_IMAGE: &str = "openebs/mayastor-operator-upgrade:develop";
/// The service port for upgrade operator.
pub const UPGRADE_OPERATOR_HTTP_PORT: &str = "http";
/// Defines the Label select for mayastor
pub(crate) const API_REST_LABEL_SELECTOR: &str = "app=api-rest";
/// Defines the default release name
pub(crate) const DEFAULT_RELEASE_NAME: &str = "mayastor";
/// Volumes with one replica
pub(crate) const SINGLE_REPLICA_VOLUME: u8 = 1;
