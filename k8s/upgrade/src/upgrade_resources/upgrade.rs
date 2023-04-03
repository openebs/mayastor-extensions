use crate::{
    constant::{
        upgrade_image_concat, upgrade_name_concat, AGENT_CORE_POD_LABEL, API_REST_LABEL_SELECTOR,
        API_REST_POD_LABEL, DEFAULT_RELEASE_NAME, IO_ENGINE_POD_LABEL,
        UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX, UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX,
        UPGRADE_JOB_IMAGE_NAME, UPGRADE_JOB_IMAGE_REPO, UPGRADE_JOB_IMAGE_TAG,
        UPGRADE_JOB_NAME_SUFFIX, UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
    },
    upgrade_resources::objects,
    user_prompt::{
        upgrade_dry_run_summary, CONTROL_PLANE_PODS_LIST, DATA_PLANE_PODS_LIST,
        DATA_PLANE_PODS_LIST_SKIP_RESTART, UPGRADE_DRY_RUN_SUMMARY,
    },
};

use anyhow::Error;
use k8s_openapi::api::{
    apps::v1::Deployment,
    batch::v1::Job,
    core::v1::{PersistentVolumeClaim, Pod, ServiceAccount},
    rbac::v1::{ClusterRole, ClusterRoleBinding},
};
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    core::ObjectList,
    Client,
};
use std::collections::HashSet;

/// Arguments to be passed for upgrade.
#[derive(clap::Subcommand, Debug)]
pub enum DeleteResources {
    /// Delete upgrade resources
    Upgrade,
}

/// Upgrade resource.
#[derive(clap::Args, Debug)]
pub struct Upgrade {}

impl Upgrade {
    /// Delete the upgrade resources
    pub async fn delete(ns: &str) {
        // Delete upgrade resources once job completes
        match upgrade_job_completed(ns).await {
            Ok(job_completed) => {
                if job_completed {
                    UpgradeResources::delete_upgrade_resources(ns).await;
                }
            }
            Err(error) => {
                eprintln!("Failure: {error}");
            }
        }
    }
}

#[derive(clap::Subcommand, Debug)]
/// Actions to be performed.
pub enum Actions {
    /// Action to create object.
    Create,
    /// Action to delete object.
    Delete,
}

/// Arguments to be passed for upgrade.
#[derive(Debug, Clone, clap::Args)]
pub struct UpgradeArgs {
    /// Display all the validations output but will not execute upgrade.
    #[clap(global = true, long, short)]
    pub dry_run: bool,

    /// If set then upgrade will skip the io-engine pods restart
    #[clap(global = true, long, short, default_value_t = false)]
    pub skip_data_plane_restart: bool,

    /// If set then it will continue with upgrade without validating singla replica volume.
    #[clap(global = true, long)]
    pub skip_single_replica_volume_validation: bool,

    /// If set then upgrade will skip the repilca rebuild in progress validation
    #[clap(global = true, long)]
    pub skip_replica_rebuild: bool,
}

impl UpgradeArgs {
    ///  Upgrade the resources.
    pub async fn apply(&self, namespace: &str) {
        // Create resources for upgrade
        UpgradeResources::create_upgrade_resources(namespace, self.skip_data_plane_restart).await;
    }

    ///  Dummy upgrade the resources.
    pub async fn dummy_apply(&self, namespace: &str) -> Result<(), Error> {
        let mut pods_names: Vec<String> = Vec::new();
        list_pods(AGENT_CORE_POD_LABEL, namespace, &mut pods_names).await?;
        list_pods(API_REST_POD_LABEL, namespace, &mut pods_names).await?;
        console_logger::info(CONTROL_PLANE_PODS_LIST, &pods_names.join("\n"));

        let mut io_engine_pods_names: Vec<String> = Vec::new();
        list_pods(IO_ENGINE_POD_LABEL, namespace, &mut io_engine_pods_names).await?;
        if self.skip_data_plane_restart {
            console_logger::info(
                DATA_PLANE_PODS_LIST_SKIP_RESTART,
                &io_engine_pods_names.join("\n"),
            );
        } else {
            console_logger::info(DATA_PLANE_PODS_LIST, &io_engine_pods_names.join("\n"));
        }
        console_logger::info(
            upgrade_dry_run_summary(UPGRADE_DRY_RUN_SUMMARY, UPGRADE_JOB_IMAGE_TAG).as_str(),
            "",
        );
        Ok(())
    }
}

pub async fn list_pods(
    label: &str,
    namespace: &str,
    pods_names: &mut Vec<String>,
) -> Result<(), Error> {
    let client = Client::try_default().await?;
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod_list: ObjectList<Pod> = pods.list(&ListParams::default().labels(label)).await?;

    for pod in pod_list.iter() {
        // Fetch the pod name
        let pod_name = pod
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("pod.metadata.name is empty"))?
            .as_str();
        let _ = &pods_names.push(pod_name.to_string());
    }
    Ok(())
}

/// Arguments to be passed for upgrade.
#[derive(Debug, Clone, clap::Args)]
pub struct GetUpgradeArgs {}

impl GetUpgradeArgs {
    ///  Upgrade the resources.
    pub async fn get_upgrade(&self, namespace: &str) {
        println!("{namespace}")
        // To be implemented
    }
}

/// K8s resources needed for upgrade operator.
pub struct UpgradeResources {
    pub(crate) service_account: Api<ServiceAccount>,
    pub(crate) cluster_role: Api<ClusterRole>,
    pub(crate) cluster_role_binding: Api<ClusterRoleBinding>,
    pub(crate) job: Api<Job>,
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
            job: Api::<Job>::namespaced(client, ns),
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
            .get_opt(&upgrade_name_concat(
                &self.release_name,
                UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
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
                            &upgrade_name_concat(
                                &self.release_name,
                                UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
                            ),
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
                        objects::upgrade_job_service_account(ns, self.release_name.clone());
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
            .get_opt(&upgrade_name_concat(
                &self.release_name,
                UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX,
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
                            &upgrade_name_concat(
                                &self.release_name,
                                UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX,
                            ),
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
                    let role = objects::upgrade_job_cluster_role(ns, self.release_name.clone());
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
            .get_opt(&upgrade_name_concat(
                &self.release_name,
                UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
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
                            &upgrade_name_concat(
                                &self.release_name,
                                UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
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
                    let role_binding =
                        objects::upgrade_job_cluster_role_binding(ns, self.release_name.clone());
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

    /// Create/Delete upgrade job
    pub async fn job_actions(
        &self,
        ns: &str,
        action: Actions,
        skip_data_plane_restart: bool,
    ) -> Result<(), kube::Error> {
        if let Some(job) = self
            .job
            .get_opt(&upgrade_name_concat(
                &self.release_name,
                UPGRADE_JOB_NAME_SUFFIX,
            ))
            .await?
        {
            match action {
                Actions::Create => {
                    println!(
                        "Job: {} in namespace: {} already exist",
                        job.metadata.name.as_ref().unwrap(),
                        job.metadata.namespace.as_ref().unwrap()
                    );
                }
                Actions::Delete => {
                    match self
                        .job
                        .delete(
                            &upgrade_name_concat(&self.release_name, UPGRADE_JOB_NAME_SUFFIX),
                            &DeleteParams::default(),
                        )
                        .await
                    {
                        Ok(_) => {
                            println!("Job deleted");
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
                    let upgrade_deploy = objects::upgrade_job(
                        ns,
                        upgrade_image_concat(
                            UPGRADE_JOB_IMAGE_REPO,
                            UPGRADE_JOB_IMAGE_NAME,
                            UPGRADE_JOB_IMAGE_TAG,
                        ),
                        self.release_name.clone(),
                        skip_data_plane_restart,
                    );
                    match self
                        .job
                        .create(&PostParams::default(), &upgrade_deploy)
                        .await
                    {
                        Ok(dep) => {
                            println!(
                                "Job: {} created in namespace: {}",
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
                    println!("Job does not exist");
                }
            }
        }
        Ok(())
    }

    /// Create the resources for upgrade
    pub async fn create_upgrade_resources(ns: &str, skip_data_plane_restart: bool) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                // Create Service Account
                let _svc_acc = uo
                    .service_account_actions(ns, Actions::Create)
                    .await
                    .map_err(|error| {
                        println!("Failed in creating ServiceAccount {error}");
                        std::process::exit(1);
                    });

                // Create Cluser role
                let _cl_role =
                    uo.cluster_role_actions(ns, Actions::Create)
                        .await
                        .map_err(|error| {
                            println!("Failed in creating ClusterRole {error}");
                            std::process::exit(1);
                        });

                // Create Cluster role binding
                let _cl_role_binding = uo
                    .cluster_role_binding_actions(ns, Actions::Create)
                    .await
                    .map_err(|error| {
                        println!("Failed in creating ClusterRoleBinding {error}");
                        std::process::exit(1);
                    });

                // Create Service Account
                let _job = uo
                    .job_actions(ns, Actions::Create, skip_data_plane_restart)
                    .await
                    .map_err(|error| {
                        println!("Failed in creating Upgrade Job {error}");
                        std::process::exit(1);
                    });
            }
            Err(e) => println!("Failed to create upgrade resources. Error {e:?}"),
        };
    }

    /// Delete the upgrade resources
    pub async fn delete_upgrade_resources(ns: &str) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                // Delete the job
                let _job = uo
                    .job_actions(ns, Actions::Delete, false)
                    .await
                    .map_err(|error| {
                        println!("Failed in deleting Upgrade Job {error}");
                        std::process::exit(1);
                    });

                // Delete cluster role binding
                let _cl_role_binding = uo
                    .cluster_role_binding_actions(ns, Actions::Delete)
                    .await
                    .map_err(|error| {
                        println!("Failed in deleting ClusterRoleBinding {error}");
                        std::process::exit(1);
                    });

                // Delete cluster role
                let _cl_role =
                    uo.cluster_role_actions(ns, Actions::Delete)
                        .await
                        .map_err(|error| {
                            println!("Failed in deleting ClusterRole {error}");
                            std::process::exit(1);
                        });

                // Delete service account
                let _svc_acc = uo
                    .service_account_actions(ns, Actions::Delete)
                    .await
                    .map_err(|error| {
                        println!("Failed in deleting ServiceAccount {error}");
                        std::process::exit(1);
                    });
            }
            Err(e) => println!("Failed to uninstall. Error {e}"),
        };
    }
}

pub async fn get_pvc_from_uuid(uuid_list: HashSet<String>) -> Result<Vec<String>, Error> {
    let client = Client::try_default().await?;
    let pvclaim = Api::<PersistentVolumeClaim>::all(client.clone());
    let lp = ListParams::default();
    let pvc_list = pvclaim.list(&lp).await?;
    let mut single_replica_volumes_pvc = Vec::new();
    for pvc in pvc_list {
        if let Some(uuid) = pvc.metadata.uid {
            if uuid_list.contains(&uuid) {
                if let Some(pvc_name) = pvc.metadata.name {
                    single_replica_volumes_pvc.push(pvc_name);
                }
            }
        }
    }
    Ok(single_replica_volumes_pvc)
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

/// Return true if upgrade job is completed
pub async fn upgrade_job_completed(ns: &str) -> Result<bool, Error> {
    match UpgradeResources::new(ns).await {
        Ok(uo) => {
            let job_name = upgrade_name_concat(&uo.release_name, UPGRADE_JOB_NAME_SUFFIX);
            let option_job = uo
                .job
                .get_opt(&job_name)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;
            match option_job {
                Some(job) => {
                    if matches!(
                        job.status
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("upgrade job.status is empty"))?
                            .succeeded
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("upgrade job has not completed yet."))?,
                        1
                    ) {
                        return Ok(true);
                    }
                }
                None => {
                    eprintln!("Upgrade job {job_name} in namespace {ns} does not exist");
                }
            }
        }
        Err(error) => {
            return Err(error);
        }
    }
    Ok(false)
}