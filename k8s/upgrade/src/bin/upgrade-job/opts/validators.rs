use crate::{
    common::{
        constants::CORE_CHART_NAME,
        error::{
            FindingHelmChart, GetNamespace, HelmCommand, HelmListCommand, HelmRelease, HelmVersion,
            HelmVersionCommand, ListStorageNodes, NotADirectory, NotAFile, ReadingFile,
            RegexCompile, Result, U8VectorToString, ValidateDirPath, ValidateFilePath,
            YamlParseFromFile,
        },
        kube_client as KubeClient,
        rest_client::RestClientSet,
    },
    helm::chart::Chart,
    vec_to_strings,
};
use regex::bytes::Regex;
use snafu::{ensure, ResultExt};
use std::{fs, path::PathBuf, process::Command, str};
use tracing::debug;

/// Validate that the helm release specified in the CLI options exists in the namespace,
/// which is also specified in the CLI options.
pub(crate) fn validate_helm_release(name: String, namespace: String) -> Result<()> {
    let command: &str = "helm";
    let args: Vec<String> =
        vec_to_strings!["list", "-n", namespace.as_str(), "--deployed", "--short"];

    debug!(%command, ?args, "Helm list command");

    // Execute `helm list` to get a list of helm chart releases in the namespace.
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

    // Validate that the release-name list has the name which is specified in the CLI options.
    let regex = format!(r"(\n)?{name}(\n)?");
    if !Regex::new(regex.as_str())
        .context(RegexCompile { expression: regex })?
        .is_match(output.stdout.as_slice())
    {
        return HelmRelease { name, namespace }.fail();
    }

    Ok(())
}

/// Validate that the helm v3 binary is present in the shell's $PATH.
pub(crate) fn validate_helmv3_in_path() -> Result<()> {
    let command: &str = "helm";
    let args: Vec<String> = vec_to_strings!["version", "--short"];

    debug!(%command, ?args, "Helm version command");

    // Execute `helm version` to verify if the binary exists.
    let output = Command::new(command)
        .args(args.clone())
        .output()
        .context(HelmCommand {
            command: command.to_string(),
            args: args.clone(),
        })?;

    let stdout_str = str::from_utf8(output.stdout.as_slice()).context(U8VectorToString)?;
    debug!(stdout=%stdout_str, "Helm version command standard output");
    ensure!(
        output.status.success(),
        HelmVersionCommand {
            command: command.to_string(),
            args,
            std_err: str::from_utf8(output.stderr.as_slice())
                .context(U8VectorToString)?
                .to_string()
        }
    );

    // Parse based on regex, to validate if the version string (semver) is v3.x.
    let regex: &str = r"^(v3\.[0-9]+\.[0-9])";
    if !Regex::new(regex)
        .context(RegexCompile {
            expression: regex.to_string(),
        })?
        .is_match(output.stdout.as_slice())
    {
        return HelmVersion {
            version: stdout_str.to_string(),
        }
        .fail();
    }

    Ok(())
}

/// Validate the input helm chart directory path.
pub(crate) fn validate_helm_chart_dir(core_dir: PathBuf) -> Result<()> {
    validate_core_helm_chart_variant_in_dir(core_dir)
}

/// Validate the input helm chart directory path:
/// - validate if the path exists.
/// - validate if the expected directory structure is present.
/// - validate if the expected helm chart files are present.
/// - validate if the chart name if the chart name in the Chart.yaml file is correct.
fn validate_core_helm_chart_variant_in_dir(dir_path: PathBuf) -> Result<()> {
    let path_exists_and_is_dir = |path: PathBuf| -> Result<bool> {
        fs::metadata(path.as_path())
            .map(|m| m.is_dir())
            .context(ValidateDirPath { path })
    };

    let path_exists_and_is_file = |path: PathBuf| -> Result<bool> {
        fs::metadata(path.as_path())
            .map(|m| m.is_file())
            .context(ValidateFilePath { path })
    };

    ensure!(
        path_exists_and_is_dir(dir_path.clone())?,
        NotADirectory { path: dir_path }
    );

    // Validate Chart.yaml file.
    let mut chart_yaml_path = dir_path.clone();
    chart_yaml_path.push("Chart.yaml");
    ensure!(
        path_exists_and_is_file(chart_yaml_path.clone())?,
        NotAFile {
            path: chart_yaml_path.clone()
        }
    );

    let chart_yaml_file = fs::read(chart_yaml_path.as_path()).context(ReadingFile {
        filepath: chart_yaml_path.clone(),
    })?;
    let chart_yaml: Chart =
        serde_yaml::from_slice(chart_yaml_file.as_slice()).context(YamlParseFromFile {
            filepath: chart_yaml_path.clone(),
        })?;

    ensure!(
        chart_yaml.name().eq(CORE_CHART_NAME),
        FindingHelmChart { path: dir_path }
    );

    // Validate charts directory, it should exist if `helm dependency update` has been executed.
    let charts_dir_path = dir_path.join("charts");
    ensure!(
        path_exists_and_is_dir(charts_dir_path.clone())?,
        NotADirectory {
            path: charts_dir_path
        }
    );

    // Validate values.yaml file.
    let mut values_yaml_path = dir_path.clone();
    values_yaml_path.push("values.yaml");
    ensure!(
        path_exists_and_is_file(values_yaml_path.clone())?,
        NotAFile {
            path: values_yaml_path.clone()
        }
    );

    // Validate README.md file.
    let mut readme_md_path = dir_path.clone();
    readme_md_path.push("README.md");
    ensure!(
        path_exists_and_is_file(readme_md_path.clone())?,
        NotAFile {
            path: readme_md_path.clone()
        }
    );

    // Validate crds directory.
    let mut crds_dir_path = dir_path.clone();
    crds_dir_path.push("charts/crds");
    ensure!(
        path_exists_and_is_dir(crds_dir_path.clone())?,
        NotADirectory {
            path: crds_dir_path.clone()
        }
    );

    // Validate templates directory.
    let mut templates_dir_path = dir_path;
    templates_dir_path.push("templates");
    ensure!(
        path_exists_and_is_dir(templates_dir_path.clone())?,
        NotADirectory {
            path: templates_dir_path.clone()
        }
    );

    Ok(())
}

/// This checks for 2 things:
/// - if the kubernetes API is reachable.
/// - if the input namespace exists.
pub(crate) async fn validate_namespace(ns: String) -> Result<()> {
    let ns_api = KubeClient::namespaces_api().await?;

    ns_api
        .get(ns.as_str())
        .await
        .context(GetNamespace { namespace: ns })?;

    Ok(())
}

/// This checks if the storage API is reachable and usable.
pub(crate) async fn validate_rest_endpoint(rest_endpoint: String) -> Result<()> {
    let rest_client = RestClientSet::new_with_url(rest_endpoint)?;

    rest_client
        .nodes_api()
        .get_nodes(None)
        .await
        .context(ListStorageNodes)?;

    Ok(())
}
