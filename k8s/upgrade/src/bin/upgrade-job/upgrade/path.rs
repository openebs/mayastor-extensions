use crate::{
    common::{
        constants::CHART_VERSION_LABEL_KEY,
        error::{
            ListDeploymentsWithLabel, NoRestDeployment, NoVersionLabelInDeployment, ReadingFile,
            Result, SemverParse, YamlParseBufferForUnsupportedVersion, YamlParseFromFile,
        },
        kube_client as KubeClient,
    },
    helm::chart::Chart,
};
use kube_client::{api::ListParams, ResourceExt};
use semver::Version;
use serde::Deserialize;
use snafu::{ensure, ResultExt};
use std::{fs, path::PathBuf};
use utils::API_REST_LABEL;

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

/// Generate a semver::Version from the CHART_VERSION_LABEL_KEY label on the Storage REST API
/// Deployment.
pub(crate) async fn version_from_rest_deployment_label(ns: &str) -> Result<Version> {
    let labels = format!("{API_REST_LABEL},{CHART_VERSION_LABEL_KEY}");

    let deployments_api = KubeClient::deployments_api(ns).await?;
    let mut deploy_list = deployments_api
        .list(&ListParams::default().labels(labels.as_str()))
        .await
        .context(ListDeploymentsWithLabel {
            namespace: ns.to_string(),
            label_selector: labels.clone(),
        })?;

    ensure!(
        !deploy_list.items.is_empty(),
        NoRestDeployment {
            namespace: ns.to_string(),
            label_selector: labels
        }
    );

    // The most recent one sits on top.
    deploy_list
        .items
        .sort_by_key(|b| std::cmp::Reverse(b.creation_timestamp()));

    // The only ways there could be more than one version of the Storage REST API Pod in the
    // namespace are: 1. More than one version of the Storage cluster is deployed, by means of
    // multiple helm charts or otherwise         This will never come to a stable state, as some
    // of the components will be trying to claim the same         resources. So, in this case
    // the Storage cluster isn't broken because of upgrade-job. Upgrade should
    //         eventually fail for these cases, because the component containers keep erroring out.
    // 2. Helm upgrade is stuck with the older REST API Pod in 'Terminating' state: This scenario is
    //    more likely than the one above. This may result is more-than-one
    // REST API deployments.         If the helm upgrade has succeeded already, we'd want to hit
    // the 'already_upgraded' case in         crate::helm::upgrade. The upgraded version will be
    // on the latest-created REST API deployment.
    let deploy = &deploy_list.items[0];
    let deploy_version = deploy.labels().get(CHART_VERSION_LABEL_KEY).ok_or(
        NoVersionLabelInDeployment {
            deployment_name: deploy.name_any(),
            namespace: ns.to_string(),
        }
        .build(),
    )?;
    Version::parse(deploy_version.as_str()).context(SemverParse {
        version_string: deploy_version.clone(),
    })
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
