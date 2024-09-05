use crate::common::{
    constants::KUBE_API_PAGE_SIZE,
    error::{
        ControllerRevisionDoesntHaveHashLabel, ControllerRevisionListEmpty, K8sClientGeneration,
        ListCtrlRevsWithLabelAndField, ListNodesWithLabelAndField, ListPodsWithLabelAndField,
        Result,
    },
};
use k8s_openapi::{
    api::{
        apps::v1::ControllerRevision,
        core::v1::{Namespace, Node, Pod},
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
