use crate::{
    common::error::{
        HelmClientNs, HelmCommand, HelmGetValuesCommand, HelmListCommand, HelmRelease,
        HelmUpgradeCommand, Result, U8VectorToString, YamlParseFromSlice,
    },
    vec_to_strings,
};
use k8s_openapi::serde;
use serde::Deserialize;
use snafu::{ensure, ResultExt};
use std::{path::Path, process::Command, str};
use tracing::debug;

/// This struct is used to deserialize the output of `helm list -n <namespace> --deployed -o yaml`.
#[derive(Clone, Deserialize)]
pub(crate) struct HelmReleaseElement {
    name: String,
    chart: String,
}

impl HelmReleaseElement {
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

    /// Build the HelmReleaseClient.
    pub(crate) fn build(self) -> Result<HelmReleaseClient> {
        let ns = self.namespace.ok_or(HelmClientNs.build())?;
        Ok(HelmReleaseClient { namespace: ns })
    }
}

/// This type has functions which execute helm commands to fetch info about and modify helm
/// releases.
#[derive(Clone)]
pub(crate) struct HelmReleaseClient {
    pub(crate) namespace: String,
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
    ) -> Result<Vec<HelmReleaseElement>>
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
    pub(crate) fn release_info<A>(&self, release_name: A) -> Result<HelmReleaseElement>
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
