use crate::{
    common::{
        constants::{TWO_DOT_O, TWO_DOT_THREE},
        error::{
            ReadingFile, Result, SemverParse, TempFileCreation, U8VectorToString, WriteToTempFile,
            YamlParseFromFile, YamlParseFromSlice,
        },
    },
    helm::{
        chart::CoreValues,
        client::HelmReleaseClient,
        yaml::yq::{YamlKey, YqV4},
    },
};
use semver::{Version, VersionReq};
use snafu::ResultExt;
use std::{fs, io::Write, path::Path, str};
use tempfile::NamedTempFile as TempFile;

/// This compiles all of the helm values options to be passed during the helm chart upgrade.
pub(crate) fn generate_values_yaml_file(
    from_version: &Version,
    chart_dir: &Path,
    client: &HelmReleaseClient,
    release_name: String,
) -> Result<TempFile> {
    // Serde object for to_values yaml.
    let to_values_filepath = chart_dir.join("values.yaml");
    let to_values_yaml = fs::read(to_values_filepath.as_path()).context(ReadingFile {
        filepath: to_values_filepath.clone(),
    })?;
    let to_values: CoreValues =
        serde_yaml::from_slice(to_values_yaml.as_slice()).context(YamlParseFromFile {
            filepath: to_values_filepath.clone(),
        })?;

    // Write from_values_yaml to a file, and also parse it and build a serde object.
    let from_values_yaml = client.get_values_as_yaml::<String, String>(release_name, None)?;
    // File
    let mut from_values_file = TempFile::new_in(chart_dir).context(TempFileCreation)?;
    from_values_file
        .write(from_values_yaml.as_slice())
        .context(WriteToTempFile {
            filepath: from_values_file.path().to_path_buf(),
        })?;
    // Serde object
    let from_values: CoreValues =
        serde_yaml::from_slice(from_values_yaml.as_slice()).context(YamlParseFromSlice {
            input_yaml: str::from_utf8(from_values_yaml.as_slice())
                .context(U8VectorToString)?
                .to_string(),
        })?;

    // Resultant values yaml for helm upgrade command.
    // Merge the source values with the target values.
    let mut upgrade_values_file = TempFile::new_in(chart_dir).context(TempFileCreation)?;
    let yq = YqV4::new()?;
    let upgrade_values_yaml =
        yq.merge_files(from_values_file.path(), to_values_filepath.as_path())?;
    upgrade_values_file
        .write(upgrade_values_yaml.as_slice())
        .context(WriteToTempFile {
            filepath: upgrade_values_file.path().to_path_buf(),
        })?;

    // Specific special-case values for version 2.0.x.
    let version_two_dot_o = VersionReq::parse(TWO_DOT_O).context(SemverParse {
        version_string: TWO_DOT_O.to_string(),
    })?;
    if version_two_dot_o.matches(from_version) {
        let log_level_to_replace = "info,io_engine=info";

        if from_values.io_engine_log_level().eq(log_level_to_replace)
            && to_values.io_engine_log_level().ne(log_level_to_replace)
        {
            yq.set_value(
                YamlKey::try_from(".io_engine.logLevel")?,
                to_values.io_engine_log_level(),
                upgrade_values_file.path(),
            )?;
        }
    }

    // Specific special-case values for version 2.3.x.
    let version_two_dot_three = VersionReq::parse(TWO_DOT_THREE).context(SemverParse {
        version_string: TWO_DOT_THREE.to_string(),
    })?;
    if version_two_dot_three.matches(from_version)
        && from_values
            .eventing_enabled()
            .ne(&to_values.eventing_enabled())
    {
        yq.set_value(
            YamlKey::try_from(".eventing.enabled")?,
            to_values.eventing_enabled(),
            upgrade_values_file.path(),
        )?;
    }

    // Default options.
    // Image tag is set because the high_priority file is the user's source options file.
    // The target's image tag needs to be set for PRODUCT upgrade.
    yq.set_value(
        YamlKey::try_from(".image.tag")?,
        to_values.image_tag(),
        upgrade_values_file.path(),
    )?;

    // RepoTags fields will be set to empty strings. This is required because we are trying
    // to get to the target container images, without setting versions for repo-specific
    // components.
    yq.set_value(
        YamlKey::try_from(".image.repoTags.controlPlane")?,
        "",
        upgrade_values_file.path(),
    )?;
    yq.set_value(
        YamlKey::try_from(".image.repoTags.dataPlane")?,
        "",
        upgrade_values_file.path(),
    )?;
    yq.set_value(
        YamlKey::try_from(".image.repoTags.extensions")?,
        "",
        upgrade_values_file.path(),
    )?;

    // The CSI sidecar images need to always be the versions set on the chart by default.
    yq.set_value(
        YamlKey::try_from(".csi.image.provisionerTag")?,
        to_values.csi_provisioner_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_value(
        YamlKey::try_from(".csi.image.attacherTag")?,
        to_values.csi_attacher_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_value(
        YamlKey::try_from(".csi.image.snapshotterTag")?,
        to_values.csi_snapshotter_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_value(
        YamlKey::try_from(".csi.image.snapshotControllerTag")?,
        to_values.csi_snapshot_controller_image_tag(),
        upgrade_values_file.path(),
    )?;
    yq.set_value(
        YamlKey::try_from(".csi.image.registrarTag")?,
        to_values.csi_node_driver_registrar_image_tag(),
        upgrade_values_file.path(),
    )?;

    // helm upgrade .. --set image.tag=<version> --set image.repoTags.controlPlane= --set
    // image.repoTags.dataPlane= --set image.repoTags.extensions=

    Ok(upgrade_values_file)
}
