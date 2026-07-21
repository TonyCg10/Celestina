use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::error::OpError;
use crate::name::validate_name;

/// Creates a new directory `name` inside `parent`, returning its full path.
///
/// One level only — `parent` must already exist. The operation never overwrites:
/// if the target exists it is left untouched and [`OpError::AlreadyExists`] is
/// returned. `create_dir` reports the collision atomically, so there is no
/// check-then-create race.
pub fn create_directory(
    parent: &Path,
    name: &OsStr,
    cancellation: &CancellationToken,
) -> Result<PathBuf, OpError> {
    let target = plan_target(parent, name, cancellation)?;
    match fs::create_dir(&target) {
        Ok(()) => Ok(target),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(OpError::AlreadyExists { path: target })
        }
        Err(error) => Err(OpError::io(&target, &error)),
    }
}

/// Creates a new empty file `name` inside `parent`, returning its full path.
///
/// Uses create-new semantics: an existing file is never opened for truncation,
/// so no content is lost — the collision is reported as
/// [`OpError::AlreadyExists`] instead.
pub fn create_file(
    parent: &Path,
    name: &OsStr,
    cancellation: &CancellationToken,
) -> Result<PathBuf, OpError> {
    let target = plan_target(parent, name, cancellation)?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
    {
        Ok(_) => Ok(target),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            Err(OpError::AlreadyExists { path: target })
        }
        Err(error) => Err(OpError::io(&target, &error)),
    }
}

/// Validates the name and joins it onto the parent, after an early cancellation
/// check so a tripped token never reaches the filesystem.
fn plan_target(
    parent: &Path,
    name: &OsStr,
    cancellation: &CancellationToken,
) -> Result<PathBuf, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    validate_name(name)?;
    Ok(parent.join(name))
}
