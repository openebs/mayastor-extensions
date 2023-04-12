use crate::constant::release_version;

/// Warning to users before doing an upgrade.
pub const UPGRADE_WARNING: &str =  "\nVolumes which make use of a single volume replica instance will be unavailable for some time during upgrade.\nIt is recommended that you do not create new volumes which make use of only one volume replica.";

/// Warning to users before doing an upgrade.
pub const REBUILD_WARNING: &str =  "\nThe cluster is rebuilding replica of some volumes.\nPlease try after some time or skip this validation by retrying with '--skip-replica-rebuild` flag.";

/// Warning to users before doing an upgrade
pub const SINGLE_REPLICA_VOLUME_WARNING: &str =  "\nThe list below shows the single replica volumes in cluster.\nThese single replica volumes may not be accessible during upgrade.\nTo skip this validation, please retry  with '--skip-single-replica-volume-validation` flag.";

/// Warning to users before doing an upgrade.
pub const CORDONED_NODE_WARNING: &str =  "\nBelow are the list of cordoned nodes. Since there are some cordoned nodes, \nthe rebuild will happen on another nodes and the rebuild may get stuck forever. \nTo skip this validation, please retry  with '--skip-cordoned-node-validation` flag.";

/// Info about the control plane pods.
pub const CONTROL_PLANE_PODS_LIST: &str =
    "\nList of control plane pods which will be restarted during upgrade.";

/// Info about the data plane pods.
pub const DATA_PLANE_PODS_LIST: &str =
    "\nList of data plane pods which will be restarted during upgrade.";

/// Info about the data plane pods.
pub const DATA_PLANE_PODS_LIST_SKIP_RESTART: &str =
    "\nList of data plane pods which need to be manually restarted to reflect upgrade as --skip-data-plane-restart flag is passed during upgrade.";

/// Append the release name to k8s objects.
pub(crate) fn upgrade_dry_run_summary(message: &str) -> String {
    let tag = release_version();
    let version = tag.unwrap_or("develop".to_string());
    format!("{message} : {version}")
}

/// Info about the data plane pods.
pub const UPGRADE_DRY_RUN_SUMMARY: &str =
    "\nFinally the cluster deployment will be upgraded to version";

/// Info about successful start
pub const UPGRADE_JOB_STARTED: &str =
    "\nThe upgrade has started. You can see the recent upgrade status using 'get upgrade-status` command.";
