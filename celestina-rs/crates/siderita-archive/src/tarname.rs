//! The one place a tar member's stored name becomes a path.
//!
//! Byte-exact by construction: a tar header carries bytes, not text, and a file
//! named with a non-UTF-8 byte survives a round trip through this domain exactly
//! as ADR 0008 requires of every other path in the suite.

use std::path::PathBuf;

use crate::error::ArchiveError;

/// The stored name of `entry` as a path, without lossy conversion.
pub(crate) fn path_of<R: std::io::Read>(
    entry: &tar::Entry<'_, R>,
) -> Result<PathBuf, ArchiveError> {
    Ok(from_bytes(&entry.path_bytes()))
}

/// The link target of `entry`, when it stores one.
pub(crate) fn link_target_of<R: std::io::Read>(entry: &tar::Entry<'_, R>) -> Option<PathBuf> {
    entry.link_name_bytes().map(|bytes| from_bytes(&bytes))
}

/// A stored byte name as a path (also the zip symlink-target decoder).
pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    from_bytes(bytes)
}

#[cfg(unix)]
fn from_bytes(bytes: &[u8]) -> PathBuf {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}
