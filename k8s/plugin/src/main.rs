use plugin::ExecuteOperation;
use resources::{init_rest, Error, Operations};

use clap::Parser;
use std::{env, ops::Deref, path::PathBuf};

pub mod resources;

#[derive(Parser, Debug)]
#[clap(name = utils::package_description!(), version = utils::version_info_str!())]
#[group(skip)]
struct CliArgs {
    /// The operation to be performed.
    #[clap(subcommand)]
    operations: Operations,

    /// Kubernetes namespace of mayastor service [default: mayastor].
    #[clap(global = true, long, short = 'n')]
    namespace: Option<String>,

    /// Kubernetes context to use.
    #[clap(global = true, long)]
    context: Option<String>,

    #[clap(flatten)]
    args: resources::CliArgs,

    /// Path to kubeconfig file.
    #[clap(global = true, long, short = 'k')]
    kube_config_path: Option<PathBuf>,

    /// Use namespace from the current `kubeconfig` context.
    #[clap(global = true, long, hide = true, default_value = "false")]
    namespace_from_context: bool,
}

impl CliArgs {
    async fn args() -> Result<Self, anyhow::Error> {
        let mut args = CliArgs::parse();
        let path = args.kube_config_path.clone();
        args.args.kubeconfig = path.clone();
        if let Operations::Dump(ref mut dump_args) = args.operations {
            dump_args.args.set_kube_config_path(path);
        }
        args.args.context = args.context.clone();
        args.args.namespace = if let Some(namespace) = &args.namespace {
            namespace.to_string()
        } else if args.namespace_from_context {
            let client = kube_proxy::client_from_kubeconfig(
                args.kube_config_path.clone(),
                args.context.clone(),
            )
            .await?;
            client.default_namespace().to_string()
        } else {
            constants::DEFAULT_PLUGIN_NAMESPACE.to_string()
        };
        Ok(args)
    }
}

impl Deref for CliArgs {
    type Target = plugin::CliArgs;

    fn deref(&self) -> &Self::Target {
        &self.args
    }
}

#[tokio::main]
async fn main() {
    let mut exit_code = 1;
    match CliArgs::args().await {
        Ok(cli_args) => {
            let _tracer_flusher = cli_args.init_tracing();
            if let Err(error) = cli_args.execute().await {
                match error {
                    Error::RestPlugin(error) => eprintln!("{error}"),
                    Error::RestClient(error) => {
                        eprintln!("Failed to initialise the REST client. Error {error}")
                    }
                    Error::Upgrade(error) => {
                        eprintln!("{error}");
                        exit_code = error.into();
                    }
                    Error::Generic(error) => eprintln!("{error}"),
                }
                std::process::exit(exit_code);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(exit_code)
        }
    }
}

impl CliArgs {
    async fn execute(self) -> Result<(), Error> {
        // Initialise the REST client.
        init_rest(&self.args).await?;

        tokio::select! {
            shutdown = shutdown::Shutdown::wait_sig() => {
                Err(anyhow::anyhow!("Interrupted by {shutdown:?}").into())
            },
            done = self.operations.execute(&self.args) => {
                done
            }
        }
    }
}
