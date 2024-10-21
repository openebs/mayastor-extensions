use crate::{
    common::{
        error::{
            Base64DecodeHelmStorage, DeserializaHelmStorageData, GzipDecoderReadToEnd,
            HelmClientNs, HelmCommand, HelmGetValuesCommand, HelmListCommand, HelmRelease,
            HelmStorageNoData, HelmStorageNoReleaseValue, HelmUpgradeCommand,
            MissingMemberInHelmStorageData, NoHelmStorageDriver, Result, U8VectorToString,
            UnsupportedStorageDriver, YamlParseFromSlice,
        },
        kube::client as KubeClient,
    },
    vec_to_strings,
};
use base64::engine::{general_purpose::STANDARD, Engine as base64_engine};
use flate2::read::GzDecoder;
use k8s_openapi::kind;
use serde::Deserialize;
use snafu::{ensure, ResultExt};
use std::{io::Read, path::Path, process::Command, str};
use tracing::debug;

/// This is used to pick out the .data field from a kubernetes secret or a configmap.
macro_rules! extract_data {
    ($source:ident) => {{
        let driver = kind(&$source);
        $source
            .data
            .ok_or(HelmStorageNoData { driver }.build())?
            .into_iter()
            .find_map(|(k, v)| k.eq("release").then_some(v))
            .ok_or(HelmStorageNoReleaseValue { driver }.build())
    }};
}

/// This is used to deserialize the JSON data in a helm storage resource (secret or configmap).
#[derive(Debug, Deserialize)]
pub(crate) struct HelmChartRelease {
    chart: Option<HelmChartReleaseChart>,
}

/// This is used to deserialize release.chart.
#[derive(Debug, Deserialize)]
pub(crate) struct HelmChartReleaseChart {
    metadata: HelmChartReleaseChartMetadata,
}

/// This is used to deserialize release.chart.metadata.
#[derive(Debug, Deserialize)]
pub(crate) struct HelmChartReleaseChartMetadata {
    dependencies: Option<Vec<HelmChartReleaseChartMetadataDependency>>,
}

/// This is used to deserialize release.chart.metadata.dependency[].
#[derive(Debug, Deserialize)]
pub(crate) struct HelmChartReleaseChartMetadataDependency {
    name: String,
    version: Option<String>,
}

impl HelmChartReleaseChartMetadataDependency {
    /// Returns the name of the dependency chart.
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the version of the dependency chart.
    pub(crate) fn version(self) -> Option<String> {
        self.version
    }
}

/// This performs a base64 decode and Gzip Decode for the data extracted from the helm storage.
fn decode_decompress_data(data: impl AsRef<[u8]>) -> Result<Vec<u8>> {
    let data_compressed =
        base64_engine::decode(&STANDARD, data).context(Base64DecodeHelmStorage)?;

    let mut gzip_decoder = GzDecoder::new(&data_compressed[..]);
    let mut data: Vec<u8> = Vec::new();
    gzip_decoder
        .read_to_end(&mut data)
        .context(GzipDecoderReadToEnd)?;

    Ok(data)
}

/// Extract list of dependencies from a chart's release_data or fail.
fn dependencies_from_release_data(
    data: Vec<u8>,
) -> Result<Vec<HelmChartReleaseChartMetadataDependency>> {
    let release: HelmChartRelease =
        serde_json::from_slice(data.as_slice()).context(DeserializaHelmStorageData)?;

    let missing_member_err = |member: &'static str| -> crate::common::error::Error {
        MissingMemberInHelmStorageData { member }.build()
    };

    release
        .chart
        .ok_or(missing_member_err(".chart"))?
        .metadata
        .dependencies
        .ok_or(missing_member_err(".chart.metadata.dependencies"))
}

/// This struct is used to deserialize the output of `helm list -n <namespace> --deployed -o yaml`.
#[derive(Clone, Deserialize)]
pub(crate) struct HelmListReleaseElement {
    name: String,
    chart: String,
}

impl HelmListReleaseElement {
    /// This is a getter function for the name of the release.
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }
    /// This is a getter function for the chart_name of the release. This also containers the chart
    /// version.
    pub(crate) fn chart(&self) -> &str {
        self.chart.as_str()
    }
}

/// This is a builder for HelmReleaseClient.
#[derive(Default)]
pub(crate) struct HelmReleaseClientBuilder {
    namespace: Option<String>,
    storage_driver: Option<String>,
}

impl HelmReleaseClientBuilder {
    /// This is a builder option to add Namespace. This is mandatory,
    /// because all helm releases are tied to a Namespace.
    #[must_use]
    pub(crate) fn with_namespace<J>(mut self, ns: J) -> Self
    where
        J: ToString,
    {
        self.namespace = Some(ns.to_string());
        self
    }

    /// Set the storage driver to use with helm commands.
    #[must_use]
    pub(crate) fn with_storage_driver(mut self, driver: String) -> Self {
        self.storage_driver = Some(driver);
        self
    }

    /// Build the HelmReleaseClient.
    pub(crate) fn build(self) -> Result<HelmReleaseClient> {
        let namespace = self.namespace.ok_or(HelmClientNs.build())?;
        let storage_driver = self.storage_driver.ok_or(NoHelmStorageDriver.build())?;
        Ok(HelmReleaseClient {
            namespace,
            storage_driver,
        })
    }
}

/// This type has functions which execute helm commands to fetch info about and modify helm
/// releases.
#[derive(Clone)]
pub(crate) struct HelmReleaseClient {
    pub(crate) namespace: String,
    /// This is the information that Helm stores on the cluster about the state of a helm release.
    /// Ref: https://github.com/helm/helm/blob/v3.15.0/pkg/action/action.go#L383
    pub(crate) storage_driver: String,
}

impl HelmReleaseClient {
    /// This creates an empty builder.
    pub(crate) fn builder() -> HelmReleaseClientBuilder {
        HelmReleaseClientBuilder::default()
    }

    /// Runs command `helm get values -n <namespace> <release_name> --all -o yaml`.
    pub(crate) fn get_values_as_yaml<A, B>(
        &self,
        release_name: A,
        maybe_extra_args: Option<Vec<B>>,
    ) -> Result<Vec<u8>>
    where
        A: ToString,
        B: ToString,
    {
        let command: &str = "helm";
        let mut args: Vec<String> = vec_to_strings![
            "get",
            "values",
            release_name,
            "-n",
            self.namespace.as_str(),
            "-a"
        ];

        // Extra args
        if let Some(extra_args) = maybe_extra_args {
            args.extend(extra_args.iter().map(ToString::to_string));
        }

        // Because this option has to be at the end for it to work.
        let output_format_args: Vec<String> = vec_to_strings!["-o", "yaml"];
        args.extend(output_format_args);

        debug!(%command, ?args, "Helm get values command");

        let output = Command::new(command)
            .env("HELM_DRIVER", self.storage_driver.as_str())
            .args(args.clone())
            .output()
            .context(HelmCommand {
                command: command.to_string(),
                args: args.clone(),
            })?;

        ensure!(
            output.status.success(),
            HelmGetValuesCommand {
                command: command.to_string(),
                args,
                std_err: str::from_utf8(output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        Ok(output.stdout)
    }

    /// Runs command `helm list -n <namespace> --deployed -o yaml`.
    pub(crate) fn list_as_yaml<A>(
        &self,
        maybe_extra_args: Option<Vec<A>>,
    ) -> Result<Vec<HelmListReleaseElement>>
    where
        A: ToString,
    {
        let command: &str = "helm";
        let mut args: Vec<String> =
            vec_to_strings!["list", "-n", self.namespace.as_str(), "--deployed"];

        // Extra args
        if let Some(extra_args) = maybe_extra_args {
            args.extend(extra_args.iter().map(ToString::to_string));
        }

        // Because this option has to be at the end for it to work.
        let output_format_args: Vec<String> = vec_to_strings!["-o", "yaml"];
        args.extend(output_format_args);

        debug!(%command, ?args, "Helm list command");

        let output = Command::new(command)
            .env("HELM_DRIVER", self.storage_driver.as_str())
            .args(args.clone())
            .output()
            .context(HelmCommand {
                command: command.to_string(),
                args: args.clone(),
            })?;

        let stdout_str = str::from_utf8(output.stdout.as_slice()).context(U8VectorToString)?;
        debug!(stdout=%stdout_str, "Helm list command standard output");
        ensure!(
            output.status.success(),
            HelmListCommand {
                command: command.to_string(),
                args,
                std_err: str::from_utf8(output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        serde_yaml::from_slice(output.stdout.as_slice()).context(YamlParseFromSlice {
            input_yaml: stdout_str.to_string(),
        })
    }

    /// Reads from the helm storage driver and returns a type with info. about dependencies.
    pub(crate) async fn get_dependencies(
        &self,
        release_name: &str,
    ) -> Result<Vec<HelmChartReleaseChartMetadataDependency>> {
        match self.storage_driver.as_str() {
            "" | "secret" | "secrets" => {
                debug!("Using helm secret as helm storage");
                let secret = KubeClient::get_helm_release_secret(
                    release_name.to_string(),
                    self.namespace.clone(),
                )
                .await?;

                let release_data = extract_data!(secret)?;
                let decoded_data = decode_decompress_data(release_data.0)?;
                let dependencies = dependencies_from_release_data(decoded_data)?;
                debug!(data=?dependencies, "Found helm chart release chart metadata dependency in helm secret");

                Ok(dependencies)
            }
            "configmap" | "configmaps" => {
                debug!("Using helm configmap as helm storage");
                let cm = KubeClient::get_helm_release_configmap(
                    release_name.to_string(),
                    self.namespace.clone(),
                )
                .await?;
                let release_data = extract_data!(cm)?;
                let decoded_data = decode_decompress_data(release_data)?;
                let dependencies = dependencies_from_release_data(decoded_data)?;
                debug!(data=?dependencies, "Found helm chart release chart metadata dependency in helm configmap");

                Ok(dependencies)
            }
            unsupported_driver => UnsupportedStorageDriver {
                driver: unsupported_driver.to_string(),
            }
            .fail(),
        }
    }

    /// Runs command `helm upgrade -n <namespace> <release_name> <chart_dir>`.
    pub(crate) async fn upgrade<A, B, P>(
        &self,
        release_name: A,
        chart_dir: P,
        maybe_extra_args: Option<Vec<B>>,
    ) -> Result<()>
    where
        A: ToString,
        B: ToString,
        P: AsRef<Path>,
    {
        let command: &str = "helm";
        let mut args: Vec<String> = vec_to_strings![
            "upgrade",
            release_name,
            chart_dir.as_ref().to_string_lossy(),
            "-n",
            self.namespace.as_str(),
            "--timeout",
            "15m"
        ];

        // Extra args
        if let Some(extra_args) = maybe_extra_args {
            args.extend(extra_args.iter().map(ToString::to_string));
        }

        debug!(%command, ?args, "Helm upgrade command");
        let output = Command::new(command)
            .env("HELM_DRIVER", self.storage_driver.as_str())
            .args(args.clone())
            .output()
            .context(HelmCommand {
                command: command.to_string(),
                args: args.clone(),
            })?;

        let stdout_str = str::from_utf8(output.stdout.as_slice()).context(U8VectorToString)?;
        debug!(stdout=%stdout_str, "Helm upgrade command standard output");
        ensure!(
            output.status.success(),
            HelmUpgradeCommand {
                command: command.to_string(),
                args,
                std_err: str::from_utf8(output.stderr.as_slice())
                    .context(U8VectorToString)?
                    .to_string()
            }
        );

        Ok(())
    }

    /// Fetches info about a Helm release in the Namespace, if it exists.
    pub(crate) fn release_info<A>(&self, release_name: A) -> Result<HelmListReleaseElement>
    where
        A: ToString,
    {
        let release_list = self.list_as_yaml::<A>(None)?;
        let release_name = release_name.to_string();

        for release in release_list.into_iter() {
            if release.name().eq(&release_name) {
                return Ok(release);
            }
        }

        // The code reaching this line means that the release is not there, even though we might
        // have seen that it exists some while back when validating the input Helm release
        // name in the input Namespace.
        HelmRelease {
            name: release_name,
            namespace: self.namespace.clone(),
        }
        .fail()
    }
}
