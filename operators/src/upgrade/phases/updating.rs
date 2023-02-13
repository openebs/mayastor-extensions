use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::debug;

use crate::upgrade::{
    common::{constants::DEFAULT_VALUES_PATH, error::Error},
    config::UpgradeConfig,
};

/// Start updating components.
pub async fn components_update(opts: Vec<(String, String)>) -> Result<(), Error> {
    let file = tempfile::NamedTempFile::new_in(DEFAULT_VALUES_PATH)?;
    let output = UpgradeConfig::get_config()
        .helm_client()
        .get_values()
        .unwrap();
    debug!("{:?}", output);

    let path = file.into_temp_path();
    let output_filepath: &Path = path.as_ref();
    fs::write(output_filepath, output)?;

    UpgradeConfig::get_config()
        .helm_client()
        .upgrade(vec![PathBuf::from(output_filepath)], opts)
        .await
}
