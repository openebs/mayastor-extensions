use crate::plugin::{
    constants::{
        get_image_version_tag, upgrade_event_selector, upgrade_image_concat, upgrade_job_img,
        upgrade_name_concat, AGENT_CORE_POD_LABEL, API_REST_LABEL_SELECTOR, API_REST_POD_LABEL,
        DEFAULT_IMAGE_REGISTRY, DEFAULT_RELEASE_NAME, IO_ENGINE_POD_LABEL, MAX_RETRY_ATTEMPTS,
        UPGRADE_CONFIG_MAP_MOUNT_PATH, UPGRADE_CONFIG_MAP_NAME_SUFFIX,
        UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX, UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX,
        UPGRADE_JOB_IMAGE_REPO, UPGRADE_JOB_NAME_SUFFIX, UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX,
    },
    error, objects,
    user_prompt::{
        upgrade_dry_run_summary, CONTROL_PLANE_PODS_LIST, DATA_PLANE_PODS_LIST,
        DATA_PLANE_PODS_LIST_SKIP_RESTART, DELETE_INCOMPLETE_JOB, HELM_UPGRADE_VALIDATION_ERROR,
        UPGRADE_DRY_RUN_SUMMARY, UPGRADE_JOB_STARTED,
    },
};
use constants::{helm_release_name_key, helm_release_version_key, upgrade_event_reason};
use k8s_openapi::api::{
    apps::v1::Deployment,
    batch::v1::Job,
    core::v1::{ConfigMap, Event, PersistentVolumeClaim, Pod, ServiceAccount},
    rbac::v1::{ClusterRole, ClusterRoleBinding},
};
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    core::ObjectList,
    Client,
};
use serde::Deserialize;
use snafu::ResultExt;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    time::Duration,
};

/// Arguments to be passed for upgrade.
#[derive(clap::Subcommand, Debug)]
pub enum DeleteResources {
    /// Delete upgrade resources
    Upgrade(DeleteUpgradeArgs),
}

/// Delete Upgrade resource.
#[derive(clap::Args, Debug)]
pub struct DeleteUpgradeArgs {
    /// If true, immediately remove upgrade resources bypass graceful deletion.
    #[clap(global = false, long, short, default_value_t = false)]
    pub force: bool,
}

impl DeleteUpgradeArgs {
    /// Delete the upgrade resources
    pub async fn delete(&self, ns: &str) -> error::Result<()> {
        match is_upgrade_job_completed(ns).await {
            Ok(job_completed) => {
                if !job_completed && !self.force {
                    console_logger::error("", DELETE_INCOMPLETE_JOB);
                }
                if job_completed || self.force {
                    UpgradeResources::delete_upgrade_resources(ns).await?;
                }
                Ok(())
            }
            Err(error) => Err(error),
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
    /// Specify the container registry for the upgrade-job image.
    #[clap(global = true, long)]
    registry: Option<String>,

    /// Allow upgrade from stable versions to unstable versions. This is implied when the
    /// '--skip-upgrade-path-validation-for-unsupported-version' option is used.
    #[clap(global = true, long, hide = true)]
    pub allow_unstable: bool,

    /// Display all the validations output but will not execute upgrade.
    #[clap(global = true, long, short)]
    pub dry_run: bool,

    /// If set then upgrade will skip the io-engine pods restart.
    #[clap(global = true, long, default_value_t = false)]
    pub skip_data_plane_restart: bool,

    /// If set then it will continue with upgrade without validating singla replica volume.
    #[clap(global = true, long)]
    pub skip_single_replica_volume_validation: bool,

    /// If set then upgrade will skip the replica rebuild in progress validation.
    #[clap(global = true, long)]
    pub skip_replica_rebuild: bool,

    /// If set then upgrade will skip the cordoned node validation.
    #[clap(global = true, long)]
    pub skip_cordoned_node_validation: bool,

    /// Upgrade to an unsupported version.
    #[clap(global = true, hide = true, long, default_value_t = false)]
    pub skip_upgrade_path_validation_for_unsupported_version: bool,

    /// The set values on the command line.
    /// (can specify multiple or separate values with commas: key1=val1,key2=val2).
    #[clap(global = true, long)]
    pub set: Vec<String>,

    /// The set values from respective files specified via the command line
    /// (can specify multiple or separate values with commas: key1=path1,key2=path2).
    #[clap(global = true, long)]
    pub set_file: Vec<String>,

    /// Use helm's --reset-then-reuse-values option.
    #[arg(long, default_value_t = false)]
    pub reset_then_reuse_values: bool,
}

impl Default for UpgradeArgs {
    fn default() -> Self {
        Self::new()
    }
}

impl UpgradeArgs {
    /// Initialise with default values.
    pub fn new() -> Self {
        Self {
            registry: None,
            allow_unstable: false,
            dry_run: false,
            skip_data_plane_restart: false,
            skip_single_replica_volume_validation: false,
            skip_replica_rebuild: false,
            skip_cordoned_node_validation: false,
            skip_upgrade_path_validation_for_unsupported_version: false,
            set: Default::default(),
            set_file: Default::default(),
            reset_then_reuse_values: false,
        }
    }
    ///  Upgrade the resources.
    pub async fn apply(&self, namespace: &str) -> error::Result<()> {
        let upgrade_event_client = UpgradeEventClient::new(namespace).await?;
        let release_name = get_release_name(namespace).await?;

        // Delete any previous upgrade events
        upgrade_event_client
            .delete_upgrade_events(release_name.clone())
            .await?;
        // Create resources for upgrade
        UpgradeResources::create_upgrade_resources(namespace, self).await?;

        for _i in 0 .. MAX_RETRY_ATTEMPTS {
            // wait for 10 seconds for the upgrade event to be published
            tokio::time::sleep(Duration::from_secs(10)).await;
            match upgrade_event_client
                .get_latest_upgrade_event(release_name.clone())
                .await
            {
                Ok(latest_event) => self.handle_upgrade_event(latest_event, namespace).await?,
                Err(_) => continue,
            }
            break;
        }

        Ok(())
    }

    /// Handle the event and errors out for invalid helm command.
    async fn handle_upgrade_event(
        &self,
        latest_event: Event,
        namespace: &str,
    ) -> error::Result<()> {
        if let Some(action) = latest_event.action {
            if action == "Validation Failed" {
                if let Some(data) = latest_event.message {
                    let ev: UpgradeEvent = serde_json::from_str(data.as_str())
                        .context(error::EventSerdeDeserialization { event: data })?;
                    console_logger::error(HELM_UPGRADE_VALIDATION_ERROR, ev.message.as_str());

                    UpgradeResources::delete_upgrade_resources(namespace).await?;
                } else {
                    return error::MessageInEventNotPresent.fail();
                }
            } else {
                console_logger::info(UPGRADE_JOB_STARTED, "");
            }
        }
        Ok(())
    }

    /// Execute the upgrade command.
    pub async fn execute(&self, namespace: &str) -> error::Result<()> {
        if self.dry_run {
            self.dummy_apply(namespace).await
        } else {
            self.apply(namespace).await
        }
    }

    /// Dummy upgrade the resources.
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
    pub async fn get_upgrade(&self, namespace: &str) -> error::Result<()> {
        // Create resources for getting upgrade status
        UpgradeEventClient::create_get_upgrade_resource(namespace).await
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

    /// Delete upgrade events.
    pub async fn delete_upgrade_events(&self, release_name: String) -> error::Result<()> {
        let selector = upgrade_event_selector(release_name.as_str(), UPGRADE_JOB_NAME_SUFFIX);
        let event_lp = ListParams {
            field_selector: Some(selector.clone()),
            ..Default::default()
        };
        self.api_client()
            .delete_collection(&DeleteParams::default(), &event_lp)
            .await
            .context(error::DeleteEventsWithFieldSelector { field: selector })?;
        Ok(())
    }

    /// Fetch latest upgrade event.
    pub async fn get_latest_upgrade_event(&self, release_name: String) -> error::Result<Event> {
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
            .filter(|e| e.reason == Some(upgrade_event_reason()))
            .collect::<Vec<_>>();

        event_list.sort_by(|a, b| b.event_time.cmp(&a.event_time));
        let latest_event = event_list
            .get(0)
            .ok_or(error::UpgradeEventNotPresent.build())?;
        Ok(latest_event.to_owned())
    }

    /// Create resources for fetching upgrade events.
    pub async fn create_get_upgrade_resource(ns: &str) -> error::Result<()> {
        let release_name = get_release_name(ns).await?;
        let upgrade_event_client = UpgradeEventClient::new(ns).await?;
        let latest_event = upgrade_event_client
            .get_latest_upgrade_event(release_name)
            .await?;
        log_upgrade_result(&latest_event).await?;
        Ok(())
    }
}

/// Print the upgrade iutput to console.
pub async fn log_upgrade_result(event: &Event) -> error::Result<()> {
    _ = match event.message.clone() {
        Some(data) => {
            let e: UpgradeEvent = serde_json::from_str(data.as_str())
                .context(error::EventSerdeDeserialization { event: data })?;
            println!("Upgrade From: {}", e.from_version);
            println!("Upgrade To: {}", e.to_version);
            println!("Upgrade Status: {}", e.message);
            Ok(())
        }
        None => error::MessageInEventNotPresent.fail(),
    };
    Ok(())
}

/// K8s resources needed for upgrade operator.
struct UpgradeResources {
    pub(crate) service_account: Api<ServiceAccount>,
    pub(crate) cluster_role: Api<ClusterRole>,
    pub(crate) cluster_role_binding: Api<ClusterRoleBinding>,
    pub(crate) job: Api<Job>,
    pub(crate) config_map: Api<ConfigMap>,
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
            config_map: Api::<ConfigMap>::namespaced(client.clone(), ns),
            job: Api::<Job>::namespaced(client, ns),
            release_name,
        })
    }

    /// Create/Delete ServiceAction.
    pub async fn service_account_actions(&self, ns: &str, action: Actions) -> error::Result<()> {
        let service_account_name =
            upgrade_name_concat(&self.release_name, UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX);
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
    pub async fn cluster_role_actions(&self, ns: &str, action: Actions) -> error::Result<()> {
        let cluster_role_name =
            upgrade_name_concat(&self.release_name, UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX);

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
    ) -> error::Result<()> {
        let cluster_role_binding_name = upgrade_name_concat(
            &self.release_name,
            UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX,
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

    /// Create/Delete the upgrade config map used to store files specified by the set_file argument.
    pub async fn config_map_actions(
        &self,
        ns: &str,
        action: Actions,
        args: &UpgradeArgs,
    ) -> error::Result<HashMap<String, String>> {
        let cm_name = upgrade_name_concat(&self.release_name, UPGRADE_CONFIG_MAP_NAME_SUFFIX);
        let data = create_config_map_data(args).await?;
        let cm = self
            .config_map
            .get_opt(&cm_name)
            .await
            .context(error::GetUpgradeConfigMap {
                name: cm_name.clone(),
            })?;
        if cm.is_some() {
            match action {
                Actions::Create => {
                    // delete and recreate every time
                    _ = self
                        .config_map
                        .delete(&cm_name, &DeleteParams::default())
                        .await
                        .context(error::UpgradeConfigMapDelete {
                            name: cm_name.clone(),
                        })?;
                    let cm: ConfigMap =
                        objects::upgrade_configmap(data.0.clone(), ns, self.release_name.clone());
                    let pp = PostParams::default();
                    self.config_map
                        .create(&pp, &cm)
                        .await
                        .context(error::UpgradeConfigMapCreate { name: cm_name })?;
                    println!(
                        "ConfigMap: {} in namespace {} created",
                        cm.metadata.name.unwrap(),
                        ns
                    );
                }
                Actions::Delete => {
                    _ = self
                        .config_map
                        .delete(&cm_name, &DeleteParams::default())
                        .await
                        .context(error::UpgradeConfigMapDelete {
                            name: cm_name.clone(),
                        })?;
                    println!("ConfigMap {cm_name} in namespace {ns} deleted");
                }
            }
        } else {
            match action {
                Actions::Create => {
                    let data = create_config_map_data(args).await?;
                    let cm: ConfigMap =
                        objects::upgrade_configmap(data.0.clone(), ns, self.release_name.clone());
                    let pp = PostParams::default();
                    self.config_map
                        .create(&pp, &cm)
                        .await
                        .context(error::UpgradeConfigMapCreate { name: cm_name })?;
                    println!(
                        "ConfigMap: {} in namespace {} created",
                        cm.metadata.name.unwrap(),
                        ns
                    );
                }
                Actions::Delete => {
                    println!("ConfigMap {cm_name} in namespace {ns} does not exist");
                }
            }
        }
        Ok(data.1.clone())
    }

    /// Create/Delete upgrade job
    pub async fn job_actions(
        &self,
        ns: &str,
        action: Actions,
        args: &UpgradeArgs,
        set_file_map: Option<HashMap<String, String>>,
    ) -> error::Result<()> {
        let job_name = upgrade_name_concat(&self.release_name, UPGRADE_JOB_NAME_SUFFIX);
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
                    let set_file = create_helm_set_file_args(args, set_file_map).await?;

                    // Image registry override check.
                    let registry: &str = match args.registry {
                        Some(ref registry_override) => registry_override,
                        None => img.registry(),
                    };

                    let upgrade_deploy = objects::upgrade_job(
                        ns,
                        upgrade_image_concat(
                            registry,
                            UPGRADE_JOB_IMAGE_REPO,
                            &upgrade_job_img(),
                            upgrade_job_image_tag.as_str(),
                        ),
                        self.release_name.clone(),
                        args,
                        set_file.unwrap_or_default(),
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
    pub async fn create_upgrade_resources(ns: &str, args: &UpgradeArgs) -> error::Result<()> {
        let uo = UpgradeResources::new(ns).await?;

        // Create Service Account
        uo.service_account_actions(ns, Actions::Create).await?;

        // Create Cluser role
        uo.cluster_role_actions(ns, Actions::Create).await?;

        // Create Cluster role binding
        uo.cluster_role_binding_actions(ns, Actions::Create).await?;

        // Create config map
        let set_file_map = uo.config_map_actions(ns, Actions::Create, args).await?;

        // Create Service Account
        uo.job_actions(ns, Actions::Create, args, Some(set_file_map))
            .await?;

        Ok(())
    }

    /// Delete the upgrade resources
    pub async fn delete_upgrade_resources(ns: &str) -> error::Result<()> {
        let uo = UpgradeResources::new(ns).await?;
        let args = &UpgradeArgs::default();

        // Delete the job
        uo.job_actions(ns, Actions::Delete, args, None).await?;

        // Delete config map
        uo.config_map_actions(ns, Actions::Delete, args).await?;

        // Delete cluster role binding
        uo.cluster_role_binding_actions(ns, Actions::Delete).await?;

        // Delete cluster role
        uo.cluster_role_actions(ns, Actions::Delete).await?;

        // Delete service account
        uo.service_account_actions(ns, Actions::Delete).await?;
        Ok(())
    }
}

pub(crate) async fn get_pvc_from_uuid(uuid_list: HashSet<String>) -> error::Result<Vec<String>> {
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
pub(crate) async fn get_deployment_for_rest(ns: &str) -> error::Result<Deployment> {
    let client = Client::try_default().await.context(error::K8sClient)?;
    let deployment = Api::<Deployment>::namespaced(client, ns);
    let lp = ListParams::default().labels(API_REST_LABEL_SELECTOR);
    let deployment_list = deployment
        .list(&lp)
        .await
        .context(error::ListDeploymantsWithLabel {
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
pub(crate) async fn get_release_name(ns: &str) -> error::Result<String> {
    let deployment = get_deployment_for_rest(ns).await?;
    match &deployment.metadata.labels {
        Some(label) => match label.get(&helm_release_name_key()) {
            Some(value) => Ok(value.to_string()),
            None => Ok(DEFAULT_RELEASE_NAME.to_string()),
        },
        None => Ok(DEFAULT_RELEASE_NAME.to_string()),
    }
}

/// Return true if upgrade job is completed
pub(crate) async fn is_upgrade_job_completed(ns: &str) -> error::Result<bool> {
    let uo = UpgradeResources::new(ns).await?;
    let job_name = upgrade_name_concat(&uo.release_name, UPGRADE_JOB_NAME_SUFFIX);
    let option_job = uo
        .job
        .get_opt(&job_name)
        .await
        .context(error::GetUpgradeJob {
            name: job_name.clone(),
        })?;
    match option_job {
        Some(job) => {
            let status = job.status.as_ref().ok_or(
                error::UpgradeJobStatusNotPresent {
                    name: job_name.clone(),
                }
                .build(),
            )?;

            let is_job_completed = match status.succeeded {
                None => false,
                Some(count) => count == 1,
            };
            Ok(is_job_completed)
        }

        None => error::UpgradeJobNotPresent {
            name: job_name,
            namespace: ns,
        }
        .fail(),
    }
}

struct ImageProperties {
    pull_secrets: Option<Vec<k8s_openapi::api::core::v1::LocalObjectReference>>,
    registry: String,
    pull_policy: Option<String>,
}

impl TryFrom<Deployment> for ImageProperties {
    type Error = crate::plugin::error::Error;

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

    fn registry(&self) -> &str {
        self.registry.as_str()
    }

    fn pull_policy(&self) -> Option<String> {
        self.pull_policy.clone()
    }
}

/// Return the installed version.
pub(crate) async fn get_source_version(ns: &str) -> error::Result<String> {
    let deployment = get_deployment_for_rest(ns).await?;
    let value = &deployment
        .metadata
        .labels
        .ok_or(error::NoDeploymentPresent.build())?
        .get(&helm_release_version_key())
        .ok_or(error::NoDeploymentPresent.build())?
        .to_string();
    Ok(value.to_string())
}

/// Parse set-file and create config map data.
pub(crate) async fn create_config_map_data(
    upgrade_args: &UpgradeArgs,
) -> error::Result<(BTreeMap<String, String>, HashMap<String, String>)> {
    let mut data_map = BTreeMap::new();
    let mut upgrade_map = HashMap::new();
    let mut index = 1;
    for file in &upgrade_args.set_file {
        let data: Vec<_> = file.split('=').collect();
        let [_key, filepath] = data[..] else {
            return error::InvalidSetFileArguments {
                arguments: file.to_string(),
            }
            .fail();
        };
        let cm_values = fs::read_to_string(filepath).context(error::ReadFromFile { filepath })?;
        // create config map key with key of set-file
        // Example : -- set-file jaeger-operator.tolerations=/root/tolerations.yaml
        // Key:value = index:content of file
        data_map.insert(index.to_string(), cm_values);
        // upgrade_map keeps a track of index to file name mapping.
        // This is used to create set fiel arguments.
        // Key:value = file absolute path:index ( example: /root/tolerations.yaml:1 )
        upgrade_map.insert(filepath.to_string(), index.to_string());
        index += 1;
    }
    Ok((data_map, upgrade_map))
}

/// Creat helm set file args.
pub(crate) async fn create_helm_set_file_args(
    upgrade_args: &UpgradeArgs,
    set_file_map: Option<HashMap<String, String>>,
) -> error::Result<Option<String>> {
    if !&upgrade_args.set_file.is_empty() {
        let mut helm_args_set_file = Vec::new();
        for file in &upgrade_args.set_file {
            // Example : -- set-file jaeger-operator.tolerations=/root/tolerations.yaml
            // gets converted to jaeger-operator.tolerations=/upgrade-config-map/1
            let data: Vec<_> = file.split('=').collect();
            if let [key, filepath] = data[..] {
                let mapped_file = set_file_map
                    .as_ref()
                    .and_then(|map| map.get(filepath))
                    .ok_or(error::SpecifiedKeyNotPresent.build())?;
                helm_args_set_file.push(format!(
                    "{key}={UPGRADE_CONFIG_MAP_MOUNT_PATH}/{mapped_file}"
                ));
            } else {
                return error::InvalidSetFileArguments {
                    arguments: file.to_string(),
                }
                .fail();
            }
        }
        return Ok(Some(helm_args_set_file.join(",")));
    }
    Ok(None)
}
