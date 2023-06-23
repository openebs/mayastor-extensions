use std::{
    env,
    path::{Path, PathBuf},
};
use utils::version_info;

/// PRODUCT is the name of the project for which this call-home component is deployed.
pub const PRODUCT: &str = "Mayastor";

/// Label for release name.
pub const HELM_RELEASE_NAME_LABEL: &str = "openebs.io/release";

/// Defines the default helm chart release name.
pub const DEFAULT_RELEASE_NAME: &str = "mayastor";

/// Defines the Label select for mayastor REST API.
pub const API_REST_LABEL_SELECTOR: &str = "app=api-rest";

/// Defines the Label key for event store.
pub const EVENT_STORE_LABLE_KEY: &str = "app";

/// Defines the suffix name for event store.
pub const EVENT_STORE: &str = "event-store";

/// Defines the key for comfig map.
pub const EVENT_STATS_DATA: &str = "stats";

/// Defines the help argument for volume stats need for promethueus library.
pub const VOLUME_STATS: &str = "Volume stats";

/// Defines the help argument for pool stats need for promethueus library.
pub const POOL_STATS: &str = "Pool stats";

/// Variable label for promethueus library.
pub const ACTION: &str = "action";

/// Create action for events.
pub const CREATED: &str = "created";

/// Delete actions for events.
pub const DELETED: &str = "deleted";

/// Field manager for Patch param, required for [`Patch::Apply`].
pub const PATCH_PARAM_FILED_MANAGER: &str = "events_store_configmap";

/// DEFAULT_ENCRYPTION_DIR_PATH is the directory path for the temporary files generated during
/// encryption.
/// The function encryption_dir() returns the user defined directory path for the encryption
/// dir as an &str.
const DEFAULT_ENCRYPTION_DIR_PATH: &str = "./";

pub fn encryption_dir() -> PathBuf {
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
const DEFAULT_ENCRYPTION_KEY_FILEPATH: &str = "./public.gpg";

pub fn key_filepath() -> PathBuf {
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
pub const RECEIVER_ENDPOINT: &str = "https://openebs.phonehome.datacore.com/openebs/report";

/// CALL_HOME_FREQUENCY_IN_HOURS is the frequency of call-home metrics transmission, in hours.
/// The function call_home_frequency() returns the frequency as an std::time::Duration.

const CALL_HOME_FREQUENCY_IN_HOURS: i64 = 24;

pub fn call_home_frequency() -> std::time::Duration {
    chrono::Duration::hours(CALL_HOME_FREQUENCY_IN_HOURS)
        .to_std()
        .map_err(|error| {
            anyhow::anyhow!("failed to parse call-home frequency duration: {:?}", error)
        })
        .unwrap()
}

/// Returns the git tag version (if tag is found) or simply returns the commit hash (12 characters).
pub fn release_version() -> String {
    let version_info = version_info!();
    match version_info.version_tag {
        Some(tag) => tag,
        None => version_info.commit_hash,
    }
}
