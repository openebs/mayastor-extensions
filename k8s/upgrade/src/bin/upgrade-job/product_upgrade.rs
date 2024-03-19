use crate::opts::CliArgs;
use upgrade::{
    constants::job_constants::PRODUCT,
    data_plane_upgrade::upgrade_data_plane,
    error::job_error::Result,
    events::event_recorder::{EventAction, EventRecorder},
    helm_upgrade::{HelmUpgradeRunner, HelmUpgraderBuilder},
    rest_client::RestClientSet,
};

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
        .build()
        .await?;

    // Update EventRecorder.
    let source_version = helm_upgrader.source_version();
    let target_version = helm_upgrader.target_version();
    // Updating the EventRecorder with version values from the HelmUpgrade.
    // These two operations are thread-safe. The EventRecorder itself is not
    // shared with any other tokio task.
    event.set_source_version(source_version.clone());
    event.set_target_version(target_version.clone());

    // Capture HA state before helm upgrade is consumed.
    let ha_is_enabled = helm_upgrader.source_values().ha_is_enabled();

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

        if let Err(error) = upgrade_data_plane(
            opts.namespace(),
            RestClientSet::new_with_url(opts.rest_endpoint())?,
            target_version,
            ha_is_enabled,
        )
        .await
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
