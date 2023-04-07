use semver::Version;
use serde::Deserialize;

#[derive(Deserialize)]
/// Chart for name and dependencies.
pub(crate) struct Chart {
    name: String,
    #[serde(deserialize_with = "Version::deserialize")]
    version: Version,
}

impl Chart {
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) fn version(&self) -> &Version {
        &self.version
    }
}

#[derive(Deserialize)]
/// UmbrellaValues has core values.
pub(crate) struct UmbrellaValues {
    #[serde(rename(deserialize = "mayastor"))]
    core: CoreValues,
}

impl UmbrellaValues {
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
/// Image has tag.
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
    log_level: String,
}

impl IoEngine {
    pub(crate) fn log_level(&self) -> &str {
        self.log_level.as_str()
    }
}
