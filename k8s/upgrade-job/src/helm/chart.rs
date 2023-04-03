use serde::Deserialize;
use semver::Version;
use crate::{
    common::constants::CORE_CHART_NAME,
};

#[derive(Deserialize)]
pub(crate) struct Chart {
    name: String,
    #[serde(deserialize_with = "Version::deserialize")]
    version: Version,
    dependencies: Vec<Dependency>,
}

impl Chart {
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) fn version(&self) -> &Version {
        &self.version
    }

    pub(crate) fn dependencies(&self) -> &[Dependency] {
        self.dependencies.as_slice()
    }
}

#[derive(Deserialize)]
pub(crate) struct Dependency {
    name: String,
    #[serde(deserialize_with = "Version::deserialize")]
    version: Version,
}

impl Dependency {
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) fn version(&self) -> &Version {
        &self.version
    }
}

#[derive(Deserialize)]
pub(crate) struct UmbrellaValues {
    #[serde(rename(deserialize = CORE_CHART_NAME))]
    core: CoreValues,
}

impl UmbrellaValues {
    pub(crate) fn core(&self) -> &CoreValues {
        &self.core
    }

    pub(crate) fn image_tag(&self) -> &str {
        self.core.image_tag()
    }

    pub(crate) fn io_engine_log_level(&self) -> &str {
        self.core.io_engine_log_level()
    }
}

#[derive(Deserialize)]
pub(crate) struct CoreValues {
    image: Image,
    io_engine: IoEngine,
}

impl CoreValues {
    pub(crate) fn image_tag(&self) -> &str {
        self.image.tag()
    }

    pub(crate) fn io_engine_log_level(&self) -> &str {
        self.io_engine.log_level()
    }
}

#[derive(Deserialize)]
pub(crate) struct Image {
    tag: String,

}

impl Image {
    pub(crate) fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub(crate) struct IoEngine {
    log_level: String
}

impl IoEngine {
    pub(crate) fn log_level(&self) -> &str {
        self.log_level.as_str()
    }
}
