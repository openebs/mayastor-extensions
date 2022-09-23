use serde::Deserialize;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crate::upgrade::common::error::Error;

/// Helm arguments that are required to run helm commands.
#[derive(Debug, Clone, Default)]
struct HelmArgs {
    release_name: String,
    chart_name: String,
    opts: Vec<(String, String)>,
    namespace: Option<String>,
    values: Vec<PathBuf>,
}

impl HelmArgs {
    /// Set a name.
    fn with_release_name(mut self, name: String) -> Self {
        self.release_name = name;
        self
    }

    /// Set chart name.
    fn with_chart_name(mut self, name: String) -> Self {
        self.chart_name = name;
        self
    }

    /// Set a single option.
    fn with_opt(mut self, key: String, value: String) -> Self {
        self.opts.push((key, value));
        self
    }

    /// Reset array of options.
    fn with_opts(mut self, options: Vec<(String, String)>) -> Self {
        self.opts = options;
        self
    }

    /// Set namespace.
    fn with_namespace(mut self, ns: Option<String>) -> Self {
        self.namespace = ns;
        self
    }

    /// Set values.
    fn with_values(mut self, values: Vec<PathBuf>) -> Self {
        self.values = values;
        self
    }

    /// Set one value.
    fn with_value(mut self, value: PathBuf) -> Self {
        self.values.push(value);
        self
    }

    /// Get release name.
    fn release_name(&self) -> &String {
        &self.release_name
    }

    /// Get chart name.
    fn chart_name(&self) -> &String {
        &self.chart_name
    }

    /// Get options.
    fn opts(&self) -> &Vec<(String, String)> {
        &self.opts
    }

    /// Get namespace.
    fn namespace(&self) -> Option<String> {
        self.namespace.clone()
    }

    /// Get values.
    fn values(&self) -> &Vec<PathBuf> {
        &self.values
    }

    /// Apply arguments for helm command.
    fn apply_args(&self, command: &mut Command) {
        if self.namespace().is_some() {
            command
                .arg("--namespace")
                .arg(self.namespace().as_ref().unwrap());
        }

        for value_path in self.values() {
            command.arg("-f").arg(value_path);
        }

        for (key, val) in self.opts() {
            command.arg("--set").arg(format!("{}={}", key, val));
        }
    }

    /// Run upgrade command.
    fn upgrade(&self) -> Result<Output, Error> {
        self.run([
            "upgrade",
            self.release_name(),
            Path::new("/home/sahil-ubuntu/mayastor-extensions/chart")
                .to_str()
                .unwrap(),
            "--wait",
        ])
    }
    /// Run get values command.
    fn get_values(&self) -> Result<Output, Error> {
        self.run(["get", "values", self.release_name(), "--output=yaml"])
    }

    /// Run helm version command.
    fn version(self) -> Result<Output, Error> {
        self.run(["version", "--short"])
    }

    /// Run helm ls command.
    fn ls(self, exact_match: &String) -> Result<Output, Error> {
        self.run(["ls", "--filter", exact_match, "--output=json"])
    }

    /// Create helm command and run.
    fn run<I, S>(&self, args: I) -> Result<Output, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("helm");
        for arg in args {
            command.arg(arg.as_ref());
        }
        self.apply_args(&mut command);
        let output = command.output();

        match output {
            Ok(out) => {
                if !out.stderr.is_empty() {
                    let stderr = String::from_utf8(out.stderr)
                        .map_err(|error| Error::Utf8 { source: error })?;
                    println!("{:?}", stderr);
                    return Err(Error::HelmStd(stderr));
                }
                Ok(out)
            }
            Err(error) => Err(Error::HelmCommandNotExecutable { source: error }),
        }
    }
}

/// Chart representation.
#[derive(Deserialize, Debug, Clone, Default)]
struct Chart {
    name: String,
    namespace: String,
    revision: String,
    updated: String,
    status: String,
    chart: String,
    app_version: String,
}

impl Chart {
    /// Get name of the chart.
    fn name(&self) -> &String {
        &self.name
    }

    /// Get namespace.
    fn namespace(&self) -> &String {
        &self.namespace
    }

    /// Get installed chart by names.
    fn get_installed_chart_by_name(name: String, namespace: String) -> Result<Chart, Error> {
        let exact_match = format!("^{}$", name);

        let output = HelmArgs::default()
            .with_namespace(Some(namespace))
            .ls(&exact_match)
            .map_err(|_| Error::HelmStd("Error while running helm ls command".to_string()))?;

        let chart: Vec<Chart> = serde_json::from_slice(&output.stdout)
            .map_err(|error| Error::SerdeDeserialization { source: error })?;

        if chart.is_empty() {
            return Err(Error::HelmChartNotFound(
                "Helm chart not installed".to_string(),
            ));
        }

        Ok(chart[0].clone())
    }
}

/// Helm client.
#[derive(Debug, Clone)]
pub(crate) struct HelmClient {
    chart: Chart,
    version: String,
    args: HelmArgs,
}

impl HelmClient {
    /// Create a new helm client if helm is installed.
    pub(crate) async fn new() -> Result<HelmClient, Error> {
        let output = HelmArgs::default()
            .version()
            .map_err(|_| Error::HelmNotInstalled("Helm not installed".to_string()))?;

        // Convert command output into a string.
        let out_str =
            String::from_utf8(output.stdout).map_err(|error| Error::Utf8 { source: error })?;

        // Check that the version command gives a version.
        if !out_str.starts_with("v3.") {
            return Err(Error::HelmVersionNotFound {
                error: "Helm version 3 not installed. Installed Version: {".to_string(),
                version: out_str,
            });
        }

        // If checks succeed, create Helm client.
        Ok(Self {
            args: HelmArgs::default(),
            version: out_str,
            chart: Chart::default(),
        })
    }

    /// Get version.
    pub(crate) fn version(&self) -> &String {
        &self.version
    }

    /// Set chart in helm client.
    pub(crate) fn with_chart(mut self, name: String, namespace: String) -> Result<Self, Error> {
        match Chart::get_installed_chart_by_name(name, namespace) {
            Ok(chart) => {
                self.chart = chart;
                Ok(self)
            }
            Err(err) => Err(err),
        }
    }

    fn args(&self) -> HelmArgs {
        self.args.clone()
    }

    /// Helm upgrade command.
    pub(crate) async fn upgrade(
        &mut self,
        values: Vec<PathBuf>,
        opts: Vec<(String, String)>,
    ) -> Result<(), Error> {
        self.args()
            .with_release_name(self.chart.name.clone())
            .with_namespace(Some(self.chart.namespace.clone()))
            .with_chart_name(self.chart.chart.clone())
            .with_values(values)
            .with_opts(opts)
            .upgrade()?;

        Ok(())
    }

    /// Command to get values of the installed chart.
    pub(crate) fn get_values(&mut self) -> Result<String, Error> {
        let output = self
            .args()
            .with_release_name(self.chart.name.clone())
            .with_namespace(Some(self.chart.namespace.clone()))
            .with_chart_name(self.chart.chart.clone())
            .get_values()?;

        let output =
            String::from_utf8(output.stdout).map_err(|error| Error::Utf8 { source: error })?;
        Ok(output)
    }
}
