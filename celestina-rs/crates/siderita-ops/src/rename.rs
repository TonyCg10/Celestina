use std::ffi::OsStr;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::error::OpError;
use crate::name::validate_name;

/// The paths a successful rename moved between.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Renamed {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Renames the entry at `path` to `new_name`, within its own parent directory.
///
/// The operation is loss-free by refusing to clobber: if a *distinct* entry
/// already holds the target name it is left untouched and
/// [`OpError::AlreadyExists`] is returned. Renaming an entry to a name that
/// resolves to the same underlying entry — a case- or encoding-only change on a
/// case-insensitive filesystem — is allowed, as is a no-op rename to the
/// identical name. The source must still exist ([`OpError::SourceMissing`]).
///
/// There is a small check-then-rename window: `std::fs::rename` would otherwise
/// overwrite silently, and a no-clobber rename syscall needs `unsafe` FFI this
/// crate forbids. For a single-user manager the guard is sufficient; the window
/// is documented rather than hidden.
pub fn rename(
    path: &Path,
    new_name: &OsStr,
    cancellation: &CancellationToken,
) -> Result<Renamed, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    validate_name(new_name)?;

    let parent = path.parent().ok_or_else(|| OpError::Io {
        path: path.to_path_buf(),
        kind: io::ErrorKind::InvalidInput,
        message: "the path has no parent to rename within".to_owned(),
    })?;
    let target = parent.join(new_name);

    let source = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing {
                path: path.to_path_buf(),
            });
        }
        Err(error) => return Err(OpError::io(path, &error)),
    };

    // A rename to the byte-identical path is a no-op once the source is known
    // to exist.
    if target == path {
        return Ok(Renamed {
            from: path.to_path_buf(),
            to: target,
        });
    }

    match fs::symlink_metadata(&target) {
        // A distinct entry already owns the target name: never destroy it.
        Ok(existing) if !same_entry(&source, &existing) => {
            return Err(OpError::AlreadyExists { path: target });
        }
        // Same underlying entry (case-/encoding-only change) — fall through.
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(OpError::io(&target, &error)),
    }

    match fs::rename(path, &target) {
        Ok(()) => Ok(Renamed {
            from: path.to_path_buf(),
            to: target,
        }),
        Err(error) => Err(OpError::io(&target, &error)),
    }
}

/// Whether two metadata records name the same underlying entry.
#[cfg(unix)]
fn same_entry(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_entry(_left: &Metadata, _right: &Metadata) -> bool {
    // Without dev/inode, treat any existing distinct-path target as a conflict.
    false
}
