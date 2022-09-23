use tracing::log::error;

use crate::upgrade::{common::error::Error, config::UpgradeOperatorConfig};

/// check node status.
async fn check_node_health() -> Result<bool, Error> {
    let nodes = UpgradeOperatorConfig::get_config()
        .k8s_client()
        .get_nodes()
        .await?;
    for n in nodes {
        match n.status {
            Some(status) => match status.conditions {
                Some(conditions) => {
                    for c in conditions {
                        if !(c.type_.eq_ignore_ascii_case("ready") && c.status == "true") {
                            return Ok(false);
                        }
                    }
                }
                None => {
                    return Err(Error::NodeConditionNotPresent {
                        node: n.metadata.name.unwrap(),
                    });
                }
            },
            None => {
                return Err(Error::NodeStatusNotPresent {
                    node: n.metadata.name.unwrap(),
                });
            }
        }
    }
    Ok(true)
}

/// check volume targets.
async fn check_volume_targets() -> Result<bool, Error> {
    let volumes = UpgradeOperatorConfig::get_config()
        .rest_client()
        .volumes_api()
        .get_volumes(0, None)
        .await;

    match volumes {
        Ok(rc) => {
            let volumes = rc.into_body();
            for volume in volumes.entries {
                if volume.spec.target.is_some() {
                    return Ok(true);
                }
            }
        }
        Err(err) => {
            error!("{:?}", err);
            return Err(Error::VolumeResponse { source: err });
        }
    };
    Ok(false)
}

/// Preflight checks to be done before upgrade.
pub async fn preflight() -> Result<bool, Error> {
    let node_health = check_node_health().await?;
    let volume_target = check_volume_targets().await?;
    if !node_health || !volume_target {
        return Ok(false);
    }
    Ok(true)
}
