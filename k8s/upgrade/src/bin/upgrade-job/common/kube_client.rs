use crate::common::error::{K8sClientGeneration, Result};
use k8s_openapi::{
    api::{
        apps::v1::Deployment,
        core::v1::{Namespace, Node, Pod},
    },
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{api::Api, Client};
use snafu::ResultExt;

/// Generate a new kube::Client.
pub(crate) async fn client() -> Result<Client> {
    Client::try_default().await.context(K8sClientGeneration)
}

/// Generate the Node api client.
pub(crate) async fn nodes_api() -> Result<Api<Node>> {
    Ok(Api::all(client().await?))
}

/// Generate the Namespace api client.
pub(crate) async fn namespaces_api() -> Result<Api<Namespace>> {
    Ok(Api::all(client().await?))
}

/// Generate the CustomResourceDefinition api client.
pub(crate) async fn crds_api() -> Result<Api<CustomResourceDefinition>> {
    Ok(Api::all(client().await?))
}

/// Generate the Pod api client.
pub(crate) async fn pods_api(namespace: &str) -> Result<Api<Pod>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate the Deployment api client.
pub(crate) async fn deployments_api(namespace: &str) -> Result<Api<Deployment>> {
    Ok(Api::namespaced(client().await?, namespace))
}
