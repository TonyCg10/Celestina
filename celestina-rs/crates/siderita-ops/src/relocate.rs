use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::copy::{copy_to, plan_destination, rollback, Progress};
use crate::error::OpError;

/// The paths a successful move went between.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Moved {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Moves `source` into `into_dir`, keeping its own file name.
///
/// On the same filesystem this is a single atomic `rename`. Across filesystems
/// it becomes copy → verify → remove-source, and the source is **removed only
/// after** the copy has completed and been revalidated — a cancelled or failed
/// cross-device move always leaves the source intact. Like copy, it refuses to
/// overwrite an existing destination.
pub fn move_entry(
    source: &Path,
    into_dir: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Moved, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }

    let destination = plan_destination(source, into_dir)?;

    // Confirm the source exists before attempting anything, for a truthful error.
    match fs::symlink_metadata(source) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing {
                path: source.to_path_buf(),
            });
        }
        Err(error) => return Err(OpError::io(source, &error)),
    }

    // The destination was just checked to be free; `rename` would still clobber
    // on a race, but the window is the same tiny one as the rename verb.
    match fs::rename(source, &destination) {
        Ok(()) => Ok(Moved {
            from: source.to_path_buf(),
            to: destination,
        }),
        Err(error) if is_cross_device(&error) => {
            relocate_by_copy(source, &destination, cancellation, progress)?;
            Ok(Moved {
                from: source.to_path_buf(),
                to: destination,
            })
        }
        Err(error) => Err(OpError::io(&destination, &error)),
    }
}

/// The cross-device path: copy onto `destination`, revalidate it against the
/// source, and only then remove the source. Any failure keeps the source and
/// rolls the partial destination back.
pub(crate) fn relocate_by_copy(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), OpError> {
    copy_to(source, destination, cancellation, progress)?;

    if let Err(error) = verify(source, destination) {
        rollback(destination);
        return Err(error);
    }

    remove_source(source)
}

/// Confirms the copy landed: same kind, and for a plain file the same length.
fn verify(source: &Path, destination: &Path) -> Result<(), OpError> {
    let source_meta = fs::symlink_metadata(source).map_err(|error| OpError::io(source, &error))?;
    let dest_meta =
        fs::symlink_metadata(destination).map_err(|error| OpError::io(destination, &error))?;

    let source_type = source_meta.file_type();
    let dest_type = dest_meta.file_type();
    let matches = if source_type.is_file() {
        dest_type.is_file() && source_meta.len() == dest_meta.len()
    } else if source_type.is_dir() {
        dest_type.is_dir()
    } else if source_type.is_symlink() {
        dest_type.is_symlink()
    } else {
        false
    };

    if matches {
        Ok(())
    } else {
        Err(OpError::Io {
            path: destination.to_path_buf(),
            kind: io::ErrorKind::Other,
            message: "the copied destination did not match the source; source kept".to_owned(),
        })
    }
}

fn remove_source(source: &Path) -> Result<(), OpError> {
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            OpError::SourceMissing {
                path: source.to_path_buf(),
            }
        } else {
            OpError::io(source, &error)
        }
    })?;

    let removed = if metadata.is_dir() {
        fs::remove_dir_all(source)
    } else {
        fs::remove_file(source)
    };
    removed.map_err(|error| OpError::io(source, &error))
}

/// Whether a `rename` failed only because the paths straddle two filesystems.
///
/// `EXDEV` is 18 on Linux, macOS and the BSDs — the platforms the suite targets.
#[cfg(unix)]
fn is_cross_device(error: &io::Error) -> bool {
    error.raw_os_error() == Some(18)
}

#[cfg(not(unix))]
fn is_cross_device(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::CrossesDevices
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::relocate_by_copy;
    use crate::error::OpError;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-reloc-{label}-{}-{nonce}",
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

    // Exercises the cross-device path directly (it cannot be reached with a
    // rename on a single test filesystem): copy, verify, then remove source.
    #[test]
    fn relocate_by_copy_moves_a_file_and_removes_the_source() {
        let dir = TestDir::new("file");
        let source = dir.path().join("data.bin");
        let destination = dir.path().join("moved.bin");
        fs::write(&source, b"payload").expect("seed source");

        relocate_by_copy(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect("relocate");

        assert!(
            !source.exists(),
            "source must be gone after a verified move"
        );
        assert_eq!(
            fs::read(&destination).expect("read destination"),
            b"payload"
        );
    }

    #[test]
    fn relocate_by_copy_moves_a_directory_tree() {
        let dir = TestDir::new("tree");
        let source = dir.path().join("src");
        fs::create_dir(&source).expect("mk src");
        fs::create_dir(source.join("nested")).expect("mk nested");
        fs::write(source.join("nested/leaf.txt"), b"leaf").expect("seed leaf");
        let destination = dir.path().join("dst");

        relocate_by_copy(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect("relocate tree");

        assert!(!source.exists());
        assert_eq!(
            fs::read(destination.join("nested/leaf.txt")).expect("read leaf"),
            b"leaf"
        );
    }

    // The guarantee: a cancelled cross-device move keeps the source and leaves
    // no partial destination behind.
    #[test]
    fn a_cancelled_relocate_keeps_the_source_and_rolls_back() {
        let dir = TestDir::new("cancel");
        let source = dir.path().join("keep.txt");
        let destination = dir.path().join("half.txt");
        fs::write(&source, b"precious").expect("seed source");

        let token = CancellationToken::new();
        token.cancel();
        let error = relocate_by_copy(&source, &destination, &token, &mut |_| {})
            .expect_err("must not complete");

        assert!(matches!(error, OpError::Cancelled));
        assert_eq!(fs::read(&source).expect("source intact"), b"precious");
        assert!(!destination.exists(), "no partial destination may survive");
    }
}
