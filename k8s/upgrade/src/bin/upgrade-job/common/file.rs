use crate::common::error::{Result, TempFileCreation, WriteToTempFile};
use snafu::ResultExt;
use std::{io::Write, path::Path};
use tempfile::NamedTempFile as TempFile;

/// Write buffer to an existing temporary file if a path is provided as an argument, else
/// create a new temporary file and write to it. Returns the file handle.
pub(crate) fn write_to_tempfile<P>(file_dir: Option<P>, buf: &[u8]) -> Result<TempFile>
where
    P: AsRef<Path>,
{
    let mut handle: TempFile = match file_dir {
        Some(dir) => TempFile::new_in(dir),
        None => TempFile::new(),
    }
    .context(TempFileCreation)?;

    handle.write(buf).context(WriteToTempFile {
        filepath: handle.path().to_path_buf(),
    })?;

    Ok(handle)
}
