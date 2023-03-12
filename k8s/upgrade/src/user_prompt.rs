/// Warning to users before doing an upgrade.
pub const UPGRADE_WARNING: &str =  "Volumes which make use of a single volume replica instance will be unavailable for some time during upgrade.\nIt is recommended that you do not create new volumes which make use of only one volume replica.";

/// Warning to users before doing an upgrade.
pub const REBUILD_WARNING: &str =  "The cluster is rebuilding replica of some volumes.\nPlease try after some time or skip this validation by retrying with '--ignore-rebuild` flag.";
