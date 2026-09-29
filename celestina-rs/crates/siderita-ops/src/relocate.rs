use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::copy::{copy_to, guard_not_inside, plan_destination, Durability, Progress};
use crate::error::OpError;

/// The paths a successful move went between.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Moved {
    pub from: PathBuf,
    pub to: PathBuf,
    /// Source entries a move across filesystems did not remove, because they
    /// arrived in a source folder or changed after they were copied. They are
    /// still at the source, and a folder holding one is still there too.
    /// Always empty for a move within one filesystem, which is one rename.
    pub left_behind: Vec<PathBuf>,
}

/// Moves `source` into `into_dir`, keeping its own file name.
///
/// On the same filesystem this is a single atomic `rename`. Across filesystems
/// it becomes copy → sync → verify → remove-source: the source is **removed
/// only after** every copied file and folder has been synced to disk and the
/// copy revalidated, and then only the entries that were copied are removed —
/// see [`Moved::left_behind`]. A cancelled or failed cross-device move always
/// leaves the source intact. Like copy, it refuses to overwrite an existing
/// destination.
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

    // The destination was just checked to be free, and the reservation keeps it
    // that way: `rename` on its own would replace whatever appeared in between.
    let source_is_directory = fs::symlink_metadata(source)
        .map(|data| data.is_dir())
        .unwrap_or(false);
    match crate::reserve::rename_without_replacing(source, &destination, source_is_directory) {
        Ok(()) => Ok(Moved {
            from: source.to_path_buf(),
            to: destination,
            left_behind: Vec::new(),
        }),
        Err(crate::reserve::RenameFailure::Io(error)) if is_cross_device(&error) => {
            let left_behind = relocate_by_copy(source, &destination, cancellation, progress)?;
            Ok(Moved {
                from: source.to_path_buf(),
                to: destination,
                left_behind,
            })
        }
        Err(failure) => Err(failure.into_op_error(&destination)),
    }
}

/// Moves `source` onto an exact `destination` path (not into a directory),
/// choosing the target name explicitly — conflict resolution's "keep both" for a
/// cut-paste. Same loss-free contract as [`move_entry`]: an atomic rename on one
/// filesystem, copy → verify → remove-source across two, and it refuses both an
/// existing destination and one inside the source.
pub fn move_as(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Moved, OpError> {
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
    match fs::symlink_metadata(source) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing {
                path: source.to_path_buf(),
            });
        }
        Err(error) => return Err(OpError::io(source, &error)),
    }

    // Same reservation as `move_entry`: the look above is the honest error, and
    // the reservation is what keeps the answer true while the rename happens.
    let source_is_directory = fs::symlink_metadata(source)
        .map(|data| data.is_dir())
        .unwrap_or(false);
    match crate::reserve::rename_without_replacing(source, destination, source_is_directory) {
        Ok(()) => Ok(Moved {
            from: source.to_path_buf(),
            to: destination.to_path_buf(),
            left_behind: Vec::new(),
        }),
        Err(crate::reserve::RenameFailure::Io(error)) if is_cross_device(&error) => {
            let left_behind = relocate_by_copy(source, destination, cancellation, progress)?;
            Ok(Moved {
                from: source.to_path_buf(),
                to: destination.to_path_buf(),
                left_behind,
            })
        }
        Err(failure) => Err(failure.into_op_error(destination)),
    }
}

/// The cross-device path: copy onto `destination` and sync every copied file
/// and folder, revalidate the copy against the source, and only then remove
/// from the source the entries that were copied, and only while each is still
/// what was copied. Returns the source entries it left (see
/// [`Moved::left_behind`]).
///
/// Any failure before the removal keeps the source whole and removes only the
/// destination entries this copy created.
pub(crate) fn relocate_by_copy(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Vec<PathBuf>, OpError> {
    let journal = copy_to(
        source,
        destination,
        cancellation,
        progress,
        Durability::Synced,
    )?;

    if let Err(error) = verify(source, destination) {
        journal.roll_back();
        return Err(error);
    }

    Ok(journal.remove_sources())
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

/// Whether a `rename` failed only because the paths straddle two filesystems.
///
/// `EXDEV` is 18 on Linux, macOS and the BSDs — the platforms the suite targets.
#[cfg(unix)]
pub(crate) fn is_cross_device(error: &io::Error) -> bool {
    error.raw_os_error() == Some(18)
}

#[cfg(not(unix))]
pub(crate) fn is_cross_device(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::CrossesDevices
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::{move_as, relocate_by_copy};
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

    #[test]
    fn move_as_moves_to_the_chosen_name() {
        let dir = TestDir::new("moveas");
        let source = dir.path().join("orig.txt");
        fs::write(&source, b"cut both").expect("seed");
        let destination = dir.path().join("orig (copia).txt");

        let moved = move_as(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect("move_as");

        assert_eq!(moved.to, destination);
        assert!(!source.exists(), "source is gone after a move");
        assert_eq!(fs::read(&destination).expect("moved"), b"cut both");
    }

    #[test]
    fn move_as_refuses_an_existing_destination() {
        let dir = TestDir::new("moveas-exists");
        let source = dir.path().join("a.txt");
        let destination = dir.path().join("b.txt");
        fs::write(&source, b"a").expect("seed source");
        fs::write(&destination, b"do not clobber").expect("seed dest");

        let error = move_as(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect_err("must refuse");
        assert!(matches!(error, OpError::AlreadyExists { .. }));
        assert!(source.exists(), "the source is kept on refusal");
        assert_eq!(
            fs::read(&destination).expect("dest intact"),
            b"do not clobber"
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

    /// SID-3: a copy-move removes only what it copied. Two sibling folders are
    /// moved; when the second one is created, the first has been listed and
    /// copied completely, and that is when a newcomer lands in each folder and
    /// both copied files change. Every newcomer and every change must still
    /// exist afterwards, at the source or at the destination.
    #[test]
    fn a_copy_move_keeps_what_arrived_or_changed_during_the_copy() {
        let dir = TestDir::new("newcomer");
        let source = dir.path().join("src");
        for folder in ["a", "b"] {
            fs::create_dir_all(source.join(folder)).expect("mk folder");
            fs::write(source.join(folder).join("old.txt"), b"old").expect("seed");
        }
        let destination = dir.path().join("dst");

        let mut planted = false;
        let plant = |progress: crate::copy::Progress| {
            // Items: the top folder, the first folder, its file, then the
            // second folder. At four the first folder is done.
            if progress.items == 4 && !planted {
                planted = true;
                for folder in ["a", "b"] {
                    fs::write(source.join(folder).join("new.txt"), b"new").expect("plant");
                    let mut old = fs::OpenOptions::new()
                        .append(true)
                        .open(source.join(folder).join("old.txt"))
                        .expect("open old");
                    std::io::Write::write_all(&mut old, b" and changed").expect("append");
                }
            }
        };
        let mut plant = plant;
        let left_behind =
            relocate_by_copy(&source, &destination, &CancellationToken::new(), &mut plant)
                .expect("move");
        assert!(planted, "the copy never reached the second folder");

        // The first folder's newcomer and its changed file were never copied
        // as they are now: both stay, and both are reported.
        assert!(
            left_behind.len() >= 2,
            "the move did not report what it left: {left_behind:?}"
        );
        for path in &left_behind {
            assert!(path.exists(), "{} was reported but is gone", path.display());
        }

        for folder in ["a", "b"] {
            for (name, content) in [
                ("new.txt", &b"new"[..]),
                ("old.txt", &b"old and changed"[..]),
            ] {
                let at_source = fs::read(source.join(folder).join(name)).ok();
                let at_destination = fs::read(destination.join(folder).join(name)).ok();
                assert!(
                    at_source.as_deref() == Some(content)
                        || at_destination.as_deref() == Some(content),
                    "{folder}/{name} was lost: source {at_source:?}, destination {at_destination:?}"
                );
            }
        }
    }

    /// Review I-1: a write into the part of a file the copy has already read
    /// happens before the copy ends, and must still keep the source. The
    /// modification time is set far back first, so the rewrite is visible even
    /// where the filesystem's clock is coarser than this test.
    #[test]
    fn a_file_rewritten_during_its_own_copy_is_kept() {
        use std::io::Write;
        let dir = TestDir::new("rewrite");
        let source = dir.path().join("big.bin");
        fs::write(&source, vec![b'o'; 256 * 1024]).expect("seed");
        let long_ago = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        fs::File::options()
            .write(true)
            .open(&source)
            .and_then(|file| file.set_modified(long_ago))
            .expect("age the source");
        let destination = dir.path().join("moved.bin");

        let mut written = false;
        let left_behind = relocate_by_copy(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |progress| {
                if progress.bytes >= 64 * 1024 && !written {
                    written = true;
                    fs::OpenOptions::new()
                        .write(true)
                        .open(&source)
                        .and_then(|mut file| file.write_all(b"NEWDATA"))
                        .expect("rewrite the start");
                }
            },
        )
        .expect("move");

        assert!(written, "the copy never reached 64 KiB");
        let kept = fs::read(&source).ok();
        let copied = fs::read(&destination).ok();
        assert!(
            kept.as_deref()
                .is_some_and(|bytes| bytes.starts_with(b"NEWDATA"))
                || copied
                    .as_deref()
                    .is_some_and(|bytes| bytes.starts_with(b"NEWDATA")),
            "the rewrite was lost: source {:?}, destination starts {:?}",
            kept.map(|bytes| bytes.len()),
            copied.map(|bytes| String::from_utf8_lossy(&bytes[..7]).into_owned())
        );
        assert_eq!(left_behind, vec![source.clone()]);
    }

    /// Review M-2: removing one name of a hard-linked file changes the
    /// inode's change time, which the other name shares. That is not a
    /// change to the file, and the move must still take both names.
    #[test]
    fn a_hard_linked_pair_is_moved_whole() {
        let dir = TestDir::new("hardlink");
        let source = dir.path().join("src");
        fs::create_dir(&source).expect("mk src");
        fs::write(source.join("a.txt"), b"shared").expect("seed");
        fs::hard_link(source.join("a.txt"), source.join("a-link.txt")).expect("link");
        let destination = dir.path().join("dst");

        let left_behind = relocate_by_copy(
            &source,
            &destination,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect("move");

        assert_eq!(left_behind, Vec::<PathBuf>::new());
        assert!(!source.exists(), "the source folder stayed");
        for name in ["a.txt", "a-link.txt"] {
            assert_eq!(fs::read(destination.join(name)).expect("copied"), b"shared");
        }
    }
}
