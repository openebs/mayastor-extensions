use crate::{
    constant::{
        get_image_version_tag, upgrade_event_selector, upgrade_image_concat, upgrade_name_concat,
        AGENT_CORE_POD_LABEL, API_REST_LABEL_SELECTOR, API_REST_POD_LABEL, DEFAULT_IMAGE_REGISTRY,
        DEFAULT_RELEASE_NAME, HELM_RELEASE_NAME_LABEL, HELM_RELEASE_VERSION_LABEL,
        IO_ENGINE_POD_LABEL, UPGRADE_EVENT_REASON, UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
        UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX, UPGRADE_JOB_IMAGE_NAME, UPGRADE_JOB_IMAGE_REPO,
        UPGRADE_JOB_NAME_SUFFIX, UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
    },
    error::Error,
    upgrade_resources::objects,
    user_prompt::{
        upgrade_dry_run_summary, CONTROL_PLANE_PODS_LIST, DATA_PLANE_PODS_LIST,
        DATA_PLANE_PODS_LIST_SKIP_RESTART, UPGRADE_DRY_RUN_SUMMARY, UPGRADE_JOB_STARTED,
    },
};
use serde::Deserialize;

use k8s_openapi::api::{
    apps::v1::Deployment,
    batch::v1::Job,
    core::v1::{Event, PersistentVolumeClaim, Pod, ServiceAccount},
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
    Upgrade(DeleteUpgradeArgs),
}

/// Delete Upgrade resource.
#[derive(clap::Args, Debug)]
pub struct DeleteUpgradeArgs {
    /// If set then upgrade will skip the io-engine pods restart
    #[clap(global = true, hide = true, long, short = 'u')]
    pub upgrade_to_branch: Option<String>,
}

impl DeleteUpgradeArgs {
    /// Delete the upgrade resources
    pub async fn delete(&self, ns: &str) {
        // Delete upgrade resources once job completes
        match upgrade_job_completed(ns, self.upgrade_to_branch.as_ref()).await {
            Ok(job_completed) => {
                if job_completed {
                    UpgradeResources::delete_upgrade_resources(ns, self.upgrade_to_branch.as_ref())
                        .await;
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

    /// If set then upgrade will skip the io-engine pods restart.
    #[clap(global = true, long, short = 'D', default_value_t = false)]
    pub skip_data_plane_restart: bool,

    /// If set then it will continue with upgrade without validating singla replica volume.
    #[clap(global = true, long, short = 'S')]
    pub skip_single_replica_volume_validation: bool,

    /// If set then upgrade will skip the repilca rebuild in progress validation.
    #[clap(global = true, long, short = 'R')]
    pub skip_replica_rebuild: bool,

    /// If set then upgrade will skip the cordoned node validation.
    #[clap(global = true, long, short = 'C')]
    pub skip_cordoned_node_validation: bool,

    /// Upgrade to the specified branch.
    #[clap(global = true, hide = true, long, short = 'u')]
    pub upgrade_to_branch: Option<String>,
}

impl UpgradeArgs {
    ///  Upgrade the resources.
    pub async fn apply(&self, namespace: &str) {
        // Create resources for upgrade
        UpgradeResources::create_upgrade_resources(
            namespace,
            self.skip_data_plane_restart,
            self.upgrade_to_branch.as_ref(),
        )
        .await;
        console_logger::info(UPGRADE_JOB_STARTED, "");
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
            upgrade_dry_run_summary(UPGRADE_DRY_RUN_SUMMARY).as_str(),
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
    let client = Client::try_default()
        .await
        .map_err(|source| Error::K8sClientError { source })?;
    let pods: Api<Pod> = Api::namespaced(client, namespace);
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
        // Create resources for getting upgrade status
        match UpgradeEventClient::create_get_upgrade_resource(namespace).await {
            Ok(_) => {}
            Err(error) => eprintln! {"Failure {error}"},
        }
    }
}

/// This struct is used to deserialize the output of ugrade events.
#[derive(Clone, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub(crate) struct UpgradeEvent {
    from_version: String,
    to_version: String,
    message: String,
}

/// Resource to be created to get upgrade status.
struct UpgradeEventClient {
    upgrade_event: Api<Event>,
}

/// Methods implemented by UpgradeEventClient.
impl UpgradeEventClient {
    pub async fn new(ns: &str) -> Result<Self, Error> {
        let client = Client::try_default().await?;
        Ok(Self {
            upgrade_event: Api::<Event>::namespaced(client, ns),
        })
    }

    /// Returns an client for upgrade events.
    pub(crate) fn api_client(&self) -> Api<Event> {
        self.upgrade_event.clone()
    }

    /// Fetch upgrade events and print.
    pub async fn get_and_log(&self, release_name: String) -> Result<(), Error> {
        let event_lp = ListParams {
            field_selector: Some(upgrade_event_selector(
                release_name.as_str(),
                UPGRADE_JOB_NAME_SUFFIX,
            )),
            ..Default::default()
        };

        let mut event_list = self
            .api_client()
            .list(&event_lp)
            .await?
            .into_iter()
            .filter(|e| e.reason == Some(UPGRADE_EVENT_REASON.to_string()))
            .collect::<Vec<_>>();

        event_list.sort_by(|a, b| b.event_time.cmp(&a.event_time));
        match event_list.get(0) {
            Some(event) => match event.message.clone() {
                Some(data) => {
                    let e: UpgradeEvent = serde_json::from_str(data.as_str())
                        .map_err(|_| Error::EventSerdeDeserializationError)?;
                    println!("Upgrade From: {}", e.from_version);
                    println!("Upgrade To: {}", e.to_version);
                    println!("Upgrade Status: {}", e.message);
                    Ok(())
                }
                None => Err(Error::MessageInEventNotPresent),
            },
            None => Err(Error::UpgradeEventNotPresent),
        }
    }

    /// Create resources for fetching upgrade events.
    pub async fn create_get_upgrade_resource(ns: &str) -> Result<(), Error> {
        let release_name = get_release_name(ns).await?;
        let gur = UpgradeEventClient::new(ns).await?;
        gur.get_and_log(release_name).await?;
        Ok(())
    }
}

/// K8s resources needed for upgrade operator.
struct UpgradeResources {
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

    /// Create/Delete ServiceAction.
    pub async fn service_account_actions(
        &self,
        ns: &str,
        action: Actions,
        upgrade_to_branch: Option<&String>,
    ) -> Result<(), Error> {
        let service_account_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
            upgrade_to_branch,
        );

        if let Some(sa) = self.service_account.get_opt(&service_account_name).await? {
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
                        .delete(&service_account_name, &DeleteParams::default())
                        .await
                    {
                        Ok(_) => {
                            println!(
                                "ServiceAccount {service_account_name} in namespace {ns} deleted"
                            );
                        }
                        Err(source) => {
                            return Err(Error::ServiceAccountDeleteError { source });
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let ns = Some(ns.to_string());
                    let service_account =
                        objects::upgrade_job_service_account(ns, service_account_name);
                    let pp = PostParams::default();
                    match self.service_account.create(&pp, &service_account).await {
                        Ok(sa) => {
                            println!(
                                "ServiceAccount: {} created in namespace: {}",
                                sa.metadata.name.unwrap(),
                                sa.metadata.namespace.unwrap()
                            )
                        }
                        Err(source) => {
                            return Err(Error::ServiceAccountCreateError { source });
                        }
                    }
                }
                Actions::Delete => {
                    println!(
                        "ServiceAccount {service_account_name} in namespace {ns} does not exist"
                    );
                }
            }
        }
        Ok(())
    }

    /// Create/Delete cluster role
    pub async fn cluster_role_actions(
        &self,
        ns: &str,
        action: Actions,
        upgrade_to_branch: Option<&String>,
    ) -> Result<(), Error> {
        let cluster_role_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX,
            upgrade_to_branch,
        );

        if let Some(cr) = self.cluster_role.get_opt(&cluster_role_name).await? {
            match action {
                Actions::Create => {
                    println!(
                        "ClusterRole: {}  in namespace {} already exist",
                        cr.metadata.name.as_ref().unwrap(),
                        ns
                    );
                }
                Actions::Delete => {
                    match self
                        .cluster_role
                        .delete(&cluster_role_name, &DeleteParams::default())
                        .await
                    {
                        Ok(_) => {
                            println!("ClusterRole {cluster_role_name} in namespace {ns} deleted");
                        }
                        Err(source) => {
                            return Err(Error::ClusterRoleDeleteError { source });
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let namespace = Some(ns.to_string());
                    let role = objects::upgrade_job_cluster_role(namespace, cluster_role_name);
                    let pp = PostParams::default();
                    match self.cluster_role.create(&pp, &role).await {
                        Ok(cr) => {
                            println!(
                                "Cluster Role: {} in namespace {} created",
                                cr.metadata.name.unwrap(),
                                ns
                            );
                        }
                        Err(source) => {
                            return Err(Error::ClusterRoleCreateError { source });
                        }
                    }
                }
                Actions::Delete => {
                    println!("cluster role {cluster_role_name} in namespace {ns} does not exist");
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
        upgrade_to_branch: Option<&String>,
    ) -> Result<(), Error> {
        let cluster_role_binding_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
            upgrade_to_branch,
        );
        if let Some(crb) = self
            .cluster_role_binding
            .get_opt(&cluster_role_binding_name)
            .await?
        {
            match action {
                Actions::Create => {
                    println!(
                        "ClusterRoleBinding: {} in namespace {} already exist",
                        crb.metadata.name.as_ref().unwrap(),
                        ns
                    );
                }
                Actions::Delete => {
                    match self
                        .cluster_role_binding
                        .delete(&cluster_role_binding_name, &DeleteParams::default())
                        .await
                    {
                        Ok(_) => {
                            println!(
                                "ClusterRoleBinding {cluster_role_binding_name} in namespace {ns} deleted"
                            );
                        }
                        Err(source) => {
                            return Err(Error::ClusterRoleBindingDeleteError { source });
                        }
                    }
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let namespace = Some(ns.to_string());
                    let role_binding = objects::upgrade_job_cluster_role_binding(
                        namespace,
                        self.release_name.clone(),
                        upgrade_to_branch,
                    );
                    let pp = PostParams::default();
                    match self.cluster_role_binding.create(&pp, &role_binding).await {
                        Ok(crb) => {
                            println!(
                                "ClusterRoleBinding: {} in namespace {} created",
                                crb.metadata.name.unwrap(),
                                ns
                            );
                        }
                        Err(source) => {
                            return Err(Error::ClusterRoleBindingCreateError { source });
                        }
                    }
                }
                Actions::Delete => {
                    println!(
                        "ClusterRoleBinding {cluster_role_binding_name} in namespace {ns} does not exist"
                    );
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
        upgrade_to_branch: Option<&String>,
    ) -> Result<(), Error> {
        let job_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_NAME_SUFFIX,
            upgrade_to_branch,
        );
        if let Some(job) = self.job.get_opt(&job_name).await? {
            match action {
                Actions::Create => {
                    println!(
                        "Job: {} in namespace: {} already exist",
                        job.metadata.name.as_ref().unwrap(),
                        job.metadata.namespace.as_ref().unwrap()
                    );
                }
                Actions::Delete => match self.job.delete(&job_name, &DeleteParams::default()).await
                {
                    Ok(_) => {
                        println!("Job {job_name} in namespace {ns} deleted");
                    }
                    Err(source) => {
                        return Err(Error::UpgradeJobDeleteError { source });
                    }
                },
            }
        } else {
            match action {
                Actions::Create => {
                    let upgrade_job_image_tag = get_image_version_tag();
                    let rest_deployment = get_deployment_for_rest(ns).await?;
                    let img = ImageProperties::try_from(rest_deployment)?;
                    let upgrade_deploy = objects::upgrade_job(
                        ns,
                        upgrade_image_concat(
                            img.registry().as_str(),
                            UPGRADE_JOB_IMAGE_REPO,
                            UPGRADE_JOB_IMAGE_NAME,
                            upgrade_job_image_tag.as_str(),
                        ),
                        self.release_name.clone(),
                        skip_data_plane_restart,
                        upgrade_to_branch,
                        img.pull_secrets(),
                        img.pull_policy(),
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
                        Err(source) => {
                            return Err(Error::UpgradeJobCreateError { source });
                        }
                    }
                }
                Actions::Delete => {
                    println!("Job {job_name} in namespace {ns} does not exist");
                }
            }
        }
        Ok(())
    }

    /// Create the resources for upgrade
    pub async fn create_upgrade_resources(
        ns: &str,
        skip_data_plane_restart: bool,
        upgrade_to_branch: Option<&String>,
    ) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                // Create Service Account
                let _svc_acc = uo
                    .service_account_actions(ns, Actions::Create, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Create Cluser role
                let _cl_role = uo
                    .cluster_role_actions(ns, Actions::Create, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Create Cluster role binding
                let _cl_role_binding = uo
                    .cluster_role_binding_actions(ns, Actions::Create, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Create Service Account
                let _job = uo
                    .job_actions(
                        ns,
                        Actions::Create,
                        skip_data_plane_restart,
                        upgrade_to_branch,
                    )
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });
            }
            Err(e) => println!("Failed to create upgrade resources. Error {e:?}"),
        };
    }

    /// Delete the upgrade resources
    pub async fn delete_upgrade_resources(ns: &str, upgrade_to_branch: Option<&String>) {
        match UpgradeResources::new(ns).await {
            Ok(uo) => {
                // Delete the job
                let _job = uo
                    .job_actions(ns, Actions::Delete, false, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Delete cluster role binding
                let _cl_role_binding = uo
                    .cluster_role_binding_actions(ns, Actions::Delete, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Delete cluster role
                let _cl_role = uo
                    .cluster_role_actions(ns, Actions::Delete, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Delete service account
                let _svc_acc = uo
                    .service_account_actions(ns, Actions::Delete, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        println!("Failure: {error}");
                        std::process::exit(1);
                    });
            }
            Err(e) => println!("Failed to uninstall. Error {e}"),
        };
    }
}

pub async fn get_pvc_from_uuid(uuid_list: HashSet<String>) -> Result<Vec<String>, Error> {
    let client = Client::try_default().await?;
    let pvclaim = Api::<PersistentVolumeClaim>::all(client);
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
pub async fn get_deployment_for_rest(ns: &str) -> Result<Deployment, Error> {
    let client = Client::try_default().await?;
    let deployment = Api::<Deployment>::namespaced(client, ns);
    let lp = ListParams::default().labels(API_REST_LABEL_SELECTOR);
    let deployment_list = deployment.list(&lp).await?;
    let deployment = deployment_list
        .items
        .first()
        .ok_or(Error::NoDeploymentPresent)?
        .clone();

    Ok(deployment)
}

/// Return the release name.
pub async fn get_release_name(ns: &str) -> Result<String, Error> {
    match get_deployment_for_rest(ns).await {
        Ok(deployment) => match &deployment.metadata.labels {
            Some(label) => match label.get(HELM_RELEASE_NAME_LABEL) {
                Some(value) => Ok(value.to_string()),
                None => Ok(DEFAULT_RELEASE_NAME.to_string()),
            },
            None => Ok(DEFAULT_RELEASE_NAME.to_string()),
        },

        Err(e) => {
            eprintln!("Failed in fetching deployment {e:?}");
            std::process::exit(1);
        }
    }
}

/// Return true if upgrade job is completed
pub async fn upgrade_job_completed(
    ns: &str,
    upgrade_to_branch: Option<&String>,
) -> Result<bool, Error> {
    match UpgradeResources::new(ns).await {
        Ok(uo) => {
            let job_name =
                upgrade_name_concat(&uo.release_name, UPGRADE_JOB_NAME_SUFFIX, upgrade_to_branch);
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

struct ImageProperties {
    pull_secrets: Option<Vec<k8s_openapi::api::core::v1::LocalObjectReference>>,
    registry: String,
    pull_policy: Option<String>,
}

impl TryFrom<Deployment> for ImageProperties {
    type Error = crate::error::Error;

    fn try_from(d: Deployment) -> Result<Self, Error> {
        let pod_spec = d
            .spec
            .ok_or(Error::ReferenceDeploymentNoSpec)?
            .template
            .spec
            .ok_or(Error::ReferenceDeploymentNoPodTemplateSpec)?;

        let container = pod_spec
            .containers
            .first()
            .ok_or(Error::ReferenceDeploymentNoContainers)?;

        let image = container
            .image
            .clone()
            .ok_or(Error::ReferenceDeploymentNoImage)?;

        let image_sections: Vec<&str> = image.split('/').collect();
        if image_sections.is_empty() || image_sections.len() == 1 {
            return Err(Error::ReferenceDeploymentInvalidImage);
        }

        let registry = match image_sections.len() {
            3 => image_sections[0],
            _ => DEFAULT_IMAGE_REGISTRY,
        }
        .to_string();

        Ok(Self {
            pull_secrets: pod_spec.image_pull_secrets.clone(),
            registry,
            pull_policy: container.image_pull_policy.clone(),
        })
    }
}

impl ImageProperties {
    fn pull_secrets(&self) -> Option<Vec<k8s_openapi::api::core::v1::LocalObjectReference>> {
        self.pull_secrets.clone()
    }

    fn registry(&self) -> String {
        self.registry.clone()
    }

    fn pull_policy(&self) -> Option<String> {
        self.pull_policy.clone()
    }
}

/// Return the installed version.
pub async fn get_source_version(ns: &str) -> Result<String, Error> {
    match get_deployment_for_rest(ns).await {
        Ok(deployment) => match &deployment.metadata.labels {
            Some(label) => match label.get(HELM_RELEASE_VERSION_LABEL) {
                Some(value) => Ok("v".to_owned() + value),
                None => Err(Error::UpgradeEventNotPresent),
            },
            None => Err(Error::UpgradeEventNotPresent),
        },
        Err(error) => Err(error),
    }
}
