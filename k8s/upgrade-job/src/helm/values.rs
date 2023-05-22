use crate::{
    common::{
        constants::TWO_DOT_O,
        error::{
            OpeningFile, Result, SemverParse, U8VectorToString, YamlParseFromFile,
            YamlParseFromSlice,
        },
    },
    helm::{chart::CoreValues, client::HelmReleaseClient},
};
use semver::{Version, VersionReq};
use snafu::ResultExt;
use std::{fs::File, path::PathBuf, str};

// TODO: Refactor this function to infer the flags based on the locally available helm chart.
/// This compiles all of the helm values options to be passed during the helm chart upgrade.
pub(crate) fn generate_values_args(
    from_version: &Version,
    values_yaml_path: PathBuf,
    client: &HelmReleaseClient,
    release_name: String,
) -> Result<Vec<String>> {
    let to_values_yaml = File::open(values_yaml_path.as_path()).context(OpeningFile {
        filepath: values_yaml_path.clone(),
    })?;
    let to_values: CoreValues =
        serde_yaml::from_reader(to_values_yaml).context(YamlParseFromFile {
            filepath: values_yaml_path,
        })?;

    let from_values_yaml = client.get_values_as_yaml::<String, String>(release_name, None)?;
    let from_values_yaml_string = str::from_utf8(from_values_yaml.as_slice())
        .context(U8VectorToString)?
        .to_string();
    let from_values: CoreValues =
        serde_yaml::from_slice(from_values_yaml.as_slice()).context(YamlParseFromSlice {
            input_yaml: from_values_yaml_string,
        })?;

    // Helm chart flags -- reuse all values, except for the image tag. For new values,
    // use from installed-release's values, if present, else use defaults from to-chart.
    let mut upgrade_args: Vec<String> = Vec::with_capacity(18);

    let version_two_dot_o = VersionReq::parse(TWO_DOT_O).context(SemverParse {
        version_string: TWO_DOT_O.to_string(),
    })?;
    if version_two_dot_o.matches(from_version) {
        let io_engine_key = "io_engine";
        let log_level_key = "logLevel";
        let log_level_to_replace = "info,io_engine=info";
        let thin_key = "agents.core.capacity.thin";
        let thin_volume_commitment_key = "volumeCommitment";
        let thin_pool_commitment_key = "poolCommitment";
        let thin_volume_commitment_init_key = "volumeCommitmentInitial";

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

        // Empty values for these three for charts which do not have
        // them on their values will result in a helm nil pointer error.
        if from_values.core_capacity_is_absent() {
            upgrade_args.push("--set".to_string());
            let core_thin_pool_commitment_val = to_values.core_thin_pool_commitment()?;
            let core_thin_pool_commitment_arg =
                format!("{thin_key}.{thin_pool_commitment_key}={core_thin_pool_commitment_val}");
            // helm upgrade .. --set agents.core.capacity.thin.poolCommitment=<value>
            upgrade_args.push(core_thin_pool_commitment_arg);

            upgrade_args.push("--set".to_string());
            let core_thin_vol_commitment_val = to_values.core_thin_volume_commitment()?;
            let core_thin_vol_commitment_arg =
                format!("{thin_key}.{thin_volume_commitment_key}={core_thin_vol_commitment_val}");
            // helm upgrade .. --set agents.core.capacity.thin.volumeCommitment=<value>
            upgrade_args.push(core_thin_vol_commitment_arg);

            upgrade_args.push("--set".to_string());
            let core_thin_vol_commitment_initial_val =
                to_values.core_thin_volume_commitment_initial()?;
            let core_thin_vol_commitment_initial_arg = format!(
                "{thin_key}.{thin_volume_commitment_init_key}={core_thin_vol_commitment_initial_val}"
            );
            // helm upgrade .. --set agents.core.capacity.thin.volumeCommitmentInitial=<value>
            upgrade_args.push(core_thin_vol_commitment_initial_arg);
        }
    }

    // Default 'set' flags.
    let image_key = "image";
    let tag_key = "tag";
    let repo_tags_key = "repoTags";
    let repo_tags_cp_key = "controlPlane";
    let repo_tags_dp_key = "dataPlane";
    let repo_tags_e_key = "extensions";

    upgrade_args.push("--set".to_string());

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

    // Mandatory flags.

    // helm upgrade .. --reuse-values
    upgrade_args.push("--reuse-values".to_string());

    // To roll back to previous release, in case helm upgrade fails, also
    // to wait for all Pods, PVCs to come to a ready state.
    upgrade_args.push("--atomic".to_string());

    Ok(upgrade_args)
}
