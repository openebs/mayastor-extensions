use crate::Report;
use mktemp::Temp;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::json;
use std::{
    fs,
    io::{Error, ErrorKind},
    process::Command,
};
use tracing::debug;

/// 'encrypt' accepts a crate::collector::Report, marshals it into JSON and encrypts it.
pub(crate) fn encrypt(
    report: &Report,
    encryption_dir: &String,
    key_filepath: &String,
) -> Result<Vec<u8>, Error> {
    // The underlying filesystem resource is garbage collected
    // when this function returns.
    // Ref: https://docs.rs/mktemp/0.4.1/mktemp/index.html
    let temp_file = Temp::new_file_in(encryption_dir)?;
    debug!("Successfully created temporary input file.");

    let path_buffer = temp_file.to_path_buf();
    let input_filepath: &str = path_buffer
        .to_str()
        .ok_or_else(|| Error::new(ErrorKind::Other, "Error converting Pathbuf to &str"))?;
    fs::write(
        input_filepath,
        <String as AsRef<[u8]>>::as_ref(&json!(report).to_string()),
    )?;
    debug!("Successfully written Report data to temporary input file.");

    let random_name: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let output_filepath = encryption_dir.clone() + "/" + &random_name + ".gpg";

    // TODO: Use a library instead of the gpg binary.
    let command = format!("gpg --yes --trust-model=always --homedir={} --keyring={} --recipient=product-health-report@caringo.com --no-default-keyring --encrypt -z=9 --output={} {}",
                          encryption_dir, key_filepath,
                          &output_filepath,
                          &input_filepath);
    let _ = Command::new("sh").args(&["-c", command.trim()]).output()?;
    debug!("Successfully executed gpg command.");

    let output = fs::read(&output_filepath)?;
    fs::remove_file(&output_filepath)?;
    debug!("Successfully deleted output file.");

    Ok(output)
}
