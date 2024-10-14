use clap::Parser;
use plugin::ExecuteOperation;
use resources::{init_rest, Error, Operations};

use std::{env, ops::Deref};

mod resources;

#[derive(Parser, Debug)]
#[clap(name = utils::package_description!(), version = utils::version_info_str!())]
#[group(skip)]
struct CliArgs {
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
