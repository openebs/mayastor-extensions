use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use core::ops::Deref;
use chrono::Utc;
use kube::api::{ListParams, PostParams, PatchParams, Patch};
use kube::runtime::finalizer;
use kube::{Client, Api, ResourceExt, CustomResourceExt};
use kube::runtime::controller::Action;
use openapi::clients;
use serde_json::json;
use tracing::{warn, debug, info,error};

use crate::upgrade::common::constants::UPGRADE_OPERATOR;
use crate::upgrade::common::error::Error;
use crate::upgrade::k8s::crd::v0::{UpgradeAction, UpgradeActionStatus, UpgradeState};


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


pub struct ControllerContext{
    /// Reference to our k8s client
    k8s: Client,
    /// Hashtable of name and the full last seen CRD
    inventory: tokio::sync::RwLock<HashMap<String, ResourceContext>>,
    /// HTTP client
    http: clients::tower::ApiClient,
    /// Interval
    interval: u64,
    /// Retries
    retries: u32,
}

impl ControllerContext{
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
        debug!(count = ?i.keys().count(), "current number of CRDS");

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
                info!(name = ?p.name_any(), "new resource_version inserted");
                resource
            }

            None => {
                let p = i.insert(resource.name_any(), resource.clone());
                assert!(p.is_none());
                resource
            }
        }
    }

    /// Remove the resource from the operator
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

impl ResourceContext{
    /// Called when putting our finalizer on top of the resource.
    #[tracing::instrument(fields(name = ?ua.name_any()))]
    pub(crate) async fn put_finalizer(ua: Arc<UpgradeAction>) -> Result<Action, Error> {
        Ok(Action::await_change())
    }

    /// Our notification that we should remove the pool and then the finalizer
    #[tracing::instrument(fields(name = ?resource.name_any()) skip(resource))]
    pub(crate) async fn delete_finalizer(resource: ResourceContext) -> Result<Action, Error> {
        let ctx = resource.ctx.clone();
       // resource.delete_pool().await?;
        ctx.remove(resource.name_any()).await;
        Ok(Action::await_change())
    }


    /// Clone the inner value of this resource
    fn inner(&self) -> Arc<UpgradeAction> {
        self.inner.clone()
    }

    /// Construct an API handle for the resource
    fn api(&self) -> Api<UpgradeAction> {
        Api::namespaced(self.ctx.k8s.clone(), &self.namespace().unwrap())
    }

    /// Patch the given dsp status to the state provided. When not online the
    /// size should be assumed to be zero.
    async fn patch_status(&self, status: UpgradeActionStatus) -> Result<UpgradeAction, Error> {
        let status = json!({ "status": status });

        let ps = PatchParams::apply(UPGRADE_OPERATOR);

        let o = self
            .api()
            .patch_status(&self.name_any(), &ps, &Patch::Merge(&status))
            .await
            .map_err(|source| Error::Kube { source })?;

        debug!(name = ?o.name_any(), old = ?self.status, new =?o.status, "status changed");

        Ok(o)
    }

    /// Create a pool when there is no status found. When no status is found for
    /// this resource it implies that it does not exist yet and so we create
    /// it. We set the state of the of the object to Creating, such that we
    /// can track the its progress
    async fn start(&self) -> Result<Action, Error> {
        let _ = self.patch_status(UpgradeActionStatus::default()).await?;
        Ok(Action::await_change())
    }

     /// Callback hooks for the finalizers
     async fn finalizer(&self) -> Result<Action, Error> {
        let _ = finalizer(
            &self.api(),
            "openebs.io/upgrade-protection",
            self.inner(),
            |event| async move {
                match event {
                    finalizer::Event::Apply(ua) => Self::put_finalizer(ua).await,
                    finalizer::Event::Cleanup(_ua) => Self::delete_finalizer(self.clone()).await,
                }
            },
        )
        .await
        .map_err(|e| error!(?e));

        Ok(Action::await_change())
    }

}

/// ensure the CRD is installed. This creates a chicken and egg problem. When the CRD is removed,
/// the operator will fail to list the CRD going into a error loop.
///
/// To prevent that, we will simply panic, and hope we can make progress after restart. Keep
/// running is not an option as the operator would be "running" and the only way to know something
/// is wrong would be to consult the logs.
async fn ensure_crd(k8s: Client) {
    let ua: Api<k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition> = Api::all(k8s);
    let lp = ListParams::default().fields(&format!("metadata.name={}", "diskpools.openebs.io"));
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
                info!(crd = ?o.name_any(), "created");
                // let the CRD settle this purely to avoid errors messages in the console
                // that are harmless but can cause some confusion maybe.
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            Err(e) => {
                error!("failed to create CRD error {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                std::process::exit(1);
            }
        }
    } else {
        info!("CRD present")
    }
}


/// Determine what we want to do when dealing with errors from the
/// reconciliation loop
fn error_policy(error: &Error, _ctx: Arc<ControllerContext>) -> Action {
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

async fn reconcile(ua:Arc<UpgradeAction>, ctx:Arc<ControllerContext>)-> Result<Action, Error>{
    let ua = ctx.upsert(ctx.clone(), ua).await;

    let _ = ua.finalizer().await;

    match ua.status {
        Some(UpgradeActionStatus {
            state: UpgradeState::NotStarted,
            ..
        }) => ua.create_or_import().await,

        Some(UpgradeActionStatus {
            state: UpgradeState::Updating,
            ..
        }) => ua.updating_components().await,

        Some(UpgradeActionStatus {
            state: UpgradeState::VerifyingUpdate,
            ..
        }) => ua.verify_update().await,
        
        Some(UpgradeActionStatus {
            state: UpgradeState::SuccessfullUpdate,
            ..
        }) => ua.successfull_update().await,

        Some(UpgradeActionStatus {
            state: UpgradeState::Error,
            ..
        }) => {
            error!(pool = ?ua.name_any(), "entered error as final state");
            Err(Error::ReconcileError {
                name: ua.name_any(),
            })
        }

        // We use this state to indicate its a new CRD however, we could (and
        // perhaps should) use the finalizer callback.
        None => ua.start().await,
    }
}