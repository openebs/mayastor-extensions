use crate::resources::GetResourcesK8s;
use anyhow::Result;
use clap::Parser;
use openapi::tower::client::Url;
use opentelemetry::global;
use plugin::{
    operations::{
        Cordoning, Drain, Get, GetBlockDevices, GetSnapshots, List, ListExt, RebuildHistory,
        ReplicaTopology, Scale,
    },
    resources::{
        blockdevice, cordon, drain, node, pool, snapshot, volume, CordonResources, DrainResources,
        GetCordonArgs, GetDrainArgs, GetResources, ScaleResources,
    },
    rest_wrapper::RestClient,
};
use resources::Operations;
use upgrade::plugin::{preflight_validations, upgrade::DeleteResources};

use std::{env, path::PathBuf};

mod resources;

#[derive(Parser, Debug)]
#[clap(name = utils::package_description!(), version = utils::version_info_str!())]
struct CliArgs {
    /// The rest endpoint to connect to.
    #[clap(global = true, long, short)]
    rest: Option<Url>,

    /// Path to kubeconfig file.
    #[clap(global = true, long, short = 'k')]
    kube_config_path: Option<PathBuf>,

    /// The operation to be performed.
    #[clap(subcommand)]
    operations: Operations,

    /// The Output, viz yaml, json.
    #[clap(global = true, default_value = plugin::resources::utils::OutputFormat::None.as_ref(), short, long)]
    output: plugin::resources::utils::OutputFormat,

    /// Trace rest requests to the Jaeger endpoint agent.
    #[clap(global = true, long, short)]
    jaeger: Option<String>,

    /// Timeout for the REST operations.
    #[clap(long, short, default_value = "10s")]
    timeout: humantime::Duration,

    /// Kubernetes namespace of mayastor service
    #[clap(global = true, long, short = 'n', default_value = "mayastor")]
    namespace: String,
}
impl CliArgs {
    fn args() -> Self {
        CliArgs::parse()
    }
}

#[tokio::main]
async fn main() {
    plugin::init_tracing(CliArgs::args().jaeger.as_ref());

    execute(CliArgs::args()).await;

    global::shutdown_tracer_provider();
}

async fn execute(cli_args: CliArgs) {
    // Initialise the REST client.
    if let Err(e) = init_rest(&cli_args).await {
        eprintln!("Failed to initialise the REST client. Error {e}");
        std::process::exit(1);
    }

    // Perform the operations based on the subcommand, with proper output format.
    let fut = async move {
        let result: std::result::Result<(), Error> = match cli_args.operations {
            Operations::Get(resource) => match resource {
                GetResourcesK8s::Rest(resource) => match resource {
                    GetResources::Cordon(get_cordon_resource) => match get_cordon_resource {
                        GetCordonArgs::Node { id: node_id } => {
                            cordon::NodeCordon::get(&node_id, &cli_args.output)
                                .await
                                .map_err(|e| e.into())
                        }
                        GetCordonArgs::Nodes => cordon::NodeCordons::list(&cli_args.output)
                            .await
                            .map_err(|e| e.into()),
                    },
                    GetResources::Drain(get_drain_resource) => match get_drain_resource {
                        GetDrainArgs::Node { id: node_id } => {
                            drain::NodeDrain::get(&node_id, &cli_args.output)
                                .await
                                .map_err(|e| e.into())
                        }
                        GetDrainArgs::Nodes => drain::NodeDrains::list(&cli_args.output)
                            .await
                            .map_err(|e| e.into()),
                    },
                    GetResources::Volumes(volume_args) => {
                        volume::Volumes::list(&cli_args.output, &volume_args)
                            .await
                            .map_err(|e| e.into())
                    }
                    GetResources::Volume { id } => volume::Volume::get(&id, &cli_args.output)
                        .await
                        .map_err(|e| e.into()),
                    GetResources::VolumeReplicaTopologies(volume_args) => {
                        volume::Volume::topologies(&cli_args.output, &volume_args)
                            .await
                            .map_err(|e| e.into())
                    }
                    GetResources::VolumeReplicaTopology { id } => {
                        volume::Volume::topology(&id, &cli_args.output)
                            .await
                            .map_err(|e| e.into())
                    }
                    GetResources::Pools => pool::Pools::list(&cli_args.output)
                        .await
                        .map_err(|e| e.into()),
                    GetResources::Pool { id } => pool::Pool::get(&id, &cli_args.output)
                        .await
                        .map_err(|e| e.into()),
                    GetResources::Nodes => node::Nodes::list(&cli_args.output)
                        .await
                        .map_err(|e| e.into()),
                    GetResources::Node(args) => node::Node::get(&args.node_id(), &cli_args.output)
                        .await
                        .map_err(|e| e.into()),
                    GetResources::BlockDevices(bdargs) => {
                        blockdevice::BlockDevice::get_blockdevices(
                            &bdargs.node_id(),
                            &bdargs.all(),
                            &cli_args.output,
                        )
                        .await
                        .map_err(|e| e.into())
                    }
                    GetResources::VolumeSnapshots(snapargs) => {
                        snapshot::VolumeSnapshots::get_snapshots(
                            &snapargs.volume(),
                            &snapargs.snapshot(),
                            &cli_args.output,
                        )
                        .await
                        .map_err(|e| e.into())
                    }
                    GetResources::RebuildHistory { id } => {
                        volume::Volume::rebuild_history(&id, &cli_args.output)
                            .await
                            .map_err(|e| e.into())
                    }
                },
                GetResourcesK8s::UpgradeStatus(resources) => resources
                    .get_upgrade(&cli_args.namespace)
                    .await
                    .map_err(|e| e.into()),
            },
            Operations::Drain(resource) => match resource {
                DrainResources::Node(drain_node_args) => node::Node::drain(
                    &drain_node_args.node_id(),
                    drain_node_args.label(),
                    drain_node_args.drain_timeout(),
                    &cli_args.output,
                )
                .await
                .map_err(|e| e.into()),
            },
            Operations::Scale(resource) => match resource {
                ScaleResources::Volume { id, replica_count } => {
                    volume::Volume::scale(&id, replica_count, &cli_args.output)
                        .await
                        .map_err(|e| e.into())
                }
            },
            Operations::Cordon(resource) => match resource {
                CordonResources::Node { id, label } => {
                    node::Node::cordon(&id, &label, &cli_args.output)
                        .await
                        .map_err(|e| e.into())
                }
            },
            Operations::Uncordon(resource) => match resource {
                CordonResources::Node { id, label } => {
                    node::Node::uncordon(&id, &label, &cli_args.output)
                        .await
                        .map_err(|e| e.into())
                }
            },
            Operations::Dump(resources) => {
                resources
                    .dump(cli_args.kube_config_path)
                    .await
                    .map_err(|e| {
                        println!("Partially collected dump information: ");
                        e.into()
                    })
            }
            Operations::Upgrade(resources) => {
                match preflight_validations::preflight_check(
                    &cli_args.namespace,
                    cli_args.kube_config_path.clone(),
                    cli_args.timeout,
                    &resources,
                )
                .await
                {
                    Ok(_) => {
                        if resources.dry_run {
                            resources
                                .dummy_apply(&cli_args.namespace)
                                .await
                                .map_err(|e| e.into())
                        } else {
                            resources
                                .apply(&cli_args.namespace)
                                .await
                                .map_err(|e| e.into())
                        }
                    }
                    Err(e) => Err(e.into()),
                }
            }

            Operations::Delete(resource) => match resource {
                DeleteResources::Upgrade(res) => {
                    res.delete(&cli_args.namespace).await.map_err(|e| e.into())
                }
            },
        };

        if let Err(error) = result {
            let mut exit_code = 1;
            match error {
                Error::RestPlugin(err) => eprintln!("{err}"),
                Error::Upgrade(err) => {
                    eprintln!("{err}");
                    exit_code = err.into();
                }
                Error::Generic(err) => eprintln!("{err}"),
            }
            std::process::exit(exit_code);
        };
    };

    tokio::select! {
        _shutdown = shutdown::Shutdown::wait_sig() => {},
        _done = fut => {}
    }
}

/// Initialise the REST client.
async fn init_rest(args: &CliArgs) -> Result<()> {
    // Use the supplied URL if there is one otherwise obtain one from the kubeconfig file.
    match args.rest.clone() {
        Some(url) => RestClient::init(url, *args.timeout),
        None => {
            let config = kube_proxy::ConfigBuilder::default_api_rest()
                .with_kube_config(args.kube_config_path.clone())
                .with_timeout(*args.timeout)
                .with_target_mod(|t| t.with_namespace(&args.namespace))
                .build()
                .await?;
            RestClient::init_with_config(config)?;
            Ok(())
        }
    }
}

enum Error {
    Upgrade(upgrade::error::Error),
    RestPlugin(plugin::resources::error::Error),
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
