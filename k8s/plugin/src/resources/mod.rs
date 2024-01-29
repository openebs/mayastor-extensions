use crate::Error;
use clap::Parser;
use plugin::{
    resources::{CordonResources, DrainResources, GetResources, ScaleResources, UnCordonResources},
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
    pub(super) kube_config_path: Option<PathBuf>,

    /// Kubernetes namespace of mayastor service
    #[clap(global = true, long, short = 'n', default_value = "mayastor")]
    pub(super) namespace: String,

    #[clap(flatten)]
    cli_args: plugin::CliArgs,
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
    /// 'Get' resources.
    #[clap(subcommand)]
    Get(GetResourcesK8s),
    /// 'Scale' resources.
    #[clap(subcommand)]
    Scale(ScaleResources),
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
            Operations::Scale(resource) => resource.execute(cli_args).await?,
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
