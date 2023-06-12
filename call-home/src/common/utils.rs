use crate::common::{
    constants::{API_REST_LABEL_SELECTOR, DEFAULT_RELEASE_NAME, HELM_RELEASE_NAME_LABEL},
    errors,
};
use k8s_openapi::api::apps::v1::Deployment;
use kube::{
    api::{Api, ListParams},
    Client,
};
use snafu::ResultExt;

/// Return the release name.
#[allow(dead_code)]
pub(crate) async fn get_release_name(ns: &str, client: Client) -> errors::Result<String> {
    let deployment = get_deployment_for_rest(ns, client).await?;
    match &deployment.metadata.labels {
        Some(label) => match label.get(HELM_RELEASE_NAME_LABEL) {
            Some(value) => Ok(value.to_string()),
            None => Ok(DEFAULT_RELEASE_NAME.to_string()),
        },
        None => Ok(DEFAULT_RELEASE_NAME.to_string()),
    }
}

/// Return results as list of deployments.
#[allow(dead_code)]
pub(crate) async fn get_deployment_for_rest(
    ns: &str,
    client: Client,
) -> errors::Result<Deployment> {
    let deployment = Api::<Deployment>::namespaced(client, ns);
    let lp = ListParams::default().labels(API_REST_LABEL_SELECTOR);
    let deployment_list = deployment
        .list(&lp)
        .await
        .context(errors::ListDeploymantsWithLabel {
            label: API_REST_LABEL_SELECTOR.to_string(),
            namespace: ns.to_string(),
        })?;
    let deployment = deployment_list
        .items
        .first()
        .ok_or(errors::NoDeploymentPresent.build())?
        .clone();
    Ok(deployment)
}
