use crate::{
    common::{
        constants::{FROM_UMBRELLA_SEMVER, TO_UMBRELLA_SEMVER},
        error::{HelmChartNameSplit, OpeningFile, Result, SemverParse, YamlParseFromFile},
    },
    helm::{chart::Chart, upgrade::HelmChart},
};
use semver::{Version, VersionReq};

use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::{collections::HashSet, fs::File, path::PathBuf};

/// Validates the upgrade path from 'from' Version to 'to' Version for 'chart_variant' helm chart.
pub(crate) fn is_valid(
    chart_variant: HelmChart,
    from: &Version,
    to: &Version,
    invalid_upgrade_path: HashSet<Version>,
) -> Result<bool> {
    match chart_variant {
        HelmChart::Umbrella => {
            let to_req = VersionReq::parse(TO_UMBRELLA_SEMVER).context(SemverParse {
                version_string: TO_UMBRELLA_SEMVER.to_string(),
            })?;

            if to_req.matches(to) {
                let from_req = VersionReq::parse(FROM_UMBRELLA_SEMVER).context(SemverParse {
                    version_string: FROM_UMBRELLA_SEMVER.to_string(),
                })?;
                return Ok(from_req.matches(from));
            }
            Ok(false)
        }
        HelmChart::Core => Ok(!invalid_upgrade_path.contains(from)),
    }
}

/// Generate a semver::Version from the helm chart in local directory.
pub(crate) fn version_from_chart_yaml_file(path: PathBuf) -> Result<Version> {
    let values_yaml = File::open(path.as_path()).context(OpeningFile {
        filepath: path.clone(),
    })?;

    let to_chart: Chart =
        serde_yaml::from_reader(values_yaml).context(YamlParseFromFile { filepath: path })?;

    Ok(to_chart.version().clone())
}

/// Generate a semver::Version from the 'chart' member of the Helm chart's ReleaseElement.
/// The output of `helm ls -n <namespace> -o yaml` is a list of ReleaseElements.
pub(crate) fn version_from_release_chart(chart_name: String) -> Result<Version> {
    let delimiter: char = '-';
    // e.g. <chart>-1.2.3-rc.5 -- here the 2nd chunk is the version
    let (_, version) = chart_name.as_str().split_once(delimiter).ok_or(
        HelmChartNameSplit {
            chart_name: chart_name.clone(),
            delimiter,
        }
        .build(),
    )?;

    Version::parse(version).context(SemverParse {
        version_string: version.to_string(),
    })
}

/// Struct to deserialize the unsupported version yaml.
#[derive(Debug, Serialize, Deserialize)]
struct UnsupportedVersions {
    unsupported_versions: Vec<String>,
}

/// Returns the HashSet of invalid source versions.
pub(crate) fn invalid_upgrade_path(path: PathBuf) -> Result<HashSet<Version>> {
    let unsupported_versions_yaml = File::open(path.as_path()).context(OpeningFile {
        filepath: path.clone(),
    })?;

    let unsupported: UnsupportedVersions = serde_yaml::from_reader(unsupported_versions_yaml)
        .context(YamlParseFromFile { filepath: path })?;

    let mut unsupported_versions_set: HashSet<Version> = HashSet::new();

    for version in unsupported.unsupported_versions.iter() {
        let unsupported_version = Version::parse(version.as_str()).context(SemverParse {
            version_string: version.clone(),
        })?;
        unsupported_versions_set.insert(unsupported_version);
    }
    Ok(unsupported_versions_set)
}
