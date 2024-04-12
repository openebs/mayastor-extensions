use crate::{
    common::{
        constants::{
            CHART_VERSION_LABEL_KEY, CORE_CHART_NAME, IO_ENGINE_LABEL,
            PARTIAL_REBUILD_DISABLE_EXTENTS, PRODUCT,
        },
        error::{PartialRebuildNotAllowed, Result},
        kube_client as KubeClient,
    },
    events::event_recorder::{EventAction, EventRecorder},
    helm::upgrade::{HelmUpgradeRunner, HelmUpgraderBuilder},
    opts::CliArgs,
};
use data_plane::upgrade_data_plane;

use k8s_openapi::api::core::v1::Pod;
use kube::ResourceExt;
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
    let final_values = match run_helm_upgrade.await {
        Ok(values) => values,
        Err(error) => {
            event.publish_unrecoverable(&error, false).await;
            return Err(error);
        }
    };

    event
        .publish_normal(
            format!("Upgraded {PRODUCT} control-plane"),
            EventAction::UpgradedCP,
        )
        .await?;

    // Data plane containers are updated in this step.
    if !opts.skip_data_plane_restart() {
        let yet_to_upgrade_io_engine_label = format!(
            "{IO_ENGINE_LABEL},{CHART_VERSION_LABEL_KEY}!={}",
            target_version.as_str()
        );
        let yet_to_upgrade_io_engine_pods = KubeClient::list_pods(
            opts.namespace(),
            Some(yet_to_upgrade_io_engine_label.clone()),
            None,
        )
        .await?;

        partial_rebuild_check(
            yet_to_upgrade_io_engine_pods.as_slice(),
            final_values.partial_rebuild_is_enabled(),
        )?;

        event
            .publish_normal(
                format!("Upgrading {PRODUCT} data-plane"),
                EventAction::UpgradingDP,
            )
            .await?;

        if let Err(error) = upgrade_data_plane(
            opts.namespace(),
            opts.rest_endpoint(),
            target_version,
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

fn partial_rebuild_check(
    yet_to_upgrade_io_engine_pods: &[Pod],
    partial_rebuild_is_enabled: bool,
) -> Result<()> {
    let partial_rebuild_disable_required = yet_to_upgrade_io_engine_pods
        .iter()
        .filter_map(|pod| pod.labels().get(CHART_VERSION_LABEL_KEY))
        .any(|v| {
            let version =
                Version::parse(v).expect("failed to parse version from io-engine Pod label");
            version.ge(&PARTIAL_REBUILD_DISABLE_EXTENTS.0)
                & version.le(&PARTIAL_REBUILD_DISABLE_EXTENTS.1)
        });

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
