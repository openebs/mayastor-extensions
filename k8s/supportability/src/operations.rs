#[cfg(debug_assertions)]
/// Represents type of VolumeID
pub(crate) type VolumeID = openapi::apis::Uuid;

#[cfg(debug_assertions)]
/// Represents type of PoolID
pub(crate) type PoolID = String;

#[cfg(debug_assertions)]
/// Represents type of NodeID
pub(crate) type NodeID = String;

/// Types of operations supported by plugin
#[derive(clap::Subcommand, Clone, Debug)]
pub(crate) enum Operations {
    /// 'Dump' creates an archive by collecting provided resource(s) information
    #[clap(subcommand)]
    Dump(Resource),
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct SystemDumpArgs {
    /// Set this to disable log collection
    #[clap(global = true, long)]
    pub(crate) disable_log_collection: bool,
}

/// Resources on which operation can be performed
#[derive(clap::Subcommand, Clone, Debug)]
pub(crate) enum Resource {
    /// Collects entire system information
    System(SystemDumpArgs),

    #[cfg(debug_assertions)]
    /// Collects information about all volumes and its descendants (replicas/pools/nodes)
    #[clap(name = "volumes")]
    Volumes,

    #[cfg(debug_assertions)]
    /// Collects information about particular volume and its descendants matching
    /// to given volume ID
    #[clap(name = "volume")]
    Volume { id: VolumeID },

    #[cfg(debug_assertions)]
    /// Collects information about all pools and its descendants (nodes)
    #[clap(name = "pools")]
    Pools,

    #[cfg(debug_assertions)]
    /// Collects information about particular pool and its descendants matching
    /// to given pool ID
    #[clap(name = "pool")]
    Pool { id: PoolID },

    #[cfg(debug_assertions)]
    /// Collects information about all nodes
    #[clap(name = "nodes")]
    Nodes,

    #[cfg(debug_assertions)]
    /// Collects information about particular node matching to given node ID
    #[clap(name = "node")]
    Node { id: NodeID },

    /// Collects information from etcd
    Etcd {
        /// Output etcd dump to stdout instead of a tar file.
        #[clap(long)]
        stdout: bool,
    },

    /// Collects the Loki logs from the product's components
    #[clap(hide = true)]
    Loki,
}
