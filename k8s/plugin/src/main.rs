use clap::Parser;
use openapi::tower::client::Url;
use plugin::{rest_wrapper::RestClient, ExecuteOperation};
use resources::Operations;

use std::{env, ops::Deref};

mod resources;

#[derive(Parser, Debug)]
#[clap(name = utils::package_description!(), version = utils::version_info_str!())]
#[group(skip)]
struct CliArgs {
    /// The rest endpoint to connect to.
    #[clap(global = true, long, short)]
    rest: Option<Url>,

    /// The operation to be performed.
    #[clap(subcommand)]
    operations: Operations,

    #[clap(flatten)]
    args: resources::CliArgs,
}

impl CliArgs {
    fn args() -> Self {
        CliArgs::parse()
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
    let cli_args = CliArgs::args();
    let _tracer_flusher = cli_args.init_tracing();

    if let Err(error) = cli_args.execute().await {
        let mut exit_code = 1;
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

impl CliArgs {
    async fn execute(self) -> Result<(), Error> {
        // Initialise the REST client.
        init_rest(&self).await?;

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

/// Initialise the REST client.
async fn init_rest(cli_args: &CliArgs) -> Result<(), Error> {
    // Use the supplied URL if there is one otherwise obtain one from the kubeconfig file.
    match cli_args.rest.clone() {
        Some(url) => RestClient::init(url, *cli_args.timeout).map_err(Error::RestClient),
        None => {
            let config = kube_proxy::ConfigBuilder::default_api_rest()
                .with_kube_config(cli_args.args.kube_config_path.clone())
                .with_timeout(*cli_args.timeout)
                .with_target_mod(|t| t.with_namespace(&cli_args.args.namespace))
                .build()
                .await?;
            RestClient::init_with_config(config)?;
            Ok(())
        }
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
