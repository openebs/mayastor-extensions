use clap::Parser;
use plugin::{
    resources::{
        CordonResources, DrainResources, GetResources, LabelResources, ScaleResources,
        SetPropertyResources, UnCordonResources,
    },
    ExecuteOperation,
};
use std::{ops::Deref, path::PathBuf};
use supportability::DumpArgs;
use upgrade::{
    plugin::upgrade::{DeleteResources, GetUpgradeArgs, UpgradeArgs},
    preflight_validations,
};

#[derive(Parser, Debug)]
#[group(skip)]
pub struct CliArgs {
    /// Path to kubeconfig file.
    #[clap(global = true, long, short = 'k')]
    pub kube_config_path: Option<PathBuf>,

    /// Kubernetes namespace of mayastor service
    #[clap(global = true, long, short = 'n', default_value = "mayastor")]
    pub namespace: String,

    #[clap(flatten)]
    pub cli_args: plugin::CliArgs,
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
    /// 'Scale' resources.
    #[clap(subcommand)]
    Scale(ScaleResources),
    /// 'Set' resources.
    #[clap(subcommand)]
    Set(SetPropertyResources),
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
    /// `Delete` the upgrade resources.
    #[clap(subcommand)]
    Delete(DeleteResources),
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
                    resources.get_upgrade(&cli_args.namespace).await?
                }
            },
            Operations::Drain(resource) => resource.execute(cli_args).await?,
            Operations::Label(resource) => resource.execute(cli_args).await?,

            Operations::Scale(resource) => resource.execute(cli_args).await?,
            Operations::Set(resource) => resource.execute(cli_args).await?,
            Operations::Cordon(resource) => resource.execute(cli_args).await?,
            Operations::Uncordon(resource) => resource.execute(cli_args).await?,
            Operations::Dump(resources) => {
                // todo: build and pass arguments
                resources.execute(&()).await.map_err(|e| {
                    // todo: check why is this here, can it be removed?
                    println!("Partially collected dump information: ");
                    e
                })?
            }
            Operations::Upgrade(resources) => {
                // todo: use generic execute trait
                preflight_validations::preflight_check(
                    &cli_args.namespace,
                    cli_args.kube_config_path.clone(),
                    cli_args.timeout,
                    resources,
                )
                .await?;
                resources.execute(&cli_args.namespace).await?
            }
            Operations::Delete(resource) => match resource {
                // todo: use generic execute trait
                DeleteResources::Upgrade(res) => res.delete(&cli_args.namespace).await?,
            },
        }
        Ok(())
    }
}

pub enum Error {
    Upgrade(upgrade::error::Error),
    RestPlugin(plugin::resources::error::Error),
    RestClient(anyhow::Error),
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
