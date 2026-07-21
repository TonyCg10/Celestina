use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::error::OpError;

/// Bytes moved per read/write step; also the cancellation granularity for a
/// single large file.
const CHUNK: usize = 64 * 1024;

/// Cumulative progress of a copy (or the copy half of a cross-device move).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Progress {
    /// Bytes of file content written so far.
    pub bytes: u64,
    /// Entries (files, directories, symlinks) finished so far.
    pub items: u64,
}

/// Copies `source` — a file, directory tree or symlink — into `into_dir`,
/// keeping the source's own file name, and returns the created destination.
///
/// Loss-free by construction: it refuses to overwrite an existing destination
/// ([`OpError::AlreadyExists`]) and removes nothing. A copy that is cancelled or
/// fails part-way rolls back the partial destination, so a half copy is never
/// left behind claiming to be complete. Symlinks are copied as links, never
/// followed. `progress` receives the running total after each chunk and item.
pub fn copy(
    source: &Path,
    into_dir: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf, OpError> {
    let destination = plan_destination(source, into_dir)?;
    copy_to(source, &destination, cancellation, progress)?;
    Ok(destination)
}

/// Copies `source` onto the exact, non-existent path `destination`, rolling the
/// partial destination back on any failure. Shared with the cross-device move.
pub(crate) fn copy_to(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), OpError> {
    let mut context = CopyContext {
        cancel: cancellation,
        progress,
        total: Progress::default(),
    };
    match copy_tree(&mut context, source, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            rollback(destination);
            Err(error)
        }
    }
}

/// Resolves `into_dir/<source name>` and refuses the two structural hazards: a
/// destination that already exists, and a destination inside the source.
pub(crate) fn plan_destination(source: &Path, into_dir: &Path) -> Result<PathBuf, OpError> {
    let name = source.file_name().ok_or_else(|| OpError::Io {
        path: source.to_path_buf(),
        kind: io::ErrorKind::InvalidInput,
        message: "the source has no file name to copy".to_owned(),
    })?;
    let destination = into_dir.join(name);
    guard_not_inside(source, into_dir, &destination)?;

    match fs::symlink_metadata(&destination) {
        Ok(_) => Err(OpError::AlreadyExists { path: destination }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(destination),
        Err(error) => Err(OpError::io(&destination, &error)),
    }
}

/// Best-effort removal of a partial destination after a failed or cancelled copy.
pub(crate) fn rollback(destination: &Path) {
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_dir() => {
            let _ = fs::remove_dir_all(destination);
        }
        Ok(_) => {
            let _ = fs::remove_file(destination);
        }
        Err(_) => {}
    }
}

fn guard_not_inside(source: &Path, into_dir: &Path, destination: &Path) -> Result<(), OpError> {
    // Canonical prefix comparison resolves symlinks and `..`; if either path
    // cannot be canonicalized yet, fall back to a lexical check.
    let inside = match (fs::canonicalize(source), fs::canonicalize(into_dir)) {
        (Ok(canon_source), Ok(canon_into)) => canon_into.starts_with(&canon_source),
        _ => into_dir == source || into_dir.starts_with(source),
    };
    if inside {
        return Err(OpError::DestinationInsideSource {
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
        });
    }
    Ok(())
}

struct CopyContext<'a> {
    cancel: &'a CancellationToken,
    progress: &'a mut dyn FnMut(Progress),
    total: Progress,
}

impl CopyContext<'_> {
    fn check(&self) -> Result<(), OpError> {
        if self.cancel.is_cancelled() {
            Err(OpError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn add_bytes(&mut self, bytes: u64) {
        self.total.bytes = self.total.bytes.saturating_add(bytes);
        (self.progress)(self.total);
    }

    fn finish_item(&mut self) {
        self.total.items = self.total.items.saturating_add(1);
        (self.progress)(self.total);
    }
}

/// Recursively copies `source` onto the non-existent `destination`, by kind.
fn copy_tree(context: &mut CopyContext, source: &Path, destination: &Path) -> Result<(), OpError> {
    context.check()?;
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            OpError::SourceMissing {
                path: source.to_path_buf(),
            }
        } else {
            OpError::io(source, &error)
        }
    })?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        copy_symlink(context, source, destination)
    } else if file_type.is_dir() {
        copy_directory(context, source, destination, &metadata)
    } else if file_type.is_file() {
        copy_file(context, source, destination, &metadata)
    } else {
        Err(OpError::UnsupportedFileType {
            path: source.to_path_buf(),
        })
    }
}

fn copy_directory(
    context: &mut CopyContext,
    source: &Path,
    destination: &Path,
    metadata: &Metadata,
) -> Result<(), OpError> {
    fs::create_dir(destination).map_err(|error| OpError::io(destination, &error))?;
    context.finish_item();

    for entry in fs::read_dir(source).map_err(|error| OpError::io(source, &error))? {
        context.check()?;
        let entry = entry.map_err(|error| OpError::io(source, &error))?;
        let child_destination = destination.join(entry.file_name());
        copy_tree(context, &entry.path(), &child_destination)?;
    }

    // Apply the source's permissions only after the contents are in, so a
    // read-only source directory never blocks writing its children.
    let _ = fs::set_permissions(destination, metadata.permissions());
    Ok(())
}

fn copy_file(
    context: &mut CopyContext,
    source: &Path,
    destination: &Path,
    metadata: &Metadata,
) -> Result<(), OpError> {
    let mut reader = File::open(source).map_err(|error| OpError::io(source, &error))?;
    let mut writer =
        File::create_new(destination).map_err(|error| OpError::io(destination, &error))?;
    let mut buffer = vec![0u8; CHUNK];

    loop {
        context.check()?;
        let read = reader
            .read(&mut buffer)
            .map_err(|error| OpError::io(source, &error))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| OpError::io(destination, &error))?;
        context.add_bytes(read as u64);
    }

    writer
        .flush()
        .map_err(|error| OpError::io(destination, &error))?;
    let _ = fs::set_permissions(destination, metadata.permissions());
    context.finish_item();
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(
    context: &mut CopyContext,
    source: &Path,
    destination: &Path,
) -> Result<(), OpError> {
    let target = fs::read_link(source).map_err(|error| OpError::io(source, &error))?;
    std::os::unix::fs::symlink(&target, destination)
        .map_err(|error| OpError::io(destination, &error))?;
    context.finish_item();
    Ok(())
}

#[cfg(not(unix))]
fn copy_symlink(
    _context: &mut CopyContext,
    source: &Path,
    _destination: &Path,
) -> Result<(), OpError> {
    Err(OpError::UnsupportedFileType {
        path: source.to_path_buf(),
    })
}
