use clap::Parser;
use plugin::ExecuteOperation;
use resources::{init_rest, Error, Operations};
pub mod resources;

use std::{env, ops::Deref};

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

    #[clap(flatten)]
    args: resources::CliArgs,

    /// Use namespace from the current `kubeconfig` context.
    #[clap(global = true, long, hide = true, default_value = "false")]
    namespace_from_context: bool,
}

impl CliArgs {
    async fn args() -> Result<Self, anyhow::Error> {
        let mut args = CliArgs::parse();
        args.args.namespace = if let Some(namespace) = &args.namespace {
            namespace.to_string()
        } else if args.namespace_from_context {
            let client = kube_proxy::client_from_kubeconfig(args.args.kube_config_path.clone())
                .await
                .map_err(|err| anyhow::anyhow!("{err}"))?;
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
