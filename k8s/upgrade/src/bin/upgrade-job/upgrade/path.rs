use crate::{
    common::{
        constants::CORE_CHART_NAME,
        error::{
            HelmChartNameSplit, InvalidDependencyVersionInHelmReleaseData, ReadingFile, Result,
            SemverParse, YamlParseBufferForUnsupportedVersion, YamlParseFromFile,
        },
    },
    helm::{chart::Chart, client::HelmReleaseClient},
};
use semver::Version;
use serde::Deserialize;
use snafu::ResultExt;
use std::{fs, path::PathBuf};

/// Validates the upgrade path from 'from' Version to 'to' Version for the Core helm chart.
pub(crate) fn is_valid_for_core_chart(from: &Version) -> Result<bool> {
    let unsupported_version_buf =
        &include_bytes!("../../../../../upgrade/config/unsupported_versions.yaml")[..];
    let unsupported_versions = UnsupportedVersions::try_from(unsupported_version_buf)
        .context(YamlParseBufferForUnsupportedVersion)?;
    Ok(!unsupported_versions.contains(from))
}

/// Generate a semver::Version from the helm chart in local directory.
pub(crate) fn version_from_chart_yaml_file(path: PathBuf) -> Result<Version> {
    let values_yaml = fs::read(path.as_path()).context(ReadingFile {
        filepath: path.clone(),
    })?;

    let to_chart: Chart = serde_yaml::from_slice(values_yaml.as_slice())
        .context(YamlParseFromFile { filepath: path })?;

    Ok(to_chart.version().clone())
}

/// Generate a semver::Version from the 'chart' member of the Helm chart's ReleaseElement.
/// The output of `helm ls -n <namespace> -o yaml` is a list of ReleaseElements.
pub(crate) fn version_from_core_chart_release(chart_name: &str) -> Result<Version> {
    let delimiter: char = '-';
    // e.g. <chart>-1.2.3-rc.5 -- here the 2nd chunk is the version
    let (_, version) = chart_name.split_once(delimiter).ok_or(
        HelmChartNameSplit {
            chart_name: chart_name.to_string(),
            delimiter,
        }
        .build(),
    )?;

    Version::parse(version).context(SemverParse {
        version_string: version.to_string(),
    })
}

pub(crate) async fn core_version_from_umbrella_release(
    client: &HelmReleaseClient,
    release_name: &str,
) -> Result<Version> {
    let deps = client.get_dependencies(release_name).await?;
    deps.into_iter()
        .find_map(|dep| {
            dep.name()
                .eq(CORE_CHART_NAME)
                .then_some(dep.version())
                .flatten()
                // Parse the String into a semver::Version.
                .and_then(|version| Version::parse(version.as_str()).ok())
        })
        .ok_or(InvalidDependencyVersionInHelmReleaseData.build())
}

/// Struct to deserialize the unsupported version yaml.
#[derive(Deserialize)]
struct UnsupportedVersions {
    unsupported_versions: Vec<Version>,
}

impl UnsupportedVersions {
    fn contains(&self, v: &Version) -> bool {
        self.unsupported_versions.contains(v)
    }
}

impl TryFrom<&[u8]> for UnsupportedVersions {
    type Error = serde_yaml::Error;

    /// Returns an UnsupportedVersions object.
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        serde_yaml::from_reader(bytes)
    }
}
