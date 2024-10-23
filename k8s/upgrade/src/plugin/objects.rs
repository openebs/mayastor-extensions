use crate::{
    plugin::constants::{
        upgrade_job_container_name, upgrade_name_concat, UPGRADE_BINARY_NAME, UPGRADE_CONFIG_MAP,
        UPGRADE_CONFIG_MAP_MOUNT_PATH, UPGRADE_CONFIG_MAP_NAME_SUFFIX,
        UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX, UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX,
        UPGRADE_JOB_NAME_SUFFIX, UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
    },
    upgrade::UpgradeArgs,
    upgrade_labels,
};

use k8s_openapi::api::{
    batch::v1::{Job, JobSpec},
    core::v1::{
        ConfigMap, ConfigMapVolumeSource, Container, EnvVar, EnvVarSource, ExecAction,
        ObjectFieldSelector, PodSpec, PodTemplateSpec, Probe, ServiceAccount, Volume, VolumeMount,
    },
    rbac::v1::{ClusterRole, ClusterRoleBinding, PolicyRule, RoleRef, Subject},
};
use std::{collections::BTreeMap, env};

use kube::core::ObjectMeta;
use maplit::btreemap;
use openapi::apis::IntoVec;

/// Defines the upgrade job service account.
pub(crate) fn upgrade_job_service_account(
    namespace: Option<String>,
    service_account_name: String,
) -> ServiceAccount {
    ServiceAccount {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!()),
            name: Some(service_account_name),
            namespace,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Defines the upgrade job cluster role.
pub(crate) fn upgrade_job_cluster_role(
    namespace: Option<String>,
    cluster_role_name: String,
) -> ClusterRole {
    ClusterRole {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!()),
            name: Some(cluster_role_name),
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
                resources: Some(
                    vec![
                        "controllerrevisions",
                        "daemonsets",
                        "replicasets",
                        "statefulsets",
                        "deployments",
                    ]
                    .into_vec(),
                ),
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
                resources: Some(vec!["namespaces"].into_vec()),
                verbs: vec!["get"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["events.k8s.io"].into_vec()),
                resources: Some(vec!["events"].into_vec()),
                verbs: vec!["create"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec![""].into_vec()),
                resources: Some(
                    vec![
                        "secrets",
                        "persistentvolumes",
                        "persistentvolumeclaims",
                        "services",
                        "configmaps",
                    ]
                    .into_vec(),
                ),
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
                verbs: vec![
                    "create", "list", "delete", "get", "patch", "escalate", "bind",
                ]
                .into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["monitoring.coreos.com"].into_vec()),
                resources: Some(vec!["prometheusrules", "podmonitors"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["networking.k8s.io"].into_vec()),
                resources: Some(vec!["networkpolicies"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["batch"].into_vec()),
                resources: Some(vec!["cronjobs"].into_vec()),
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
                ..Default::default()
            },
            PolicyRule {
                api_groups: Some(vec!["jaegertracing.io"].into_vec()),
                resources: Some(vec!["jaegers"].into_vec()),
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
                api_groups: Some(vec!["storage.k8s.io"].into_vec()),
                resources: Some(vec!["storageclasses"].into_vec()),
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

/// Defines the upgrade job cluster role binding.
pub(crate) fn upgrade_job_cluster_role_binding(
    namespace: Option<String>,
    release_name: String,
) -> ClusterRoleBinding {
    ClusterRoleBinding {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!()),
            name: Some(upgrade_name_concat(
                &release_name,
                UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
            )),
            namespace: namespace.clone(),
            ..Default::default()
        },
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".to_string(),
            kind: "ClusterRole".to_string(),
            name: upgrade_name_concat(&release_name, UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX),
        },
        subjects: Some(vec![Subject {
            kind: "ServiceAccount".to_string(),
            name: upgrade_name_concat(&release_name, UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX),
            namespace,
            ..Default::default()
        }]),
    }
}

pub(crate) fn upgrade_configmap(
    data: BTreeMap<String, String>,
    namespace: &str,
    release_name: String,
) -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!()),
            name: Some(upgrade_name_concat(
                &release_name,
                UPGRADE_CONFIG_MAP_NAME_SUFFIX,
            )),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        data: Some(data),
        immutable: Some(true),
        ..Default::default()
    }
}

pub(crate) fn upgrade_job(
    namespace: &str,
    upgrade_image: String,
    release_name: String,
    args: &UpgradeArgs,
    set_file: String,
    image_pull_secrets: Option<Vec<k8s_openapi::api::core::v1::LocalObjectReference>>,
    image_pull_policy: Option<String>,
) -> Job {
    let helm_args_set = args.set.join(",");
    let mut job_args: Vec<String> = vec![
        format!("--rest-endpoint=http://{release_name}-api-rest:8081"),
        format!("--namespace={namespace}"),
        format!("--release-name={release_name}"),
        format!("--helm-args-set={helm_args_set}"),
        format!("--helm-args-set-file={set_file}"),
    ];
    if args.skip_data_plane_restart {
        job_args.push("--skip-data-plane-restart".to_string());
    }
    if args.skip_upgrade_path_validation_for_unsupported_version {
        job_args.push("--skip-upgrade-path-validation".to_string());
    }
    if args.reset_then_reuse_values {
        job_args.push("--helm-reset-then-reuse-values".to_string());
    }

    Job {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!()),
            name: Some(upgrade_name_concat(&release_name, UPGRADE_JOB_NAME_SUFFIX)),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: Some(JobSpec {
            // Backoff for unrecoverable errors, recoverable errors are handled by the Job process
            // Investigate backoff with `kubectl -n <namespace> logs job/<job-name>`.
            // Non-recoverable errors also often emit Job event, `kubectl mayastor get
            // upgrade-status` fetches the most recent Job event.
            backoff_limit: Some(6),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(upgrade_labels!()),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    image_pull_secrets,
                    restart_policy: Some("OnFailure".to_string()),
                    containers: vec![Container {
                        args: Some(job_args),
                        image: Some(upgrade_image),
                        image_pull_policy,
                        name: upgrade_job_container_name(),
                        env: Some(vec![
                            EnvVar {
                                name: "RUST_LOG".to_string(),
                                value: Some(env::var("RUST_LOG").unwrap_or("info".to_string())),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "POD_NAME".to_string(),
                                value_from: Some(EnvVarSource {
                                    field_ref: Some(ObjectFieldSelector {
                                        field_path: "metadata.name".to_string(),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            EnvVar {
                                // Ref: https://github.com/helm/helm/blob/main/cmd/helm/helm.go#L76
                                name: "HELM_DRIVER".to_string(),
                                value: Some(env::var("HELM_DRIVER").unwrap_or_default()),
                                ..Default::default()
                            },
                        ]),
                        liveness_probe: Some(Probe {
                            exec: Some(ExecAction {
                                command: Some(vec![
                                    "pgrep".to_string(),
                                    UPGRADE_BINARY_NAME.to_string(),
                                ]),
                            }),
                            initial_delay_seconds: Some(10),
                            period_seconds: Some(60),
                            ..Default::default()
                        }),
                        volume_mounts: Some(vec![VolumeMount {
                            read_only: Some(true),
                            mount_path: UPGRADE_CONFIG_MAP_MOUNT_PATH.to_string(),
                            name: UPGRADE_CONFIG_MAP.to_string(),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }],
                    service_account_name: Some(upgrade_name_concat(
                        &release_name,
                        UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
                    )),
                    volumes: Some(vec![Volume {
                        name: UPGRADE_CONFIG_MAP.to_string(),
                        config_map: Some(ConfigMapVolumeSource {
                            name: Some(upgrade_name_concat(
                                &release_name,
                                UPGRADE_CONFIG_MAP_NAME_SUFFIX,
                            )),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}
