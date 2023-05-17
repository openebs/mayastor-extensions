use crate::{
    common::{
        constants::{CORE_CHART_NAME, TO_UMBRELLA_SEMVER, UMBRELLA_CHART_NAME},
        error::{
            CoreChartUpgradeNoneChartDir, HelmUpgradeOptionsAbsent, InvalidHelmUpgrade,
            InvalidUpgradePath, NoInputHelmChartDir, NotAKnownHelmChart, RegexCompile, Result,
            RollbackForbidden, UmbrellaChartNotUpgraded,
        },
    },
    helm::{client::HelmReleaseClient, values::generate_values_args},
    upgrade,
};
use regex::Regex;
use semver::Version;

use snafu::{ensure, ResultExt};
use std::path::PathBuf;
use tracing::info;

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
    core_chart_dir: Option<PathBuf>,
    skip_upgrade_path_validation: bool,
}

impl HelmUpgradeBuilder {
    /// This is a builder option to add the Namespace of the helm chart to be upgraded.
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
    pub(crate) fn with_core_chart_dir(mut self, dir: PathBuf) -> Self {
        self.core_chart_dir = Some(dir);
        self
    }

    /// This sets the flag to skip upgrade path validation.
    #[must_use]
    pub(crate) fn with_skip_upgrade_path_validation(
        mut self,
        skip_upgrade_path_validation: bool,
    ) -> Self {
        self.skip_upgrade_path_validation = skip_upgrade_path_validation;
        self
    }

    /// This builds the HelmUpgrade object.
    pub(crate) async fn build(self) -> Result<HelmUpgrade> {
        ensure!(
            self.release_name.is_some() && self.namespace.is_some(),
            HelmUpgradeOptionsAbsent
        );
        let release_name = self.release_name.clone().unwrap();
        let namespace = self.namespace.clone().unwrap();

        // Generate HelmReleaseClient.
        let client = HelmReleaseClient::builder()
            .with_namespace(namespace.clone())
            .build()?;

        // Get HelmReleaseElement object for the release specified in CLI options.
        let chart = client.release_info(release_name.clone())?.chart();

        // The version of the Core helm chart (installed as a the parent chart or as a dependent
        // chart) which is installed in the cluster.
        let from_version: Version =
            upgrade::path::version_from_rest_deployment_label(namespace.as_str()).await?;

        // The version of the Core chart which we are (maybe) going to.
        let chart_dir: PathBuf = self.core_chart_dir.ok_or(
            NoInputHelmChartDir {
                chart_name: CORE_CHART_NAME.to_string(),
            }
            .build(),
        )?;
        let chart_yaml_path = chart_dir.join("Chart.yaml");
        let to_version: Version = upgrade::path::version_from_chart_yaml_file(chart_yaml_path)?;

        // Basic validation.
        // Rollbacks not supported.
        ensure!(
            to_version.ge(&from_version),
            RollbackForbidden {
                from_version: from_version.to_string(),
                to_version: to_version.to_string()
            }
        );
        // Check if already upgraded.
        let already_upgraded = to_version.eq(&from_version);

        // Define regular expression to pick out the chart name from the
        // <chart-name>-<chart-version> string.
        let umbrella_chart_regex = format!(r"^({UMBRELLA_CHART_NAME}-[0-9]+\.[0-9]+\.[0-9]+)$");
        // Accepts pre-release and release, both.
        let core_chart_regex =
            format!(r"^({CORE_CHART_NAME}-[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?)$");

        // Validate if already upgraded for Umbrella chart, and prepare for upgrade for Core chart.
        let chart_variant: HelmChart;
        let mut core_chart_dir: Option<String> = None;
        let mut core_chart_extra_args: Option<Vec<String>> = None;

        if Regex::new(umbrella_chart_regex.as_str()) // Case: HelmChart::Umbrella.
            .context(RegexCompile {
                expression: umbrella_chart_regex.clone(),
            })?
            .is_match(chart.as_str())
        {
            chart_variant = HelmChart::Umbrella;
            ensure!(already_upgraded, UmbrellaChartNotUpgraded);
        } else if Regex::new(core_chart_regex.as_str()) // Case: HelmChart::Core.
            .context(RegexCompile {
                expression: core_chart_regex.clone(),
            })?
            .is_match(chart.as_str())
        {
            chart_variant = HelmChart::Core;

            // Skip upgrade-path validation and allow all upgrades for the Core helm chart, if the
            // flag is set.
            if !self.skip_upgrade_path_validation {
                let upgrade_path_is_valid = upgrade::path::is_valid_for_core_chart(&from_version)?;
                ensure!(upgrade_path_is_valid, InvalidUpgradePath);
            }

            // Generate args to pass to the `helm upgrade` command.
            let values_yaml_path = chart_dir.join("values.yaml");
            let upgrade_args: Vec<String> = generate_values_args(
                &from_version,
                values_yaml_path,
                &client,
                release_name.clone(),
            )?;

            core_chart_dir = Some(chart_dir.to_string_lossy().to_string());
            core_chart_extra_args = Some(upgrade_args);
        } else {
            // Case: Helm chart release is not a known helm chart installation.
            return NotAKnownHelmChart { chart_name: chart }.fail();
        }

        Ok(HelmUpgrade {
            chart_variant,
            already_upgraded,
            core_chart_dir,
            release_name,
            client,
            core_chart_extra_args,
            from_version,
            to_version,
        })
    }
}

/// This type can generate and execute the `helm upgrade` command.
pub(crate) struct HelmUpgrade {
    chart_variant: HelmChart,
    already_upgraded: bool,
    core_chart_dir: Option<String>,
    release_name: String,
    client: HelmReleaseClient,
    core_chart_extra_args: Option<Vec<String>>,
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
        match self.chart_variant {
            HelmChart::Umbrella if self.already_upgraded => {
                info!(
                    "Verified that {UMBRELLA_CHART_NAME} helm chart release '{}' has version {TO_UMBRELLA_SEMVER}",
                    self.release_name.as_str()
                );
            }
            HelmChart::Umbrella if !self.already_upgraded => {
                // It should be impossible to hit this error.
                return UmbrellaChartNotUpgraded.fail();
            }
            HelmChart::Core if self.already_upgraded => {
                info!(
                    "Skipping helm upgrade, as the version of the installed helm chart is the same \
                    as that of this upgrade-job's helm chart"
                );
            }
            HelmChart::Core if !self.already_upgraded => {
                // It should be impossible to hit this error.
                let chart_dir = self
                    .core_chart_dir
                    .ok_or(CoreChartUpgradeNoneChartDir.build())?;

                info!("Starting helm upgrade...");
                self.client
                    .upgrade(self.release_name, chart_dir, self.core_chart_extra_args)?;
                info!("Helm upgrade successful!");
            }
            _ => {
                return InvalidHelmUpgrade.fail();
            }
        }

        Ok(())
    }

    pub(crate) fn upgrade_from_version(&self) -> String {
        self.from_version.to_string()
    }

    pub(crate) fn upgrade_to_version(&self) -> String {
        self.to_version.to_string()
    }
}
