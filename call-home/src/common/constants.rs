use heck::ToTrainCase;
use std::{
    env,
    path::{Path, PathBuf},
};
use utils::version_info;

/// CALLHOME_PRODUCT_NAME_ENV is the name of the ENV which configures the product-name for callhome.
pub const CALLHOME_PRODUCT_NAME_ENV: &str = "CALLHOME_PRODUCT_NAME";

/// PRODUCT is the name of the project for which this call-home component is deployed.
pub fn product() -> String {
    env::var(CALLHOME_PRODUCT_NAME_ENV)
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| v.to_train_case())
        .unwrap_or(::constants::product_train())
}

/// Defines the default helm chart release name.
pub const DEFAULT_RELEASE_NAME: &str = ::constants::DEFAULT_RELEASE_NAME;

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

/// Defines the help argument for nexus stats need for promethueus library.
pub const NEXUS_STATS: &str = "Nexus stats";

/// Variable label for prometheus library.
pub const ACTION: &str = "action";

/// Create action for events.
pub const CREATED: &str = "created";

/// Delete actions for events.
pub const DELETED: &str = "deleted";

/// Rebuild started action for events.
pub const REBUILD_STARTED: &str = "rebuild_started";

/// rebuild ended action for events.
pub const REBUILD_ENDED: &str = "rebuild_ended";

/// Label for Volume.
pub const VOLUME: &str = "volume";

/// Label for Pool.
pub const POOL: &str = "pool";

/// Label for Nexus.
pub const NEXUS: &str = "nexus";

/// Field manager for Patch param, required for [`Patch::Apply`].
pub const PATCH_PARAM_FILED_MANAGER: &str = "events_store_configmap";

/// Default mbus url.
pub const DEFAULT_MBUS_URL: &str = "nats://mayastor-nats:4222";

/// Defines the default namespace.
pub const DEFAULT_NAMESPACE: &str = "mayastor";

/// Number of bytes in a disk sector.
pub const BYTES_PER_SECTOR: u64 = 512;

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
pub const RECEIVER_ENDPOINT: &str = ::constants::CALL_HOME_ENDPOINT;

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_product() {
        use crate::common::constants::{product, CALLHOME_PRODUCT_NAME_ENV};
        use std::env::{remove_var as unset, set_var as set};

        set(CALLHOME_PRODUCT_NAME_ENV, "ma8");
        assert_eq!(product(), "Ma8".to_string());

        set(CALLHOME_PRODUCT_NAME_ENV, "foo bar");
        assert_eq!(product(), "Foo-Bar".to_string());

        unset(CALLHOME_PRODUCT_NAME_ENV);
        assert_eq!(product(), "Mayastor".to_string());

        set(CALLHOME_PRODUCT_NAME_ENV, "");
        assert_eq!(product(), "Mayastor".to_string());
    }
}
