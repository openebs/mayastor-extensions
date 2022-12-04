use crate::{
    constant::{
        UPGRADE_CONTROLLER_DEPLOYMENT, UPGRADE_IMAGE, UPGRADE_OPERATOR_CLUSTER_ROLE,
        UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING, UPGRADE_OPERATOR_SERVICE,
        UPGRADE_OPERATOR_SERVICE_ACCOUNT,
    },
    resources::{objects, uo_client::UpgradeOperatorClient},
};
use anyhow::Error;
use k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::{Service, ServiceAccount},
    rbac::v1::{ClusterRole, ClusterRoleBinding},
};
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    core::ObjectList,
    Client,
};
use std::{path::PathBuf, time::Duration};

/// The types of resources that support the upgrade operator.
#[derive(clap::Subcommand, Debug)]
pub enum UpgradeOperator {
    /// Install, Uninstall upgrade resources.
    UpgradeOperator,
}
#[derive(clap::Subcommand, Debug)]
pub enum UpgradeStatus {
    /// Get the upgrade status.
    UpgradeStatus,
}

/// K8s resources needed for upgrade operator
pub struct UpgradeResources {
    pub(crate) upgrade_service_account: Api<ServiceAccount>,
    pub(crate) upgrade_cluster_role: Api<ClusterRole>,
    pub(crate) upgrade_cluster_role_binding: Api<ClusterRoleBinding>,
    pub(crate) upgrade_deployment: Api<Deployment>,
    pub(crate) upgrade_service: Api<Service>,
}

/// Methods implemented by UpgradesResources
impl UpgradeResources {
    /// Returns an instance of UpgradesResources
    pub async fn new(ns: &str) -> anyhow::Result<Self, Error> {
        let client = Client::try_default().await?;
        Ok(Self {
            upgrade_service_account: Api::<ServiceAccount>::namespaced(client.clone(), ns),
            upgrade_cluster_role: Api::<ClusterRole>::all(client.clone()),
            upgrade_cluster_role_binding: Api::<ClusterRoleBinding>::all(client.clone()),
            upgrade_deployment: Api::<Deployment>::namespaced(client.clone(), ns),
            upgrade_service: Api::<Service>::namespaced(client, ns),
        })
    }

    /// Install the upgrade resources
    pub async fn install(ns: &str) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                let pp = PostParams::default();
                // Create a service account
                let sa = uo.get_service_account().await;
                if sa.iter().count() == 0 {
                    let ns: Option<String> = Some(ns.to_string());
                    let service_account = objects::upgrade_operator_service_account(ns);
                    let pp = PostParams::default();
                    match uo
                        .upgrade_service_account
                        .create(&pp.clone(), &service_account)
                        .await
                    {
                        Ok(_) => {
                            println!("service account created");
                        }
                        Err(e) => {
                            println!("Failed in creating service account {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("service account already present! ");
                }
                // Create Cluser role
                let cr = uo.get_cluster_role().await;
                if cr.iter().count() == 0 {
                    let ns: Option<String> = Some(ns.to_string());
                    let role = objects::upgrade_operator_cluster_role(ns);
                    match uo.upgrade_cluster_role.create(&pp.clone(), &role).await {
                        Ok(_) => {
                            println!("cluster role created");
                        }
                        Err(e) => {
                            println!("Failed in creating cluster role {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("cluster role already present!");
                }

                // Create Cluster role binding
                let crb = uo.get_cluster_role_binding().await;
                if crb.iter().count() == 0 {
                    let ns: Option<String> = Some(ns.to_string());
                    let role_binding = objects::upgrade_operator_cluster_role_binding(ns);
                    match uo
                        .upgrade_cluster_role_binding
                        .create(&pp.clone(), &role_binding)
                        .await
                    {
                        Ok(_) => {
                            println!("cluster role binding created");
                        }
                        Err(e) => {
                            println!("Failed in creating cluster role binding {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("cluster role binding already present!");
                }

                // Create Deployment
                let crb = uo.get_deployment().await;
                if crb.iter().count() == 0 {
                    let ns: Option<String> = Some(ns.to_string());
                    let upgrade_deploy =
                        objects::upgrade_operator_deployment(ns, UPGRADE_IMAGE.to_string());
                    match uo.upgrade_deployment.create(&pp, &upgrade_deploy).await {
                        Ok(_) => {
                            println!("deployment created");
                        }
                        Err(e) => {
                            println!("Failed in creating deployment {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("deployment already present!");
                }

                // Create Service
                let crb = uo.get_service().await;
                if crb.iter().count() == 0 {
                    let ns: Option<String> = Some(ns.to_string());
                    let upgrade_service = objects::upgrade_operator_service(ns);
                    match uo.upgrade_service.create(&pp, &upgrade_service).await {
                        Ok(_) => {
                            println!("service created");
                        }
                        Err(e) => {
                            println!("Failed in creating service {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("service already present!");
                }
            }
            Err(e) => println!("Failed to install. Error {}", e),
        };
    }

    /// Uninstall the upgrade resources
    pub async fn uninstall(ns: &str) {
        match UpgradeResources::new(ns).await {
            Ok(duo) => {
                let dp = DeleteParams::default();
                // delete service account
                let svca = duo.get_service_account().await;
                if svca.iter().count() == 1 {
                    match duo
                        .upgrade_service_account
                        .delete(UPGRADE_OPERATOR_SERVICE_ACCOUNT, &dp)
                        .await
                    {
                        Ok(_) => {
                            println!("service account deleted");
                        }
                        Err(e) => {
                            println!("Failed in deleting service account {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("service account does not exist!");
                }

                // delete cluster role
                let cr = duo.get_cluster_role().await;
                if cr.iter().count() == 1 {
                    match duo
                        .upgrade_cluster_role
                        .delete(UPGRADE_OPERATOR_CLUSTER_ROLE, &dp)
                        .await
                    {
                        Ok(_) => {
                            println!("cluster role deleted");
                        }
                        Err(e) => {
                            println!("Failed in deleting cluster role {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("cluster role does not exist!");
                }

                // delete cluster role binding
                let crb = duo.get_cluster_role_binding().await;
                if crb.iter().count() == 1 {
                    match duo
                        .upgrade_cluster_role_binding
                        .delete(UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING, &dp)
                        .await
                    {
                        Ok(_) => {
                            println!("cluster role binding deleted");
                        }
                        Err(e) => {
                            println!("Failed in deleting cluster role binding {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("cluster role binding does not exist!");
                }

                // delete deployment
                let deployment = duo.get_deployment().await;
                if deployment.iter().count() == 1 {
                    match duo
                        .upgrade_deployment
                        .delete(UPGRADE_CONTROLLER_DEPLOYMENT, &dp)
                        .await
                    {
                        Ok(_) => {
                            println!("deployment deleted");
                        }
                        Err(e) => {
                            println!("Failed in deleting deployment {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("deployment does not exist!");
                }

                // delete service
                let svc = duo.get_service().await;
                if svc.iter().count() == 1 {
                    match duo
                        .upgrade_service
                        .delete(UPGRADE_OPERATOR_SERVICE, &dp.clone())
                        .await
                    {
                        Ok(_) => {
                            println!("service deleted");
                        }
                        Err(e) => {
                            println!("Failed in deleting service {:?}", e);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("service does not exist!");
                }
            }
            Err(e) => println!("Failed to uninstall. Error {}", e),
        };
    }

    /// Upgrades the cluster
    pub async fn apply(
        uri: Option<String>,
        namespace: &str,
        kube_config_path: Option<PathBuf>,
        timeout: humantime::Duration,
    ) {
        let client_m =
            UpgradeOperatorClient::new(uri, namespace.to_string(), kube_config_path, timeout).await;

        if let Some(mut client) = client_m {
            if let Err(err) = client.apply_upgrade().await {
                println!("Error while  upgrading {:?}", err);
            }
        }
    }

    /// Upgrades the cluster
    pub async fn get(
        uri: Option<String>,
        namespace: &str,
        kube_config_path: Option<PathBuf>,
        timeout: humantime::Duration,
    ) {
        let client_m =
            UpgradeOperatorClient::new(uri, namespace.to_string(), kube_config_path, timeout).await;

        if let Some(mut client) = client_m {
            if let Err(err) = client.get_upgrade().await {
                println!("Error while  getting upgrade {:?}", err);
            }
        }
    }

    /// List service account
    pub async fn get_service_account(&self) -> ObjectList<ServiceAccount> {
        let lp = ListParams::default().fields(&format!(
            "metadata.name={}",
            UPGRADE_OPERATOR_SERVICE_ACCOUNT
        ));
        self.upgrade_service_account
            .list(&lp)
            .await
            .expect("failed to list service accounts")
    }

    /// List cluster role
    pub async fn get_cluster_role(&self) -> ObjectList<ClusterRole> {
        let lp = ListParams::default()
            .fields(&format!("metadata.name={}", UPGRADE_OPERATOR_CLUSTER_ROLE));
        self.upgrade_cluster_role
            .list(&lp)
            .await
            .expect("failed to list cluster role")
    }

    /// List cluster role binding
    pub async fn get_cluster_role_binding(&self) -> ObjectList<ClusterRoleBinding> {
        let lp = ListParams::default().fields(&format!(
            "metadata.name={}",
            UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING
        ));
        self.upgrade_cluster_role_binding
            .list(&lp)
            .await
            .expect("failed to list cluster role binding")
    }

    /// List deployment
    pub async fn get_deployment(&self) -> ObjectList<Deployment> {
        let lp = ListParams::default()
            .fields(&format!("metadata.name={}", UPGRADE_CONTROLLER_DEPLOYMENT));
        self.upgrade_deployment
            .list(&lp)
            .await
            .expect("failed to list deployment")
    }

    /// List service
    pub async fn get_service(&self) -> ObjectList<Service> {
        let lp =
            ListParams::default().fields(&format!("metadata.name={}", UPGRADE_OPERATOR_SERVICE));
        self.upgrade_service
            .list(&lp)
            .await
            .expect("failed to list service")
    }
}
