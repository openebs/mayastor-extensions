use std::{
    env,
    path::{Path, PathBuf},
};

/// PRODUCT is the name of the project for which this call-home component is deployed.
pub(crate) const PRODUCT: &str = "Mayastor";

/// DEFAULT_ENCRYPTION_DIR_PATH is the directory path for the temporary files generated during
/// encryption.
/// The function encryption_dir() returns the user defined directory path for the encryption
/// dir as an &str.
const DEFAULT_ENCRYPTION_DIR_PATH: &str = "./";
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
const DEFAULT_ENCRYPTION_KEY_FILEPATH: &str = "./public.gpg";
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
pub(crate) const RECEIVER_ENDPOINT: &str = "https://openebs.phonehome.datacore.com/openebs/report";

/// CALL_HOME_FREQUENCY_IN_HOURS is the frequency of call-home metrics transmission, in hours.
/// The function call_home_frequency() returns the frequency as an std::time::Duration.
const CALL_HOME_FREQUENCY_IN_HOURS: i64 = 24;
pub(crate) fn call_home_frequency() -> std::time::Duration {
    chrono::Duration::hours(CALL_HOME_FREQUENCY_IN_HOURS)
        .to_std()
        .map_err(|error| {
            anyhow::anyhow!("failed to parse call-home frequency duration: {:?}", error)
        })
        .unwrap()
}

/// Release version
pub(crate) const RELEASE_VERSION: &str = "2.0.1";
