/// Warning to users before doing an upgrade.
pub const UPGRADE_WARNING: &str =  "\nVolumes which make use of a single volume replica instance will be unavailable for some time during upgrade.\nIt is recommended that you do not create new volumes which make use of only one volume replica.";

/// Warning to users before doing an upgrade.
pub const REBUILD_WARNING: &str =  "\nThe cluster is rebuilding replica of some volumes.\nPlease try after some time or skip this validation by retrying with '--ignore-rebuild` flag.";

/// Warning to users before doing an upgrade
pub const SINGLE_REPLICA_VOLUME_WARNING: &str =  "\nThe list below shows the single replica volumes in cluster.\nThese single replica volumes may not be accessible during upgrade.\nTo skip this validation, please retry  with '--skip-single-replica-volume-validation` flag.";

/// Warning to users before doing an upgrade.
pub const CORDONED_NODE_WARNING: &str =  "\nBelow are the list of cordoned nodes. Since there are some cordoned nodes, \nthe rebuild will happen on another nodes so please have enough available free space or else the rebuild will get stuck forever.";
