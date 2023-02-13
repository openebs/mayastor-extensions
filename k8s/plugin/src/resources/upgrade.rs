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
use std::path::PathBuf;

/// The types of resources that support the upgrade operator.
#[derive(clap::Subcommand, Debug)]
pub enum UpgradeOperator {
    /// Install, Uninstall upgrade resources.
    UpgradeOperator,
}

#[derive(clap::Subcommand, Debug)]
/// Actions to be performed.
pub enum Actions {
    /// Action to create object.
    Create,
    /// Action to delete object.
    Delete,
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

    /// Create/Delete ServiceAction
    pub async fn service_account_actions(
        &self,
        ns: &str,
        action: Actions,
    ) -> Result<(), kube::Error> {
        if let Some(sa) = self
            .service_account
            .get_opt(&upgrade_group(
                &self.release_name,
                UPGRADE_OPERATOR_SERVICE_ACCOUNT,
            ))
            .await?
        {
            match action {
                Actions::Create => {
                    println!(
                        "ServiceAccount: {} in namespace: {} already exist.",
                        sa.metadata.name.as_ref().unwrap(),
                        sa.metadata.namespace.as_ref().unwrap()
                    );
                }
                Actions::Delete => {
                    match self
                        .service_account
                        .delete(
                            &upgrade_group(&self.release_name, UPGRADE_OPERATOR_SERVICE_ACCOUNT),
                            &DeleteParams::default(),
                        )
                        .await
                    {
                        Ok(_) => {
                            println!("ServiceAccount deleted");
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let ns = Some(ns.to_string());
                    let service_account =
                        objects::upgrade_operator_service_account(ns, self.release_name.clone());
                    let pp = PostParams::default();
                    match self.service_account.create(&pp, &service_account).await {
                        Ok(sa) => {
                            println!(
                                "ServiceAccount: {} created in namespace: {}",
                                sa.metadata.name.unwrap(),
                                sa.metadata.namespace.unwrap()
                            )
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
                Actions::Delete => {
                    println!("ServiceAccount does not exist");
                }
            }
        }
        Ok(())
    }

    /// Create/Delete cluster role
    pub async fn cluster_role_actions(&self, ns: &str, action: Actions) -> Result<(), kube::Error> {
        if let Some(cr) = self
            .cluster_role
            .get_opt(&upgrade_group(
                &self.release_name,
                UPGRADE_OPERATOR_CLUSTER_ROLE,
            ))
            .await?
        {
            match action {
                Actions::Create => {
                    println!(
                        "ClusterRole: {} already exist",
                        cr.metadata.name.as_ref().unwrap()
                    );
                }
                Actions::Delete => {
                    match self
                        .cluster_role
                        .delete(
                            &upgrade_group(&self.release_name, UPGRADE_OPERATOR_CLUSTER_ROLE),
                            &DeleteParams::default(),
                        )
                        .await
                    {
                        Ok(_) => {
                            println!("ClusterRole deleted");
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let ns = Some(ns.to_string());
                    let role =
                        objects::upgrade_operator_cluster_role(ns, self.release_name.clone());
                    let pp = PostParams::default();
                    match self.cluster_role.create(&pp, &role).await {
                        Ok(cr) => {
                            println!("Cluster Role: {} created", cr.metadata.name.unwrap());
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
                Actions::Delete => {
                    println!("cluster role does not exist");
                }
            }
        }
        Ok(())
    }

    /// Create/Delete cluster role binding
    pub async fn cluster_role_binding_actions(
        &self,
        ns: &str,
        action: Actions,
    ) -> Result<(), kube::Error> {
        if let Some(crb) = self
            .cluster_role_binding
            .get_opt(&upgrade_group(
                &self.release_name,
                UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING,
            ))
            .await?
        {
            match action {
                Actions::Create => {
                    println!(
                        "ClusterRoleBinding: {} already exist",
                        crb.metadata.name.as_ref().unwrap()
                    );
                }
                Actions::Delete => {
                    match self
                        .cluster_role_binding
                        .delete(
                            &upgrade_group(
                                &self.release_name,
                                UPGRADE_OPERATOR_CLUSTER_ROLE_BINDING,
                            ),
                            &DeleteParams::default(),
                        )
                        .await
                    {
                        Ok(_) => {
                            println!("ClusterRoleBinding deleted");
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let ns = Some(ns.to_string());
                    let role_binding = objects::upgrade_operator_cluster_role_binding(
                        ns,
                        self.release_name.clone(),
                    );
                    let pp = PostParams::default();
                    match self.cluster_role_binding.create(&pp, &role_binding).await {
                        Ok(crb) => {
                            println!("ClusterRoleBinding: {} created", crb.metadata.name.unwrap());
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
                Actions::Delete => {
                    println!("ClusterRoleBinding does not exist");
                }
            }
        }
        Ok(())
    }

    /// Create/Delete deployment
    pub async fn deployment_actions(&self, ns: &str, action: Actions) -> Result<(), kube::Error> {
        if let Some(deployment) = self
            .deployment
            .get_opt(&upgrade_group(
                &self.release_name,
                UPGRADE_CONTROLLER_DEPLOYMENT,
            ))
            .await?
        {
            match action {
                Actions::Create => {
                    println!(
                        "Deployment: {} in namespace: {} already exist",
                        deployment.metadata.name.as_ref().unwrap(),
                        deployment.metadata.namespace.as_ref().unwrap()
                    );
                }
                Actions::Delete => {
                    match self
                        .deployment
                        .delete(
                            &upgrade_group(&self.release_name, UPGRADE_CONTROLLER_DEPLOYMENT),
                            &DeleteParams::default(),
                        )
                        .await
                    {
                        Ok(_) => {
                            println!("Deployment deleted");
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let upgrade_deploy = objects::upgrade_operator_deployment(
                        ns,
                        UPGRADE_IMAGE.to_string(),
                        self.release_name.clone(),
                    );
                    match self
                        .deployment
                        .create(&PostParams::default(), &upgrade_deploy)
                        .await
                    {
                        Ok(dep) => {
                            println!(
                                "Deployment: {} created in namespace: {}",
                                dep.metadata.name.unwrap(),
                                dep.metadata.namespace.unwrap()
                            );
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
                Actions::Delete => {
                    println!("Deployment does not exist");
                }
            }
        }
        Ok(())
    }

    /// Create/Delete service
    pub async fn service_actions(&self, ns: &str, action: Actions) -> Result<(), kube::Error> {
        if let Some(svc) = self
            .service
            .get_opt(&upgrade_group(&self.release_name, UPGRADE_OPERATOR_SERVICE))
            .await?
        {
            match action {
                Actions::Create => {
                    println!(
                        "Service: {} in namespace: {} already exist",
                        svc.metadata.name.as_ref().unwrap(),
                        svc.metadata.namespace.as_ref().unwrap()
                    );
                }
                Actions::Delete => {
                    match self
                        .service
                        .delete(
                            &upgrade_group(&self.release_name, UPGRADE_OPERATOR_SERVICE),
                            &DeleteParams::default(),
                        )
                        .await
                    {
                        Ok(_) => {
                            println!("Service deleted");
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let ns = Some(ns.to_string());
                    let service = objects::upgrade_operator_service(ns, self.release_name.clone());
                    match self.service.create(&PostParams::default(), &service).await {
                        Ok(svc) => {
                            println!(
                                "Service: {} created in namespace: {}",
                                svc.metadata.name.unwrap(),
                                svc.metadata.namespace.unwrap()
                            );
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                }
                Actions::Delete => {
                    println!("Service does not exist");
                }
            }
        }
        Ok(())
    }

    /// Install the upgrade resources
    pub async fn install(ns: &str) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                // Create Service Account
                match uo.service_account_actions(ns, Actions::Create).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in creating ServiceAccount {error}");
                        std::process::exit(1)
                    }
                }

                // Create Cluser role
                match uo.cluster_role_actions(ns, Actions::Create).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in creating ClusterRole {error}");
                        std::process::exit(1)
                    }
                }

                // Create Cluster role binding
                match uo.cluster_role_binding_actions(ns, Actions::Create).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in creating ClusterRoleBinding {error}");
                        std::process::exit(1)
                    }
                }

                // Create Deployment
                match uo.deployment_actions(ns, Actions::Create).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in creating Deployment {error}");
                        std::process::exit(1)
                    }
                }

                // Create Service
                match uo.service_actions(ns, Actions::Create).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in creating Service {error}");
                        std::process::exit(1)
                    }
                }
            }
            Err(e) => println!("Failed to install. Error {e:?}"),
        };
    }
    /// Uninstall the upgrade resources
    pub async fn uninstall(ns: &str) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                // Delete deployment
                match uo.deployment_actions(ns, Actions::Delete).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in creating Deployment {error}");
                        std::process::exit(1)
                    }
                }

                // Delete service
                match uo.service_actions(ns, Actions::Delete).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in deleting Service {error}");
                        std::process::exit(1)
                    }
                }

                // Delete cluster role binding
                match uo.cluster_role_binding_actions(ns, Actions::Delete).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in deleting ClusterRoleBinding {error}");
                        std::process::exit(1)
                    }
                }

                // Delete cluster role
                match uo.cluster_role_actions(ns, Actions::Delete).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in deleting ClusterRole {error}");
                        std::process::exit(1)
                    }
                }

                // Delete service account
                match uo.service_account_actions(ns, Actions::Delete).await {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Failed in deleting ServiceAccount {error}");
                        std::process::exit(1)
                    }
                }
            }
            Err(e) => println!("Failed to uninstall. Error {e}"),
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
                    println!("Error while  upgrading {err:?}");
                }
            }
            Err(e) => {
                println!("Failed to create client for upgrade {e:?}");
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
                    println!("Error while  upgrading {err:?}");
                }
            }
            Err(e) => {
                println!("Failed to create client for upgrade {e:?}");
                std::process::exit(1);
            }
        }
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
                Some(label) => match label.get("openebs.io/release") {
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
            println!("Failed in fetching deployment {e:?}");
            std::process::exit(1);
        }
    }
}
