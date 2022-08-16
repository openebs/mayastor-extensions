
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

pub const APP: &str = "app.kubernetes.io/component";
pub const LABEL: &str = upgrade_group!("component");

pub const UPGRADE:&str="upgrade";
pub const UPGRADE_OPERATOR:&str="upgrade-operator";
pub const UPGRADE_OPERATOR_SERVICE_ACCOUNT:&str="upgrade-operator-service-account";
pub const UPGRADE_OPERATOR_CLUSTER_ROLE:&str="upgrade-operator-role";
pub const UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING:&str="upgrade-operator-role-binding";
pub const UPGRADE_CONTROLLER_DEPLOYMENT:&str="upgrade-operator-deployment";
pub const UPGRADE_OPERATOR_SERVICE:&str="upgrade-operator-service";
pub const UPGRADE_OPERATOR_SERVICE_PORT:i32=8080;
pub const UPGRADE_OPERATOR_INTERNAL_PORT:i32=8080;
