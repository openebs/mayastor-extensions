use crate::{
    common::{
        constants::CORE_CHART_NAME,
        error::{Result, YamlStructure, YamlParseFromFile, OpeningFile, U8VectorToString, YamlParseFromSlice},
    },
    helm::{
        upgrade::HelmChart,
        chart::{CoreValues, UmbrellaValues},
    },
};
use snafu::ResultExt;
use serde_yaml::Value;
use std::{
    str,
    path::PathBuf,
    fs::File,
};
use crate::helm::client::HelmReleaseClient;
use crate::upgrade::upgrade;

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
    let from_values_yaml_string = str::from_utf8(from_values_yaml.as_slice()).context(U8VectorToString)?.to_string();
    // Helm chart flags -- reuse all values, except for the image tag. Modify
    // io_engine DaemonSet PodSpec logLevel if set to from-chart default, and
    // to-chart default differs from the from-chart default.
    let mut upgrade_args: Vec<String> = Vec::with_capacity(6);
    upgrade_args.push("--set".to_string());

    let mut image_tag_arg: String = Default::default();
    let image_key: &str = "image";
    let tag_key: &str = "tag";
    let io_engine_key = "io_engine";
    let log_level_key = "logLevel";
    let log_level_to_replace: &str = "info,io_engine=info";
    match chart_variant {
        HelmChart::Umbrella => {

            image_tag_arg.push_str(CORE_CHART_NAME);
            image_tag_arg.push('.');
            image_tag_arg.push_str(image_key);
            image_tag_arg.push('.');
            image_tag_arg.push_str(tag_key);
            image_tag_arg.push('=');

            let to_values: UmbrellaValues =
                serde_yaml::from_reader(to_values_yaml).context(YamlParseFromFile {
                    filepath: values_yaml_path,
                })?;

            image_tag_arg.push_str(to_values.image_tag());

            // helm upgrade .. --set <core-chart>.image.tag=<version>
            upgrade_args.push(image_tag_arg);

            let from_values: UmbrellaValues = serde_yaml::from_slice(from_values_yaml.as_slice()).context(YamlParseFromSlice {
                input_yaml: from_values_yaml_string,
            })?;
            if from_values.io_engine_log_level().eq(log_level_to_replace) && to_values.io_engine_log_level().ne(log_level_to_replace) {
                upgrade_args.push("--set".to_string());

                let mut io_engine_log_level_arg: String = Default::default();
                io_engine_log_level_arg.push_str(CORE_CHART_NAME);
                io_engine_log_level_arg.push('.');
                io_engine_log_level_arg.push_str(io_engine_key);
                io_engine_log_level_arg.push('.');
                io_engine_log_level_arg.push_str(log_level_key);
                io_engine_log_level_arg.push('=');
                io_engine_log_level_arg.push_str(to_values.io_engine_log_level());

                // helm upgrade .. --set <core-chart>.image.tag=<version> --set
                // <core-chart>.io-engine.loglevel=info
                upgrade_args.push(io_engine_log_level_arg);
            }

        },
        HelmChart::Core => {

            image_tag_arg.push_str(image_key);
            image_tag_arg.push('.');
            image_tag_arg.push_str(tag_key);
            image_tag_arg.push('=');

            let to_values: CoreValues =
                serde_yaml::from_reader(to_values_yaml).context(YamlParseFromFile {
                    filepath: values_yaml_path,
                })?;

            image_tag_arg.push_str(to_values.image_tag());

            // helm upgrade .. --set image.tag=<version>
            upgrade_args.push(image_tag_arg);

            let from_values: CoreValues = serde_yaml::from_slice(from_values_yaml.as_slice()).context(YamlParseFromSlice {
                input_yaml: from_values_yaml_string,
            })?;
            if from_values.io_engine_log_level().eq(log_level_to_replace) && to_values.io_engine_log_level().ne(log_level_to_replace) {
                upgrade_args.push("--set".to_string());

                let mut io_engine_log_level_arg: String = Default::default();
                io_engine_log_level_arg.push_str(io_engine_key);
                io_engine_log_level_arg.push('.');
                io_engine_log_level_arg.push_str(log_level_key);
                io_engine_log_level_arg.push('=');
                io_engine_log_level_arg.push_str(to_values.io_engine_log_level());

                // helm upgrade .. --set image.tag=<version> --set io-engine.loglevel=info
                upgrade_args.push(io_engine_log_level_arg);
            }
        },
    }

    // helm upgrade .. --set image.tag=<version> --reuse-values
    // or,
    // helm upgrade .. --set <core-chart>.image.tag=<version> --reuse-values
    upgrade_args.push("--reuse-values".to_string());

    Ok(upgrade_args)
}
