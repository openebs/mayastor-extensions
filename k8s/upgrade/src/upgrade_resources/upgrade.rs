use crate::{
    constant::{
        get_image_version_tag, upgrade_event_selector, upgrade_image_concat, upgrade_name_concat,
        AGENT_CORE_POD_LABEL, API_REST_LABEL_SELECTOR, API_REST_POD_LABEL, DEFAULT_IMAGE_REGISTRY,
        DEFAULT_RELEASE_NAME, HELM_RELEASE_NAME_LABEL, HELM_RELEASE_VERSION_LABEL,
        IO_ENGINE_POD_LABEL, UPGRADE_EVENT_REASON, UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
        UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX, UPGRADE_JOB_IMAGE_NAME, UPGRADE_JOB_IMAGE_REPO,
        UPGRADE_JOB_NAME_SUFFIX, UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
    },
    error,
    upgrade_resources::objects,
    user_prompt::{
        upgrade_dry_run_summary, CONTROL_PLANE_PODS_LIST, DATA_PLANE_PODS_LIST,
        DATA_PLANE_PODS_LIST_SKIP_RESTART, UPGRADE_DRY_RUN_SUMMARY, UPGRADE_JOB_STARTED,
    },
};
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
use serde::Deserialize;
use snafu::ResultExt;
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

        match is_upgrade_job_completed(ns, self.upgrade_to_branch.as_ref()).await {
            Ok(job_completed) => {
                if job_completed {
                    UpgradeResources::delete_upgrade_resources(ns, self.upgrade_to_branch.as_ref())
                        .await;
                }
            }
            Err(error) => {
                eprintln!(
                    "error occured while fetching the completion status of upgrade job {error}"
                );
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
    pub async fn dummy_apply(&self, namespace: &str) -> error::Result<()> {
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

pub(crate) async fn list_pods(
    label: &str,
    namespace: &str,
    pods_names: &mut Vec<String>,
) -> error::Result<()> {
    let client = Client::try_default().await.context(error::K8sClient)?;
    let pods: Api<Pod> = Api::namespaced(client, namespace);
    let pod_list: ObjectList<Pod> = pods
        .list(&ListParams::default().labels(label))
        .await
        .context(error::ListPodsWithLabel {
            label: label.to_string(),
            namespace: namespace.to_string(),
        })?;

    for pod in pod_list.iter() {
        // Fetch the pod name
        let pod_name = pod
            .metadata
            .name
            .as_ref()
            .ok_or(error::PodNameNotPresent.build())?
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
    pub async fn new(ns: &str) -> error::Result<Self> {
        let client = Client::try_default()
            .await
            .context(error::K8sClientGeneration)?;
        Ok(Self {
            upgrade_event: Api::<Event>::namespaced(client, ns),
        })
    }

    /// Returns an client for upgrade events.
    pub(crate) fn api_client(&self) -> Api<Event> {
        self.upgrade_event.clone()
    }

    /// Fetch upgrade events and print.
    pub async fn get_and_log(&self, release_name: String) -> error::Result<()> {
        let selector = upgrade_event_selector(release_name.as_str(), UPGRADE_JOB_NAME_SUFFIX);
        let event_lp = ListParams {
            field_selector: Some(selector.clone()),
            ..Default::default()
        };

        let mut event_list = self
            .api_client()
            .list(&event_lp)
            .await
            .context(error::ListEventsWithFieldSelector { field: selector })?
            .into_iter()
            .filter(|e| e.reason == Some(UPGRADE_EVENT_REASON.to_string()))
            .collect::<Vec<_>>();

        event_list.sort_by(|a, b| b.event_time.cmp(&a.event_time));
        match event_list.get(0) {
            Some(event) => match event.message.clone() {
                Some(data) => {
                    let e: UpgradeEvent = serde_json::from_str(data.as_str())
                        .context(error::EventSerdeDeserialization { event: data })?;
                    println!("Upgrade From: {}", e.from_version);
                    println!("Upgrade To: {}", e.to_version);
                    println!("Upgrade Status: {}", e.message);
                    Ok(())
                }
                None => error::MessageInEventNotPresent.fail(),
            },
            None => error::UpgradeEventNotPresent.fail(),
        }
    }

    /// Create resources for fetching upgrade events.
    pub async fn create_get_upgrade_resource(ns: &str) -> error::Result<()> {
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
    pub async fn new(ns: &str) -> error::Result<Self> {
        let client = Client::try_default()
            .await
            .context(error::K8sClientGeneration)?;
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
    ) -> error::Result<()> {
        let service_account_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
            upgrade_to_branch,
        );

        if let Some(sa) = self
            .service_account
            .get_opt(&service_account_name)
            .await
            .context(error::GetServiceAccount {
                name: service_account_name.clone(),
            })?
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
                    let _sa = self
                        .service_account
                        .delete(&service_account_name, &DeleteParams::default())
                        .await
                        .context(error::ServiceAccountDelete {
                            name: service_account_name.clone(),
                        })?;
                    println!("ServiceAccount {service_account_name} in namespace {ns} deleted");
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let ns = Some(ns.to_string());
                    let service_account =
                        objects::upgrade_job_service_account(ns, service_account_name.clone());
                    let pp = PostParams::default();
                    let sa = self
                        .service_account
                        .create(&pp, &service_account)
                        .await
                        .context(error::ServiceAccountCreate {
                            name: service_account_name.clone(),
                        })?;
                    println!(
                        "ServiceAccount: {} created in namespace: {}",
                        sa.metadata.name.unwrap(),
                        sa.metadata.namespace.unwrap()
                    );
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
    ) -> error::Result<()> {
        let cluster_role_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX,
            upgrade_to_branch,
        );

        if let Some(cr) = self
            .cluster_role
            .get_opt(&cluster_role_name)
            .await
            .context(error::GetClusterRole {
                name: cluster_role_name.clone(),
            })?
        {
            match action {
                Actions::Create => {
                    println!(
                        "ClusterRole: {}  in namespace {} already exist",
                        cr.metadata.name.as_ref().unwrap(),
                        ns
                    );
                }
                Actions::Delete => {
                    let _role = self
                        .cluster_role
                        .delete(&cluster_role_name, &DeleteParams::default())
                        .await
                        .context(error::ClusterRoleDelete {
                            name: cluster_role_name.clone(),
                        })?;
                    println!("ClusterRole {cluster_role_name} in namespace {ns} deleted");
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let namespace = Some(ns.to_string());
                    let role =
                        objects::upgrade_job_cluster_role(namespace, cluster_role_name.clone());
                    let pp = PostParams::default();
                    let cr = self.cluster_role.create(&pp, &role).await.context(
                        error::ClusterRoleCreate {
                            name: cluster_role_name,
                        },
                    )?;
                    println!(
                        "Cluster Role: {} in namespace {} created",
                        cr.metadata.name.unwrap(),
                        ns
                    );
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
    ) -> error::Result<()> {
        let cluster_role_binding_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
            upgrade_to_branch,
        );
        if let Some(crb) = self
            .cluster_role_binding
            .get_opt(&cluster_role_binding_name)
            .await
            .context(error::GetClusterRoleBinding {
                name: cluster_role_binding_name.clone(),
            })?
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
                    let _crb = self
                        .cluster_role_binding
                        .delete(&cluster_role_binding_name, &DeleteParams::default())
                        .await
                        .context(error::ClusterRoleBindingDelete {
                            name: cluster_role_binding_name.clone(),
                        })?;
                    println!(
                        "ClusterRoleBinding {cluster_role_binding_name} in namespace {ns} deleted"
                    );
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
                    let crb = self
                        .cluster_role_binding
                        .create(&pp, &role_binding)
                        .await
                        .context(error::ClusterRoleBindingCreate {
                            name: cluster_role_binding_name,
                        })?;
                    println!(
                        "ClusterRoleBinding: {} in namespace {} created",
                        crb.metadata.name.unwrap(),
                        ns
                    );
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
    ) -> error::Result<()> {
        let job_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_NAME_SUFFIX,
            upgrade_to_branch,
        );
        if let Some(job) = self
            .job
            .get_opt(&job_name)
            .await
            .context(error::GetUpgradeJob {
                name: job_name.clone(),
            })?
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
                    let _job = self
                        .job
                        .delete(&job_name, &DeleteParams::default())
                        .await
                        .context(error::UpgradeJobDelete {
                            name: job_name.clone(),
                        })?;
                    println!("Job {job_name} in namespace {ns} deleted");
                }
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
                    let dep = self
                        .job
                        .create(&PostParams::default(), &upgrade_deploy)
                        .await
                        .context(error::UpgradeJobCreate { name: job_name })?;
                    println!(
                        "Job: {} created in namespace: {}",
                        dep.metadata.name.unwrap(),
                        dep.metadata.namespace.unwrap()
                    );
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
                        eprintln!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Create Cluser role
                let _cl_role = uo
                    .cluster_role_actions(ns, Actions::Create, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        eprintln!("Failure: {error}");
                        std::process::exit(1);
                    });

                // Create Cluster role binding
                let _cl_role_binding = uo
                    .cluster_role_binding_actions(ns, Actions::Create, upgrade_to_branch)
                    .await
                    .map_err(|error| {
                        eprintln!("Failure: {error}");
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
                        eprintln!("Failure: {error}");
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

pub async fn get_pvc_from_uuid(uuid_list: HashSet<String>) -> error::Result<Vec<String>> {
    let client = Client::try_default().await.context(error::K8sClient)?;
    let pvclaim = Api::<PersistentVolumeClaim>::all(client);
    let lp = ListParams::default();
    let pvc_list = pvclaim.list(&lp).await.context(error::ListPVC)?;
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
pub async fn get_deployment_for_rest(ns: &str) -> error::Result<Deployment> {
    let client = Client::try_default().await.context(error::K8sClient)?;
    let deployment = Api::<Deployment>::namespaced(client, ns);
    let lp = ListParams::default().labels(API_REST_LABEL_SELECTOR);
    let deployment_list = deployment
        .list(&lp)
        .await
        .context(error::ListDeploymentsWithLabel {
            label: API_REST_LABEL_SELECTOR.to_string(),
            namespace: ns.to_string(),
        })?;
    let deployment = deployment_list
        .items
        .first()
        .ok_or(error::NoDeploymentPresent.build())?
        .clone();

    Ok(deployment)
}

/// Return the release name.
pub async fn get_release_name(ns: &str) -> error::Result<String> {
    let deployment = get_deployment_for_rest(ns).await?;
    match &deployment.metadata.labels {
        Some(label) => match label.get(HELM_RELEASE_NAME_LABEL) {
            Some(value) => Ok(value.to_string()),
            None => Ok(DEFAULT_RELEASE_NAME.to_string()),
        },
        None => Ok(DEFAULT_RELEASE_NAME.to_string()),
    }
}

/// Return true if upgrade job is completed
pub async fn is_upgrade_job_completed(
    ns: &str,
    upgrade_to_branch: Option<&String>,
) -> error::Result<bool> {
    let uo = UpgradeResources::new(ns).await?;
    let job_name =
        upgrade_name_concat(&uo.release_name, UPGRADE_JOB_NAME_SUFFIX, upgrade_to_branch);
    let option_job = uo
        .job
        .get_opt(&job_name)
        .await
        .context(error::GetUpgradeJob {
            name: job_name.clone(),
        })?;
    match option_job {
        Some(job) => {
            if matches!(
                job.status
                    .as_ref()
                    .ok_or(
                        error::UpgradeJobStatusNotPresent {
                            name: job_name.clone()
                        }
                        .build()
                    )?
                    .succeeded
                    .as_ref()
                    .ok_or(
                        error::UpgradeJobNotCompleted {
                            name: job_name.clone()
                        }
                        .build()
                    )?,
                1
            ) {
                return Ok(true);
            }
        }
        None => {
            eprintln!("Upgrade job {job_name} in namespace {ns} does not exist");
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

    fn try_from(d: Deployment) -> error::Result<Self> {
        let pod_spec = d
            .spec
            .ok_or(error::ReferenceDeploymentNoSpec.build())?
            .template
            .spec
            .ok_or(error::ReferenceDeploymentNoPodTemplateSpec.build())?;

        let container = pod_spec
            .containers
            .first()
            .ok_or(error::ReferenceDeploymentNoContainers.build())?;

        let image = container
            .image
            .clone()
            .ok_or(error::ReferenceDeploymentNoImage.build())?;

        let image_sections: Vec<&str> = image.split('/').collect();
        if image_sections.is_empty() || image_sections.len() == 1 {
            return error::ReferenceDeploymentInvalidImage.fail();
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
pub async fn get_source_version(ns: &str) -> error::Result<String> {
    let deployment = get_deployment_for_rest(ns).await?;
    let value = &deployment
        .metadata
        .labels
        .ok_or(error::NoDeploymentPresent.build())?
        .get(HELM_RELEASE_VERSION_LABEL)
        .ok_or(error::NoDeploymentPresent.build())?
        .to_string();
    Ok(value.to_string())
}
