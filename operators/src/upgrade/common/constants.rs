#[macro_export]
macro_rules! upgrade_group {
    () => {
        "openebs.io"
    };
    ($s:literal) => {
        concat!(upgrade_group!(), "/", $s)
    };
}

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

pub(crate) const APP: &str = "app.kubernetes.io/component";
pub(crate) const LABEL: &str = upgrade_group!("component");

pub(crate) const UPGRADE_OPERATOR: &str = "upgrade-operator";
pub(crate) const UPGRADE_OPERATOR_SERVICE_ACCOUNT: &str = "upgrade-operator-service-account";
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE: &str = "upgrade-operator-role";
pub(crate) const UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING: &str = "upgrade-operator-role-binding";
pub(crate) const UPGRADE_CONTROLLER_DEPLOYMENT: &str = "upgrade-operator-deployment";
pub(crate) const UPGRADE_OPERATOR_SERVICE: &str = "upgrade-operator-service";
pub(crate) const UPGRADE_OPERATOR_SERVICE_PORT: i32 = 8080;
pub(crate) const UPGRADE_OPERATOR_INTERNAL_PORT: i32 = 8080;
