use crate::{
    common::{constants::PRODUCT, error::Result},
    events::event_recorder::EventRecorder,
    helm::upgrade::HelmUpgrade,
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
    let helm_upgrade = HelmUpgrade::builder()
        .with_namespace(opts.namespace())
        .with_release_name(opts.release_name())
        .with_core_chart_dir(opts.core_chart_dir())
        .with_skip_upgrade_path_validation(opts.skip_upgrade_path_validation())
        .with_custom_image_tag(opts.custom_image_tag())
        .build()
        .await?;

    let from_version = helm_upgrade.upgrade_from_version();
    let to_version = helm_upgrade.upgrade_to_version();

    let event = EventRecorder::builder()
        .with_pod_name(&opts.pod_name())
        .with_namespace(&opts.namespace())
        .with_from_version(from_version)
        .with_to_version(to_version.clone())
        .build()
        .await?;

    event
        .publish_normal(
            format!("Starting {PRODUCT} upgrade..."),
            "Upgrading control-plane",
        )
        .await?;

    event
        .publish_normal(
            format!("Upgrading {PRODUCT} control-plane"),
            "Upgrading control-plane",
        )
        .await?;

    // Control plane containers are updated in this step.
    if let Err(error) = helm_upgrade.run() {
        event.publish_unrecoverable(&error).await;
        return Err(error);
    }

    event
        .publish_normal(
            format!("Upgraded {PRODUCT} control-plane"),
            "Upgraded control-plane",
        )
        .await?;

    // Data plane containers are updated in this step.
    if !opts.skip_data_plane_restart() {
        event
            .publish_normal(
                format!("Upgrading {PRODUCT} data-plane"),
                "Upgrading data-plane",
            )
            .await?;

        if let Err(error) =
            upgrade_data_plane(opts.namespace(), opts.rest_endpoint(), to_version).await
        {
            event.publish_unrecoverable(&error).await;
            return Err(error);
        }

        event
            .publish_normal(
                format!("Upgraded {PRODUCT} data-plane"),
                "Upgraded data-plane",
            )
            .await?;
    }

    event
        .publish_normal(format!("Successfully upgraded {PRODUCT}"), "Successful")
        .await?;

    event.shutdown_worker().await;
    Ok(())
}
