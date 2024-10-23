use crate::{
    common::{
        constants::{CORE_CHART_NAME, UMBRELLA_CHART_NAME},
        error::{
            HelmUpgradeOptionNamespaceAbsent, HelmUpgradeOptionReleaseNameAbsent,
            InvalidUpgradePath, NoHelmStorageDriver, NoInputHelmChartDir, NotAKnownHelmChart,
            Result, RollbackForbidden, UmbrellaChartNotUpgraded,
        },
        regex::Regex,
    },
    helm::{
        chart::{CoreValues, HelmValuesCollection, UmbrellaValues},
        client::HelmReleaseClient,
        values::generate_values_yaml_file,
    },
    upgrade::path::{
        core_version_from_umbrella_release, is_valid_for_core_chart, version_from_chart_yaml_file,
        version_from_core_chart_release,
    },
    vec_to_strings,
};
use async_trait::async_trait;
use semver::Version;
use snafu::ensure;
use std::{future::Future, path::PathBuf, pin::Pin, str};
use tempfile::NamedTempFile as TempFile;
use tracing::info;

/// HelmUpgradeRunner is returned after an upgrade is validated and dry-run-ed. Running
/// it carries out helm upgrade.
pub(crate) type HelmUpgradeRunner =
    Pin<Box<dyn Future<Output = Result<Box<dyn HelmValuesCollection>>>>>;

/// A trait object of type HelmUpgrader is either CoreHelmUpgrader or an UmbrellaHelmUpgrader.
/// They either deal with upgrading the Core helm chart or the Umbrella helm chart respectively.
/// The Umbrella helm chart is not upgraded using this binary, as it is out of scope.
#[async_trait]
pub(crate) trait HelmUpgrader {
    /// Returns a closure which runs the real upgrade, post-dry-run.
    async fn dry_run(self: Box<Self>) -> Result<HelmUpgradeRunner>;

    /// Return the source helm chart version as a String.
    fn source_version(&self) -> Version;

    /// Return the target helm chart version as a String.
    fn target_version(&self) -> Version;
}

/// This is a builder for the Helm chart upgrade.
#[derive(Default)]
pub(crate) struct HelmUpgraderBuilder {
    release_name: Option<String>,
    namespace: Option<String>,
    core_chart_dir: Option<PathBuf>,
    skip_upgrade_path_validation: bool,
    helm_args_set: Option<String>,
    helm_args_set_file: Option<String>,
    helm_storage_driver: Option<String>,
    // Defaults to false, which is what we want for the time being.
    helm_reset_then_reuse_values: bool,
}

impl HelmUpgraderBuilder {
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

    /// This is a builder option to add set flags during helm upgrade.
    pub(crate) fn with_helm_args_set<J>(mut self, helm_args_set: J) -> Self
    where
        J: ToString,
    {
        self.helm_args_set = Some(helm_args_set.to_string());
        self
    }

    /// This is a builder option to add set-file options during helm upgrade.
    pub(crate) fn with_helm_args_set_file<J>(mut self, helm_args_set_file: J) -> Self
    where
        J: ToString,
    {
        self.helm_args_set_file = Some(helm_args_set_file.to_string());
        self
    }

    /// This is a builder option to add the value of the helm storage driver.
    #[must_use]
    pub(crate) fn with_helm_storage_driver(mut self, driver: String) -> Self {
        self.helm_storage_driver = Some(driver);
        self
    }

    /// This is a builder option to enable the use of the helm --reset-then-reuse-values flag.
    pub(crate) fn with_helm_reset_then_reuse_values(mut self, use_it: bool) -> Self {
        self.helm_reset_then_reuse_values = use_it;
        self
    }

    /// This builds the HelmUpgrade object.
    pub(crate) async fn build(self) -> Result<Box<dyn HelmUpgrader>> {
        // Unwrapping builder inputs. Fails for mandatory inputs.
        let release_name = self
            .release_name
            .ok_or(HelmUpgradeOptionReleaseNameAbsent.build())?;
        let namespace = self
            .namespace
            .ok_or(HelmUpgradeOptionNamespaceAbsent.build())?;
        let chart_dir = self.core_chart_dir.ok_or(
            NoInputHelmChartDir {
                chart_name: CORE_CHART_NAME.to_string(),
            }
            .build(),
        )?;
        let storage_driver = self
            .helm_storage_driver
            .ok_or(NoHelmStorageDriver.build())?;
        let helm_args_set = self.helm_args_set.unwrap_or_default();
        let helm_args_set_file = self.helm_args_set_file.unwrap_or_default();

        // Generate HelmReleaseClient.
        let client = HelmReleaseClient::builder()
            .with_namespace(namespace.as_str())
            .with_storage_driver(storage_driver)
            .build()?;

        // Get the chart_name from the HelmListReleaseElement object for the release specified in
        // CLI options.
        let helm_release = client.release_info(release_name.as_str())?;
        let chart = helm_release.chart();

        // source_values from installed helm chart release.
        let source_values_buf =
            client.get_values_as_yaml::<&str, String>(release_name.as_str(), None)?;

        let chart_dot_yaml_path = chart_dir.join("Chart.yaml");
        // The version of the Core chart which we are (maybe) upgrading to.
        let target_version = version_from_chart_yaml_file(chart_dot_yaml_path)?;

        // Define regular expression to pick out the chart name from the
        // <chart-name>-<chart-version> string.
        let umbrella_regex = format!(r"^({UMBRELLA_CHART_NAME}-[0-9]+\.[0-9]+\.[0-9]+)$");
        // Accepts pre-release and release, both.
        // Q: How do I read this regex?
        // A: This regular expressions is bounded by the '^' and '$' characters, which means
        //    that the input string has to match all of the expression exactly. It is not enough
        //    if a substring within the input string matches the regular expression. The pattern
        //    requires the following conditions to be met:
        //    1. The string must start with the literal contained in the literal CORE_CHART_NAME.
        //       e.g.: mayastor-2.2.0 starts with 'mayastor'
        //    2. A '-' followed by three sets of numbers (each with one or more) separated by '.',
        //       must sit after the value of CORE_CHART_NAME. e.g. mayastor-4.56.789 is a valid
        //       chart-name.
        //    3. A '-' followed by one or many alphanumeric characters may optionally sit after a
        //       chart-name like 'mayastor-1.2.3'. e.g.: mayastor-1.2.3-testing,
        //       mayastor-1.2.3-testing-upgrade-23-35-25-05-2023, mayastor-2.3.0-rc-3
        //    4. The optional group of character(s) mentioned in (3) above, may optionally contain a
        //       '.' followed by a set of numbers. e.g.: mayastor-2.3.4-rc.1, mayastor-2.3.4-alpha.2
        let core_regex =
            format!(r"^({CORE_CHART_NAME}-[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+(\.[0-9]+)?)*)$");

        // Determine chart variant.
        if Regex::new(umbrella_regex.as_str())?.is_match(chart.as_ref()) {
            // The version of the Core helm chart (installed as a dependent chart) which is
            // installed in the cluster.
            let source_version =
                core_version_from_umbrella_release(&client, helm_release.name()).await?;
            info!(version=%source_version, "Found version of dependency chart {CORE_CHART_NAME}");

            // Check if already upgraded.
            let already_upgraded = target_version.eq(&source_version);

            // Fail if the Umbrella chart isn't already upgraded.
            ensure!(
                already_upgraded,
                UmbrellaChartNotUpgraded {
                    target_version: target_version.to_string()
                }
            );

            Ok(Box::new(UmbrellaHelmUpgrader {
                release_name,
                client,
                source_version,
                target_version,
            }))
        } else if Regex::new(core_regex.as_str())?.is_match(chart) {
            // The version of the Core helm chart (installed as the parent chart)
            // which is installed in the cluster.
            let source_version = version_from_core_chart_release(chart)?;
            info!(version=%source_version, "Found version of chart {CORE_CHART_NAME}");

            // Skip upgrade-path validation and allow all upgrades for the Core helm chart, if
            // the flag is set.
            if !self.skip_upgrade_path_validation {
                // Rollbacks not supported.
                ensure!(
                    target_version.ge(&source_version),
                    RollbackForbidden {
                        source_version: source_version.to_string(),
                        target_version: target_version.to_string()
                    }
                );

                // Check if upgrade path is explicitly disallowed via config file.
                let upgrade_path_is_valid = is_valid_for_core_chart(&source_version)?;
                ensure!(upgrade_path_is_valid, InvalidUpgradePath);
            }

            // target_values from values.yaml file.
            let target_values_filepath = chart_dir.join("values.yaml");
            let target_values = CoreValues::try_from(target_values_filepath.as_path())?;

            // source_values from installed helm chart release.
            let source_values = CoreValues::try_from(source_values_buf.as_slice())?;

            // Generate values yaml file for upgrade by merging target_values and source_values
            // yaml files.
            let upgrade_values_file = generate_values_yaml_file(
                &source_version,
                &target_version,
                &source_values,
                &target_values,
                source_values_buf,
                target_values_filepath.as_path(),
                chart_dir.as_path(),
            )
            .await?;

            let mut helm_upgrade_extra_args: Vec<String>;
            if self.helm_reset_then_reuse_values {
                // helm upgrade .. --reset-then-reuse-values --set <a> --set-file <args> --atomic
                helm_upgrade_extra_args = vec_to_strings!["--reset-then-reuse-values"];
            } else {
                // helm upgrade .. -f <values-yaml> --set <a> --set-file <args> --atomic
                helm_upgrade_extra_args =
                    vec_to_strings!["-f", upgrade_values_file.path().to_string_lossy()];
            }
            helm_upgrade_extra_args.extend(vec_to_strings![
                "--set",
                helm_args_set,
                "--set-file",
                helm_args_set_file,
                "--atomic"
            ]);

            Ok(Box::new(CoreHelmUpgrader {
                chart_dir,
                release_name,
                client,
                helm_upgrade_extra_args,
                source_version,
                target_version,
                upgrade_values_file,
            }))
        } else {
            // Case: Helm chart release is not a known helm chart installation.
            return NotAKnownHelmChart { chart_name: chart }.fail();
        }
    }
}

/// This is a HelmUpgrader for the core helm chart. Unlike the UmbrellaHelmUpgrader,
/// this actually can set up a helm upgrade.
pub(crate) struct CoreHelmUpgrader {
    chart_dir: PathBuf,
    release_name: String,
    client: HelmReleaseClient,
    helm_upgrade_extra_args: Vec<String>,
    source_version: Version,
    target_version: Version,
    #[allow(dead_code)]
    upgrade_values_file: TempFile,
}

#[async_trait]
impl HelmUpgrader for CoreHelmUpgrader {
    /// This validates helm upgrade and runs the 'helm upgrade --dry-run' command. Successful exits
    /// from this method returns a HelmUpgradeRunner which is a Future which runs 'helm upgrade'
    /// when awaited on.
    async fn dry_run(self: Box<Self>) -> Result<HelmUpgradeRunner> {
        // Running 'helm upgrade --dry-run'.
        let mut dry_run_extra_args = self.helm_upgrade_extra_args.clone();
        dry_run_extra_args.push("--dry-run".to_string());
        info!("Running helm upgrade dry-run...");
        self.client
            .upgrade(
                self.release_name.as_str(),
                self.chart_dir.as_path(),
                Some(dry_run_extra_args),
            )
            .await?;
        info!("Helm upgrade dry-run succeeded!");

        // Returning HelmUpgradeRunner.
        Ok(Box::pin(async move {
            // Pinning the helm values file handle to this closure so that it is not
            // dropped. This file handle needs to exist in memory for
            // the helm upgrade's "-f <values_file>" argument to work.
            // This handle is dropped when this closure returns, after helm upgrade.
            let _values_file = self.upgrade_values_file;

            info!("Starting helm upgrade...");
            self.client
                .upgrade(
                    self.release_name.as_str(),
                    self.chart_dir,
                    Some(self.helm_upgrade_extra_args),
                )
                .await?;
            info!("Helm upgrade successful!");

            let final_values_buf = self
                .client
                .get_values_as_yaml::<&str, String>(self.release_name.as_str(), None)?;
            let final_values: Box<dyn HelmValuesCollection> =
                Box::new(CoreValues::try_from(final_values_buf.as_slice())?);

            Ok(final_values)
        }))
    }

    fn source_version(&self) -> Version {
        self.source_version.clone()
    }

    fn target_version(&self) -> Version {
        self.target_version.clone()
    }
}

/// This is a HelmUpgrader for the Umbrella chart. This gathers information, and doesn't
/// set up a helm upgrade or a dry-run in any way.
pub(crate) struct UmbrellaHelmUpgrader {
    release_name: String,
    client: HelmReleaseClient,
    source_version: Version,
    target_version: Version,
}

#[async_trait]
impl HelmUpgrader for UmbrellaHelmUpgrader {
    async fn dry_run(self: Box<Self>) -> Result<HelmUpgradeRunner> {
        Ok(Box::pin(async move {
            info!(
                "Verified that '{UMBRELLA_CHART_NAME}' helm chart release '{}' has dependency '{CORE_CHART_NAME}' of version '{}'",
                self.release_name.as_str(),
                self.target_version.to_string()
            );

            let final_values_buf = self
                .client
                .get_values_as_yaml::<&str, String>(self.release_name.as_str(), None)?;
            let final_values: Box<dyn HelmValuesCollection> =
                Box::new(UmbrellaValues::try_from(final_values_buf.as_slice())?);

            Ok(final_values)
        }))
    }

    fn source_version(&self) -> Version {
        self.source_version.clone()
    }

    fn target_version(&self) -> Version {
        self.target_version.clone()
    }
}
