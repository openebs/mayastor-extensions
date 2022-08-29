use serde::Deserialize;
use std::{path::PathBuf, process::Command};

use super::error::HelmError;

/// helm arguments that are required to run helm commands
#[derive(Debug, Clone)]
pub struct HelmArgs {
    name: Option<String>,
    opts: Vec<(String, String)>,
    namespace: Option<String>,
    values: Vec<PathBuf>,
}

impl Default for HelmArgs {
    fn default() -> Self {
        Self {
            name: None,
            opts: vec![],
            namespace: None,
            values: vec![],
        }
    }
}

impl HelmArgs {
    /// set a name
    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    /// set a single option
    pub fn set_opt(&mut self, key: String, value: String) {
        self.opts.push((key.into(), value.into()));
    }

    /// reset array of options
    pub fn set_opts(&mut self, options: Vec<(String, String)>) {
        self.opts = options;
    }

    /// set namepsace
    pub fn set_namespace(&mut self, ns: String) {
        self.namespace = Some(ns);
    }

    /// set one value
    pub fn set_values(&mut self, values: Vec<PathBuf>) {
        self.values = values;
    }

    /// set one value
    pub fn set_value(&mut self, value: PathBuf) {
        self.values.push(value);
    }

    /// set a name
    pub fn get_name(&self) -> &String {
        &self.name.as_ref().unwrap()
    }

    /// set a single option
    pub fn get_opts(&self) -> &Vec<(String, String)> {
        &self.opts
    }

    /// set namepsace
    pub fn get_namespace(&self) -> &String {
        &self.namespace.as_ref().unwrap()
    }

    /// set one value
    pub fn get_values(&self) -> &Vec<PathBuf> {
        &self.values
    }
}

/// chart representation
#[derive(Deserialize, Debug, Clone)]
pub struct Chart {
    name: String,
    namespace: String,
    revision: String,
    updated: String,
    status: String,
    chart: String,
    app_version: String,
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            name: Default::default(),
            namespace: Default::default(),
            revision: Default::default(),
            updated: Default::default(),
            status: Default::default(),
            chart: Default::default(),
            app_version: Default::default(),
        }
    }
}

impl Chart {
    /// get name of the chart
    pub fn name(&self) -> &String {
        &self.name
    }

    /// get namespace
    pub fn namespace(&self) -> &String {
        &self.namespace
    }

    /// get installed chart by name s
    pub fn get_installed_chart_by_name(
        name: String,
        namespace: String,
    ) -> Result<Chart, HelmError> {
        let exact_match = format!("^{}$", name);
        println!("{}", exact_match);
        let mut command = Command::new("helm");
        command
            .arg("ls")
            .arg("--filter")
            .arg(exact_match)
            .arg("--namespace")
            .arg(namespace)
            .arg("--output")
            .arg("json");

        let output = command.output().map_err(|_| {
            HelmError::HelmStdError("Error while running helm ls command".to_string())
        })?;

        let chart: Vec<Chart> = serde_json::from_slice(&output.stdout).map_err(|_| {
            HelmError::SerdeDeserializationError("Deserialization error".to_string())
        })?;

        if chart.len() == 0 {
            return Err(HelmError::HelmChartNotFound(
                "Helm chart not installed".to_string(),
            ));
        }

        Ok(chart[0].clone())
    }
}

/// Helm client
#[derive(Debug, Clone)]
pub struct HelmClient {
    chart: Chart,
    version: Option<String>,
    args: HelmArgs,
}

impl HelmClient {
    /// create a new helm client if helm is installed
    pub fn new() -> Result<HelmClient, HelmError> {
        let output = Command::new("helm")
            .arg("version")
            .arg("--short")
            .output()
            .map_err(|_| HelmError::HelmNotInstalled("Helm not installed".to_string()))?;

        // Convert command output into a string
        let out_str = String::from_utf8(output.stdout)
            .map_err(|_| HelmError::Utf8Error("Unable to convert into string".to_string()))?;

        // Check that the version command gives a version.
        if !out_str.starts_with("v3.") {
            return Err(HelmError::HelmVersionNotFound(
                "Helm version 3 not installed, installed version:{}".to_string(),
                out_str,
            ));
        }

        // If checks succeed, create Helm client
        Ok(Self {
            args: HelmArgs::default(),
            version: Some(out_str),
            chart: Chart::default(),
        })
    }

    /// get version
    pub fn version(&self) -> &String {
        self.version.as_ref().unwrap()
    }

    /// set chart in helm client
    pub fn set_chart(mut self, name: String, namespace: String) -> Result<Self, HelmError> {
        match Chart::get_installed_chart_by_name(name, namespace) {
            Ok(chart) => {
                self.chart = chart.clone();
            }
            Err(err) => {
                return Err(err);
            }
        }
        Ok(self)
    }

    /// set arguments for the helm commands
    pub fn set_args(&mut self, values: Option<Vec<PathBuf>>, opts: Option<Vec<(String, String)>>) {
        if !self.chart.namespace().is_empty() {
            self.args.set_namespace(self.chart.namespace().to_string());
        }
        if !self.chart.name().is_empty() {
            self.args.set_name(self.chart.name().to_string());
        }
        if values.is_some() {
            self.args.set_values(values.unwrap());
        }
        if opts.is_some() {
            self.args.set_opts(opts.unwrap());
        }
    }

    /// apply arguments for helm command
    fn apply_args(&self, command: &mut Command) {
        command
            .arg(self.args.get_name())
            .arg("--namespace")
            .arg(self.args.get_namespace());

        for value_path in self.args.get_values() {
            command.arg("--values").arg(value_path);
        }

        for (key, val) in self.args.get_opts() {
            command.arg("--set").arg(format!("{}={}", key, val));
        }
    }

    /// helm upgrade command
    pub fn upgrade(
        &mut self,
        values: Vec<PathBuf>,
        opts: Vec<(String, String)>,
    ) -> Result<(), HelmError> {
        self.set_args(Some(values), Some(opts));
        let mut command = Command::new("helm");
        command.arg("upgrade");
        self.apply_args(&mut command);
        command.output().map_err(|_| {
            HelmError::HelmStdError("Error while running helm get command".to_string())
        })?;
        Ok(())
    }

    /// command to get values of the installed chart
    pub fn get_values(&mut self) -> Result<(), HelmError> {
        self.set_args(None, None);
        let mut command = Command::new("helm");
        command.args(&["get", "values"]);
        self.apply_args(&mut command);
        command.arg("--output=yaml");
        let output = command.output().map_err(|_| {
            HelmError::HelmStdError("Error while running helm get command".to_string())
        })?;

        let _ = String::from_utf8(output.stdout)
            .map_err(|_| HelmError::Utf8Error("Unable to parse values into string".to_string()))?;

        Ok(())
    }
}
