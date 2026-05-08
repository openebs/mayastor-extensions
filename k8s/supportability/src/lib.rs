use crate::collect::resource_dump::ResourceDumper;
use collect::{
    common::{DumpConfig, OutputFormat},
    error::Error,
    utils::log,
};
use operations::Resource;
use plugin::ExecuteOperation;

use std::path::PathBuf;

pub mod collect;
pub mod operations;

/// Collects state & log information of mayastor services running in the system and dump them.
#[derive(Debug, Clone, clap::Args)]
pub struct SupportArgs {
    /// Specifies the timeout value to interact with other modules of system
    #[clap(global = true, long, short, default_value = "10s")]
    timeout: humantime::Duration,

    /// Period states to collect all logs from last specified duration
    #[clap(global = true, long, short, default_value = "24h")]
    since: humantime::Duration,

    /// Endpoint of LOKI service, if left empty then it will try to parse endpoint
    /// from Loki service(K8s service resource), if the tool is unable to parse
    /// from service then logs will be collected using Kube-apiserver
    #[clap(global = true, short, long)]
    loki_endpoint: Option<String>,

    /// Endpoint of ETCD service, if left empty then will be parsed from the internal service name
    #[clap(global = true, short, long)]
    etcd_endpoint: Option<String>,

    /// Output directory path to store archive file
    #[clap(global = true, long, short = 'd', default_value = "./")]
    output_directory_path: String,

    /// K8s connection context arguments.
    #[clap(skip)]
    ctx: K8sCtxArgs,

    /// The tenant id to be used to query loki logs.
    #[clap(global = true, long, default_value = "openebs")]
    tenant_id: String,

    /// Logging label selectors
    #[clap(global = true, long, default_value = "openebs.io/logging=true")]
    logging_label_selectors: String,
}

impl SupportArgs {
    /// Sets the kubeconfig file and context name, used to interact with the Kube-Apiserver.
    ///
    /// # Arguments
    ///
    /// * `path` - An optional `PathBuf` representing the kubeconfig path.
    /// * `context` - An optional context in the kubeconfig file.
    pub fn set_kube_config(&mut self, path: Option<std::path::PathBuf>, context: Option<String>) {
        self.ctx.kubeconfig = crate::KubeConfigArgs {
            path,
            opts: kube::config::KubeConfigOptions {
                context,
                ..Default::default()
            },
        };
    }
    /// Sets the namespace of the target install.
    pub fn set_namespace(&mut self, namespace: String) {
        self.ctx.namespace = namespace;
    }
}

/// Supportability - collects state & log information of services and dumps it to a tar file.
#[derive(Debug, Clone, clap::Args)]
#[clap(
    after_help = "Supportability - collects state & log information of services and dumps it to a tar file."
)]
pub struct DumpArgs {
    #[clap(flatten)]
    pub args: SupportArgs,
    #[clap(subcommand)]
    resource: Resource,
}

#[async_trait::async_trait(?Send)]
impl ExecuteOperation for DumpArgs {
    type Args = K8sCtxArgs;
    type Error = anyhow::Error;
    async fn execute(&self, cli_args: &Self::Args) -> Result<(), Self::Error> {
        let args = SupportArgs {
            ctx: cli_args.clone(),
            ..self.args.clone()
        };

        self.resource.execute(&args).await
    }
}

#[async_trait::async_trait(?Send)]
impl ExecuteOperation for Resource {
    type Args = SupportArgs;
    type Error = anyhow::Error;

    async fn execute(&self, cli_args: &Self::Args) -> Result<(), Self::Error> {
        execute_resource_dump(cli_args.clone(), self.clone())
            .await
            .map_err(|e| anyhow::anyhow!("{:?}", e))
    }
}

// Holds prefix of archive file name
pub(crate) const ARCHIVE_PREFIX: &str = "mayastor";

async fn execute_resource_dump(cli_args: SupportArgs, resource: Resource) -> Result<(), Error> {
    let mut config = DumpConfig::new(
        cli_args.output_directory_path,
        cli_args.ctx.namespace,
        cli_args.loki_endpoint,
        cli_args.etcd_endpoint,
        cli_args.since,
        cli_args.ctx.kubeconfig,
        cli_args.timeout,
        OutputFormat::Tar,
        cli_args.tenant_id,
        cli_args.logging_label_selectors,
    );
    let mut errors = Vec::new();
    match resource {
        Resource::Loki => {
            let mut system_dumper = collect::system_dump::SystemDumper::get_or_panic_system_dumper(
                config,
                ARCHIVE_PREFIX,
            )
            .await;
            log("Completed collection of topology information");

            system_dumper.collect_and_dump_loki_logs().await?;
            if let Err(e) = system_dumper.fill_archive_and_delete_tmp() {
                log(format!("Failed to copy content to archive, error: {e:?}"));
                errors.push(e);
            }
        }
        Resource::System(args) => {
            let mut system_dumper = collect::system_dump::SystemDumper::get_or_panic_system_dumper(
                config,
                ARCHIVE_PREFIX,
            )
            .await;
            if !args.disable_log_collection() {
                if let Err(e) = system_dumper.collect_and_dump_loki_logs().await {
                    errors.push(e);
                }
            }
            if let Err(e) = system_dumper.dump_common_k8s_resources().await {
                errors.push(e);
            }
            if let Err(e) = system_dumper.dump_mayastor().await {
                errors.push(e);
            }
            if let Err(e) = system_dumper.fill_archive_and_delete_tmp() {
                log(format!("Failed to copy content to archive, error: {e:?}"));
                errors.push(e);
            }
        }
        Resource::Etcd { stdout } => {
            let format = if stdout {
                OutputFormat::Stdout
            } else {
                OutputFormat::Tar
            };
            config.set_output_format(format);

            let mut dumper =
                ResourceDumper::get_or_panic_resource_dumper(config, ARCHIVE_PREFIX).await;
            if let Err(e) = dumper.dump_etcd().await {
                log(format!("Failed to dump etcd information, Error: {e:?}"));
                errors.push(e);
            }
        }
    }
    if !errors.is_empty() {
        log("Failed to dump system state");
        return Err(Error::MultipleErrors(errors));
    }
    println!("Completed collection of dump !!");
    Ok(())
}

/// Kubeconfig arguments.
#[derive(Default, Clone)]
pub struct KubeConfigArgs {
    /// The path to the kubeconfig file, otherwise it's inferred.
    pub path: Option<PathBuf>,
    /// Options used when loading the kubeconfig file.
    pub opts: kube::config::KubeConfigOptions,
}

impl std::fmt::Debug for KubeConfigArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KubeConfigOpts")
            .field("kubeconfig", &self.path)
            .field("cluster", &self.opts.cluster)
            .field("context", &self.opts.context)
            .field("user", &self.opts.user)
            .finish()
    }
}

/// K8s contextual arguments.
#[derive(Default, Debug, Clone)]
pub struct K8sCtxArgs {
    /// The namespace where we're installed.
    pub namespace: String,
    /// Options used when loading the kubeconfig file.
    /// This is necessary because the existing code uses these directly, even though the
    /// initial connection and context namespace is retrieved at the start.
    pub kubeconfig: KubeConfigArgs,
}
