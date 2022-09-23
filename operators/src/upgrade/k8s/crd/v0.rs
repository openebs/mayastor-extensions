use crate::upgrade::k8s::crd::SEMVER_RE;
use chrono::{DateTime, Utc};
use kube::CustomResource;
use schemars::JsonSchema;
pub use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};
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

/// Define spec for upgrade action.
pub struct UpgradeActionSpec {
    /// Records the current version of the product.
    #[validate(regex = "SEMVER_RE")]
    current_version: String,
    /// Records the desired version of the product.
    #[validate(regex = "SEMVER_RE")]
    target_version: String,
    /// Records all the components present for the product.
    components: HashMap<String, Vec<String>>,
}

impl UpgradeActionSpec {
    /// Create a new upgrade action spec.
    pub fn new(
        current_version: Version,
        target_version: Version,
        componens: HashMap<String, Vec<String>>,
    ) -> Self {
        UpgradeActionSpec {
            current_version: current_version.to_string(),
            target_version: target_version.to_string(),
            components: componens,
        }
    }

    /// Returns the current version of the product.
    pub fn current_version(&self) -> Version {
        Version::from_str(&self.current_version).unwrap()
    }

    /// Returns the desired version of the product.
    pub fn target_version(&self) -> Version {
        Version::from_str(&self.target_version).unwrap()
    }

    /// Returns all the components.
    pub fn components(&self) -> HashMap<String, Vec<String>> {
        self.components.clone()
    }
}

/// 'UpgradePhase' defines the status of each components.
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub enum UpgradePhase {
    /// Components in Waiting phase.
    Waiting,
    /// Components in Updating phase.
    Updating,
    /// Components in Verifying phase which comnes after updationg.
    Verifying,
    /// Components in Completed phase which denotes updateion is complete.
    Completed,
    /// Components in Error phase.
    Error,
}

impl Default for UpgradePhase {
    fn default() -> Self {
        UpgradePhase::Waiting
    }
}

/// converts the Upgrade phase into a string.
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
/// Upgrade phase into a string.
impl From<UpgradePhase> for String {
    fn from(u: UpgradePhase) -> Self {
        u.to_string()
    }
}

/// 'UpgradeState' defines the status of upgradeaction resource.
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub enum UpgradeState {
    /// Upgrade in NotStarted phase, which denotes cr is created.
    NotStarted,
    /// Upgrade in Updating phase, which denotes upgrade has been started.
    Updating,
    /// Upgrade in VerifyingUpdate phase, which denotes components has completed updating phase.
    VerifyingUpdate,
    /// Upgrade in SuccessfullUpdate phase, which denotes upgrade has been successfully verified.
    SuccessfullUpdate,
    /// Upgrade in Error state.
    Error,
}

impl Default for UpgradeState {
    fn default() -> Self {
        UpgradeState::NotStarted
    }
}

/// Converts the Upgrade Condition Type into a string.
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
/// Upgrade Condition Type into a string.
impl From<UpgradeState> for String {
    fn from(u: UpgradeState) -> Self {
        u.to_string()
    }
}

/// Upgrade action status.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq, JsonSchema)]
pub struct UpgradeActionStatus {
    /// UpgradeAction state.
    pub state: UpgradeState,
    /// Last time the condition transit from one status to another.
    last_transition_time: String,
    /// Components State.
    components_state: HashMap<String, HashMap<String, UpgradePhase>>,
}

/// 'UpgradeActionStatus' defines the current state of upgrade, the status is updated by the
/// operator.
impl UpgradeActionStatus {
    /// Current state of upgrade.
    pub fn state(&self) -> UpgradeState {
        self.state.clone()
    }

    /// Last transition time of upgrade.
    pub fn last_transition_time(&self) -> String {
        self.last_transition_time.clone()
    }

    /// Records components state.
    pub fn components_state(&self) -> HashMap<String, HashMap<String, UpgradePhase>> {
        self.components_state.clone()
    }

    pub fn new(
        state: UpgradeState,
        state_transition_timestamp: DateTime<Utc>,
        components_state: HashMap<String, HashMap<String, UpgradePhase>>,
    ) -> Self {
        let state_transition_timestamp = state_transition_timestamp.to_rfc3339();
        Self {
            state,
            last_transition_time: state_transition_timestamp,
            components_state,
        }
    }
}
