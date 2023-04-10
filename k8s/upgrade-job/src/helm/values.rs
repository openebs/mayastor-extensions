use crate::{
    common::{
        constants::{CORE_CHART_NAME, TO_UMBRELLA_SEMVER},
        error::{OpeningFile, Result, U8VectorToString, YamlParseFromFile, YamlParseFromSlice},
    },
    helm::{
        chart::{CoreValues, UmbrellaValues},
        client::HelmReleaseClient,
        upgrade::HelmChart,
    },
};

use snafu::ResultExt;
use std::{fs::File, path::PathBuf, str};

/// This compiles all of the helm values options to be passed during the helm chart upgrade.
pub(crate) fn generate_values_args(
    chart_variant: HelmChart,
    values_yaml_path: PathBuf,
    client: &HelmReleaseClient,
    release_name: String,
) -> Result<Vec<String>> {
    let to_values_yaml = File::open(values_yaml_path.as_path()).context(OpeningFile {
        filepath: values_yaml_path.clone(),
    })?;

    let from_values_yaml = client.get_values_as_yaml::<String, String>(release_name, None)?;
    let from_values_yaml_string = str::from_utf8(from_values_yaml.as_slice())
        .context(U8VectorToString)?
        .to_string();
    // Helm chart flags -- reuse all values, except for the image tag. Modify
    // io_engine DaemonSet PodSpec logLevel if set to from-chart default, and
    // to-chart default differs from the from-chart default.
    let mut upgrade_args: Vec<String> = Vec::with_capacity(12);

    let image_key: &str = "image";
    let tag_key: &str = "tag";
    let repo_tags_key: &str = "repoTags";
    let repo_tags_cp_key: &str = "controlPlane";
    let repo_tags_dp_key: &str = "dataPlane";
    let repo_tags_e_key: &str = "extensions";
    let io_engine_key = "io_engine";
    let log_level_key = "logLevel";
    let log_level_to_replace: &str = "info,io_engine=info";
    match chart_variant {
        HelmChart::Umbrella => {
            upgrade_args.push("--set".to_string());
            let to_values: UmbrellaValues =
                serde_yaml::from_reader(to_values_yaml).context(YamlParseFromFile {
                    filepath: values_yaml_path,
                })?;
            let image_tag_arg = format!(
                "{CORE_CHART_NAME}.{image_key}.{tag_key}={}",
                to_values.image_tag()
            );
            // helm upgrade .. --set <core-chart>.image.tag=<version>
            upgrade_args.push(image_tag_arg);

            // RepoTags fields will be set to empty strings. This is required because we are trying
            // to get to crate::common::constants::TO_UMBRELLA_SEMVER completely, without setting
            // versions for repo-specific components.
            // Also, the default template function uses the CORE_CHART_NAME.image.repoTags.* values,
            // and leaving them empty will result in a nil pointer error in helm.
            upgrade_args.push("--set".to_string());
            let repo_tag_ctrl_plane_arg: String =
                format!("{CORE_CHART_NAME}.{image_key}.{repo_tags_key}.{repo_tags_cp_key}=");
            // helm upgrade .. --set <core-chart>.image.tag=<version> --set
            // <core-chart>.image.repoTags.controlPlane=
            upgrade_args.push(repo_tag_ctrl_plane_arg);
            upgrade_args.push("--set".to_string());
            let repo_tag_data_plane_arg: String =
                format!("{CORE_CHART_NAME}.{image_key}.{repo_tags_key}.{repo_tags_dp_key}=");
            // helm upgrade .. --set <core-chart>.image.tag=<version> --set
            // <core-chart>.image.repoTags.controlPlane= --set
            // <core-chart>.image.repoTags.dataPlane=
            upgrade_args.push(repo_tag_data_plane_arg);
            upgrade_args.push("--set".to_string());
            let repo_tag_e_arg: String =
                format!("{CORE_CHART_NAME}.{image_key}.{repo_tags_key}.{repo_tags_e_key}=");
            // helm upgrade .. --set <core-chart>.image.tag=<version> --set
            // <core-chart>.image.repoTags.controlPlane= --set
            // <core-chart>.image.repoTags.dataPlane= --set <core-chart>.image.repoTags.extensions=
            upgrade_args.push(repo_tag_e_arg);

            let from_values: UmbrellaValues = serde_yaml::from_slice(from_values_yaml.as_slice())
                .context(YamlParseFromSlice {
                input_yaml: from_values_yaml_string,
            })?;
            if from_values.io_engine_log_level().eq(log_level_to_replace)
                && to_values.io_engine_log_level().ne(log_level_to_replace)
            {
                upgrade_args.push("--set".to_string());
                let io_engine_log_level_arg: String = format!(
                    "{CORE_CHART_NAME}.{io_engine_key}.{log_level_key}={}",
                    to_values.io_engine_log_level()
                );
                // helm upgrade .. --set <core-chart>.image.tag=<version> --set
                // <core-chart>.image.repoTags.controlPlane= --set
                // <core-chart>.image.repoTags.dataPlane= --set
                // <core-chart>.image.repoTags.extensions= --set <core-chart>.
                // io-engine.loglevel=info
                upgrade_args.push(io_engine_log_level_arg);

                // helm upgrade .. --set release.version=<umbrella-chart-semver>
                upgrade_args.push("--set".to_string());
                let umbrella_release_arg: String = format!("release.version={TO_UMBRELLA_SEMVER}");
                upgrade_args.push(umbrella_release_arg);
            }
        }
        HelmChart::Core => {
            upgrade_args.push("--set".to_string());

            let to_values: CoreValues =
                serde_yaml::from_reader(to_values_yaml).context(YamlParseFromFile {
                    filepath: values_yaml_path,
                })?;
            let image_tag_arg = format!("{image_key}.{tag_key}={}", to_values.image_tag());

            // helm upgrade .. --set image.tag=<version>
            upgrade_args.push(image_tag_arg);

            // RepoTags fields will be set to empty strings. This is required because we are trying
            // to get to crate::common::constants::TO_CORE_SEMVER completely, without setting
            // versions for repo-specific components.
            // Also, the default template function uses the image.repoTags.* values, and leaving
            // them empty will result in a nil pointer error in helm.
            upgrade_args.push("--set".to_string());
            let repo_tag_ctrl_plane_arg: String =
                format!("{image_key}.{repo_tags_key}.{repo_tags_cp_key}=");
            // helm upgrade .. --set <core-chart>.image.tag=<version> --set
            // <core-chart>.image.repoTags.controlPlane=
            upgrade_args.push(repo_tag_ctrl_plane_arg);
            upgrade_args.push("--set".to_string());
            let repo_tag_data_plane_arg: String =
                format!("{image_key}.{repo_tags_key}.{repo_tags_dp_key}=");
            // helm upgrade .. --set image.tag=<version> --set image.repoTags.controlPlane= --set
            // image.repoTags.dataPlane=
            upgrade_args.push(repo_tag_data_plane_arg);
            upgrade_args.push("--set".to_string());
            let repo_tag_e_arg: String = format!("{image_key}.{repo_tags_key}.{repo_tags_e_key}=");
            // helm upgrade .. --set image.tag=<version> --set image.repoTags.controlPlane= --set
            // image.repoTags.dataPlane= --set image.repoTags.extensions=
            upgrade_args.push(repo_tag_e_arg);

            let from_values: CoreValues = serde_yaml::from_slice(from_values_yaml.as_slice())
                .context(YamlParseFromSlice {
                    input_yaml: from_values_yaml_string,
                })?;
            if from_values.io_engine_log_level().eq(log_level_to_replace)
                && to_values.io_engine_log_level().ne(log_level_to_replace)
            {
                upgrade_args.push("--set".to_string());

                let io_engine_log_level_arg: String = format!(
                    "{io_engine_key}.{log_level_key}={}",
                    to_values.io_engine_log_level()
                );

                // helm upgrade .. --set image.tag=<version> --set image.repoTags.controlPlane=
                // --set image.repoTags.dataPlane= --set image.repoTags.extensions= --set
                // io-engine.loglevel=info
                upgrade_args.push(io_engine_log_level_arg);
            }
        }
    }

    // helm upgrade .. --reuse-values
    upgrade_args.push("--reuse-values".to_string());

    Ok(upgrade_args)
}
