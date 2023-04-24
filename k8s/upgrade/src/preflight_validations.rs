use crate::{
    constant::{get_image_version_tag, SINGLE_REPLICA_VOLUME, VALID_UPGRADE_PATHS},
    error::Error,
    upgrade_lib,
    upgrade_resources::upgrade::{get_pvc_from_uuid, get_source_version},
    user_prompt,
};
use openapi::models::CordonDrainState;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use tracing::error;

/// Validation to be done before applying upgrade.
pub async fn preflight_check(
    namespace: &str,
    kube_config_path: Option<PathBuf>,
    timeout: humantime::Duration,
    skip_single_replica_volume_validation: bool,
    skip_replica_rebuild: bool,
    skip_cordoned_node_validation: bool,
) -> Result<(), Error> {
    console_logger::info(user_prompt::UPGRADE_WARNING, "");
    // Initialise the REST client.
    let config = kube_proxy::ConfigBuilder::default_api_rest()
        .with_kube_config(kube_config_path.clone())
        .with_timeout(*timeout)
        .with_target_mod(|t| t.with_namespace(namespace))
        .build()
        .await
        .map_err(|error| Error::OpenapiClientConfigurationErr {
            source: anyhow::anyhow!(
                "Failed to create openapi configuration, Error: '{:?}'",
                error
            ),
        })?;
    let rest_client = upgrade_lib::RestClient::new_with_config(config);

    // version validation
    version_check(namespace).await?;

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

/// Version validation.
pub async fn version_check(ns: &str) -> Result<(), Error> {
    let source_version = get_source_version(ns).await?;
    let destination_version = get_image_version_tag();
    if source_version.eq(&destination_version) {
        console_logger::error(
            user_prompt::SOURCE_TARGET_VERSION_SAME,
            &destination_version,
        );
        return Err(Error::SourceTargetVersionSame);
    }

    let valid_paths = parse_key_value_pairs(VALID_UPGRADE_PATHS);
    let data = get_valid_upgrade_paths(valid_paths.clone());
    match valid_paths.get(&source_version) {
        Some(target_set) => {
            if !target_set.contains(&destination_version) {
                console_logger::error(user_prompt::NOT_A_VALID_UPGRADE_PATH, data.as_str());
                return Err(Error::NotAValidUpgradePath);
            }
        }
        None => {
            console_logger::error(user_prompt::NOT_A_VALID_SOURCE_FOR_UPGRADE, data.as_ref());
            return Err(Error::NotAValidSourceForUpgrade);
        }
    }
    Ok(())
}

// This function creates the output valid upgrade paths from the map.
fn get_valid_upgrade_paths(upgrade_paths: HashMap<String, HashSet<String>>) -> String {
    let mut valid_paths: String = String::new();
    for (key, val) in upgrade_paths.iter() {
        for dest in val.iter() {
            valid_paths.push_str(key);
            valid_paths.push_str(" -> ");
            valid_paths.push_str(dest);
            valid_paths.push('\n');
        }
    }
    valid_paths
}

// The function parses VALID_UPGRADE_PATHS and creates map
// where key is source version value is hashset of destination versions.
fn parse_key_value_pairs(input: &str) -> HashMap<String, HashSet<String>> {
    input
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut iter = s.split(':');
            (
                iter.next().unwrap_or("").to_string(),
                iter.next().unwrap_or("").to_string(),
            )
        })
        .fold(
            HashMap::new(),
            |mut acc: HashMap<String, HashSet<String>>, (key, value)| {
                acc.entry(key).or_insert_with(HashSet::new).insert(value);
                acc
            },
        )
}

/// Prompt to user and error out if some nodes are already in cordoned state.
pub async fn already_cordoned_nodes_validation(
    client: &upgrade_lib::RestClient,
) -> Result<(), Error> {
    let mut cordoned_nodes_list = Vec::new();
    let nodes = client.nodes_api().get_nodes().await?;
    let nodelist = nodes.into_body();
    for node in nodelist {
        let node_spec = match node.spec {
            Some(node_spec) => node_spec,
            None => return Err(Error::NodeSpecNotPresent { node: node.id }),
        };
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
        return Err(Error::NodesInCordonedState);
    }
    Ok(())
}

/// Prompt to user and error out if the cluster has single replica volume.
pub async fn single_volume_replica_validation(
    client: &upgrade_lib::RestClient,
) -> Result<(), Error> {
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
            .map_err(|error| {
                error!(?error, "Failed to list volumes");
                error
            })?;

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
        return Err(Error::SingleReplicaVolumeErr);
    }
    Ok(())
}

/// Prompt to user and error out if any rebuild in progress.
pub async fn rebuild_in_progress_validation(client: &upgrade_lib::RestClient) -> Result<(), Error> {
    if is_rebuild_in_progress(client).await? {
        console_logger::error(user_prompt::REBUILD_WARNING, "");
        return Err(Error::VolumeRebuildInProgressErr);
    }
    Ok(())
}

/// Check for rebuild in progress.
pub async fn is_rebuild_in_progress(client: &upgrade_lib::RestClient) -> Result<bool, Error> {
    // The number of volumes to get per request.
    let max_entries = 200;
    let mut starting_token = Some(0_isize);

    // The last paginated request will set the `starting_token` to `None`.
    while starting_token.is_some() {
        let vols = client
            .volumes_api()
            .get_volumes(max_entries, None, starting_token)
            .await
            .map_err(|error| {
                error!(?error, "Failed to list volumes");
                error
            })?;

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
