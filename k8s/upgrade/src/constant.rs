/// This is used to create labels for the upgrade job.
#[macro_export]
macro_rules! upgrade_labels {
    () => {
        btreemap! {
           "app" => UPGRADE_JOB_NAME_SUFFIX,
        }
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    };
}

/// Append the release name to k8s objects.
pub(crate) fn upgrade_name_concat(release_name: &str, component_name: &str) -> String {
    format!("{release_name}-{component_name}")
}

/// Append the tag to image in upgrade.
pub(crate) fn upgrade_image_concat(image_repo: &str, image_name: &str, image_tag: &str) -> String {
    format!("{image_repo}/{image_name}:{image_tag}")
}

/// Upgrade job container image tag.
pub(crate) const UPGRADE_JOB_IMAGE_TAG: &str = "develop";
/// Upgrade job container image repository.
pub(crate) const UPGRADE_JOB_IMAGE_REPO: &str = "openebs";
/// Upgrade job container image name.
pub(crate) const UPGRADE_JOB_IMAGE_NAME: &str = "mayastor-upgrade-job";
/// Upgrade job name suffix.
pub(crate) const UPGRADE_JOB_NAME_SUFFIX: &str = "upgrade";
/// ServiceAccount name suffix for upgrade job.
pub(crate) const UPGRADE_JOB_SERVICEACCOUNT_NAME_SUFFIX: &str = "upgrade-service-account";
/// ClusterRole name suffix for upgrade job.
pub(crate) const UPGRADE_JOB_CLUSTERROLE_NAME_SUFFIX: &str = "upgrade-role";
/// ClusterRoleBinding for upgrade job.
pub(crate) const UPGRADE_JOB_CLUSTERROLEBINDING_NAME_SUFFIX: &str = "upgrade-role-binding";
/// Upgrade job binary name.
pub(crate) const UPGRADE_BINARY_NAME: &str = "upgrade-job";
/// Upgrade job container name.
pub(crate) const UPGRADE_JOB_CONTAINER_NAME: &str = "mayastor-upgrade-job";
/// Defines the Label select for mayastor REST API.
pub(crate) const API_REST_LABEL_SELECTOR: &str = "app=api-rest";
/// Defines the default helm chart release name.
pub(crate) const DEFAULT_RELEASE_NAME: &str = "mayastor";
/// Volumes with one replica
pub(crate) const SINGLE_REPLICA_VOLUME: u8 = 1;
/// IO_ENGINE_POD_LABEL is the Kubernetes Pod label set on mayastor-io-engine Pods.
pub(crate) const IO_ENGINE_POD_LABEL: &str = "app=io-engine";
/// AGENT_CORE_POD_LABEL is the Kubernetes Pod label set on mayastor-agent-core Pods.
pub(crate) const AGENT_CORE_POD_LABEL: &str = "app=agent-core";
/// API_REST_POD_LABEL is the Kubernetes Pod label set on mayastor-api-rest Pods.
pub(crate) const API_REST_POD_LABEL: &str = "app=api-rest";
