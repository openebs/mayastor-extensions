use crate::{
    constant::SINGLE_REPLICA_VOLUME, error::Error, upgrade_lib,
    upgrade_resources::upgrade::get_pvc_from_uuid, user_prompt,
};
use openapi::models::CordonDrainState;
use std::{collections::HashSet, path::PathBuf};
use tracing::error;

/// Validation to be done before applying upgrade.
pub async fn preflight_check(
    namespace: &str,
    kube_config_path: Option<PathBuf>,
    timeout: humantime::Duration,
    ignore_single_replica: bool,
    skip_replica_rebuild: bool,
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

    if !skip_replica_rebuild {
        rebuild_in_progress_validation(&rest_client).await?;
    }

    already_cordoned_nodes_validation(&rest_client).await?;
    if !ignore_single_replica {
        single_volume_replica_validation(&rest_client).await?;
    }
    Ok(())
}

/// Check for already cordoned node
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
        console_logger::warn(
            user_prompt::CORDONED_NODE_WARNING,
            &cordoned_nodes_list.join("\n"),
        );
    }
    Ok(())
}

/// Check single replica volumes.
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

        console_logger::warn(user_prompt::SINGLE_REPLICA_VOLUME_WARNING, &data);
        std::process::exit(1);
    }
    Ok(())
}

/// Prompt to user if any rebuild in progress.
pub async fn rebuild_in_progress_validation(client: &upgrade_lib::RestClient) -> Result<(), Error> {
    if is_rebuild_in_progress(client).await? {
        console_logger::warn(user_prompt::REBUILD_WARNING, "");
        std::process::exit(1);
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
