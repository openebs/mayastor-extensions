use std::{
    fs,
    path::{Path, PathBuf},
};

use mktemp::Temp;
use rand::{distributions::Alphanumeric, Rng};

use crate::upgrade::{
    common::{constants::DEFAULT_VALUES_PATH, error::Error},
    config::UpgradeOperatorConfig,
};

// Create a new file to store values.
pub(crate) fn values_pathfile() -> Result<PathBuf, Error> {
    let _ = Temp::new_file_in(DEFAULT_VALUES_PATH)?;

    let random_name: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let output_filepath = Path::new(DEFAULT_VALUES_PATH).join(random_name + ".yaml");

    Ok(output_filepath)
}

/// Start updating components.
pub async fn components_update(opts: Vec<(String, String)>) -> Result<(), Error> {
    let output_filepath = values_pathfile().unwrap();
    let output = UpgradeOperatorConfig::get_config()
        .helm_client()
        .get_values()
        .unwrap();
    println!("{:?}", output);

    fs::write(output_filepath.clone(), output).expect("Unable to write values in the yaml file");

    match UpgradeOperatorConfig::get_config()
        .helm_client()
        .upgrade(vec![output_filepath.clone()], opts)
        .await
    {
        Ok(_) => {
            fs::remove_file(&output_filepath)?;
            Ok(())
        }
        Err(err) => {
            fs::remove_file(&output_filepath)?;
            Err(err)
        }
    }
}
