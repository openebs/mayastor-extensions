use k8s_openapi::{api::{apps::v1::{Deployment, DeploymentSpec,DeploymentStrategy},core::v1::{Service, ServiceSpec, PodTemplateSpec, PodSpec, Container, ServicePort}},apimachinery::pkg::{apis::meta::v1::LabelSelector, util::intstr::IntOrString}};
use kube::core::ObjectMeta;
use maplit::btreemap;
use crate::{upgrade::common::constants::{APP,LABEL, UPGRADE_CONTROLLER_DEPLOYMENT, UPGRADE_OPERATOR_SERVICE_ACCOUNT, UPGRADE_OPERATOR, UPGRADE_OPERATOR_SERVICE, UPGRADE_OPERATOR_SERVICE_PORT, UPGRADE_OPERATOR_INTERNAL_PORT}, upgrade_labels};

/// Defines the upgrade-operator deployment
pub fn upgrade_operator_deployment(namespace:Option<String>,upgrade_image:String)->Deployment{
    Deployment {
        metadata: ObjectMeta {
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(UPGRADE_CONTROLLER_DEPLOYMENT.to_string()),
            namespace:namespace.clone(),
            ..Default::default()
        },
        spec: Some(DeploymentSpec{
            replicas:Some(1),
            selector:LabelSelector{
                match_labels: Some(
                    btreemap! { LABEL.to_string() => UPGRADE_OPERATOR.to_string()},
                ),
                ..Default::default()
            },
            strategy:Some(DeploymentStrategy{
                type_: Some("Recreate".to_string()),
                ..Default::default()
            }),
            template:PodTemplateSpec{
                metadata: Some(ObjectMeta{
                    labels: Some(
                        btreemap! { LABEL.to_string() => UPGRADE_OPERATOR.to_string()},
                    ),
                    namespace,
                    ..Default::default()
                }),
                spec: Some(PodSpec{
                    containers:vec![Container{
                        image: Some(upgrade_image),
                        image_pull_policy: Some("Always".to_string()),
                        name: UPGRADE_OPERATOR.to_string(),
                        command: Some(vec!["./upgrade-operator".to_string()]),
                        ..Default::default()
                    }],
                    service_account_name:Some(UPGRADE_OPERATOR_SERVICE_ACCOUNT.to_string()),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Defines the upgrade-operator service
pub fn upgrade_operator_service(namespace:Option<String>)->Service{
    Service {
        metadata: ObjectMeta{
            labels: Some(upgrade_labels!(UPGRADE_OPERATOR)),
            name: Some(UPGRADE_OPERATOR_SERVICE.to_string()),
            namespace,
            ..Default::default()
        },
        spec: Some(ServiceSpec{
            selector: Some(btreemap! {
                LABEL.to_string() => UPGRADE_OPERATOR.to_string()
            }),
            ports: Some(vec![ServicePort {
                port: UPGRADE_OPERATOR_SERVICE_PORT,
                target_port: Some(IntOrString::Int(UPGRADE_OPERATOR_INTERNAL_PORT)),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    }
}
