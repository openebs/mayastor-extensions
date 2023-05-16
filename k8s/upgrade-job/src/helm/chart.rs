use crate::common::error::{Result, ThinProvisioningOptionsAbsent};
use semver::Version;
use serde::Deserialize;

/// This struct is used to deserialize helm charts' Chart.yaml file.
#[derive(Deserialize)]
pub(crate) struct Chart {
    /// This is the name of the helm chart.
    name: String,
    /// This is the version of the helm chart.
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

/// This is used to deserialize the values.yaml of the Core chart.
#[derive(Deserialize)]
pub(crate) struct CoreValues {
    /// This is the yaml object which contains values for the container image registry, repository,
    /// tag, etc.
    image: Image,
    /// This is the yaml object which contains the configuration for the io-engine DaemonSet.
    io_engine: IoEngine,
    /// This is the .agents yaml object in the helm value.yaml.
    agents: Agents,
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

    pub(crate) fn core_capacity_is_absent(&self) -> bool {
        self.agents.core_capacity_is_absent()
    }

    pub(crate) fn core_thin_pool_commitment(&self) -> Result<String> {
        self.agents.core_thin_pool_commitment()
    }

    pub(crate) fn core_thin_volume_commitment(&self) -> Result<String> {
        self.agents.core_thin_volume_commitment()
    }

    pub(crate) fn core_thin_volume_commitment_initial(&self) -> Result<String> {
        self.agents.core_thin_volume_commitment_initial()
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

#[derive(Deserialize)]
pub(crate) struct Agents {
    core: Core,
}

impl Agents {
    pub(crate) fn core_capacity_is_absent(&self) -> bool {
        self.core.capacity_is_absent()
    }

    pub(crate) fn core_thin_pool_commitment(&self) -> Result<String> {
        self.core.thin_pool_commitment()
    }

    pub(crate) fn core_thin_volume_commitment(&self) -> Result<String> {
        self.core.thin_volume_commitment()
    }

    pub(crate) fn core_thin_volume_commitment_initial(&self) -> Result<String> {
        self.core.thin_volume_commitment_initial()
    }
}

#[derive(Deserialize)]
pub(crate) struct Core {
    capacity: Option<Capacity>,
}

impl Core {
    pub(crate) fn capacity_is_absent(&self) -> bool {
        self.capacity.is_none()
    }

    pub(crate) fn thin_pool_commitment(&self) -> Result<String> {
        Ok(self
            .capacity
            .as_ref()
            .ok_or(ThinProvisioningOptionsAbsent.build())?
            .thin_pool_commitment())
    }

    pub(crate) fn thin_volume_commitment(&self) -> Result<String> {
        Ok(self
            .capacity
            .as_ref()
            .ok_or(ThinProvisioningOptionsAbsent.build())?
            .thin_volume_commitment())
    }

    pub(crate) fn thin_volume_commitment_initial(&self) -> Result<String> {
        Ok(self
            .capacity
            .as_ref()
            .ok_or(ThinProvisioningOptionsAbsent.build())?
            .thin_volume_commitment_initial())
    }
}

#[derive(Clone, Deserialize)]
pub(crate) struct Capacity {
    thin: Thin,
}

impl Capacity {
    pub(crate) fn thin_pool_commitment(&self) -> String {
        self.thin.pool_commitment()
    }

    pub(crate) fn thin_volume_commitment(&self) -> String {
        self.thin.volume_commitment()
    }

    pub(crate) fn thin_volume_commitment_initial(&self) -> String {
        self.thin.volume_commitment_initial()
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub(crate) struct Thin {
    pool_commitment: String,
    volume_commitment: String,
    volume_commitment_initial: String,
}

impl Thin {
    pub(crate) fn pool_commitment(&self) -> String {
        self.pool_commitment.clone()
    }

    pub(crate) fn volume_commitment(&self) -> String {
        self.volume_commitment.clone()
    }

    pub(crate) fn volume_commitment_initial(&self) -> String {
        self.volume_commitment_initial.clone()
    }
}
