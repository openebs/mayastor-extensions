use crate::plugin::{
    constants::{get_image_version_tag, SINGLE_REPLICA_VOLUME, UPGRADE_TO_DEVELOP_BRANCH},
    error,
    upgrade::{get_pvc_from_uuid, get_source_version},
    user_prompt,
};
use openapi::{
    clients::tower::{self, Configuration},
    models::CordonDrainState,
};
use semver::Version;
use serde::Deserialize;
use serde_yaml;
use snafu::ResultExt;
use std::{collections::HashSet, ops::Deref, path::PathBuf};

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
    let rest_client = RestClient::new_with_config(config);

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
pub(crate) async fn already_cordoned_nodes_validation(client: &RestClient) -> error::Result<()> {
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
pub(crate) async fn single_volume_replica_validation(client: &RestClient) -> error::Result<()> {
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
pub(crate) async fn rebuild_in_progress_validation(client: &RestClient) -> error::Result<()> {
    if is_rebuild_in_progress(client).await? {
        console_logger::error(user_prompt::REBUILD_WARNING, "");
        return error::VolumeRebuildInProgress.fail();
    }
    Ok(())
}

/// Check for rebuild in progress.
pub(crate) async fn is_rebuild_in_progress(client: &RestClient) -> error::Result<bool> {
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
#[derive(Deserialize)]
struct UnsupportedVersions {
    unsupported_versions: Vec<Version>,
}

impl UnsupportedVersions {
    fn contains(&self, v: &Version) -> bool {
        self.unsupported_versions.contains(v)
    }
}

impl TryFrom<&[u8]> for UnsupportedVersions {
    type Error = serde_yaml::Error;

    /// Returns an UnsupportedVersions object.
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        serde_yaml::from_reader(bytes)
    }
}

pub(crate) async fn upgrade_path_validation(namespace: &str) -> error::Result<()> {
    let unsupported_version_buf =
        &std::include_bytes!("../../config/unsupported_versions.yaml")[..];
    let unsupported_versions = UnsupportedVersions::try_from(unsupported_version_buf)
        .context(error::YamlParseBufferForUnsupportedVersion)?;
    let source_version = get_source_version(namespace).await?;

    let source = Version::parse(source_version.as_str()).context(error::SemverParse {
        version_string: source_version.clone(),
    })?;

    if unsupported_versions.contains(&source) {
        let mut invalid_source_list: String = Default::default();
        for val in unsupported_versions.unsupported_versions.iter() {
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

    if destination_version.contains(UPGRADE_TO_DEVELOP_BRANCH) {
        console_logger::error("", user_prompt::UPGRADE_TO_UNSUPPORTED_VERSION);
        return error::InvalidUpgradePath.fail();
    }
    Ok(())
}

/// New-Type for a RestClient over the tower openapi client.
#[derive(Clone, Debug)]
pub struct RestClient {
    client: tower::ApiClient,
}

impl Deref for RestClient {
    type Target = tower::ApiClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl RestClient {
    /// Create new Rest Client from the given `Configuration`.
    pub fn new_with_config(config: Configuration) -> RestClient {
        Self {
            client: tower::ApiClient::new(config),
        }
    }
}
