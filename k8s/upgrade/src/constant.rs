use utils::version_info;

/// This is used to create labels for the upgrade job.
#[macro_export]
macro_rules! upgrade_labels {
    () => {
        btreemap! {
           "app" => UPGRADE_JOB_NAME_SUFFIX,
           "openebs.io/logging" => "true",
        }
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
    };
}

/// Append the release name to k8s objects.
pub(crate) fn upgrade_name_concat(
    release_name: &str,
    component_name: &str,
    upgrade_to_branch: Option<&String>,
) -> String {
    let version = match upgrade_to_branch {
        Some(tag) => tag.to_string(),
        None => upgrade_obj_suffix(),
    };
    format!("{release_name}-{component_name}-{version}")
}

/// Fetch the image tag to append to upgrade resources.
pub(crate) fn upgrade_obj_suffix() -> String {
    let version = match release_version() {
        Some(upgrade_job_image_tag) => upgrade_job_image_tag,
        None => UPGRADE_JOB_IMAGE_TAG.to_string(),
    };
    version.replace('.', "-")
}

/// Fetch the image tag for upgrade job's pod.
pub(crate) fn get_image_version_tag() -> String {
    let version = release_version();
    match version {
        Some(upgrade_job_image_tag) => upgrade_job_image_tag,
        None => UPGRADE_JOB_IMAGE_TAG.to_string(),
    }
}

/// Returns the git tag version (if tag is found) or simply returns the commit hash (12 characters).
pub(crate) fn release_version() -> Option<String> {
    let version_info = version_info!();
    println!("version_info {:#?}", version_info);
    version_info.version_tag
}

/// Append the tag to image in upgrade.
pub(crate) fn upgrade_image_concat(
    image_registry: &str,
    image_repo: &str,
    image_name: &str,
    image_tag: &str,
) -> String {
    format!("{image_registry}/{image_repo}/{image_name}:{image_tag}")
}

/// Append the release name to k8s objects.
pub(crate) fn upgrade_event_selector(release_name: &str, component_name: &str) -> String {
    let kind = "involvedObject.kind=Job";
    let name_key = "involvedObject.name";
    let tag = upgrade_obj_suffix();
    let name_value = format!("{release_name}-{component_name}-{tag}");
    format!("{kind},{name_key}={name_value}")
}
/// Installed release name.
pub(crate) const HELM_RELEASE_NAME_LABEL: &str = "openebs.io/release";
/// Installed release version.
pub(crate) const HELM_RELEASE_VERSION_LABEL: &str = "openebs.io/version";
/// Default image repository.
pub(crate) const DEFAULT_IMAGE_REGISTRY: &str = "docker.io";
/// The upgrade job will use the UPGRADE_JOB_IMAGE_NAME image (below) with this tag.
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
/// UPGRADE_EVENT_REASON is the reason field in upgrade job.
pub(crate) const UPGRADE_EVENT_REASON: &str = "MayastorUpgrade";

/// This is the allowed upgrade to-version/to-version-range for the Core chart.
pub(crate) const TO_CORE_SEMVER: &str = ">=2.2.0-rc.0, <=2.2.0";

/// This is the allowed upgrade to-version/to-version-range for the Core chart.
pub(crate) const TO_DEVELOP_SEMVER: &str = "0.0.0";

/// This version range will be only allowed to upgrade to TO_CORE_SEMVER above. This range applies
/// to the Core chart.
pub(crate) const FROM_CORE_SEMVER: &str = ">=2.0.0, <=2.1.0";
