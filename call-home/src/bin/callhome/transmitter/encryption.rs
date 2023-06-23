use crate::collector::report_models::Report;
use obs::common::errors::EncryptError;
use rand::{distributions::Alphanumeric, Rng};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::debug;

/// 'encrypt' accepts a crate::collector::Report, marshals it into JSON and encrypts it.
pub(crate) fn encrypt(
    report: &Report,
    encryption_dir: &PathBuf,
    key_filepath: &Path,
) -> Result<Vec<u8>, EncryptError> {
    // The underlying filesystem resource is garbage collected
    // when this function returns.
    let temp_file = tempfile::NamedTempFile::new_in(encryption_dir)?;
    debug!("Successfully created temporary input file.");

    let input_filepath = temp_file.into_temp_path();
    let report_content = serde_json::to_vec(report)?;
    fs::write(&input_filepath, report_content)?;
    debug!("Successfully written Report data to temporary input file.");

    let random_name: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let output_filepath = Path::new(encryption_dir).join(random_name + ".gpg");

    // TODO: Use a library instead of the gpg binary.
    let command = format!("gpg --yes --trust-model=always --homedir={} --keyring={} --recipient=openebs-phonehome@datacore.com --no-default-keyring --encrypt -z=9 --output={} {}",
                          encryption_dir.to_string_lossy(),
                          key_filepath.to_string_lossy(),
                          output_filepath.to_string_lossy(),
                          input_filepath.to_string_lossy());
    let _ = Command::new("sh").args(["-c", command.trim()]).output()?;
    debug!("Successfully executed gpg command.");

    let output = fs::read(&output_filepath)?;
    fs::remove_file(&output_filepath)?;
    debug!("Successfully deleted output file.");

    Ok(output)
}
