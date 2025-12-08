use crate::common::{
    constants::KUBE_API_PAGE_SIZE,
    error::{
        ControllerRevisionDoesntHaveHashLabel, ControllerRevisionListEmpty,
        FailedToDeleteStatefulSet, FailedToListMetadataPaginated, FailedToListPaginated,
        InvalidNoOfHelmConfigMaps, InvalidNoOfHelmSecrets, K8sClientGeneration, Result,
    },
};
use k8s_openapi::{
    api::{
        apps::v1::{ControllerRevision, StatefulSet},
        core::v1::{ConfigMap, Namespace, Node, Pod, Secret},
    },
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{
    api::{Api, DeleteParams, ListParams},
    core::PartialObjectMeta,
    Client, Resource, ResourceExt,
};
use serde::de::DeserializeOwned;
use snafu::{ensure, ResultExt};

/// Generate a new kube::Client.
pub async fn client() -> Result<Client> {
    Client::try_default().await.context(K8sClientGeneration)
}

/// Generate the Node api client.
pub async fn nodes_api() -> Result<Api<Node>> {
    Ok(Api::all(client().await?))
}

/// Generate the Namespace api client.
pub async fn namespaces_api() -> Result<Api<Namespace>> {
    Ok(Api::all(client().await?))
}

/// Generate the CustomResourceDefinition api client.
pub async fn crds_api() -> Result<Api<CustomResourceDefinition>> {
    Ok(Api::all(client().await?))
}

/// Generate the StatefulSet api client.
pub async fn sts_api(namespace: &str) -> Result<Api<StatefulSet>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate ControllerRevision api client.
pub async fn controller_revisions_api(namespace: &str) -> Result<Api<ControllerRevision>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate the Pod api client.
pub async fn pods_api(namespace: &str) -> Result<Api<Pod>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate the Secret api client.
pub async fn secrets_api(namespace: &str) -> Result<Api<Secret>> {
    Ok(Api::namespaced(client().await?, namespace))
}

/// Generate the Configmap api client.
pub async fn configmaps_api(namespace: &str) -> Result<Api<ConfigMap>> {
    Ok(Api::namespaced(client().await?, namespace))
}

async fn delete_sts(
    namespace: &str,
    delete_params: &DeleteParams,
    list_params: &ListParams,
) -> Result<()> {
    let sts_api = sts_api(namespace).await?;

    sts_api
        .delete_collection(delete_params, list_params)
        .await
        .map(|_| ())
        .or_else(|error| match error {
            // Handling NotFound case.
            kube::Error::Api(resp) if resp.code == 404 => Ok(()),
            something_else => Err(something_else),
        })
        .context(FailedToDeleteStatefulSet {
            namespace: namespace.to_string(),
        })
}

pub async fn delete_loki_sts(release_name: String, namespace: String) -> Result<()> {
    let label_selector = format!("app=loki,release={release_name}");

    let list_params = ListParams::default().labels(label_selector.as_str());
    let delete_params = DeleteParams::foreground();
    delete_sts(namespace.as_str(), &delete_params, &list_params).await
}

pub async fn list_pods(
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

    paginated_list(pods_api, &mut pods, Some(list_params)).await?;

    Ok(pods)
}

/// List the .metadata section of all CustomResourceDefinition resources in a paginated way.
pub async fn list_crds_metadata() -> Result<Vec<PartialObjectMeta<CustomResourceDefinition>>> {
    let mut crds: Vec<PartialObjectMeta<CustomResourceDefinition>> =
        Vec::with_capacity(KUBE_API_PAGE_SIZE as usize);

    paginated_list_metadata(crds_api().await?, &mut crds, None).await?;

    Ok(crds)
}

/// List Nodes metadata in the kubernetes cluster.
pub async fn list_nodes_metadata(
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

    paginated_list_metadata(nodes_api, &mut nodes, Some(list_params)).await?;

    Ok(nodes)
}

/// List ControllerRevisions in a Kubernetes namespace.
pub async fn list_controller_revisions(
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

    paginated_list(controller_revisions_api, &mut ctrl_revs, Some(list_params)).await?;

    Ok(ctrl_revs)
}

/// Returns the controller-revision-hash of the latest revision of a resource's ControllerRevisions.
pub async fn latest_controller_revision_hash(
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
pub async fn list_secrets(
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

    paginated_list(secrets_api, &mut secrets, Some(list_params)).await?;

    Ok(secrets)
}

/// This returns a list of ConfigMaps based on filtering criteria. Returns all if criteria is
/// absent.
pub async fn list_configmaps(
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

    paginated_list(configmaps_api, &mut configmaps, Some(list_params)).await?;

    Ok(configmaps)
}

/// GET the helm release secret for a helm release in a namespace.
pub async fn get_helm_release_secret(release_name: String, namespace: String) -> Result<Secret> {
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
pub async fn get_helm_release_configmap(
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

/// List Kubernetes resource object with pagination.
pub async fn paginated_list<K>(
    resource_api: Api<K>,
    resources: &mut Vec<K>,
    list_params: Option<ListParams>,
) -> Result<()>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug,
{
    let mut list_params = list_params.unwrap_or_default().limit(KUBE_API_PAGE_SIZE);

    loop {
        let resource_list = resource_api
            .list(&list_params)
            .await
            .context(FailedToListPaginated)?;

        let maybe_token = resource_list.metadata.continue_.clone();

        resources.extend(resource_list);

        match maybe_token {
            Some(ref token) if !token.is_empty() => {
                list_params = list_params.continue_token(token);
            }
            _ => break,
        }
    }

    Ok(())
}

/// Lists Kubernetes resource metadata section with pagination.
pub async fn paginated_list_metadata<K>(
    resource_api: Api<K>,
    resources: &mut Vec<PartialObjectMeta<K>>,
    list_params: Option<ListParams>,
) -> Result<()>
where
    K: Resource + Clone + DeserializeOwned + std::fmt::Debug,
{
    let mut list_params = list_params.unwrap_or_default().limit(KUBE_API_PAGE_SIZE);

    loop {
        let resource_list = resource_api
            .list_metadata(&list_params)
            .await
            .context(FailedToListMetadataPaginated)?;

        let maybe_token = resource_list.metadata.continue_.clone();

        resources.extend(resource_list);

        match maybe_token {
            Some(ref token) if !token.is_empty() => {
                list_params = list_params.continue_token(token);
            }
            _ => break,
        }
    }

    Ok(())
}
