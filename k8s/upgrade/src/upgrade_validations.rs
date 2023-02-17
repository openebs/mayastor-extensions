use crate::constant::SINGLE_REPLICA_VOLUME;
use anyhow::Error;
use openapi::apis::Uuid;
use plugin::rest_wrapper::RestClient;

/// Warning to users before doing an upgrade
pub const UPGRADE_WARNING: &str =  "Volumes which make use of a single volume replica instance will be unavailable for some time during upgrade.\nIt is recommended that you do not create new volumes which make use of only one volume replica.";

/// Warning to users before doing an upgrade
pub const REBUILD_WARNING: &str =  "The cluster is rebuilding replica of some volumes.\nPlease try after some time or skip this validation by retrying with '--ignore-rebuild` flag.";

/// Warning to users before doing an upgrade
pub const SINGLE_REPLICA_VOLUME_WARNING: &str =  "The list below shows the single replica volumes in cluster.\nThese single replica volumes may not be accessible during upgrade.\nPlease retry  with '--skip-single-replica-volume-validation` flag to skip this validation.\n";

#[derive(Debug)]
pub struct UpgradeValidation {
    pub rebuild_in_progress: bool,
    pub single_replica_volumes: Vec<Uuid>,
}

/// Cluster lever validation before upgrade.
/// 1. Check for any volume rebuild in progress across the cluster
/// 2. List all single replica volumes
pub async fn upgrade_validations() -> Result<UpgradeValidation, Error> {
    let rebuild_in_progress = false;
    let mut validation = UpgradeValidation {
        rebuild_in_progress,
        single_replica_volumes: Vec::new(),
    };
    match RestClient::client()
        .volumes_api()
        .get_volumes(0, None, None)
        .await
    {
        Ok(volumes) => {
            for volume in volumes.into_body().entries {
                if let Some(target) = &volume.state.target {
                    validation.rebuild_in_progress = target
                        .children
                        .iter()
                        .any(|child| child.rebuild_progress.is_some());
                }

                if volume.spec.num_replicas == SINGLE_REPLICA_VOLUME {
                    validation.single_replica_volumes.push(volume.spec.uuid);
                }
            }
            if rebuild_in_progress {
                validation.rebuild_in_progress = true
            }
        }
        Err(error) => {
            return Err(error.into());
        }
    }
    Ok(validation)
}
