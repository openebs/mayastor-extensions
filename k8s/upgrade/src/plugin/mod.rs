/// Module for k8s objects for upgrade.
pub(crate) mod objects;

/// Module for mayastor upgrade.
pub mod upgrade;

/// Validations before applying upgrade.
pub mod preflight_validations;

/// Module for user messages.
pub(crate) mod user_prompt;
