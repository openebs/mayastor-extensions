use crate::{
    constant::{get_image_version_tag, SINGLE_REPLICA_VOLUME, UNSUPPORTED_VERSION_FILE},
    error, upgrade_lib,
    upgrade_resources::upgrade::{get_pvc_from_uuid, get_source_version},
    user_prompt,
};
use openapi::models::CordonDrainState;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_yaml;
use snafu::ResultExt;
use std::{collections::HashSet, fs::File, path::PathBuf};

/// Validation to be done before applying upgrade.
pub async fn preflight_check(
    namespace: &str,
    kube_config_path: Option<PathBuf>,
    timeout: humantime::Duration,
    skip_single_replica_volume_validation: bool,
    skip_replica_rebuild: bool,
    skip_cordoned_node_validation: bool,
    skip_upgrade_path_validation: bool,
) -> error::Result<()> {
    console_logger::info(user_prompt::UPGRADE_WARNING, "");
    // Initialise the REST client.
    let config = kube_proxy::ConfigBuilder::default_api_rest()
        .with_kube_config(kube_config_path.clone())
        .with_timeout(*timeout)
        .with_target_mod(|t| t.with_namespace(namespace))
        .build()
        .await
        .context(error::OpenapiClientConfiguration)?;
    let rest_client = upgrade_lib::RestClient::new_with_config(config);

    if !skip_upgrade_path_validation {
        upgrade_path_validation(namespace).await?;
    }

    if !skip_replica_rebuild {
        rebuild_in_progress_validation(&rest_client).await?;
    }

    if !skip_cordoned_node_validation {
        already_cordoned_nodes_validation(&rest_client).await?;
    }

    if !skip_single_replica_volume_validation {
        single_volume_replica_validation(&rest_client).await?;
    }
    Ok(())
}

/// Prompt to user and error out if some nodes are already in cordoned state.
pub async fn already_cordoned_nodes_validation(
    client: &upgrade_lib::RestClient,
) -> error::Result<()> {
    let mut cordoned_nodes_list = Vec::new();
    let nodes = client
        .nodes_api()
        .get_nodes()
        .await
        .context(error::ListStorageNodes)?;
    let nodelist = nodes.into_body();
    for node in nodelist {
        let node_spec = node.spec.ok_or(
            error::NodeSpecNotPresent {
                node: node.id.to_string(),
            }
            .build(),
        )?;

        if matches!(
            node_spec.cordondrainstate,
            Some(CordonDrainState::cordonedstate(_))
        ) {
            cordoned_nodes_list.push(node.id);
        }
    }
    if !cordoned_nodes_list.is_empty() {
        console_logger::error(
            user_prompt::CORDONED_NODE_WARNING,
            &cordoned_nodes_list.join("\n"),
        );
        return error::NodesInCordonedState.fail();
    }
    Ok(())
}

/// Prompt to user and error out if the cluster has single replica volume.
pub async fn single_volume_replica_validation(
    client: &upgrade_lib::RestClient,
) -> error::Result<()> {
    // let mut single_replica_volumes = Vec::new();
    // The number of volumes to get per request.
    let max_entries = 200;
    let mut starting_token = Some(0_isize);
    let mut volumes = Vec::with_capacity(max_entries as usize);

    // The last paginated request will set the `starting_token` to `None`.
    while starting_token.is_some() {
        let vols = client
            .volumes_api()
            .get_volumes(max_entries, None, starting_token)
            .await
            .context(error::ListVolumes)?;

        let v = vols.into_body();
        let single_rep_vol_ids: Vec<String> = v
            .entries
            .into_iter()
            .filter(|volume| volume.spec.num_replicas == SINGLE_REPLICA_VOLUME)
            .map(|volume| volume.spec.uuid.to_string())
            .collect();
        volumes.extend(single_rep_vol_ids);
        starting_token = v.next_token;
    }

    if !volumes.is_empty() {
        let data = get_pvc_from_uuid(HashSet::from_iter(volumes))
            .await?
            .join("\n");

        console_logger::error(user_prompt::SINGLE_REPLICA_VOLUME_WARNING, &data);
        return error::SingleReplicaVolumeErr.fail();
    }
    Ok(())
}

/// Prompt to user and error out if any rebuild in progress.
pub async fn rebuild_in_progress_validation(client: &upgrade_lib::RestClient) -> error::Result<()> {
    if is_rebuild_in_progress(client).await? {
        console_logger::error(user_prompt::REBUILD_WARNING, "");
        return error::VolumeRebuildInProgress.fail();
    }
    Ok(())
}

/// Check for rebuild in progress.
pub async fn is_rebuild_in_progress(client: &upgrade_lib::RestClient) -> error::Result<bool> {
    // The number of volumes to get per request.
    let max_entries = 200;
    let mut starting_token = Some(0_isize);

    // The last paginated request will set the `starting_token` to `None`.
    while starting_token.is_some() {
        let vols = client
            .volumes_api()
            .get_volumes(max_entries, None, starting_token)
            .await
            .context(error::ListVolumes)?;
        let volumes = vols.into_body();
        starting_token = volumes.next_token;
        for volume in volumes.entries {
            if let Some(target) = &volume.state.target {
                if target
                    .children
                    .iter()
                    .any(|child| child.rebuild_progress.is_some())
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Struct to deserialize the unsupported version yaml.
#[derive(Debug, Serialize, Deserialize)]
struct UnsupportedVersions {
    unsupported_versions: Vec<String>,
}

pub async fn upgrade_path_validation(namespace: &str) -> error::Result<()> {
    let path = std::env::current_dir()
        .context(error::GetCurrentDirectory)?
        .join(UNSUPPORTED_VERSION_FILE);

    let unsupported_versions = get_unsupported_versions(path)?;
    let source_version = get_source_version(namespace).await?;

    let source = Version::parse(source_version.as_str()).context(error::SemverParse {
        version_string: source_version.clone(),
    })?;

    if unsupported_versions.contains(&source) {
        let mut invalid_source_list: String = String::new();
        for val in unsupported_versions.iter() {
            invalid_source_list.push_str(val.to_string().as_str());
            invalid_source_list.push('\n');
        }
        console_logger::error(
            user_prompt::UPGRADE_PATH_NOT_VALID,
            invalid_source_list.as_str(),
        );
        return error::NotAValidSourceForUpgrade.fail();
    }
    let destination_version = get_image_version_tag();

    if destination_version.contains("develop") {
        console_logger::error("", user_prompt::UPGRADE_TO_UNSUPPORTED_VERSION);
        return error::InvalidUpgradePath.fail();
    }
    Ok(())
}

/// Return the unsupported version by parsing the unsupported_versions.yaml file.
pub fn get_unsupported_versions(path: PathBuf) -> error::Result<HashSet<Version>> {
    let unsupported_versions_yaml = File::open(path.as_path()).context(error::OpeningFile {
        filepath: path.clone(),
    })?;

    let unsupported: UnsupportedVersions = serde_yaml::from_reader(unsupported_versions_yaml)
        .context(error::YamlParseFromFile { filepath: path })?;

    let mut unsupported_versions_set: HashSet<Version> = HashSet::new();

    for version in unsupported.unsupported_versions.iter() {
        let unsupported_version = Version::parse(version.as_str()).context(error::SemverParse {
            version_string: version.clone(),
        })?;
        unsupported_versions_set.insert(unsupported_version);
    }
    Ok(unsupported_versions_set)
}
