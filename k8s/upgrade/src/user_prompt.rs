use crate::constant::release_version;

/// Warning to users before doing an upgrade.
pub const UPGRADE_WARNING: &str =  "\nVolumes which make use of a single volume replica instance will be unavailable for some time during upgrade.\nIt is recommended that you do not create new volumes which make use of only one volume replica.";

/// Warning to users before doing an upgrade.
pub const REBUILD_WARNING: &str =  "\nThe cluster is rebuilding replica of some volumes.\nTo skip this validation please run after some time or re-run with '--skip-replica-rebuild` flag.";

/// Warning to users before doing an upgrade
pub const SINGLE_REPLICA_VOLUME_WARNING: &str =  "\nThe list below shows the single replica volumes in cluster.\nThese single replica volumes may not be accessible during upgrade.\nTo skip this validation, please re-run with '--skip-single-replica-volume-validation` flag.";

/// Warning to users before doing an upgrade.
pub const CORDONED_NODE_WARNING: &str =  "\nOne or more nodes in this cluster are in a Mayastor cordoned state.\nThis implies that the storage space of DiskPools on these nodes cannot be utilized for volume replica rebuilds.\nPlease ensure remaining storage nodes have enough available DiskPool space to accommodate volume replica rebuilds,\nthat get triggered during the upgrade process.\nTo skip this validation, please re-run with '--skip-cordoned-node-validation` flag.\nBelow is a list of the Mayastor cordoned nodes:";

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

/// Information about successful start of upgrade process.
pub const UPGRADE_JOB_STARTED: &str =
    "\nThe upgrade has started. You can see the recent upgrade status using 'get upgrade-status` command.";

/// Source and target version are same.
pub const SOURCE_TARGET_VERSION_SAME: &str =
    "\nThe upgrade cannot proceed as target version is same as source version.\nVersion:";

/// Info about the data plane pods.
pub const UPGRADE_PATH_NOT_VALID: &str = "\nThe upgrade path is not valid.";
