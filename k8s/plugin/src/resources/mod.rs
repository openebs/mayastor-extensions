mod diskpool_cleanup;
mod io_engine_label_check;

use anyhow::anyhow;
use clap::Parser;
use k8s_openapi::api::core::v1 as core_v1;
use kube::api::GroupVersionKind;
use openapi::{
    apis::StatusCode,
    tower::client::{configuration::ClientSecurity, Url},
};
use plugin::{
    resources::{
        node, pool, snapshot, ClearErrors, CordonResources, DrainResources, ExpandResources,
        GetResources, LabelResources, ScaleResources, SetPropertyResources, UnCordonResources,
        VolumeId,
    },
    rest_wrapper::RestClient,
    ExecuteOperation,
};
use std::{ops::Deref, path::PathBuf};
use supportability::DumpArgs;
use upgrade::{
    plugin::upgrade::{GetUpgradeArgs, UpgradeArgs},
    preflight_validations,
    upgrade::DeleteUpgradeArgs,
};

#[derive(Parser, Debug)]
#[group(skip)]
pub struct CliArgs {
    /// The rest endpoint to connect to.
    #[clap(global = true, long, short)]
    pub rest: Option<Url>,

    /// Path to kubeconfig file.
    #[clap(skip)]
    pub kubeconfig: Option<PathBuf>,

    /// Kubernetes namespace of mayastor service
    #[clap(skip)]
    pub namespace: String,

    /// Kubernetes context in the kubeconfig file.
    #[clap(skip)]
    pub context: Option<String>,

    #[clap(flatten)]
    pub cli_args: plugin::CliArgs,
}

impl CliArgs {
    /// The kube client, with the correct install namespace.
    pub async fn client(&self) -> anyhow::Result<kube::Client> {
        let opts = kube_proxy::kubeconfig_options_from_context(self.context.clone());
        let mut config = kube_proxy::config_from_kubeconfig(self.kubeconfig.clone(), opts).await?;
        // If taking namespace from context, we already know self.namespace has been set
        // from the context.
        config.default_namespace = self.namespace.clone();
        Ok(kube::Client::try_from(config)?)
    }
    /// Get the [`core_v1::PersistentVolume`] api client for the client namespace.
    pub async fn pv_api(&self) -> anyhow::Result<kube::Api<core_v1::PersistentVolume>> {
        Ok(kube::Api::all(self.client().await?))
    }
    fn snap_content(&self) -> kube::api::ApiResource {
        kube::api::ApiResource::from_gvk(&GroupVersionKind {
            group: "snapshot.storage.k8s.io".into(),
            version: "v1".into(),
            kind: "VolumeSnapshotContent".into(),
        })
    }
    /// Get the cluster-scoped `VolumeSnapshotContent` [`kube::api::DynamicObject`] api client.
    pub async fn snap_content_api(&self) -> anyhow::Result<kube::Api<kube::api::DynamicObject>> {
        Ok(kube::Api::all_with(
            self.client().await?,
            &self.snap_content(),
        ))
    }
}

impl Deref for CliArgs {
    type Target = plugin::CliArgs;

    fn deref(&self) -> &Self::Target {
        &self.cli_args
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum GetResourcesK8s {
    #[clap(flatten)]
    Rest(GetResources),
    /// Get upgrade status
    UpgradeStatus(GetUpgradeArgs),
}

/// The types of operations that are supported.
#[derive(Parser, Debug)]
pub enum Operations {
    /// 'Drain' resources.
    #[clap(subcommand)]
    Drain(DrainResources),
    /// 'Label' resources.
    #[clap(subcommand)]
    Label(LabelResources),
    /// 'Get' resources.
    #[clap(subcommand)]
    Get(GetResourcesK8s),
    /// 'Expand' resources.
    #[clap(subcommand)]
    Expand(ExpandResources),
    /// 'Scale' resources.
    #[clap(subcommand)]
    Scale(ScaleResources),
    /// 'Set' resources.
    #[clap(subcommand)]
    Set(SetPropertyResources),
    /// `ClearError` resources.
    #[clap(subcommand)]
    ClearErrors(ClearErrors),
    /// 'Cordon' resources.
    #[clap(subcommand)]
    Cordon(CordonResources),
    /// 'Uncordon' resources.
    #[clap(subcommand)]
    Uncordon(UnCordonResources),
    /// `Dump` resources.
    Dump(DumpArgs),
    /// `Upgrade` the deployment.
    Upgrade(UpgradeArgs),
    /// `Delete` resources.
    Delete(DeleteArgs),
}

/// Delete resources.
#[derive(Debug, clap::Args)]
pub struct DeleteArgs {
    /// Ignore error if resource is not found.
    #[clap(long, short, global = true)]
    ignore_not_found: bool,

    /// Automatically confirm and assume yes for all prompts.
    #[clap(long, short, global = true)]
    pub yes: bool,

    #[clap(subcommand)]
    resource: DeleteResources,
}

/// The type of resources which support the delete operation.
#[derive(clap::Subcommand, Debug)]
pub enum DeleteResources {
    /// Delete upgrade resources
    Upgrade(DeleteUpgradeArgs),
    /// Deletes the specified pool resource.
    Pool(PoolDeleteArgs),
    /// Deletes the specified node and all its resources (purge).
    Node(NodeDeleteArgs),
    /// Deletes the specified volume resource.
    Volume {
        /// The id of the volume to delete.
        id: VolumeId,
    },
    /// Deletes the specified volume snapshot resource.
    VolumeSnapshot(snapshot::DelVolumeSnapshotArgs),
}

/// Shared DiskPool CR cleanup arguments, flattened into delete commands that
/// support `--cleanup-dsp`.
#[derive(Debug, Clone, clap::Args)]
pub struct CleanupDspArgs {
    /// Also delete the DiskPool CustomResource(s) via the Kubernetes API.{n}
    /// Deletes each CR and waits for the pool operator to process the
    /// finalizer before returning.
    #[clap(long)]
    pub cleanup_dsp: bool,

    /// Timeout per DiskPool CR deletion when using --cleanup-dsp.
    #[clap(long, default_value = "60s")]
    pub cleanup_dsp_timeout: humantime::Duration,
}

/// Arguments for deleting a pool.
///
/// Wraps the control-plane pool delete arguments and adds `--cleanup-dsp`
/// for also deleting the DiskPool CustomResource via the Kubernetes API.
#[derive(Debug, Clone, clap::Args)]
pub struct PoolDeleteArgs {
    #[clap(flatten)]
    rest_args: pool::DeletePoolArgs,

    #[clap(flatten)]
    cleanup: CleanupDspArgs,
}

/// Arguments for deleting a node.
///
/// Wraps the control-plane node delete arguments. When `--cleanup-dsp` is set,
/// the DiskPool CRs for all pools that were on this node are deleted
/// concurrently after the REST node delete succeeds.
#[derive(Debug, Clone, clap::Args)]
pub struct NodeDeleteArgs {
    #[clap(flatten)]
    rest_args: node::DeleteNodeArgs,

    #[clap(flatten)]
    cleanup: CleanupDspArgs,
}

#[async_trait::async_trait(?Send)]
impl ExecuteOperation for Operations {
    type Args = CliArgs;
    type Error = Error;
    async fn execute(&self, cli_args: &CliArgs) -> Result<(), Error> {
        match self {
            Operations::Get(resource) => match resource {
                GetResourcesK8s::Rest(resource) => resource.execute(cli_args).await?,
                GetResourcesK8s::UpgradeStatus(resources) => {
                    // todo: use generic execute trait
                    let client = cli_args.client().await?;
                    resources.get_upgrade(&cli_args.namespace, &client).await?
                }
            },
            Operations::Drain(resource) => resource.execute(cli_args).await?,
            Operations::Label(resource) => resource.execute(cli_args).await?,
            Operations::Expand(resource) => resource.execute(cli_args).await?,
            Operations::Scale(resource) => resource.execute(cli_args).await?,
            Operations::Set(resource) => resource.execute(cli_args).await?,
            Operations::ClearErrors(resource) => resource.execute(cli_args).await?,
            Operations::Cordon(resource) => resource.execute(cli_args).await?,
            Operations::Uncordon(resource) => resource.execute(cli_args).await?,
            Operations::Dump(resources) => {
                resources
                    .execute(&supportability::K8sCtxArgs {
                        namespace: cli_args.namespace.clone(),
                        kubeconfig: supportability::KubeConfigArgs {
                            path: cli_args.kubeconfig.clone(),
                            opts: Default::default(),
                        },
                    })
                    .await
                    .inspect_err(|_| {
                        // todo: check why is this here, can it be removed?
                        println!("Partially collected dump information: ");
                    })?
            }
            Operations::Upgrade(resources) => {
                // todo: use generic execute trait
                let client = cli_args.client().await?;
                preflight_validations::preflight_check(
                    &cli_args.namespace,
                    cli_args.kubeconfig.clone(),
                    cli_args.context.clone(),
                    cli_args.timeout,
                    resources,
                    &client,
                )
                .await?;
                resources.execute(&cli_args.namespace, &client).await?
            }
            Operations::Delete(args) => {
                match &args.resource {
                    // todo: use generic execute trait
                    DeleteResources::Upgrade(res) => {
                        let client = cli_args.client().await?;
                        res.delete(&cli_args.namespace, &client).await?
                    }
                    DeleteResources::Pool(pool_args) if !pool_args.cleanup.cleanup_dsp => {
                        // No cleanup — delegate entirely like Volume/VolumeSnapshot.
                        plugin::resources::DeleteArgs {
                            ignore_not_found: args.ignore_not_found,
                            yes: args.yes,
                            resource: plugin::resources::DeleteResources::Pool(
                                pool_args.rest_args.clone(),
                            ),
                        }
                        .execute(cli_args)
                        .await?
                    }
                    DeleteResources::Pool(pool_args) => {
                        // --cleanup-dsp: run REST delete (never swallow 404), then
                        // DSP CR cleanup.
                        let rest_result = plugin::resources::DeleteArgs {
                            ignore_not_found: false,
                            yes: args.yes,
                            resource: plugin::resources::DeleteResources::Pool(
                                pool_args.rest_args.clone(),
                            ),
                        }
                        .execute(cli_args)
                        .await;

                        let pool_deleted = match &rest_result {
                            Ok(()) => true,
                            Err(plugin::resources::error::Error::DeletePoolError {
                                source,
                                ..
                            }) if source.status() == Some(StatusCode::NOT_FOUND) => false,
                            _ => return rest_result.map_err(Into::into),
                        };
                        let client = cli_args.client().await?;
                        diskpool_cleanup::cleanup_dsp(
                            client,
                            &cli_args.namespace,
                            &pool_args.rest_args.pool_id,
                            pool_args.cleanup.cleanup_dsp_timeout,
                            pool_deleted,
                            args.ignore_not_found,
                        )
                        .await?;
                    }
                    DeleteResources::Node(node_args) if !node_args.cleanup.cleanup_dsp => {
                        // Pre-flight: refuse if the io-engine DaemonSet would
                        // still schedule onto this Kubernetes Node.
                        io_engine_label_check::ensure_node_unlabelled(
                            cli_args.client().await?,
                            &cli_args.namespace,
                            &node_args.rest_args.node_id,
                        )
                        .await?;

                        // No DSP cleanup — delegate entirely to the control-plane
                        // plugin lib. show_impact, purge, accept_* flags, and the
                        // confirm prompt bypass are all handled there.
                        plugin::resources::DeleteArgs {
                            ignore_not_found: args.ignore_not_found,
                            yes: args.yes,
                            resource: plugin::resources::DeleteResources::Node(
                                node_args.rest_args.clone(),
                            ),
                        }
                        .execute(cli_args)
                        .await?
                    }
                    DeleteResources::Node(node_args) => {
                        let node_id = &node_args.rest_args.node_id;

                        let client = cli_args.client().await?;

                        // Pre-flight: refuse if the io-engine DaemonSet would
                        // still schedule onto this Kubernetes Node.
                        io_engine_label_check::ensure_node_unlabelled(
                            client.clone(),
                            &cli_args.namespace,
                            node_id,
                        )
                        .await?;

                        // REST delete first. If it fails with anything other than
                        // NOT_FOUND we propagate the error immediately and never
                        // touch the K8s API.
                        let rest_result = plugin::resources::DeleteArgs {
                            ignore_not_found: false,
                            yes: args.yes,
                            resource: plugin::resources::DeleteResources::Node(
                                node_args.rest_args.clone(),
                            ),
                        }
                        .execute(cli_args)
                        .await;

                        let node_deleted = match &rest_result {
                            Ok(()) => true,
                            Err(plugin::resources::error::Error::DeleteNodeError {
                                source,
                                ..
                            }) if source.status() == Some(StatusCode::NOT_FOUND) => false,
                            _ => return rest_result.map_err(Into::into),
                        };

                        // List DiskPool CRs directly from Kubernetes so the
                        // lookup works even when the node spec or pool specs
                        // are already gone from the control plane.
                        let pool_ids = diskpool_cleanup::list_diskpool_ids_for_node(
                            client.clone(),
                            &cli_args.namespace,
                            node_id,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("Failed to list DiskPool CR(s) for node {node_id}: {e}")
                        })?;

                        // Spawn one task per pool so all DSP CR deletions run
                        // concurrently. kube::Client is Arc-backed, cheap to clone.
                        let namespace = cli_args.namespace.clone();
                        let timeout = node_args.cleanup.cleanup_dsp_timeout;
                        let ignore_not_found = args.ignore_not_found;
                        let mut tasks = tokio::task::JoinSet::new();

                        for pool_id in pool_ids {
                            let client = client.clone();
                            let namespace = namespace.clone();
                            tasks.spawn(async move {
                                diskpool_cleanup::cleanup_dsp(
                                    client,
                                    &namespace,
                                    pool_id.as_str(),
                                    timeout,
                                    node_deleted,
                                    ignore_not_found,
                                )
                                .await
                            });
                        }

                        // Collect all results; first error cancels the rest.
                        while let Some(result) = tasks.join_next().await {
                            result.map_err(|e| {
                                anyhow!("Failed to delete DiskPool CR(s) for node {node_id}: {e}")
                            })??;
                        }
                    }
                    DeleteResources::Volume { id } => {
                        // 1. ensure PV is not present
                        let pv_name = format!("pvc-{id}");
                        let client = cli_args.pv_api().await?;
                        let pv = client.get_opt(&pv_name).await.map_err(|error| {
                            anyhow!("Failed to fetch PV {pv_name} from K8s api-server: {error}")
                        })?;
                        if pv.is_some() {
                            return Err(Error::Generic(anyhow!(
                                "The volume is still being referenced by PV {pv_name}"
                            )));
                        }

                        // 2. delete volume
                        plugin::resources::DeleteArgs {
                            ignore_not_found: args.ignore_not_found,
                            yes: args.yes,
                            resource: plugin::resources::DeleteResources::Volume { id: *id },
                        }
                        .execute(cli_args)
                        .await?
                    }
                    DeleteResources::VolumeSnapshot(snap_args) => {
                        // 1. ensure the K8s VolumeSnapshotContent CR is not present.
                        let vsc_name = format!("snapcontent-{}", snap_args.snapshot);
                        let client = cli_args.snap_content_api().await?;
                        let vsc = client.get_opt(&vsc_name).await.map_err(|error| {
                            anyhow!("Failed to fetch VSC {vsc_name} from K8s api-server: {error}")
                        })?;
                        if vsc.is_some() {
                            return Err(Error::Generic(anyhow!(
                                "The volume snapshot is still being referenced by VSC {vsc_name}"
                            )));
                        }

                        // 2. go ahead and try to delete the mayastor volume snapshot
                        plugin::resources::DeleteArgs {
                            ignore_not_found: args.ignore_not_found,
                            yes: args.yes,
                            resource: plugin::resources::DeleteResources::VolumeSnapshot(
                                snap_args.clone(),
                            ),
                        }
                        .execute(cli_args)
                        .await?
                    }
                }
            }
        }
        Ok(())
    }
}

/// Common error wrapper for the plugin.
pub enum Error {
    /// This variant maps upgrade job errors.
    Upgrade(upgrade::error::Error),
    /// Control plane specific errors.
    RestPlugin(plugin::resources::error::Error),
    /// Rest client specific errors.
    RestClient(anyhow::Error),
    /// Generic errors.
    Generic(anyhow::Error),
}

impl From<upgrade::error::Error> for Error {
    fn from(e: upgrade::error::Error) -> Self {
        Error::Upgrade(e)
    }
}

impl From<plugin::resources::error::Error> for Error {
    fn from(e: plugin::resources::error::Error) -> Self {
        Error::RestPlugin(e)
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Generic(e)
    }
}

impl From<io_engine_label_check::LabelCheckError> for Error {
    fn from(e: io_engine_label_check::LabelCheckError) -> Self {
        Error::Generic(anyhow::Error::new(e))
    }
}

impl From<kube_proxy::Error> for Error {
    fn from(e: kube_proxy::Error) -> Self {
        if let error @ kube_proxy::Error::Forward {
            source: kube_forward::Error::ServiceNotFound { .. },
        } = e
        {
            return Error::Generic(anyhow!("{error}\n Are you on the correct namespace?"));
        }
        Error::Generic(anyhow!(e))
    }
}

/// Initialise the REST client.
pub async fn init_rest(cli_args: &CliArgs) -> Result<(), Error> {
    // Use the supplied URL if there is one otherwise obtain one from the kubeconfig file.
    match cli_args.rest.clone() {
        Some(url) => RestClient::init(url, false, *cli_args.timeout, ClientSecurity::default())
            .map_err(Error::RestClient),
        None => {
            let config = kube_proxy::ConfigBuilder::default_api_rest()
                .with_kube_config(cli_args.kubeconfig.clone())
                .with_context(cli_args.context.clone())
                .with_timeout(*cli_args.timeout)
                .with_target_mod(|t| t.with_namespace(&cli_args.namespace))
                .build()
                .await?;
            RestClient::init_with_config(config)?;
            Ok(())
        }
    }
}
