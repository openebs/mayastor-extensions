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

    /// Reports Loki's own configured max query-time-range limit, as JSON
    /// (`{"maxQueryLength":"<duration>"}`, or `null` if it can't be determined).
    /// Uses the same TLS-aware discovery/connection as every other Loki
    /// operation this tool performs - a caller wanting this value should use
    /// this instead of building its own connection to Loki.
    LokiLimit,
}

impl SystemDumpArgs {
    pub fn disable_log_collection(&self) -> bool {
        self.disable_log_collection
    }
}
