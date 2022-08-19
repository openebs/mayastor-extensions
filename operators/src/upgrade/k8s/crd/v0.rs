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

/// 'UpgradeConditionType' defines the status of upgradeaction resource
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub enum UpgradeConditionType {
    NotStarted,

    Updating,

    VerifyingUpdate,

    SuccessfullUpdate,

    Error,
}

impl Default for UpgradeConditionType {
    fn default() -> Self {
        UpgradeConditionType::NotStarted
    }
}

/// converts the Upgrade Condition Type into a string
impl ToString for UpgradeConditionType {
    fn to_string(&self) -> String {
        match self {
            UpgradeConditionType::NotStarted => "NotStarted",
            UpgradeConditionType::Updating => "Updating",
            UpgradeConditionType::VerifyingUpdate => "VerifyingUpdate",
            UpgradeConditionType::SuccessfullUpdate => "SuccessfullUpdate",
            UpgradeConditionType::Error => "Error",
        }
        .to_string()
    }
}
/// Upgrade Condition Type into a string
impl From<UpgradeConditionType> for String {
    fn from(u: UpgradeConditionType) -> Self {
        u.to_string()
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
pub struct UpgradeCondition {
    /// Type of upgrade condition
    #[validate(required)]
    pub type_: Option<UpgradeConditionType>,

    /// Last time the condition transit from one status to another.
    pub last_transition_time: Option<String>,

    ///Human readable message indicating details about last transition.
    pub message: Option<String>,

    ///(one line) reason for the condition's last transition.
    pub reason: Option<String>,

    /// Status of the cpondition, one of True, False, Unknown.
    pub status: Option<String>,
}

impl Default for UpgradeCondition {
    fn default() -> Self {
        let current_time = Some(Utc::now());
        let state_transition_timestamp = current_time.map(|ts| ts.to_rfc3339());
        Self {
            type_: Some(Default::default()),
            last_transition_time: state_transition_timestamp,
            status: Some("True".to_string()),
            message: None,
            reason: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq, JsonSchema)]
pub struct UpgradeActionStatus {
    conditions: Option<Vec<UpgradeCondition>>,
    #[validate(required)]
    components_state: Option<HashMap<String, HashMap<String, UpgradePhase>>>,
}

// impl Default for UpgradeActionStatus{
//     fn default() -> Self {
//         Self {
//             conditions: Some(vec!(Default::default())),
//             components_state: Some(Default::default())
//         }
//     }
// }

/// 'UpgradeActionStatus' defines the current state of upgrade, the status is updated by the operator
impl UpgradeActionStatus {
    /// Records the status
    pub fn conditions(&self) -> Option<Vec<UpgradeCondition>> {
        self.conditions.clone()
    }

    /// Recors components state
    pub fn components_state(&self) -> Option<HashMap<String, HashMap<String, UpgradePhase>>> {
        self.components_state.clone()
    }
}
