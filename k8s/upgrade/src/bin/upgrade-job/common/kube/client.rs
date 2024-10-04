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
    Client, ResourceExt,
};
use snafu::{ensure, ResultExt};

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

    let list_pods_error_ctx = ListPodsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    loop {
        let pod_list = pods_api(namespace.as_str())
            .await?
            .list(&list_params)
            .await
            .context(list_pods_error_ctx.clone())?;

        let continue_ = pod_list.metadata.continue_.clone();

        pods.extend(pod_list);

        match continue_ {
            Some(token) => {
                list_params = list_params.continue_token(token.as_str());
            }
            None => break,
        }
    }

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

    let list_nodes_error_ctx = ListNodesWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
    };

    loop {
        let nodes_list = nodes_api()
            .await?
            .list_metadata(&list_params)
            .await
            .context(list_nodes_error_ctx.clone())?;

        let maybe_token = nodes_list.metadata.continue_.clone();

        nodes.extend(nodes_list);

        match maybe_token {
            Some(ref token) => {
                list_params = list_params.continue_token(token);
            }
            None => break,
        }
    }

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

    let list_ctrl_revs_error_ctx = ListCtrlRevsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    loop {
        let ctrl_revs_list = controller_revisions_api(namespace.as_str())
            .await?
            .list(&list_params)
            .await
            .context(list_ctrl_revs_error_ctx.clone())?;

        let maybe_token = ctrl_revs_list.metadata.continue_.clone();

        ctrl_revs.extend(ctrl_revs_list);

        match maybe_token {
            Some(ref token) => {
                list_params = list_params.continue_token(token);
            }
            None => break,
        }
    }

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

    let list_secrets_error = ListSecretsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    loop {
        let secrets_list = secrets_api(namespace.as_str())
            .await?
            .list(&list_params)
            .await
            .context(list_secrets_error.clone())?;

        let maybe_token = secrets_list.metadata.continue_.clone();

        secrets.extend(secrets_list);

        match maybe_token {
            Some(ref token) => {
                list_params = list_params.continue_token(token);
            }
            None => break,
        }
    }

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

    let list_configmaps_error = ListConfigMapsWithLabelAndField {
        label: label_selector.unwrap_or_default(),
        field: field_selector.unwrap_or_default(),
        namespace: namespace.clone(),
    };

    loop {
        let configmaps_list = configmaps_api(namespace.as_str())
            .await?
            .list(&list_params)
            .await
            .context(list_configmaps_error.clone())?;

        let maybe_token = configmaps_list.metadata.continue_.clone();

        configmaps.extend(configmaps_list);

        match maybe_token {
            Some(ref token) => {
                list_params = list_params.continue_token(token);
            }
            None => break,
        }
    }

    Ok(configmaps)
}

/// GET the helm release secret for a helm release in a namespace.
pub(crate) async fn get_helm_release_secret(
    release_name: String,
    namespace: String,
) -> Result<Secret> {
    let secrets = list_secrets(
        namespace.clone(),
        Some(format!("name={}", release_name.as_str())),
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
        Some(format!("name={},owner=helm", release_name.as_str())),
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
