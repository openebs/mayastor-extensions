use crate::{
    common::{
        constants::{FROM_CORE_SEMVER, FROM_UMBRELLA_SEMVER, TO_CORE_SEMVER, TO_UMBRELLA_SEMVER},
        error::{OpeningFile, Result, SemverParse, YamlParseFromFile},
    },
    helm::{chart::Chart, upgrade::HelmChart},
};

use semver::{Version, VersionReq};

use snafu::ResultExt;
use std::{fs::File, path::PathBuf};

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
            Ok(true)
        }
        HelmChart::Core => {
            let to_req = VersionReq::parse(TO_CORE_SEMVER).context(SemverParse {
                version_string: TO_CORE_SEMVER.to_string(),
            })?;

            if to_req.matches(to) {
                let from_req = VersionReq::parse(FROM_CORE_SEMVER).context(SemverParse {
                    version_string: FROM_CORE_SEMVER.to_string(),
                })?;
                return Ok(from_req.matches(from));
            }
            Ok(true)
        }
    }
}

pub(crate) fn version_from_chart_yaml_file(path: PathBuf) -> Result<Version> {
    let values_yaml = File::open(path.as_path()).context(OpeningFile {
        filepath: path.clone(),
    })?;

    let to_chart: Chart =
        serde_yaml::from_reader(values_yaml).context(YamlParseFromFile { filepath: path })?;

    Ok(to_chart.version().clone())
}

pub(crate) fn version_from_release_chart(chart: String) -> Result<Version> {
    let chart_sections: Vec<&str> = chart.split('-').collect();
    // e.g. <chart>-1.2.3-rc.5 -- here the 2nd chunk is the version
    let version = chart_sections[1].to_string();

    Version::parse(version.as_str()).context(SemverParse {
        version_string: version,
    })
}
