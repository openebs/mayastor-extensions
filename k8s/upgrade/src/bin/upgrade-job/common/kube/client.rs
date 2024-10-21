use crate::common::{
    constants::KUBE_API_PAGE_SIZE,
    error::{
        ControllerRevisionDoesntHaveHashLabel, ControllerRevisionListEmpty,
        InvalidNoOfHelmConfigMaps, InvalidNoOfHelmSecrets, K8sClientGeneration,
        ListConfigMapsWithLabelAndField, ListCtrlRevsWithLabelAndField, ListNodesWithLabelAndField,
        ListPodsWithLabelAndField, ListSecretsWithLabelAndField, Result,
    },
};
use k8s_openapi::{
    api::{
        apps::v1::ControllerRevision,
        core::v1::{ConfigMap, Namespace, Node, Pod, Secret},
    },
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{
    api::{Api, ListParams},
    core::PartialObjectMeta,
    Client, Resource, ResourceExt,
};
use serde::de::DeserializeOwned;
use snafu::{ensure, ErrorCompat, IntoError, ResultExt};

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

/// Generate ControllerRevision api client.
pub(crate) async fn controller_revisions_api(namespace: &str) -> Result<Api<ControllerRevision>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate the Pod api client.
pub(crate) async fn pods_api(namespace: &str) -> Result<Api<Pod>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate the Secret api client.
pub(crate) async fn secrets_api(namespace: &str) -> Result<Api<Secret>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate the Configmap api client.
pub(crate) async fn configmaps_api(namespace: &str) -> Result<Api<ConfigMap>> {
    Ok(Api::namespaced(client().await?, namespace))
}

pub(crate) async fn list_pods(
    namespace: String,
    label_selector: Option<String>,
    field_selector: Option<String>,
) -> Result<Vec<Pod>> {
    let mut pods: Vec<Pod> = Vec::with_capacity(KUBE_API_PAGE_SIZE as usize);

    let mut list_params = ListParams::default().limit(KUBE_API_PAGE_SIZE);
    if let Some(ref labels) = label_selector {
        list_params = list_params.labels(labels);
    }
    if let Some(ref fields) = field_selector {
        list_params = list_params.fields(fields);
    }

    let pods_api = pods_api(namespace.as_str()).await?;

    let list_pods_error_ctx = ListPodsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    paginated_list(pods_api, &mut pods, Some(list_params), list_pods_error_ctx).await?;

    Ok(pods)
}

/// List Nodes metadata in the kubernetes cluster.
pub(crate) async fn list_nodes_metadata(
    label_selector: Option<String>,
    field_selector: Option<String>,
) -> Result<Vec<PartialObjectMeta<Node>>> {
    let mut nodes: Vec<PartialObjectMeta<Node>> = Vec::with_capacity(KUBE_API_PAGE_SIZE as usize);

    let mut list_params = ListParams::default().limit(KUBE_API_PAGE_SIZE);
    if let Some(ref labels) = label_selector {
        list_params = list_params.labels(labels);
    }
    if let Some(ref fields) = field_selector {
        list_params = list_params.fields(fields);
    }

    let nodes_api = nodes_api().await?;

    let list_nodes_error_ctx = ListNodesWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
    };

    paginated_list_metadata(
        nodes_api,
        &mut nodes,
        Some(list_params),
        list_nodes_error_ctx,
    )
    .await?;

    Ok(nodes)
}

/// List ControllerRevisions in a Kubernetes namespace.
pub(crate) async fn list_controller_revisions(
    namespace: String,
    label_selector: Option<String>,
    field_selector: Option<String>,
) -> Result<Vec<ControllerRevision>> {
    let mut ctrl_revs: Vec<ControllerRevision> = Vec::with_capacity(KUBE_API_PAGE_SIZE as usize);

    let mut list_params = ListParams::default().limit(KUBE_API_PAGE_SIZE);
    if let Some(ref labels) = label_selector {
        list_params = list_params.labels(labels);
    }
    if let Some(ref fields) = field_selector {
        list_params = list_params.fields(fields);
    }

    let controller_revisions_api = controller_revisions_api(namespace.as_str()).await?;

    let list_ctrl_revs_error_ctx = ListCtrlRevsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    paginated_list(
        controller_revisions_api,
        &mut ctrl_revs,
        Some(list_params),
        list_ctrl_revs_error_ctx,
    )
    .await?;

    Ok(ctrl_revs)
}

/// Returns the controller-revision-hash of the latest revision of a resource's ControllerRevisions.
pub(crate) async fn latest_controller_revision_hash(
    namespace: String,
    label_selector: Option<String>,
    field_selector: Option<String>,
    hash_label_key: String,
) -> Result<String> {
    let mut ctrl_revs = list_controller_revisions(
        namespace.clone(),
        label_selector.clone(),
        field_selector.clone(),
    )
    .await?;
    // Fail if ControllerRevisions list is empty.
    ensure!(
        !ctrl_revs.is_empty(),
        ControllerRevisionListEmpty {
            namespace: namespace.clone(),
            label_selector: label_selector.unwrap_or_default(),
            field_selector: field_selector.unwrap_or_default()
        }
    );

    // Sort non-ascending by revision no.
    ctrl_revs.sort_unstable_by(|a, b| b.revision.cmp(&a.revision));

    ctrl_revs[0]
        .labels()
        .get(&hash_label_key)
        .map(|s| s.into())
        .ok_or(
            ControllerRevisionDoesntHaveHashLabel {
                name: ctrl_revs[0].name_unchecked(),
                namespace,
                hash_label_key,
            }
            .build(),
        )
}

/// This returns a list of Secrets based on filtering criteria. Returns all if criteria is absent.
pub(crate) async fn list_secrets(
    namespace: String,
    label_selector: Option<String>,
    field_selector: Option<String>,
) -> Result<Vec<Secret>> {
    let mut secrets: Vec<Secret> = Vec::with_capacity(KUBE_API_PAGE_SIZE as usize);

    let mut list_params = ListParams::default().limit(KUBE_API_PAGE_SIZE);
    if let Some(ref labels) = label_selector {
        list_params = list_params.labels(labels);
    }
    if let Some(ref fields) = field_selector {
        list_params = list_params.fields(fields);
    }

    let secrets_api = secrets_api(namespace.as_str()).await?;

    let list_secrets_error = ListSecretsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    paginated_list(
        secrets_api,
        &mut secrets,
        Some(list_params),
        list_secrets_error,
    )
    .await?;

    Ok(secrets)
}

/// This returns a list of ConfigMaps based on filtering criteria. Returns all if criteria is
/// absent.
pub(crate) async fn list_configmaps(
    namespace: String,
    label_selector: Option<String>,
    field_selector: Option<String>,
) -> Result<Vec<ConfigMap>> {
    let mut configmaps: Vec<ConfigMap> = Vec::with_capacity(KUBE_API_PAGE_SIZE as usize);

    let mut list_params = ListParams::default().limit(KUBE_API_PAGE_SIZE);
    if let Some(ref labels) = label_selector {
        list_params = list_params.labels(labels);
    }
    if let Some(ref fields) = field_selector {
        list_params = list_params.fields(fields);
    }

    let configmaps_api = configmaps_api(namespace.as_str()).await?;

    let list_configmaps_error = ListConfigMapsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    paginated_list(
        configmaps_api,
        &mut configmaps,
        Some(list_params),
        list_configmaps_error,
    )
    .await?;

    Ok(configmaps)
}

/// GET the helm release secret for a helm release in a namespace.
pub(crate) async fn get_helm_release_secret(
    release_name: String,
    namespace: String,
) -> Result<Secret> {
    let secrets = list_secrets(
        namespace.clone(),
        Some(format!("name={release_name},status=deployed")),
        Some("type=helm.sh/release.v1".to_string()),
    )
    .await?;
    let wrong_no_of_secrets = InvalidNoOfHelmSecrets {
        release_name,
        namespace,
        count: secrets.len(),
    };
    ensure!(secrets.len() == 1, wrong_no_of_secrets.clone());

    secrets
        .into_iter()
        .next()
        .ok_or(wrong_no_of_secrets.build())
}

/// GET the helm release configmap for a helm release in a namespace.
pub(crate) async fn get_helm_release_configmap(
    release_name: String,
    namespace: String,
) -> Result<ConfigMap> {
    let cms = list_configmaps(
        namespace.clone(),
        Some(format!("name={release_name},owner=helm,status=deployed")),
        None,
    )
    .await?;
    let wrong_no_of_cms = InvalidNoOfHelmConfigMaps {
        release_name,
        namespace,
        count: cms.len(),
    };
    ensure!(cms.len() == 1, wrong_no_of_cms.clone());

    cms.into_iter().next().ok_or(wrong_no_of_cms.build())
}

async fn paginated_list<K, C, E2>(
    resource_api: Api<K>,
    resources: &mut Vec<K>,
    list_params: Option<ListParams>,
    list_err_ctx: C,
) -> Result<()>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug,
    C: IntoError<E2, Source = kube::Error> + Clone,
    E2: std::error::Error + ErrorCompat,
    crate::common::error::Error: From<E2>,
{
    let mut list_params = list_params.unwrap_or_default().limit(KUBE_API_PAGE_SIZE);

    loop {
        let resource_list = resource_api
            .list(&list_params)
            .await
            .context(list_err_ctx.clone())?;

        let maybe_token = resource_list.metadata.continue_.clone();

        resources.extend(resource_list);

        match maybe_token {
            Some(ref token) => {
                list_params = list_params.continue_token(token);
            }
            None => break,
        }
    }

    Ok(())
}

async fn paginated_list_metadata<K, C, E2>(
    resource_api: Api<K>,
    resources: &mut Vec<PartialObjectMeta<K>>,
    list_params: Option<ListParams>,
    list_err_ctx: C,
) -> Result<()>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug,
    C: IntoError<E2, Source = kube::Error> + Clone,
    E2: std::error::Error + ErrorCompat,
    crate::common::error::Error: From<E2>,
{
    let mut list_params = list_params.unwrap_or_default().limit(KUBE_API_PAGE_SIZE);

    loop {
        let resource_list = resource_api
            .list_metadata(&list_params)
            .await
            .context(list_err_ctx.clone())?;

        let maybe_token = resource_list.metadata.continue_.clone();

        resources.extend(resource_list);

        match maybe_token {
            Some(ref token) => {
                list_params = list_params.continue_token(token);
            }
            None => break,
        }
    }

    Ok(())
}
