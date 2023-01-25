use crate::upgrade::{
    common::constants::components,
    config::UpgradeConfig,
    k8s::crd::v0::{UpgradeAction, UpgradeActionSpec},
};
use k8s_openapi::{
    api::core::v1::Node,
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{
    api::{ListParams, PostParams},
    core::ObjectList,
    Api, Client, CustomResourceExt,
};
use semver::Version;
use std::time::Duration;
use tracing::{error, info};

use crate::upgrade::common::{constants::NODE_LABEL, error::Error};

/// K8sClient is used to talk to the kube-apiserver.
#[derive(Clone)]
pub struct K8sClient {
    client: kube::Client,
}

impl K8sClient {
    /// Create a new K8sClient with default configuration.
    pub(crate) async fn new() -> Result<Self, Error> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }

    /// This is a getter for K8sClient.client.
    pub(crate) fn client(&self) -> kube::Client {
        self.client.clone()
    }

    /// Get nodes present in the cluster with mayastor labels present.
    pub(crate) async fn get_nodes(&self) -> Result<ObjectList<Node>, Error> {
        let nodes: Api<Node> = Api::all(self.client.clone());
        let lp = ListParams::default().labels(NODE_LABEL);
        let list = nodes.list(&lp).await?;
        Ok(list)
    }

    /// Get nodes present in the cluster with mayastor labels present.
    pub(crate) async fn get_upgrade_action(&self) -> Result<Api<UpgradeAction>, Error> {
        let uas: Api<UpgradeAction> = Api::all(self.client.clone());
        Ok(uas)
    }

    /// Lists UpgradeAction CustomResources from the k8s cluster's kube-apiserver.
    pub(crate) async fn get_crds(&self) -> Result<ObjectList<CustomResourceDefinition>, Error> {
        let ua: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        let lp =
            ListParams::default().fields(&format!("metadata.name={}", "upgradeactions.openebs.io"));
        Ok(ua.list(&lp).await?)
    }

    /// Creates UpgradeAction CustomResource
    pub async fn create_upgrade_action_crd(&self) -> Result<(), Error> {
        let ua: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        let crds = self.get_crds().await?;

        if crds.iter().count() == 0 {
            let crd = UpgradeAction::crd();

            let pp = PostParams::default();
            match ua.create(&pp, &crd).await {
                Ok(_) => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    return Err(Error::K8sClientError { source: e });
                }
            }
        } else {
            info!("UpgradeAction CustomResourceDefinition already present in the cluster")
        }
        Ok(())
    }

    /// GETs UpgradeAction CustomResource from kube-apiserver.
    pub(crate) async fn get_upgrade_action_resource(&self) -> Result<UpgradeAction, Error> {
        let uas: Api<UpgradeAction> =
            Api::namespaced(self.client.clone(), UpgradeConfig::get_config().namespace());

        match uas.get("upgrade").await {
            Ok(u) => Ok(u),
            Err(e) => {
                info!("UpgradeAction CustomResource not present");
                Err(Error::K8sClientError { source: e })
            }
        }
    }

    /// get_upgrade_action_resource creates UpgradeAction CustomResource.
    pub(crate) async fn create_upgrade_action_resource(&self) -> Result<UpgradeAction, Error> {
        let uas: Api<UpgradeAction> =
            Api::namespaced(self.client.clone(), UpgradeConfig::get_config().namespace());
        match self.get_upgrade_action_resource().await {
            Ok(u) => {
                return Ok(u);
            }
            Err(_) => {
                info!("UpgradeAction CustomResource is not present");
            }
        }

        let ua = UpgradeAction::new(
            "upgrade",
            UpgradeActionSpec::new(Version::new(1, 2, 0), Version::new(2, 0, 0), components()),
        );

        match uas.create(&PostParams::default(), &ua).await {
            Ok(o) => Ok(o),
            Err(error) => {
                error!(?error, "Failed to create CustomResource");
                tokio::time::sleep(Duration::from_secs(1)).await;
                Err(Error::K8sClientError { source: error })
            }
        }
    }
}
