use crate::upgrade::k8s::crd::SEMVER_RE;
use chrono::Utc;
use kube::CustomResource;
use schemars::JsonSchema;
pub use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use validator::Validate;

#[derive(
    CustomResource,
    Serialize,
    Deserialize,
    Debug,
    Default,
    Eq,
    PartialEq,
    Clone,
    Validate,
    JsonSchema,
)]
#[kube(
    group = "openebs.io",
    version = "v0",
    kind = "UpgradeAction",
    singular = "upgradeaction",
    plural = "upgradeactions",
    namespaced,
    status = "UpgradeActionStatus",
    derive = "Default",
    derive = "PartialEq",
    shortname = "ua",
    printcolumn = r#"{"name":"Components state", "type":"string", "jsonPath":".status.components_state"}"#,
    printcolumn = r#"{"name":"Target Version", "type":"string", "jsonPath":".spec.target_version"}"#,
    printcolumn = r#"{"name":"Current Version", "type":"string", "jsonPath":".spec.current_version"}"#
)]

pub struct UpgradeActionSpec {
    /// Records the current version of the product
    #[validate(required, regex = "SEMVER_RE")]
    current_version: Option<String>,
    /// Records the desired version of the product
    #[validate(required, regex = "SEMVER_RE")]
    target_version: Option<String>,
    /// Records all the components present for the product
    #[validate(required)]
    components: Option<HashMap<String, Vec<String>>>,
}

impl UpgradeActionSpec {
    pub fn new(
        current_version: Option<Version>,
        target_version: Option<Version>,
        componens: Option<HashMap<String, Vec<String>>>,
    ) -> Self {
        UpgradeActionSpec {
            current_version: current_version.map(|v| v.to_string()),
            target_version: target_version.map(|v| v.to_string()),
            components: componens,
        }
    }

    /// Returns the current version of the product
    pub fn current_version(&self) -> Option<Version> {
        self.current_version
            .as_ref()
            .map(|v| Version::from_str(v).unwrap())
    }

    /// Returns the desired version of the product
    pub fn target_version(&self) -> Option<Version> {
        self.target_version
            .as_ref()
            .map(|v| Version::from_str(v).unwrap())
    }

    /// Returns all the components
    pub fn components(&self) -> Option<HashMap<String, Vec<String>>> {
        self.components.clone()
    }
}

/// 'UpgradePhase' defines the status of each components
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub enum UpgradePhase {
    Waiting,

    Updating,

    Verifying,

    Completed,

    Error,
}

impl Default for UpgradePhase {
    fn default() -> Self {
        UpgradePhase::Waiting
    }
}

/// converts the Upgrade phase into a string
impl ToString for UpgradePhase {
    fn to_string(&self) -> String {
        match self {
            UpgradePhase::Waiting => "Waiting",
            UpgradePhase::Updating => "Updating",
            UpgradePhase::Verifying => "Verifying",
            UpgradePhase::Completed => "Completed",
            UpgradePhase::Error => "Error",
        }
        .to_string()
    }
}
/// Upgrade phase into a string
impl From<UpgradePhase> for String {
    fn from(u: UpgradePhase) -> Self {
        u.to_string()
    }
}

/// 'UpgradeState' defines the status of upgradeaction resource
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub enum UpgradeState {
    NotStarted,

    Updating,

    VerifyingUpdate,

    SuccessfullUpdate,

    Error,
}

impl Default for UpgradeState {
    fn default() -> Self {
        UpgradeState::NotStarted
    }
}

/// converts the Upgrade Condition Type into a string
impl ToString for UpgradeState {
    fn to_string(&self) -> String {
        match self {
            UpgradeState::NotStarted => "NotStarted",
            UpgradeState::Updating => "Updating",
            UpgradeState::VerifyingUpdate => "VerifyingUpdate",
            UpgradeState::SuccessfullUpdate => "SuccessfullUpdate",
            UpgradeState::Error => "Error",
        }
        .to_string()
    }
}
/// Upgrade Condition Type into a string
impl From<UpgradeState> for String {
    fn from(u: UpgradeState) -> Self {
        u.to_string()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq, JsonSchema)]
pub struct UpgradeActionStatus {
    /// UpgradeAction state
    state: Option<UpgradeState>,
    /// Last time the condition transit from one status to another.
    last_transition_time: String,
    /// Components State
    #[validate(required)]
    components_state: Option<HashMap<String, HashMap<String, UpgradePhase>>>,
}

/// 'UpgradeActionStatus' defines the current state of upgrade, the status is updated by the operator
impl UpgradeActionStatus {
    pub fn state(&self) -> Option<UpgradeState> {
        self.state.clone()
    }

    pub fn last_transition_time(&self) -> String {
        self.last_transition_time.clone()
    }

    /// Records components state
    pub fn components_state(&self) -> Option<HashMap<String, HashMap<String, UpgradePhase>>> {
        self.components_state.clone()
    }
}
