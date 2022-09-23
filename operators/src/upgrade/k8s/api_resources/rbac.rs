use crate::{
    upgrade::common::constants::{
        APP, LABEL, UPGRADE_OPERATOR, UPGRADE_OPERATOR_CLUSTER_ROLE,
        UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING, UPGRADE_OPERATOR_SERVICE_ACCOUNT,
    },
    upgrade_labels,
};
use k8s_openapi::api::{
    core::v1::ServiceAccount,
    rbac::v1::{ClusterRole, ClusterRoleBinding, PolicyRule, RoleRef, Subject},
};
use kube::core::ObjectMeta;
use maplit::btreemap;

/// Defines the upgrade-operator service account.
pub(crate) fn upgrade_operator_service_account(namespace: Option<String>) -> ServiceAccount {
    ServiceAccount {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(UPGRADE_OPERATOR_SERVICE_ACCOUNT.to_string()),
            namespace,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Defines the upgrade-operator cluster role.
pub(crate) fn upgrade_operator_cluster_role(namespace: Option<String>) -> ClusterRole {
    ClusterRole {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(UPGRADE_OPERATOR_CLUSTER_ROLE.to_string()),
            namespace,
            ..Default::default()
        },
        rules: Some(vec![
            PolicyRule {
                api_groups: Some(vec!["apiextensions.k8s.io".to_string()]),
                resources: Some(vec!["customresourcedefinitions".to_string()]),
                verbs: vec!["create", "list"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["openebs.io".to_string()]),
                resources: Some(vec!["upgradeactions".to_string()]),
                verbs: vec!["get", "list", "watch", "update", "replace", "patch"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["openebs.io".to_string()]),
                resources: Some(vec!["upgradeactions/status".to_string()]),
                verbs: vec!["update", "patch"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["apps/v1".to_string()]),
                resources: Some(vec!["deployments".to_string()]),
                verbs: vec![
                    "create",
                    "delete",
                    "deletecollection",
                    "get",
                    "list",
                    "patch",
                    "update",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["".to_string()]),
                resources: Some(vec!["pods".to_string()]),
                verbs: vec!["get", "list", "watch", "delete"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["".to_string()]),
                resources: Some(vec!["nodes".to_string()]),
                verbs: vec!["get", "list", "patch"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["".to_string()]),
                resources: Some(vec!["secrets".to_string()]),
                verbs: vec!["get", "list"].iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
        ]),
        ..Default::default()
    }
}

/// Defines the upgrade-operator cluster role binding.
pub(crate) fn upgrade_operator_cluster_role_binding(
    namespace: Option<String>,
) -> ClusterRoleBinding {
    ClusterRoleBinding {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING.to_string()),
            namespace: namespace.clone(),
            ..Default::default()
        },
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".to_string(),
            kind: "ClusterRole".to_string(),
            name: UPGRADE_OPERATOR_CLUSTER_ROLE.to_string(),
        },
        subjects: Some(vec![Subject {
            kind: "ServiceAccount".to_string(),
            name: UPGRADE_OPERATOR_SERVICE_ACCOUNT.to_string(),
            namespace,
            ..Default::default()
        }]),
    }
}
