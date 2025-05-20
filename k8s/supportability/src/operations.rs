/// Types of operations supported by plugin
#[derive(clap::Subcommand, Clone, Debug)]
pub(crate) enum Operations {
    /// 'Dump' creates an archive by collecting provided resource(s) information
    #[clap(subcommand)]
    Dump(Resource),
}

#[derive(Debug, Clone, clap::Args)]
pub struct SystemDumpArgs {
    /// Set this to disable log collection
    #[clap(global = true, long)]
    disable_log_collection: bool,
}

/// Resources on which operation can be performed
#[derive(clap::Subcommand, Clone, Debug)]
pub(crate) enum Resource {
    /// Collects entire system information
    System(SystemDumpArgs),

    /// Collects information from etcd
    Etcd {
        /// Output etcd dump to stdout instead of a tar file.
        #[clap(long)]
        stdout: bool,
    },

    /// Collects the Loki logs from the product's components
    Loki,
}

impl SystemDumpArgs {
    pub fn disable_log_collection(&self) -> bool {
        self.disable_log_collection
    }
}
