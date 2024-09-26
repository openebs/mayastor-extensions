use heck::ToTrainCase;
use utils::constants::PRODUCT_DOMAIN_NAME;
pub use utils::PRODUCT_NAME;

/// Name of the product.
pub fn product_train() -> String {
    PRODUCT_NAME.to_train_case()
}

/// Helm release name label's key.
pub fn helm_release_name_key() -> String {
    format!("{PRODUCT_DOMAIN_NAME}/release")
}

/// Upgrade job container image name.
pub fn upgrade_job_img() -> String {
    format!("{PRODUCT_NAME}-upgrade-job")
}
/// Upgrade job container image name.
pub fn upgrade_job_container_name() -> String {
    format!("{PRODUCT_NAME}-upgrade-job")
}
/// UPGRADE_EVENT_REASON is the reason field in upgrade job.
pub fn upgrade_event_reason() -> String {
    format!("{}Upgrade", product_train())
}
/// Upgrade job container image repository.
pub const UPGRADE_JOB_IMAGE_REPO: &str = "openebs";
/// This is the user docs URL for the Umbrella chart.
pub const UMBRELLA_CHART_UPGRADE_DOCS_URL: &str = "https://openebs.io/docs/user-guides/upgrade";

/// Defines the default helm chart release name.
pub const DEFAULT_RELEASE_NAME: &str = PRODUCT_NAME;

/// Installed release version.
pub fn helm_release_version_key() -> String {
    format!("{PRODUCT_DOMAIN_NAME}/version")
}

/// Loki Logging key.
pub fn loki_logging_key() -> String {
    format!("{PRODUCT_DOMAIN_NAME}/logging")
}

/// This is the name of the Helm chart which included the core chart as a sub-chart.
/// Under the hood, this installs the Core Helm chart (see below).
pub const UMBRELLA_CHART_NAME: &str = "openebs";

/// RECEIVER_API_ENDPOINT is the URL to anonymous call-home metrics collection endpoint.
pub const CALL_HOME_ENDPOINT: &str = "https://openebs.phonehome.datacore.com/openebs/report";

/// Label key containing controller revision hash for a controller resource for DaemonSets.
pub const DS_CONTROLLER_REVISION_HASH_LABEL_KEY: &str = "controller-revision-hash";
