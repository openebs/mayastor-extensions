use crate::{
    common::{
        constants::{
            FROM_CORE_SEMVER, FROM_UMBRELLA_SEMVER, TO_CORE_SEMVER, TO_DEVELOP_SEMVER,
            TO_UMBRELLA_SEMVER,
        },
        error::{HelmChartNameSplit, OpeningFile, Result, SemverParse, YamlParseFromFile},
    },
    helm::{chart::Chart, upgrade::HelmChart},
};

use semver::{Version, VersionReq};

use snafu::ResultExt;
use std::{fs::File, path::PathBuf};

/// Validates the upgrade path from 'from' Version to 'to' Version for 'chart_variant' helm chart.
pub(crate) fn is_valid(chart_variant: HelmChart, from: &Version, to: &Version) -> Result<bool> {
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
        HelmChart::Core => {
            let to_req = VersionReq::parse(TO_CORE_SEMVER).context(SemverParse {
                version_string: TO_CORE_SEMVER.to_string(),
            })?;

            let to_develop = VersionReq::parse(TO_DEVELOP_SEMVER).context(SemverParse {
                version_string: TO_DEVELOP_SEMVER.to_string(),
            })?;

            if to_req.matches(to) || to_develop.matches(to) {
                let from_req = VersionReq::parse(FROM_CORE_SEMVER).context(SemverParse {
                    version_string: FROM_CORE_SEMVER.to_string(),
                })?;
                return Ok(from_req.matches(from));
            }
            Ok(false)
        }
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
