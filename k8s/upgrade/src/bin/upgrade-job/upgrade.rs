use crate::{
    common::{constants::PRODUCT, error::Result},
    events::event_recorder::{EventAction, EventRecorder},
    helm::upgrade::{HelmUpgrade, HelmUpgradeRunner},
    opts::CliArgs,
};
use data_plane::upgrade_data_plane;

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
    let helm_upgrade = HelmUpgrade::builder()
        .with_namespace(opts.namespace())
        .with_release_name(opts.release_name())
        .with_core_chart_dir(opts.core_chart_dir())
        .with_skip_upgrade_path_validation(opts.skip_upgrade_path_validation())
        .with_values(opts.values())
        .build()
        .await?;

    let from_version = helm_upgrade.upgrade_from_version();
    let to_version = helm_upgrade.upgrade_to_version();

    // Updating the EventRecorder with version values from the HelmUpgrade.
    // These two operations are thread-safe. The EventRecorder itself is not
    // shared with any other tokio task.
    event.set_from_version(from_version.clone());
    event.set_to_version(to_version.clone());

    // Dry-run helm upgrade.
    let dry_run_result: Result<HelmUpgradeRunner> = helm_upgrade.dry_run().await;
    let run_helm_upgrade = match dry_run_result {
        Err(error) => {
            event.publish_unrecoverable(&error, true).await;
            Err(error)
        }
        Ok(run_helm_upgrade) => Ok(run_helm_upgrade),
    }?;

    event
        .publish_normal(
            format!("Starting {PRODUCT} upgrade..."),
            EventAction::UpgradingCP,
        )
        .await?;

    event
        .publish_normal(
            format!("Upgrading {PRODUCT} control-plane"),
            EventAction::UpgradingCP,
        )
        .await?;

    // Control plane containers are updated in this step.
    if let Err(error) = run_helm_upgrade.await {
        event.publish_unrecoverable(&error, false).await;
        return Err(error);
    }

    event
        .publish_normal(
            format!("Upgraded {PRODUCT} control-plane"),
            EventAction::UpgradedCP,
        )
        .await?;

    // Data plane containers are updated in this step.
    if !opts.skip_data_plane_restart() {
        event
            .publish_normal(
                format!("Upgrading {PRODUCT} data-plane"),
                EventAction::UpgradingDP,
            )
            .await?;

        if let Err(error) =
            upgrade_data_plane(opts.namespace(), opts.rest_endpoint(), to_version).await
        {
            event.publish_unrecoverable(&error, false).await;
            return Err(error);
        }

        event
            .publish_normal(
                format!("Upgraded {PRODUCT} data-plane"),
                EventAction::UpgradedDP,
            )
            .await?;
    }

    event
        .publish_normal(
            format!("Successfully upgraded {PRODUCT}"),
            EventAction::Successful,
        )
        .await?;

    Ok(())
}
