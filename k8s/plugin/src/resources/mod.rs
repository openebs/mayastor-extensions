use clap::Parser;
use plugin::resources::{CordonResources, DrainResources, GetResources, ScaleResources};
use supportability::DumpArgs;

pub mod objects;
pub mod upgrade;
use upgrade::UpgradeOperator;

/// The types of operations that are supported.
#[derive(Parser, Debug)]
pub enum Operations {
    /// 'Drain' resources.
    #[clap(subcommand)]
    Drain(DrainResources),
    /// 'Get' resources.
    #[clap(subcommand)]
    Get(GetResources),
    /// 'Scale' resources.
    #[clap(subcommand)]
    Scale(ScaleResources),
    /// 'Cordon' resources.
    #[clap(subcommand)]
    Cordon(CordonResources),
    /// 'Uncordon' resources.
    #[clap(subcommand)]
    Uncordon(CordonResources),
    /// `Dump` resources.
    Dump(DumpArgs),
    /// `Install` upgrade operator.
    #[clap(subcommand)]
    Install(UpgradeOperator),
    /// `Uninstall` upgrade operator.
    #[clap(subcommand)]
    Uninstall(UpgradeOperator),
    /// `Upgrade` the operator.
    Upgrade,
}
