pub mod plugin;

/// Module for mayastor upgrade.
pub use plugin::upgrade;

/// Validations before applying upgrade.
pub use plugin::preflight_validations;

/// Module for plugin constant.
pub use plugin::constants;

/// Module for upgrade client errors.
pub use plugin::error;

/// Contains libraries for error handling, interacting with k8s, etc.
pub mod common;
/// Contains APIs for publishing progress on to kubernetes Events.
pub mod events;
/// Contains APIs for interacting with helm releases and registries.
pub mod helm;
/// Contains the data-plane upgrade logic.
pub mod upgrade_data_plane;
/// Tools to validate upgrade path.
pub mod upgrade_path;
/// Contains upgrade utilities.
pub(crate) mod upgrade_utils;
