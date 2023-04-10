use semver::Version;
use serde::Deserialize;

/// This struct is used to deserialize helm charts' Chart.yaml file.
#[derive(Deserialize)]
pub(crate) struct Chart {
    /// This is the name of the helm chart.
    name: String,
    /// This is the version of the helm chart.
    #[serde(deserialize_with = "Version::deserialize")]
    version: Version,
}

impl Chart {
    /// This is a getter for the helm chart name.
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    /// This is a getter for the helm chart version.
    pub(crate) fn version(&self) -> &Version {
        &self.version
    }
}

/// This is used to deserialize the values.yaml file of the Umbrella chart.
#[derive(Deserialize)]
pub(crate) struct UmbrellaValues {
    /// The Umbrella chart embeds the values options of the Core chart in a yaml object with the
    /// same name as the name of the Core chart. The Core chart is a dependency-chart for the
    /// Umbrella chart.
    #[serde(rename(deserialize = "mayastor"))]
    core: CoreValues,
}

impl UmbrellaValues {
    /// This is a getter for the container image tag of the Umbrella chart.
    pub(crate) fn image_tag(&self) -> &str {
        self.core.image_tag()
    }

    /// This is the logLevel of the io-engine DaemonSet Pods.
    pub(crate) fn io_engine_log_level(&self) -> &str {
        self.core.io_engine_log_level()
    }
}

/// This is used to deserialize the values.yaml of the Core chart.
#[derive(Deserialize)]
pub(crate) struct CoreValues {
    /// This is the yaml object which contains values for the container image registry, repository,
    /// tag, etc.
    image: Image,
    /// This is the yaml object which contains the configuration for the io-engine DaemonSet.
    io_engine: IoEngine,
}

impl CoreValues {
    /// This is a getter for the container image tag of the Core chart.
    pub(crate) fn image_tag(&self) -> &str {
        self.image.tag()
    }

    /// This is a getter for the io-engine DaemonSet Pods' logLevel.
    pub(crate) fn io_engine_log_level(&self) -> &str {
        self.io_engine.log_level()
    }
}

/// This is used to deserialize the yaml object "image", which contains details required for pulling
/// container images.
#[derive(Deserialize)]
pub(crate) struct Image {
    /// The container image tag.
    tag: String,
}

impl Image {
    /// This is a getter for the container image tag used across the helm chart release.
    pub(crate) fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the yaml object "io_engine", which contains configuration for the
/// io-engine DaemonSet.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub(crate) struct IoEngine {
    /// Tracing Loglevel details for the io-engine DaemonSet Pods.
    log_level: String,
}

impl IoEngine {
    /// This is a getter for the io-engine DaemonSet Pod's tracing logLevel.
    pub(crate) fn log_level(&self) -> &str {
        self.log_level.as_str()
    }
}
