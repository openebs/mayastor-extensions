use crate::{
    common::{
        constants::{CORE_CHART_NAME, UMBRELLA_CHART_NAME},
        error::{
            HelmUpgradeOptionsAbsent, InvalidUpgradePath, NoInputHelmChartDir, NotAKnownHelmChart,
            RegexCompile, Result,
        },
    },
    helm::{client::HelmReleaseClient, values::generate_values_args},
    upgrade,
};
use regex::Regex;
use semver::Version;

use snafu::{ensure, ResultExt};
use std::path::PathBuf;

/// This is the helm chart variant of the helm chart installed in the cluster.
/// The PRODUCT may be installed using either of these options, but never both.
#[derive(Clone, Default)]
pub(crate) enum HelmChart {
    #[default]
    Umbrella,
    Core,
}

/// This is a builder for the Helm chart upgrade.
#[derive(Default)]
pub(crate) struct HelmUpgradeBuilder {
    release_name: Option<String>,
    namespace: Option<String>,
    umbrella_chart_dir: Option<PathBuf>,
    core_chart_dir: Option<PathBuf>,
}

impl HelmUpgradeBuilder {
    /// This is a builder option to add the Namespace of the helm chart to be upgrade.
    #[must_use]
    pub(crate) fn with_namespace<J>(mut self, ns: J) -> Self
    where
        J: ToString,
    {
        self.namespace = Some(ns.to_string());
        self
    }

    /// This is a builder option to add the release name of the helm chart to be upgraded.
    #[must_use]
    pub(crate) fn with_release_name<J>(mut self, release_name: J) -> Self
    where
        J: ToString,
    {
        self.release_name = Some(release_name.to_string());
        self
    }

    /// This is a builder option to set the directory path of the Umbrella helm chart CLI option.
    #[must_use]
    pub(crate) fn with_umbrella_chart_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.umbrella_chart_dir = dir;
        self
    }

    /// This is a builder option to set the directory path of the Umbrella helm chart CLI option.
    #[must_use]
    pub(crate) fn with_core_chart_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.core_chart_dir = dir;
        self
    }

    /// This adds buiilds the HelmUpgrade object.
    pub(crate) fn build(self) -> Result<HelmUpgrade> {
        ensure!(
            self.release_name.is_some() && self.namespace.is_some(),
            HelmUpgradeOptionsAbsent
        );

        let release_name = self.release_name.clone().unwrap();
        // Generate HelmReleaseClient.
        let client = HelmReleaseClient::builder()
            .with_namespace(self.namespace.clone().unwrap())
            .build()?;
        // Get HelmReleaseElement object for the release specified in CLI options.
        let chart = client.release_info(release_name.clone())?.chart();

        // Define regular expression to pick out the chart name from the
        // <chart-name>-<chart-version> string.
        let umbrella_chart_regex = format!(r"^({UMBRELLA_CHART_NAME}-[0-9]+\.[0-9]+\.[0-9]+)$");
        // Accepts pre-release and release, both.
        let core_chart_regex =
            format!(r"^({CORE_CHART_NAME}-[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?)$");

        // Assign HelmChart variant and validate directory path input for the said
        // variant's chart based on the 'chart' member of the HelmReleaseElement.
        let mut chart_variant: HelmChart = Default::default();
        let mut chart_dir: PathBuf = Default::default();
        // Case: HelmChart::Umbrella.
        if Regex::new(umbrella_chart_regex.as_str())
            .context(RegexCompile {
                expression: umbrella_chart_regex.clone(),
            })?
            .is_match(chart.as_str())
        {
            chart_variant = HelmChart::Umbrella;
            match self.umbrella_chart_dir {
                Some(umbrella_dir) => chart_dir = umbrella_dir,
                None => NoInputHelmChartDir {
                    chart_name: UMBRELLA_CHART_NAME.to_string(),
                }
                .fail()?,
            }
        } else if Regex::new(core_chart_regex.as_str()) // Case: HelmChart::Core.
            .context(RegexCompile {
                expression: core_chart_regex.clone(),
            })?
            .is_match(chart.as_str())
        {
            chart_variant = HelmChart::Core;
            match self.core_chart_dir {
                Some(core_dir) => chart_dir = core_dir,
                None => NoInputHelmChartDir {
                    chart_name: CORE_CHART_NAME.to_string(),
                }
                .fail()?,
            }
        } else {
            // Case: Helm chart release is not a known helm chart installation.
            NotAKnownHelmChart {
                chart_name: chart.clone(),
            }
            .fail()?;
        }

        // Validating upgrade path.
        let mut chart_yaml_path = chart_dir.clone();
        chart_yaml_path.push("Chart.yaml");
        let to_version = upgrade::path::version_from_chart_yaml_file(chart_yaml_path)?;
        let from_version = upgrade::path::version_from_release_chart(chart)?;
        let upgrade_path_is_valid =
            upgrade::path::is_valid(chart_variant.clone(), &from_version, &to_version)?;
        ensure!(upgrade_path_is_valid, InvalidUpgradePath);

        // Generate args to pass to the `helm upgrade` command.
        let mut values_yaml_path = chart_dir.clone();
        values_yaml_path.push("values.yaml");
        let mut upgrade_args: Vec<String> = generate_values_args(
            chart_variant,
            values_yaml_path,
            &client,
            release_name.clone(),
        )?;
        upgrade_args.push("--wait".to_string()); // To wait for all Pods, PVCs to come to a ready state.

        Ok(HelmUpgrade {
            chart_dir: chart_dir.to_string_lossy().to_string(),
            release_name,
            client,
            extra_args: upgrade_args,
            from_version,
            to_version,
        })
    }
}

/// This type can generate and execute the `helm upgrade` command.
pub(crate) struct HelmUpgrade {
    chart_dir: String,
    release_name: String,
    client: HelmReleaseClient,
    extra_args: Vec<String>,
    from_version: Version,
    to_version: Version,
}

impl HelmUpgrade {
    /// This creates a default instance of the HelmUpgradeBuilder.
    pub(crate) fn builder() -> HelmUpgradeBuilder {
        HelmUpgradeBuilder::default()
    }

    /// Use the HelmReleaseClient's upgrade method to upgrade the installed helm release.
    pub(crate) fn run(self) -> Result<()> {
        self.client
            .upgrade(self.release_name, self.chart_dir, Some(self.extra_args))
    }

    pub(crate) fn installed_version(&self) -> String {
        self.from_version.to_string()
    }

    pub(crate) fn to_version(&self) -> String {
        self.to_version.to_string()
    }
}
