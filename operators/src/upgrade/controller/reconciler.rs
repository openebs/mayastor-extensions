use chrono::Utc;
use core::ops::Deref;
use futures::StreamExt;
use k8s_openapi::{
    api::core::v1::Pod,
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{
    api::{DeleteParams, ListParams, ObjectList, Patch, PatchParams, PostParams},
    runtime::{controller::Action, finalizer, Controller},
    Api, Client, CustomResourceExt, ResourceExt,
};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tracing::{debug, error, info, trace, warn};

use crate::upgrade::{
    common::{
        constants::{IO_ENGINE_POD_LABEL, UPGRADE_ACTION_FINALIZER, UPGRADE_OPERATOR},
        error::Error,
    },
    config::UpgradeConfig,
    controller::utils::*,
    k8s::crd::v0::{UpgradeAction, UpgradeActionStatus, UpgradePhase, UpgradeState},
    phases::{init::ComponentsState, updating::components_update},
};

/// Additional per resource context during the runtime; it is volatile
#[derive(Clone)]
pub(crate) struct ResourceContext {
    /// The latest CRD known to us
    inner: Arc<UpgradeAction>,
    /// Counter that keeps track of how many times the reconcile loop has run
    /// within the current state
    num_retries: u32,
    /// Reference to the operator context
    ctx: Arc<ControllerContext>,
}

impl Deref for ResourceContext {
    type Target = UpgradeAction;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// ControllerContext is used to control create/update/remove UpgradeAction CustomResource to/from
/// the work queue.
pub struct ControllerContext {
    /// Reference to our k8s client.
    k8s: Client,
    /// Hashtable of name and the full last seen CRD.
    inventory: tokio::sync::RwLock<HashMap<String, ResourceContext>>,
}

impl ControllerContext {
    /// Upsert the potential new CRD into the operator context. If an existing
    /// resource with the same name is present, the old resource is
    /// returned.
    pub(crate) async fn upsert(
        &self,
        ctx: Arc<ControllerContext>,
        ua: Arc<UpgradeAction>,
    ) -> ResourceContext {
        let resource = ResourceContext {
            inner: ua,
            num_retries: 0,
            ctx,
        };

        let mut i = self.inventory.write().await;
        debug!(count = i.keys().count(), "current number of CRDs");

        match i.get_mut(&resource.name_any()) {
            Some(p) => {
                if p.resource_version() == resource.resource_version() {
                    debug!(status =? resource.status, "duplicate event or long running operation");

                    // The status should be the same here as well
                    assert_eq!(&p.status, &resource.status);
                    p.num_retries += 1;
                    return p.clone();
                }

                // Its a new resource version which means we will swap it out
                // to reset the counter.
                let p = i
                    .insert(resource.name_any(), resource.clone())
                    .expect("existing resource should be present");
                info!(name = p.name_any(), "new resource_version inserted");
                resource
            }

            None => {
                let p = i.insert(resource.name_any(), resource.clone());
                assert!(p.is_none());
                resource
            }
        }
    }

    /// Remove the resource from the operator.
    pub(crate) async fn remove(&self, name: String) -> Option<ResourceContext> {
        let mut i = self.inventory.write().await;
        let removed = i.remove(&name);
        if let Some(removed) = removed {
            info!(name =? removed.name_any(), "removed from inventory");
            return Some(removed);
        }
        None
    }
}

impl ResourceContext {
    /// Called when putting our finalizer on top of the resource.
    #[tracing::instrument(fields(name = ua.name_any()))]
    pub(crate) async fn put_finalizer(ua: Arc<UpgradeAction>) -> Result<Action, Error> {
        Ok(Action::await_change())
    }

    /// Deletes the finalizer of the UpgradeAction resource.
    #[tracing::instrument(fields(name = resource.name_any()) skip(resource))]
    pub(crate) async fn delete_finalizer(resource: ResourceContext) -> Result<Action, Error> {
        let ctx = resource.ctx.clone();
        ctx.remove(resource.name_any()).await;
        Ok(Action::await_change())
    }

    /// Clone the inner value of this resource.
    fn inner(&self) -> Arc<UpgradeAction> {
        self.inner.clone()
    }

    /// Construct an API handle for the resource.
    fn api(&self) -> Api<UpgradeAction> {
        Api::namespaced(self.ctx.k8s.clone(), &self.namespace().unwrap())
    }

    /// This patches the Status of an UpgradeAction resource.
    async fn patch_status(&self, status: UpgradeActionStatus) -> Result<UpgradeAction, Error> {
        let status = json!({ "status": status });

        let ps = PatchParams::apply(UPGRADE_OPERATOR);

        let o = self
            .api()
            .patch_status(&self.name_any(), &ps, &Patch::Merge(&status))
            .await
            .map_err(|source| Error::K8sClientError { source })?;

        debug!(name = o.name_any(), old = ?self.status, new = ?o.status, "status changed");

        Ok(o)
    }

    /// Mark the resource as errored which is its final state. A upgrade action in the error state
    /// will not be deleted.
    async fn mark_error(&self, components_phase: UpgradePhase) -> Result<Action, Error> {
        let _ = self
            .patch_status(UpgradeActionStatus::new(
                UpgradeState::Error,
                Utc::now(),
                ComponentsState::with_state(components_phase).convert_into_hash(),
            ))
            .await?;

        error!(name = self.name_any(), "status set to error");
        Ok(Action::await_change())
    }

    /// Start upgrade process.
    async fn start(&self) -> Result<Action, Error> {
        let _ = self
            .patch_status(UpgradeActionStatus::new(
                UpgradeState::NotStarted,
                Utc::now(),
                ComponentsState::with_state(UpgradePhase::Waiting).convert_into_hash(),
            ))
            .await?;
        Ok(Action::await_change())
    }

    /// Start Updating phase.
    async fn updating(&self) -> Result<Action, Error> {
        let _ = self
            .patch_status(UpgradeActionStatus::new(
                UpgradeState::UpdatingControlPlane,
                Utc::now(),
                ComponentsState::with_state(UpgradePhase::Updating).convert_into_hash(),
            ))
            .await?;
        let opts: Vec<(String, String)> = vec![("image.tag".to_string(), "develop".to_string())];

        match components_update(opts).await {
            Ok(_) => Ok(Action::await_change()),
            Err(_) => {
                self.mark_error(UpgradePhase::Error).await?;
                // we updated the resource as an error stop reconciliation
                Err(Error::ReconcileError {
                    name: self.name_any(),
                })
            }
        }
    }

    /// This manually deletes io-engine DaemonSet pods because the
    /// the DaemonSet update strategy is set is 'OnDelete'.
    /// Ref: https://github.com/openebs/mayastor-extensions/blob/HEAD/chart/templates/mayastor/io/io-engine-daemonset.yaml
    async fn restart_io_engine(&self) -> Result<Action, Error> {
        let _ = self
            .patch_status(UpgradeActionStatus::new(
                UpgradeState::UpdatingDataPlane,
                Utc::now(),
                ComponentsState::with_state(UpgradePhase::Updating).convert_into_hash(),
            ))
            .await?;

        let pods: Api<Pod> = Api::namespaced(
            UpgradeConfig::get_config().k8s_client().client(),
            UpgradeConfig::get_config().namespace(),
        );

        let io_engine_listparam = ListParams::default().labels(IO_ENGINE_POD_LABEL);
        let initial_io_engine_pod_list: ObjectList<Pod> = pods.list(&io_engine_listparam).await?;

        let mut initial_io_engine_pod_uids: HashSet<&String> =
            HashSet::with_capacity(initial_io_engine_pod_list.iter().count());
        for pod in initial_io_engine_pod_list.iter() {
            match pod.metadata.uid.as_ref() {
                Some(uid) => {
                    // Building a set of io-engine Pod uid values. These values may be used to check
                    // for io-engine Pod restarts.
                    initial_io_engine_pod_uids.insert(uid);
                }
                None => {
                    // All kubernetes objects should have uids.
                    error!("Could not list all io-engine Pods' metadata.uids before restart");
                    return Err(Error::K8sApiError {
                        name: self.name_any(),
                        reason: format!(
                            "Pod '{}' in the namespace '{}' does not have a 'metadata.uid'",
                            pod.name_any(),
                            UpgradeConfig::get_config().namespace()
                        ),
                    });
                }
            }
        }

        // Checking to see if all volumes are unpublished before proceeding.
        if at_least_one_volume_is_published(UpgradeConfig::get_config().rest_client()).await? {
            return Err(Error::VolumesNotUnpublishedError {
                reason: "All volumes are not unpublished".to_string(),
            });
        }
        info!("All volumes are unpublished");

        info!("Restarting io-engine pods...");
        match pods
            .delete_collection(&DeleteParams::default(), &io_engine_listparam)
            .await?
        {
            either::Left(list) => {
                let names: Vec<_> = list.iter().map(ResourceExt::name_any).collect();
                debug!(?names, "Deleting collection of pods");
            }
            either::Right(status) => {
                debug!(?status, "Deleted collection of pods: status");
            }
        }
        debug!("Deleted io-engine Pods");

        debug!("Waiting for new io-engine Pods to start...");

        let max_retries = 20;
        let mut current_retries = 0;
        while current_retries < max_retries {
            let final_io_engine_pod_list: ObjectList<Pod> = pods.list(&io_engine_listparam).await?;
            if at_least_one_pod_uid_exists(&initial_io_engine_pod_uids, final_io_engine_pod_list) {
                debug!("Not all io-engine Pods have been restarted yet. Waiting for 3 seconds...");
                tokio::time::sleep(Duration::from_secs(3_u64)).await;
                current_retries += 1;
                continue;
            } else {
                info!("All io-engine Pods have been restarted");
                return Ok(Action::await_change());
            }
        }

        Err(Error::ReconcileError {
            name: self.name_any(),
        })
    }

    /// Starts verifying phase.
    async fn verifying(&self) -> Result<Action, Error> {
        let _ = self
            .patch_status(UpgradeActionStatus::new(
                UpgradeState::VerifyingUpdate,
                Utc::now(),
                ComponentsState::with_state(UpgradePhase::Verifying).convert_into_hash(),
            ))
            .await?;

        UpgradeConfig::get_config()
            .rest_client()
            .nodes_api()
            .get_nodes()
            .await?;

        // Verifying if the io-engine Pods are ready
        let pods: Api<Pod> = Api::namespaced(
            UpgradeConfig::get_config().k8s_client().client(),
            UpgradeConfig::get_config().namespace(),
        );

        let lp = ListParams::default().labels(IO_ENGINE_POD_LABEL);
        let pod_list: ObjectList<Pod> = pods.list(&lp).await?;
        if !all_pods_are_ready(pod_list) {
            return Err(Error::ReconcileError {
                name: self.name_any(),
            });
        }

        Ok(Action::await_change())
    }

    /// Starts successful update phase.
    async fn successful_update(&self) -> Result<Action, Error> {
        let _ = self
            .patch_status(UpgradeActionStatus::new(
                UpgradeState::SuccessfulUpdate,
                Utc::now(),
                ComponentsState::with_state(UpgradePhase::Completed).convert_into_hash(),
            ))
            .await?;
        Ok(Action::await_change())
    }

    /// Starts delete phase.
    async fn delete(&self) -> Result<Action, Error> {
        Self::delete_finalizer(self.clone()).await?;
        Ok(Action::await_change())
    }

    /// Callback hooks for the finalizers.
    async fn finalizer(&self) -> Result<Action, Error> {
        let _ = finalizer(
            &self.api(),
            UPGRADE_ACTION_FINALIZER,
            self.inner(),
            |event| async move {
                match event {
                    finalizer::Event::Apply(ua) => Self::put_finalizer(ua).await,
                    finalizer::Event::Cleanup(_ua) => Self::delete_finalizer(self.clone()).await,
                }
            },
        )
        .await
        .map_err(|error| error!(?error, "Failed to create/delete finalizer"));

        Ok(Action::await_change())
    }
}

/// Ensure the CRD is installed. This creates a chicken and egg problem. When the CRD is removed,
/// the operator will fail to list the CRD going into a error loop.
///
/// To prevent that, we will simply panic, and hope we can make progress after restart. Keep
/// running is not an option as the operator would be "running" and the only way to know something
/// is wrong would be to consult the logs.
async fn ensure_crd(k8s: Client) {
    let ua: Api<CustomResourceDefinition> = Api::all(k8s);
    let lp =
        ListParams::default().fields(&format!("metadata.name={}", "upgradeactions.openebs.io"));
    let crds = ua.list(&lp).await.expect("failed to list CRDS");

    // the CRD has not been installed yet, to avoid overwriting (and create upgrade issues) only
    // install it when there is no crd with the given name
    if crds.iter().count() == 0 {
        let crd = UpgradeAction::crd();
        info!(
            "Creating CRD: {}",
            serde_json::to_string_pretty(&crd).unwrap()
        );

        let pp = PostParams::default();
        match ua.create(&pp, &crd).await {
            Ok(o) => {
                info!(crd = o.name_any(), "created");
                // let the CRD settle this purely to avoid errors messages in the console
                // that are harmless but can cause some confusion maybe.
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            Err(error) => {
                error!(?error, "Failed to create CRD");
                tokio::time::sleep(Duration::from_secs(1)).await;
                std::process::exit(1);
            }
        }
    } else {
        info!("UpgradeAction CRD is already present.");
    }
}

/// Determine what we want to do when dealing with errors from the
/// reconciliation loop
fn error_policy(
    _object: Arc<UpgradeAction>,
    error: &Error,
    _ctx: Arc<ControllerContext>,
) -> Action {
    let duration = Duration::from_secs(match error {
        Error::Duplicate { timeout } | Error::SpecError { timeout, .. } => (*timeout).into(),

        Error::ReconcileError { .. } => {
            return Action::await_change();
        }
        _ => 5,
    });

    let when = Utc::now()
        .checked_add_signed(chrono::Duration::from_std(duration).unwrap())
        .unwrap();
    warn!(
        "{}, retry scheduled @{} ({} seconds from now)",
        error,
        when.to_rfc2822(),
        duration.as_secs()
    );
    Action::requeue(duration)
}

async fn reconcile(ua: Arc<UpgradeAction>, ctx: Arc<ControllerContext>) -> Result<Action, Error> {
    ensure_crd(ctx.k8s.clone()).await;
    let ua = ctx.upsert(ctx.clone(), ua).await;

    let _ = ua.finalizer().await;

    match ua.status {
        Some(UpgradeActionStatus {
            state: UpgradeState::NotStarted,
            ..
        }) => ua.updating().await,

        Some(UpgradeActionStatus {
            state: UpgradeState::UpdatingControlPlane,
            ..
        }) => {
            // Retry for up to 30 seconds, if there's an error.
            let max_retries = 6;
            let mut current_retries = 0;
            let mut err: Error = Error::ReconcileError {
                name: ua.name_any(),
            };
            while current_retries < max_retries {
                err = match ua.restart_io_engine().await {
                    Ok(action) => return Ok(action),
                    Err(error) => error,
                };
                tokio::time::sleep(Duration::from_secs(5)).await;
                current_retries += 1;
            }

            Err(err)
        }

        Some(UpgradeActionStatus {
            state: UpgradeState::UpdatingDataPlane,
            ..
        }) => ua.verifying().await,

        Some(UpgradeActionStatus {
            state: UpgradeState::VerifyingUpdate,
            ..
        }) => ua.successful_update().await,

        Some(UpgradeActionStatus {
            state: UpgradeState::SuccessfulUpdate,
            ..
        }) => ua.delete().await,

        Some(UpgradeActionStatus {
            state: UpgradeState::Error,
            ..
        }) => {
            error!(upgrade = ua.name_any(), "entered error as final state");
            Err(Error::ReconcileError {
                name: ua.name_any(),
            })
        }

        // We use this state to indicate its a new CRD however, we could (and
        // perhaps should) use the finalizer callback.
        None => ua.start().await,
    }
}
/// Upgrade controller that has its own resource and controller context.
pub async fn start_upgrade_worker() {
    let ua: Api<UpgradeAction> = Api::namespaced(
        UpgradeConfig::get_config().k8s_client().client(),
        UpgradeConfig::get_config().namespace(),
    );
    let lp = ListParams::default();

    let context = ControllerContext {
        k8s: UpgradeConfig::get_config().k8s_client().client(),
        inventory: tokio::sync::RwLock::new(HashMap::new()),
    };

    Controller::new(ua, lp)
        .run(reconcile, error_policy, Arc::new(context))
        .for_each(|res| async move {
            trace!(?res);
        })
        .await;
}
