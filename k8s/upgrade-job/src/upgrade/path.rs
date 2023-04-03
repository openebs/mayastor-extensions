use std::{
    path::PathBuf,
    fs::File,
};
use std::any::Any;
use k8s_openapi::http::header::FROM;
use snafu::{ensure, ResultExt};
use crate::common::{
    error::{Result, OpeningFile, YamlParseFromFile, YamlStructure, SemverParse, CoreNotASubchartOfUmbrella, UmbrellaChartVersionInvalid},
    constants::{TO_CORE_SEMVER, FROM_CORE_SEMVER, CORE_CHART_NAME, TO_UMBRELLA_SEMVER, FROM_UMBRELLA_SEMVER},
};
use semver::{Version, VersionReq};
use crate::{
    helm::{
        upgrade::{HelmChart},
        chart::{Chart, Dependency},
    },
};
use std::collections::HashMap;
use openapi::tower::client::hyper::HeaderMap;
use serde::Deserialize;

pub(crate) fn is_valid(chart_variant: HelmChart, from: &Version, to: &Version) -> Result<bool> {
    match chart_variant {
        HelmChart::Umbrella => {
            let to_req = VersionReq::parse(TO_UMBRELLA_SEMVER).context(SemverParse { version_string: TO_UMBRELLA_SEMVER.to_string() })?;

            if to_req.matches(to) {
                let from_req = VersionReq::parse(FROM_UMBRELLA_SEMVER).context(SemverParse { version_string: FROM_UMBRELLA_SEMVER.to_string() })?;
                return Ok(
                    from_req.matches(from)
                )
            }
            Ok(true)
        }
        HelmChart::Core => {
            let to_req = VersionReq::parse(TO_CORE_SEMVER).context(SemverParse { version_string: TO_CORE_SEMVER.to_string() })?;

            if to_req.matches(to) {
                let from_req = VersionReq::parse(FROM_CORE_SEMVER).context(SemverParse { version_string: FROM_CORE_SEMVER.to_string() })?;
                return Ok(
                    from_req.matches(from)
                )
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
        serde_yaml::from_reader(values_yaml).context(YamlParseFromFile {
            filepath: path,
        })?;

    Ok(to_chart.version().clone())
  /*
    match chart_variant {
        HelmChart::Umbrella => {
            for dep in to_chart.dependencies().iter() {
                if dep.name().eq(CORE_CHART_NAME) {
                    return Ok(dep.version().clone())
                }
            }
            CoreNotASubchartOfUmbrella.fail()?
        },
        HelmChart::Core => Ok(to_chart.version().clone())
    }

   */
}

pub(crate) fn version_from_release_chart(chart: String) -> Result<Version> {
    let chart_sections: Vec<&str> = chart.split('-').collect();
    // e.g. <chart>-1.2.3-rc.5 -- here the 2nd chunk is the version
    let version = chart_sections[1].to_string();

    Version::parse(version.as_str()).context(SemverParse { version_string: version })
    /*
    match chart_variant {
        HelmChart::Umbrella => {
            ensure!(umbrella_to_core_version_map.contains_key(version.as_str()), UmbrellaChartVersionInvalid { version });
            let core_version = umbrella_to_core_version_map[version.as_str()];
            Version::parse(core_version).context(SemverParse { version_string: core_version.to_string() })
        },
        HelmChart::Core => {
            Version::parse(version.as_str()).context(SemverParse { version_string: version })
        },
    }

     */
}