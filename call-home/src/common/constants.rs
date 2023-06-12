use std::{
    env,
    path::{Path, PathBuf},
};
use utils::version_info;

/// PRODUCT is the name of the project for which this call-home component is deployed.
#[allow(dead_code)]
pub(crate) const PRODUCT: &str = "Mayastor";

/// Label for release name.
#[allow(dead_code)]
pub(crate) const HELM_RELEASE_NAME_LABEL: &str = "openebs.io/release";

/// Defines the default helm chart release name.
#[allow(dead_code)]
pub(crate) const DEFAULT_RELEASE_NAME: &str = "mayastor";

/// Defines the Label select for mayastor REST API.
#[allow(dead_code)]
pub(crate) const API_REST_LABEL_SELECTOR: &str = "app=api-rest";

/// Defines the Label key for event store.
#[allow(dead_code)]
pub(crate) const EVENT_STORE_LABLE_KEY: &str = "app";

/// Defines the suffix name for event store.
#[allow(dead_code)]
pub(crate) const EVENT_STORE: &str = "event-store";

/// Defines the key for comfig map.
#[allow(dead_code)]
pub(crate) const EVENT_STATS_DATA: &str = "stats";

/// Defines the key for comfig map.
#[allow(dead_code)]
pub(crate) const DEFAULT_VALUE_EVENT_SET: &str = r#"{"pool":{"pool_created":0,"pool_deleted":0},"volume":{"volume_created":0,"volume_deleted":0}}"#;     

/// Field manager for Patch param, required for [`Patch::Apply`].
#[allow(dead_code)]
pub(crate) const PATCH_PARAM_FILED_MANAGER: &str = "events_store_configmap";

/// DEFAULT_ENCRYPTION_DIR_PATH is the directory path for the temporary files generated during
/// encryption.
/// The function encryption_dir() returns the user defined directory path for the encryption
/// dir as an &str.
#[allow(dead_code)]
const DEFAULT_ENCRYPTION_DIR_PATH: &str = "./";

#[allow(dead_code)]
pub(crate) fn encryption_dir() -> PathBuf {
    const KEY: &str = "ENCRYPTION_DIR";
    match env::var(KEY) {
        Ok(input) => {
            let path = Path::new(&input);
            // Validate path.
            match path.exists() && path.is_dir() {
                true => path.to_path_buf(),
                false => {
                    panic!("validation failed for {} value \"{}\": path must exist and must be that of a directory", KEY, &input);
                }
            }
        }
        Err(_) => Path::new(DEFAULT_ENCRYPTION_DIR_PATH).to_path_buf(),
    }
}

/// DEFAULT_ENCRYPTION_KEY_FILEPATH is the path to the encryption key.
/// The function key_filepath() returns the user defined path for the encryption key.
#[allow(dead_code)]
const DEFAULT_ENCRYPTION_KEY_FILEPATH: &str = "./public.gpg";

#[allow(dead_code)]
pub(crate) fn key_filepath() -> PathBuf {
    const KEY: &str = "KEY_FILEPATH";
    match env::var(KEY) {
        Ok(input) => {
            let path = Path::new(&input);
            // Validate path.
            match path.exists() && path.is_file() {
                true => path.to_path_buf(),
                false => {
                    panic!("validation failed for {} value \"{}\": path must exist and must be that of a file", KEY, &input);
                }
            }
        }
        Err(_) => Path::new(DEFAULT_ENCRYPTION_KEY_FILEPATH).to_path_buf(),
    }
}

/// RECEIVER_API_ENDPOINT is the URL to anonymous call-home metrics collection endpoint.
#[allow(dead_code)]
pub(crate) const RECEIVER_ENDPOINT: &str = "https://openebs.phonehome.datacore.com/openebs/report";

/// CALL_HOME_FREQUENCY_IN_HOURS is the frequency of call-home metrics transmission, in hours.
/// The function call_home_frequency() returns the frequency as an std::time::Duration.

#[allow(dead_code)]
const CALL_HOME_FREQUENCY_IN_HOURS: i64 = 24;

#[allow(dead_code)]
pub(crate) fn call_home_frequency() -> std::time::Duration {
    chrono::Duration::hours(CALL_HOME_FREQUENCY_IN_HOURS)
        .to_std()
        .map_err(|error| {
            anyhow::anyhow!("failed to parse call-home frequency duration: {:?}", error)
        })
        .unwrap()
}

#[allow(dead_code)]
/// Returns the git tag version (if tag is found) or simply returns the commit hash (12 characters).
pub(crate) fn release_version() -> String {
    let version_info = version_info!();
    match version_info.version_tag {
        Some(tag) => tag,
        None => version_info.commit_hash,
    }
}
