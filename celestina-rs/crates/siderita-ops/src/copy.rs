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

/// Copies `source` onto an exact `destination` path (not into a directory),
/// choosing the target name explicitly. This is conflict resolution's "keep
/// both": the caller supplies a freed name so the copy lands beside — never on
/// top of — the entry it collided with. Refuses an existing destination and one
/// that lies inside the source, exactly like [`copy`].
pub fn copy_as(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    if let Some(parent) = destination.parent() {
        guard_not_inside(source, parent, destination)?;
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(OpError::AlreadyExists {
                path: destination.to_path_buf(),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(OpError::io(destination, &error)),
    }
    copy_to(source, destination, cancellation, progress)
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

pub(crate) fn guard_not_inside(
    source: &Path,
    into_dir: &Path,
    destination: &Path,
) -> Result<(), OpError> {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::copy_as;
    use crate::error::OpError;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-copyas-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn copy_as_places_a_copy_under_the_chosen_name() {
        let dir = TestDir::new("basic");
        let source = dir.path().join("orig.txt");
        fs::write(&source, b"keep both").expect("seed");
        let destination = dir.path().join("orig (copia).txt");

        copy_as(&source, &destination, &CancellationToken::new(), &mut |_| {}).expect("copy_as");

        assert_eq!(fs::read(&source).expect("source kept"), b"keep both");
        assert_eq!(fs::read(&destination).expect("copy made"), b"keep both");
    }

    #[test]
    fn copy_as_refuses_an_existing_destination() {
        let dir = TestDir::new("exists");
        let source = dir.path().join("a.txt");
        let destination = dir.path().join("b.txt");
        fs::write(&source, b"a").expect("seed source");
        fs::write(&destination, b"do not clobber").expect("seed dest");

        let error = copy_as(&source, &destination, &CancellationToken::new(), &mut |_| {})
            .expect_err("must refuse");
        assert!(matches!(error, OpError::AlreadyExists { .. }));
        assert_eq!(fs::read(&destination).expect("dest intact"), b"do not clobber");
    }
}
