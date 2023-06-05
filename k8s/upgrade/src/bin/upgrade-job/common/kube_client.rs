use crate::common::error::{K8sClientGeneration, KubeClientSetBuilderNs, Result};
use k8s_openapi::api::{
    apps::v1::Deployment,
    core::v1::{Namespace, Pod},
};
use kube::{api::Api, Client};
use snafu::ResultExt;

/// Builder for Kubernetes clients.
#[derive(Default)]
pub(crate) struct KubeClientSetBuilder {
    namespace: Option<String>,
}

impl KubeClientSetBuilder {
    /// Build Kubernetes API clients for a specific namespace (for namespaced object only).
    #[must_use]
    pub(crate) fn with_namespace<T>(mut self, namespace: T) -> Self
    where
        T: ToString,
    {
        self.namespace = Some(namespace.to_string());
        self
    }

    // TODO: Make the builder option validations error out at compile-time, using std::compile_error
    // or something similar.
    /// Build the KubeClientSet.
    pub(crate) async fn build(self) -> Result<KubeClientSet> {
        // Namespace must be used.
        let namespace = self.namespace.ok_or(KubeClientSetBuilderNs.build())?;

        let client = Client::try_default().await.context(K8sClientGeneration)?;
        return Ok(KubeClientSet {
            client: client.clone(),
            pods_api: Api::namespaced(client.clone(), namespace.as_str()),
            namespaces_api: Api::all(client.clone()),
            deployments_api: Api::namespaced(client, namespace.as_str()),
        });
    }
}

/// This is a wrapper around kube::Client with helper methods to generate Api<?> clients.
pub(crate) struct KubeClientSet {
    client: Client,
    pods_api: Api<Pod>,
    namespaces_api: Api<Namespace>,
    deployments_api: Api<Deployment>,
}

impl KubeClientSet {
    pub(crate) fn builder() -> KubeClientSetBuilder {
        KubeClientSetBuilder::default()
    }
    /// Generate the Pod api client.
    pub(crate) fn pods_api(&self) -> &Api<Pod> {
        &self.pods_api
    }

    /// Generate the Namespace api client.
    pub(crate) fn namespaces_api(&self) -> &Api<Namespace> {
        &self.namespaces_api
    }

    /// Generate the Deployment api client.
    pub(crate) fn deployments_api(&self) -> &Api<Deployment> {
        &self.deployments_api
    }

    /// Get a clone of the kube::Client.
    pub(crate) fn client(&self) -> Client {
        self.client.clone()
    }
}
