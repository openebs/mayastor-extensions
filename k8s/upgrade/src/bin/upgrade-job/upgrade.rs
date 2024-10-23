use crate::{
    common::{
        constants::{
            product_train, CORE_CHART_NAME, IO_ENGINE_LABEL, PARTIAL_REBUILD_DISABLE_EXTENTS,
        },
        error::{PartialRebuildNotAllowed, Result},
        kube::client as KubeClient,
    },
    events::event_recorder::{EventAction, EventRecorder},
    helm::upgrade::{HelmUpgradeRunner, HelmUpgraderBuilder},
    opts::CliArgs,
};
use constants::DS_CONTROLLER_REVISION_HASH_LABEL_KEY;
use data_plane::upgrade_data_plane;

use semver::Version;
use tracing::error;

/// Contains the data-plane upgrade logic.
pub(crate) mod data_plane;

/// Contains upgrade utilities.
pub(crate) mod utils;

/// Tools to validate upgrade path.
pub(crate) mod path;

/// This function starts and sees upgrade through to the end.
pub(crate) async fn upgrade(opts: &CliArgs) -> Result<()> {
    let mut event = EventRecorder::builder()
        .with_pod_name(&opts.pod_name())
        .with_namespace(&opts.namespace())
        .build()
        .await?;

    let result = upgrade_product(opts, &mut event).await;

    // This makes sure that the event worker attempts to publish
    // all of its events. It waits for the event worker to exit.
    event.shutdown_worker().await;

    result
}

/// This carries out the helm upgrade validation, actual helm upgrade, and the io-engine Pod
/// restarts.
async fn upgrade_product(opts: &CliArgs, event: &mut EventRecorder) -> Result<()> {
    let helm_upgrader = HelmUpgraderBuilder::default()
        .with_namespace(opts.namespace())
        .with_release_name(opts.release_name())
        .with_core_chart_dir(opts.core_chart_dir())
        .with_skip_upgrade_path_validation(opts.skip_upgrade_path_validation())
        .with_helm_args_set(opts.helm_args_set())
        .with_helm_args_set_file(opts.helm_args_set_file())
        .with_helm_storage_driver(opts.helm_storage_driver())
        .with_helm_reset_then_reuse_values(opts.helm_reset_then_reuse_values())
        .build()
        .await?;

    // Update EventRecorder.
    let source_version = helm_upgrader.source_version();
    let target_version = helm_upgrader.target_version();
    // Updating the EventRecorder with version values from the HelmUpgrade.
    // These two operations are thread-safe. The EventRecorder itself is not
    // shared with any other tokio task.
    event.set_source_version(source_version.to_string());
    event.set_target_version(target_version.to_string());

    // Dry-run helm upgrade.
    let dry_run_result: Result<HelmUpgradeRunner> = helm_upgrader.dry_run().await;
    let run_helm_upgrade = match dry_run_result {
        Err(error) => {
            event.publish_unrecoverable(&error, true).await;
            Err(error)
        }
        Ok(run_helm_upgrade) => Ok(run_helm_upgrade),
    }?;

    event
        .publish_normal(
            format!("Starting {} upgrade...", product_train()),
            EventAction::UpgradingCP,
        )
        .await?;

    event
        .publish_normal(
            format!("Upgrading {} control-plane", product_train()),
            EventAction::UpgradingCP,
        )
        .await?;

    // Control plane containers are updated in this step.
    let final_values = match run_helm_upgrade.await {
        Ok(values) => values,
        Err(error) => {
            event.publish_unrecoverable(&error, false).await;
            return Err(error);
        }
    };

    event
        .publish_normal(
            format!("Upgraded {} control-plane", product_train()),
            EventAction::UpgradedCP,
        )
        .await?;

    // Data plane containers are updated in this step.
    if !opts.skip_data_plane_restart() {
        partial_rebuild_check(&source_version, final_values.partial_rebuild_is_enabled())?;

        let latest_io_engine_ctrl_rev_hash = KubeClient::latest_controller_revision_hash(
            opts.namespace(),
            Some(IO_ENGINE_LABEL.to_string()),
            None,
            DS_CONTROLLER_REVISION_HASH_LABEL_KEY.to_string(),
        )
        .await?;

        let yet_to_upgrade_io_engine_label = format!(
            "{IO_ENGINE_LABEL},{DS_CONTROLLER_REVISION_HASH_LABEL_KEY}!={}",
            latest_io_engine_ctrl_rev_hash.as_str()
        );

        let yet_to_upgrade_io_engine_pods = KubeClient::list_pods(
            opts.namespace(),
            Some(yet_to_upgrade_io_engine_label.clone()),
            None,
        )
        .await?;

        event
            .publish_normal(
                format!("Upgrading {} data-plane", product_train()),
                EventAction::UpgradingDP,
            )
            .await?;

        if let Err(error) = upgrade_data_plane(
            opts.namespace(),
            opts.rest_endpoint(),
            latest_io_engine_ctrl_rev_hash,
            final_values.ha_is_enabled(),
            yet_to_upgrade_io_engine_label,
            yet_to_upgrade_io_engine_pods,
        )
        .await
        {
            event.publish_unrecoverable(&error, false).await;
            return Err(error);
        }

        event
            .publish_normal(
                format!("Upgraded {} data-plane", product_train()),
                EventAction::UpgradedDP,
            )
            .await?;
    }

    event
        .publish_normal(
            format!("Successfully upgraded {}", product_train()),
            EventAction::Successful,
        )
        .await?;

    Ok(())
}

fn partial_rebuild_check(source_version: &Version, partial_rebuild_is_enabled: bool) -> Result<()> {
    let partial_rebuild_disable_required = source_version.ge(&PARTIAL_REBUILD_DISABLE_EXTENTS.0)
        && source_version.le(&PARTIAL_REBUILD_DISABLE_EXTENTS.1);

    if partial_rebuild_disable_required && partial_rebuild_is_enabled {
        error!("Partial rebuild must be disabled for upgrades from {CORE_CHART_NAME} chart versions >= {}, <= {}", PARTIAL_REBUILD_DISABLE_EXTENTS.0, PARTIAL_REBUILD_DISABLE_EXTENTS.1);
        return PartialRebuildNotAllowed {
            chart_name: CORE_CHART_NAME.to_string(),
            lower_extent: PARTIAL_REBUILD_DISABLE_EXTENTS.0.to_string(),
            upper_extent: PARTIAL_REBUILD_DISABLE_EXTENTS.1.to_string(),
        }
        .fail();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_partial_rebuild_check() {
        use crate::upgrade::partial_rebuild_check;
        use semver::Version;

        let source = Version::new(2, 1, 0);
        assert!(matches!(partial_rebuild_check(&source, true), Ok(())));
        let source = Version::new(2, 2, 0);
        assert!(partial_rebuild_check(&source, true).is_err());
        let source = Version::new(2, 5, 0);
        assert!(partial_rebuild_check(&source, true).is_err());
        let source = Version::new(2, 6, 0);
        assert!(matches!(partial_rebuild_check(&source, true), Ok(())));
    }
}
