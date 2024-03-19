use crate::error::job_error::{K8sClientGeneration, KubeClientSetBuilderNs, Result};
use k8s_openapi::{
    api::{
        apps::v1::Deployment,
        core::v1::{Namespace, Node, Pod},
    },
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{api::Api, Client};
use snafu::ResultExt;

/// Builder for Kubernetes clients.
#[derive(Default)]
pub struct KubeClientSetBuilder {
    namespace: Option<String>,
}

impl KubeClientSetBuilder {
    /// Build Kubernetes API clients for a specific namespace (for namespaced object only).
    #[must_use]
    pub fn with_namespace<T>(mut self, namespace: T) -> Self
    where
        T: ToString,
    {
        self.namespace = Some(namespace.to_string());
        self
    }

    // TODO: Make the builder option validations error out at compile-time, using std::compile_error
    // or something similar.
    /// Build the KubeClientSet.
    pub async fn build(self) -> Result<KubeClientSet> {
        // Namespace must be used.
        let namespace = self.namespace.ok_or(KubeClientSetBuilderNs.build())?;

        let client = Client::try_default().await.context(K8sClientGeneration)?;
        return Ok(KubeClientSet {
            client: client.clone(),
            nodes_api: Api::all(client.clone()),
            pods_api: Api::namespaced(client.clone(), namespace.as_str()),
            namespaces_api: Api::all(client.clone()),
            deployments_api: Api::namespaced(client.clone(), namespace.as_str()),
            crd_api: Api::all(client),
        });
    }
}

/// This is a wrapper around kube::Client with helper methods to generate Api<?> clients.
pub struct KubeClientSet {
    client: Client,
    nodes_api: Api<Node>,
    pods_api: Api<Pod>,
    namespaces_api: Api<Namespace>,
    deployments_api: Api<Deployment>,
    crd_api: Api<CustomResourceDefinition>,
}

impl KubeClientSet {
    pub fn builder() -> KubeClientSetBuilder {
        KubeClientSetBuilder::default()
    }

    /// Generate the Node api client.
    pub fn nodes_api(&self) -> &Api<Node> {
        &self.nodes_api
    }

    /// Generate the Pod api client.
    pub fn pods_api(&self) -> &Api<Pod> {
        &self.pods_api
    }

    /// Generate the Namespace api client.
    pub fn namespaces_api(&self) -> &Api<Namespace> {
        &self.namespaces_api
    }

    /// Generate the Deployment api client.
    pub fn deployments_api(&self) -> &Api<Deployment> {
        &self.deployments_api
    }

    /// Generate the CustomResourceDefinition api client.
    pub fn crd_api(&self) -> &Api<CustomResourceDefinition> {
        &self.crd_api
    }

    /// Get a clone of the kube::Client.
    pub fn client(&self) -> Client {
        self.client.clone()
    }
}
