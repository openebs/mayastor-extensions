// macros to define mayastor related objects.
#[macro_export]
macro_rules! upgrade_group {
    () => {
        "mayastor"
    };
    ($s:literal) => {
        concat!(upgrade_group!(), "-", $s)
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
pub(crate) const LABEL: &str = "app";
/// Upgrade operator.
pub(crate) const UPGRADE_OPERATOR: &str = "upgrade-operator";
/// Service account name for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_SERVICE_ACCOUNT: &str =
    upgrade_group!("upgrade-operator-service-account");
/// Role constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE: &str = upgrade_group!("upgrade-operator-role");
/// Role binding constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING: &str =
    upgrade_group!("upgrade-operator-role-binding");
/// Deployment constant for upgrade operator.
pub(crate) const UPGRADE_CONTROLLER_DEPLOYMENT: &str =
    upgrade_group!("upgrade-operator-deployment");
/// Service name constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_SERVICE: &str = upgrade_group!("upgrade-operator-service");
/// Service port constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_SERVICE_PORT: i32 = 8080;
/// Service internal port constant for upgrade operator.
pub(crate) const UPGRADE_OPERATOR_INTERNAL_PORT: i32 = 8080;
/// Upgrade image tag
pub(crate) const UPGRADE_IMAGE: &str = "openebs/mayastor-operator-upgrade:develop";
/// The service port for upgrade operator.
pub const UPGRADE_OPERATOR_HTTP_PORT: &str = "http";
