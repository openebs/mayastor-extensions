use crate::{
    constant::{
        upgrade_group, APP, LABEL, UPGRADE_CONTROLLER_JOB_POD, UPGRADE_JOB, UPGRADE_OPERATOR,
        UPGRADE_OPERATOR_CLUSTER_ROLE, UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING,
        UPGRADE_OPERATOR_SERVICE_ACCOUNT,
    },
    upgrade_labels,
};

use k8s_openapi::api::{
    batch::v1::{Job, JobSpec},
    core::v1::{Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, ServiceAccount},
    rbac::v1::{ClusterRole, ClusterRoleBinding, PolicyRule, RoleRef, Subject},
};

use kube::core::ObjectMeta;
use maplit::btreemap;
use openapi::apis::IntoVec;

/// Defines the upgrade-operator service account.
pub(crate) fn upgrade_operator_service_account(
    namespace: Option<String>,
    release_name: String,
) -> ServiceAccount {
    ServiceAccount {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(upgrade_group(
                &release_name,
                UPGRADE_OPERATOR_SERVICE_ACCOUNT,
            )),
            namespace,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Defines the upgrade-operator cluster role.
pub(crate) fn upgrade_operator_cluster_role(
    namespace: Option<String>,
    release_name: String,
) -> ClusterRole {
    ClusterRole {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(upgrade_group(&release_name, UPGRADE_OPERATOR_CLUSTER_ROLE)),
            namespace,
            ..Default::default()
        },
        rules: Some(vec![
            PolicyRule {
                api_groups: Some(vec!["apiextensions.k8s.io"].into_vec()),
                resources: Some(vec!["customresourcedefinitions"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["openebs.io"].into_vec()),
                resources: Some(vec!["upgradeactions"].into_vec()),
                verbs: vec![
                    "get", "create", "list", "watch", "update", "replace", "patch",
                ]
                .into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["openebs.io"].into_vec()),
                resources: Some(vec!["upgradeactions/status"].into_vec()),
                verbs: vec!["update", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["apps"].into_vec()),
                resources: Some(vec!["deployments"].into_vec()),
                verbs: vec!["create", "delete", "get", "list", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["apps"].into_vec()),
                resources: Some(vec!["statefulsets"].into_vec()),
                verbs: vec!["create", "delete", "get", "list", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["apps"].into_vec()),
                resources: Some(vec!["daemonsets"].into_vec()),
                verbs: vec!["create", "delete", "get", "list", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["apps"].into_vec()),
                resources: Some(vec!["replicasets"].into_vec()),
                verbs: vec!["create", "delete", "get", "list", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec![""].into_vec()),
                resources: Some(vec!["serviceaccounts"].into_vec()),
                verbs: vec!["create", "get", "list", "delete", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec![""].into_vec()),
                resources: Some(vec!["pods"].into_vec()),
                verbs: vec![
                    "create",
                    "get",
                    "list",
                    "delete",
                    "patch",
                    "deletecollection",
                ]
                .into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec![""].into_vec()),
                resources: Some(vec!["nodes"].into_vec()),
                verbs: vec!["get", "list"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec![""].into_vec()),
                resources: Some(vec!["secrets"].into_vec()),
                verbs: vec![
                    "get",
                    "list",
                    "watch",
                    "create",
                    "delete",
                    "deletecollection",
                    "patch",
                    "update",
                ]
                .into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["rbac.authorization.k8s.io"].into_vec()),
                resources: Some(vec!["roles"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["rbac.authorization.k8s.io"].into_vec()),
                resources: Some(vec!["rolebindings"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["rbac.authorization.k8s.io"].into_vec()),
                resources: Some(vec!["clusterroles"].into_vec()),
                verbs: vec![
                    "create", "list", "delete", "get", "patch", "escalate", "bind",
                ]
                .into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["rbac.authorization.k8s.io"].into_vec()),
                resources: Some(vec!["clusterrolebindings"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec![""].into_vec()),
                resources: Some(vec!["services"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["storage.k8s.io"].into_vec()),
                resources: Some(vec!["storageclasses"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec![""].into_vec()),
                resources: Some(vec!["configmaps"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["scheduling.k8s.io"].into_vec()),
                resources: Some(vec!["priorityclasses"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["policy"].into_vec()),
                resources: Some(vec!["poddisruptionbudgets"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
        ]),
        ..Default::default()
    }
}

/// Defines the upgrade-operator cluster role binding.
pub(crate) fn upgrade_operator_cluster_role_binding(
    namespace: Option<String>,
    release_name: String,
) -> ClusterRoleBinding {
    ClusterRoleBinding {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(upgrade_group(
                &release_name,
                UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING,
            )),
            namespace: namespace.clone(),
            ..Default::default()
        },
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".to_string(),
            kind: "ClusterRole".to_string(),
            name: upgrade_group(&release_name, UPGRADE_OPERATOR_CLUSTER_ROLE),
        },
        subjects: Some(vec![Subject {
            kind: "ServiceAccount".to_string(),
            name: upgrade_group(&release_name, UPGRADE_OPERATOR_SERVICE_ACCOUNT),
            namespace,
            ..Default::default()
        }]),
    }
}

pub(crate) fn upgrade_job(
    namespace: &str,
    upgrade_image: String,
    release_name: String,
    skip_data_plane_restart: bool,
) -> Job {
    let mut job_args: Vec<String> = vec![
        format!("--rest-endpoint=http://{release_name}-api-rest:8081"),
        format!("--namespace={namespace}"),
        format!("--release-name={release_name}"),
    ];
    if skip_data_plane_restart {
        job_args.push("--skip-data-plane-restart".to_string());
    }

    Job {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_JOB)),
            name: Some(upgrade_group(&release_name, UPGRADE_JOB)),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: Some(JobSpec {
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(
                        btreemap! { LABEL.to_string() => UPGRADE_CONTROLLER_JOB_POD.to_string()},
                    ),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    restart_policy: Some("OnFailure".to_string()),
                    containers: vec![Container {
                        args: Some(job_args),
                        image: Some(upgrade_image),
                        image_pull_policy: Some("Always".to_string()),
                        name: UPGRADE_JOB.to_string(),
                        command: Some(vec!["operator-upgrade".to_string()]),
                        ports: Some(vec![ContainerPort {
                            container_port: 8080,
                            name: Some("http".to_string()),
                            ..Default::default()
                        }]),
                        env: Some(vec![EnvVar {
                            name: "RUST_LOG".to_string(),
                            value: Some("info".to_string()),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }],
                    service_account_name: Some(upgrade_group(
                        &release_name,
                        UPGRADE_OPERATOR_SERVICE_ACCOUNT,
                    )),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}
