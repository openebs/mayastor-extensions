use crate::{
    constant::{
        get_image_version_tag, FROM_CORE_SEMVER, SINGLE_REPLICA_VOLUME, TO_CORE_SEMVER,
        TO_DEVELOP_SEMVER,
    },
    error, upgrade_lib,
    upgrade_resources::upgrade::{get_pvc_from_uuid, get_source_version},
    user_prompt,
};
use openapi::models::CordonDrainState;
use semver::{Version, VersionReq};
use snafu::ResultExt;
use std::{collections::HashSet, path::PathBuf};

/// Validation to be done before applying upgrade.
pub async fn preflight_check(
    namespace: &str,
    kube_config_path: Option<PathBuf>,
    timeout: humantime::Duration,
    skip_single_replica_volume_validation: bool,
    skip_replica_rebuild: bool,
    skip_cordoned_node_validation: bool,
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

    // upgrade path validation
    upgrade_path_validation(namespace).await?;

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

/// Upgrade path validation.
pub async fn upgrade_path_validation(ns: &str) -> error::Result<()> {
    let source_version = get_source_version(ns).await?;
    let mut destination_version = get_image_version_tag();

    // if the tag contains develop as substring
    // then treat the destination version as 0.0.0.
    if destination_version.contains("develop") {
        destination_version = "0.0.0".to_string();
    } else {
        // removes the first character v from git tag
        destination_version.remove(0);
    }

    if source_version.eq(&destination_version) {
        console_logger::error(
            user_prompt::SOURCE_TARGET_VERSION_SAME,
            &destination_version,
        );
        return error::SourceTargetVersionSame.fail();
    }
    let upgrade_path_is_valid = is_valid_upgrade_path(source_version, destination_version)?;
    if !upgrade_path_is_valid {
        console_logger::error(user_prompt::UPGRADE_PATH_NOT_VALID, "");
        return error::InvalidUpgradePath.fail();
    }
    Ok(())
}

/// Validates the upgrade path from 'from' Version to 'to' Version for 'chart_variant' helm chart.
pub(crate) fn is_valid_upgrade_path(
    source_version: String,
    destination_version: String,
) -> error::Result<bool> {
    let source = Version::parse(source_version.as_str()).context(error::SemverParse {
        version_string: source_version,
    })?;

    let destination = Version::parse(destination_version.as_str()).context(error::SemverParse {
        version_string: destination_version.to_string(),
    })?;

    let to_req = VersionReq::parse(TO_CORE_SEMVER).context(error::SemverParse {
        version_string: TO_CORE_SEMVER,
    })?;

    let to_develop = VersionReq::parse(TO_DEVELOP_SEMVER).context(error::SemverParse {
        version_string: TO_DEVELOP_SEMVER.to_string(),
    })?;

    if to_req.matches(&destination) || to_develop.matches(&destination) {
        let from_req = VersionReq::parse(FROM_CORE_SEMVER).context(error::SemverParse {
            version_string: FROM_CORE_SEMVER.to_string(),
        })?;
        return Ok(from_req.matches(&source));
    }
    Ok(false)
}
