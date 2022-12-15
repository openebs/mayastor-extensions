use crate::{
    constant::{
        upgrade_group, APP, LABEL, UPGRADE_CONTROLLER_DEPLOYMENT, UPGRADE_OPERATOR,
        UPGRADE_OPERATOR_CLUSTER_ROLE, UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING,
        UPGRADE_OPERATOR_HTTP_PORT, UPGRADE_OPERATOR_INTERNAL_PORT, UPGRADE_OPERATOR_SERVICE,
        UPGRADE_OPERATOR_SERVICE_ACCOUNT, UPGRADE_OPERATOR_SERVICE_PORT,
    },
    upgrade_labels, CliArgs,
};

use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec, DeploymentStrategy},
        core::v1::{
            Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, Service, ServiceAccount,
            ServicePort, ServiceSpec,
        },
        rbac::v1::{ClusterRole, ClusterRoleBinding, PolicyRule, RoleRef, Subject},
    },
    apimachinery::pkg::{apis::meta::v1::LabelSelector, util::intstr::IntOrString},
};

use plugin::rest_wrapper::RestClient;

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
                verbs: vec!["create", "list", "delete", "get", "patch"].into_vec(),
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

/// Defines the upgrade-operator deployment.
pub(crate) fn upgrade_operator_deployment(
    namespace: Option<String>,
    upgrade_image: String,
    release_name: String,
) -> Deployment {
    let rest_endpoint_arg = format!("--rest-endpoint={}", RestClient::get_or_panic().uri());
    let namespace_arg = format!("--namespace={}", CliArgs::args().namespace);
    let chart_name_arg = format!("--chart-name={}", &release_name);
    Deployment {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(upgrade_group(&release_name, UPGRADE_CONTROLLER_DEPLOYMENT)),
            namespace: namespace.clone(),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(1),
            selector: LabelSelector {
                match_labels: Some(btreemap! { LABEL.to_string() => UPGRADE_OPERATOR.to_string()}),
                ..Default::default()
            },
            strategy: Some(DeploymentStrategy {
                type_: Some("Recreate".to_string()),
                ..Default::default()
            }),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(btreemap! { LABEL.to_string() => UPGRADE_OPERATOR.to_string()}),
                    namespace,
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        args: Some(vec![rest_endpoint_arg, namespace_arg, chart_name_arg]),
                        image: Some(upgrade_image),
                        image_pull_policy: Some("Always".to_string()),
                        name: UPGRADE_OPERATOR.to_string(),
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

/// Defines the upgrade-operator service.
pub(crate) fn upgrade_operator_service(namespace: Option<String>, release_name: String) -> Service {
    Service {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(upgrade_group(&release_name, UPGRADE_OPERATOR_SERVICE)),
            namespace,
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some(btreemap! {
                LABEL.to_string() => UPGRADE_OPERATOR.to_string()
            }),
            ports: Some(vec![ServicePort {
                port: UPGRADE_OPERATOR_SERVICE_PORT,
                name: Some(UPGRADE_OPERATOR_HTTP_PORT.to_string()),
                target_port: Some(IntOrString::Int(UPGRADE_OPERATOR_INTERNAL_PORT)),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    }
}
