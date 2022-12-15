use crate::{
    constant::{
        upgrade_group, API_REST_LABEL_SELECTOR, DEFAULT_RELEASE_NAME,
        UPGRADE_CONTROLLER_DEPLOYMENT, UPGRADE_IMAGE, UPGRADE_OPERATOR_CLUSTER_ROLE,
        UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING, UPGRADE_OPERATOR_SERVICE,
        UPGRADE_OPERATOR_SERVICE_ACCOUNT,
    },
    resources::{objects, uo_client::UpgradeOperatorClient},
};
use anyhow::Error;
use http::Uri;
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

/// K8s resources needed for upgrade operator.
pub(crate) struct UpgradeResources {
    pub(crate) service_account: Api<ServiceAccount>,
    pub(crate) cluster_role: Api<ClusterRole>,
    pub(crate) cluster_role_binding: Api<ClusterRoleBinding>,
    pub(crate) deployment: Api<Deployment>,
    pub(crate) service: Api<Service>,
    pub(crate) release_name: String,
}

/// Methods implemented by UpgradesResources.
impl UpgradeResources {
    /// Returns an instance of UpgradesResources
    pub async fn new(ns: &str) -> anyhow::Result<Self, Error> {
        let client = Client::try_default().await?;
        let release_name = get_release_name(ns).await?;
        Ok(Self {
            service_account: Api::<ServiceAccount>::namespaced(client.clone(), ns),
            cluster_role: Api::<ClusterRole>::all(client.clone()),
            cluster_role_binding: Api::<ClusterRoleBinding>::all(client.clone()),
            deployment: Api::<Deployment>::namespaced(client.clone(), ns),
            service: Api::<Service>::namespaced(client, ns),
            release_name,
        })
    }

    /// Install the upgrade resources
    pub async fn install(ns: &str) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                let pp = PostParams::default();
                // Create a service account
                match uo.get_service_account().await {
                    Ok(sa) => {
                        if sa.iter().count() == 0 {
                            let ns = Some(ns.to_string());
                            let service_account = objects::upgrade_operator_service_account(
                                ns,
                                uo.release_name.clone(),
                            );
                            let pp = PostParams::default();
                            match uo.service_account.create(&pp, &service_account).await {
                                Ok(sa) => {
                                    println!(
                                        "Service Account : {} created in namespace : {}.",
                                        sa.metadata.name.unwrap(),
                                        sa.metadata.namespace.unwrap(),
                                    );
                                }
                                Err(e) => {
                                    println!("Failed in creating service account  {:?}", e);
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            println!(
                                "Service Account : {} in namespace : {} already exist.",
                                sa.items[0].metadata.name.as_ref().unwrap(),
                                sa.items[0].metadata.namespace.as_ref().unwrap(),
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed in fetching service account {:?}", e);
                        std::process::exit(1);
                    }
                }

                // Create Cluser role
                match uo.get_cluster_role().await {
                    Ok(cr) => {
                        if cr.iter().count() == 0 {
                            let ns = Some(ns.to_string());
                            let role =
                                objects::upgrade_operator_cluster_role(ns, uo.release_name.clone());
                            match uo.cluster_role.create(&pp, &role).await {
                                Ok(cr) => {
                                    println!(
                                        "Cluster Role : {} created.",
                                        cr.metadata.name.unwrap(),
                                    );
                                }
                                Err(e) => {
                                    println!("Failed in creating cluster role {:?}", e);
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            println!(
                                "Cluster Role : {} already exist.",
                                cr.items[0].metadata.name.as_ref().unwrap(),
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed in fetching cluster role {:?}", e);
                        std::process::exit(1);
                    }
                }

                // Create Cluster role binding
                match uo.get_cluster_role_binding().await {
                    Ok(crb) => {
                        if crb.iter().count() == 0 {
                            let ns = Some(ns.to_string());
                            let role_binding = objects::upgrade_operator_cluster_role_binding(
                                ns,
                                uo.release_name.clone(),
                            );
                            match uo.cluster_role_binding.create(&pp, &role_binding).await {
                                Ok(crb) => {
                                    println!(
                                        "Cluster Role Binding : {} created.",
                                        crb.metadata.name.unwrap(),
                                    );
                                }
                                Err(e) => {
                                    println!("Failed in creating cluster role binding {:?}", e);
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            println!(
                                "Cluster Role Binding : {} already exist.",
                                crb.items[0].metadata.name.as_ref().unwrap(),
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed in fetching cluster role binding {:?}", e);
                        std::process::exit(1);
                    }
                }

                // Create Deployment
                match uo.get_deployment().await {
                    Ok(deployment) => {
                        if deployment.iter().count() == 0 {
                            let ns = Some(ns.to_string());
                            let upgrade_deploy = objects::upgrade_operator_deployment(
                                ns,
                                UPGRADE_IMAGE.to_string(),
                                uo.release_name.clone(),
                            );
                            match uo.deployment.create(&pp, &upgrade_deploy).await {
                                Ok(dep) => {
                                    println!(
                                        "Deployment : {} created in namespace : {}.",
                                        dep.metadata.name.unwrap(),
                                        dep.metadata.namespace.unwrap(),
                                    );
                                }
                                Err(e) => {
                                    println!("Failed in creating deployment {}", e);
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            println!(
                                "Deployment : {} in namespace : {} already exist.",
                                deployment.items[0].metadata.name.as_ref().unwrap(),
                                deployment.items[0].metadata.namespace.as_ref().unwrap(),
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed in fetching deployment {:?}", e);
                        std::process::exit(1);
                    }
                }

                // Create Service
                match uo.get_service().await {
                    Ok(svc) => {
                        if svc.iter().count() == 0 {
                            let ns = Some(ns.to_string());
                            let service = objects::upgrade_operator_service(ns, uo.release_name);
                            match uo.service.create(&pp, &service).await {
                                Ok(svc) => {
                                    println!(
                                        "Service : {} created in namespace : {}.",
                                        svc.metadata.name.unwrap(),
                                        svc.metadata.namespace.unwrap(),
                                    );
                                }
                                Err(e) => {
                                    println!("Failed in creating service {:?}", e);
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            println!(
                                "Service : {} in namespace : {} already exist.",
                                svc.items[0].metadata.name.as_ref().unwrap(),
                                svc.items[0].metadata.namespace.as_ref().unwrap(),
                            );
                        }
                    }
                    Err(e) => {
                        println!("Failed in fetching service {:?}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => println!("Failed to install. Error {:?}", e),
        };
    }
    /// Uninstall the upgrade resources
    pub async fn uninstall(ns: &str) {
        match UpgradeResources::new(ns).await {
            Ok(duo) => {
                let dp = DeleteParams::default();

                // delete deployment
                match duo.get_deployment().await {
                    Ok(deployment) => {
                        if deployment.iter().count() == 1 {
                            match duo
                                .deployment
                                .delete(
                                    &upgrade_group(
                                        &duo.release_name,
                                        UPGRADE_CONTROLLER_DEPLOYMENT,
                                    ),
                                    &dp,
                                )
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
                    }
                    Err(e) => {
                        println!("Failed in fetching deployment {:?}", e);
                        std::process::exit(1);
                    }
                }

                // delete service
                match duo.get_service().await {
                    Ok(svc) => {
                        if svc.iter().count() == 1 {
                            match duo
                                .service
                                .delete(
                                    &upgrade_group(&duo.release_name, UPGRADE_OPERATOR_SERVICE),
                                    &dp,
                                )
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
                    Err(e) => {
                        println!("Failed in fetching service {:?}", e);
                        std::process::exit(1);
                    }
                }

                // delete cluster role binding\
                match duo.get_cluster_role_binding().await {
                    Ok(crb) => {
                        if crb.iter().count() == 1 {
                            match duo
                                .cluster_role_binding
                                .delete(
                                    &upgrade_group(
                                        &duo.release_name,
                                        UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING,
                                    ),
                                    &dp,
                                )
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
                    }
                    Err(e) => {
                        println!("Failed in fetching cluster role binding  {:?}", e);
                        std::process::exit(1);
                    }
                }

                // delete cluster role
                match duo.get_cluster_role().await {
                    Ok(cr) => {
                        if cr.iter().count() == 1 {
                            match duo
                                .cluster_role
                                .delete(
                                    &upgrade_group(
                                        &duo.release_name,
                                        UPGRADE_OPERATOR_CLUSTER_ROLE,
                                    ),
                                    &dp,
                                )
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
                    }
                    Err(e) => {
                        println!("Failed in fetching cluster role {:?}", e);
                        std::process::exit(1);
                    }
                }

                // delete service account
                match duo.get_service_account().await {
                    Ok(svca) => {
                        if svca.iter().count() == 1 {
                            match duo
                                .service_account
                                .delete(
                                    &upgrade_group(
                                        &duo.release_name,
                                        UPGRADE_OPERATOR_SERVICE_ACCOUNT,
                                    ),
                                    &dp,
                                )
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
                    }
                    Err(e) => {
                        println!("Failed in fetching deployment {:?}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => println!("Failed to uninstall. Error {}", e),
        };
    }

    /// Upgrades the cluster
    pub async fn apply(
        uri: Option<Uri>,
        namespace: &str,
        kube_config_path: Option<PathBuf>,
        timeout: humantime::Duration,
    ) {
        match UpgradeOperatorClient::new(uri, namespace.to_string(), kube_config_path, timeout)
            .await
        {
            Ok(mut client) => {
                if let Err(err) = client.apply_upgrade().await {
                    println!("Error while  upgrading {:?}", err);
                }
            }
            Err(e) => {
                println!("Failed to create client for upgrade {:?}", e);
                std::process::exit(1);
            }
        }
    }

    /// Upgrades the cluster
    pub async fn get(
        uri: Option<Uri>,
        namespace: &str,
        kube_config_path: Option<PathBuf>,
        timeout: humantime::Duration,
    ) {
        match UpgradeOperatorClient::new(uri, namespace.to_string(), kube_config_path, timeout)
            .await
        {
            Ok(mut client) => {
                if let Err(err) = client.get_upgrade().await {
                    println!("Error while  upgrading {:?}", err);
                }
            }
            Err(e) => {
                println!("Failed to create client for upgrade {:?}", e);
                std::process::exit(1);
            }
        }
    }

    /// Return results as list of service accounts.
    pub async fn get_service_account(&self) -> Result<ObjectList<ServiceAccount>, Error> {
        let lp = ListParams::default().fields(&format!(
            "metadata.name={}",
            upgrade_group(&self.release_name, UPGRADE_OPERATOR_SERVICE_ACCOUNT)
        ));
        Ok(self.service_account.list(&lp).await?)
    }

    /// Return results as list of cluster role.
    pub async fn get_cluster_role(&self) -> Result<ObjectList<ClusterRole>, Error> {
        let lp = ListParams::default().fields(&format!(
            "metadata.name={}",
            upgrade_group(&self.release_name, UPGRADE_OPERATOR_CLUSTER_ROLE)
        ));
        Ok(self.cluster_role.list(&lp).await?)
    }

    /// Return results as list of cluster role binding.
    pub async fn get_cluster_role_binding(&self) -> Result<ObjectList<ClusterRoleBinding>, Error> {
        let lp = ListParams::default().fields(&format!(
            "metadata.name={}",
            &upgrade_group(&self.release_name, UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING)
        ));
        Ok(self.cluster_role_binding.list(&lp).await?)
    }

    /// Return results as list of deployments.
    pub async fn get_deployment(&self) -> Result<ObjectList<Deployment>, Error> {
        let lp = ListParams::default().fields(&format!(
            "metadata.name={}",
            &upgrade_group(&self.release_name, UPGRADE_CONTROLLER_DEPLOYMENT)
        ));
        Ok(self.deployment.list(&lp).await?)
    }

    /// Return results as list of service.
    pub async fn get_service(&self) -> Result<ObjectList<Service>, Error> {
        let lp = ListParams::default().fields(&format!(
            "metadata.name={}",
            &upgrade_group(&self.release_name, UPGRADE_OPERATOR_SERVICE)
        ));
        Ok(self.service.list(&lp).await?)
    }
}

/// Return results as list of deployments.
pub async fn get_deployment_for_rest(ns: &str) -> Result<ObjectList<Deployment>, Error> {
    let client = Client::try_default().await?;
    let deployment = Api::<Deployment>::namespaced(client.clone(), ns);
    let lp = ListParams::default().labels(API_REST_LABEL_SELECTOR);
    Ok(deployment.list(&lp).await?)
}

/// Return the release name.
pub async fn get_release_name(ns: &str) -> Result<String, Error> {
    match get_deployment_for_rest(ns).await {
        Ok(deployments) => match deployments.items.get(0) {
            Some(deployment) => match &deployment.metadata.labels {
                Some(label) => match label.get("helm-release") {
                    Some(value) => Ok(value.to_string()),
                    None => Ok(DEFAULT_RELEASE_NAME.to_string()),
                },
                None => Ok(DEFAULT_RELEASE_NAME.to_string()),
            },
            None => {
                println!("No deployment present.");
                std::process::exit(1);
            }
        },
        Err(e) => {
            println!("Failed in fetching deployment {:?}", e);
            std::process::exit(1);
        }
    }
}
