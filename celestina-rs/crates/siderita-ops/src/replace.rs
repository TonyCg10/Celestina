//! Replacing an entry with another without a moment where neither exists.
//!
//! "Reemplazar" in a paste used to send the old entry to the Trash first and
//! place the new one second, so a placement that then failed or was cancelled
//! left neither version where the person expected one, and replacing a folder
//! that held the source trashed the source along with it.
//!
//! Here the order is the reverse. The new entry is placed first, complete,
//! under a hidden name beside the old one; only then is the old entry sent to
//! the Trash; and the hidden name is finally renamed onto the freed one,
//! never replacing anything. A failure at any step gives back what the
//! earlier steps took: the hidden copy is removed, or the moved entry is
//! moved back to where it came from.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use celestina_core::CancellationToken;

use crate::copy::{copy_to, guard_not_inside, Durability, Journal, Progress};
use crate::error::OpError;
use crate::relocate::move_as;
use crate::reserve::rename_without_replacing;
use crate::trash::Trashed;

/// How many hidden names a replacement tries before it gives up.
const STAGING_ATTEMPTS: u32 = 16;

static NEXT_STAGING: AtomicU64 = AtomicU64::new(1);

/// What a replacement did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Replaced {
    /// Where the new entry now lives: the replaced name.
    pub placed: PathBuf,
    /// The old entry, now in the Trash; `None` when it had already gone by
    /// the time the new one was ready.
    pub trashed: Option<Trashed>,
    /// For a move across filesystems, source entries that arrived or changed
    /// during the copy and so were not moved; see
    /// [`Moved::left_behind`](crate::Moved::left_behind).
    pub left_behind: Vec<PathBuf>,
}

/// Replaces `target` with a copy of `source`: the copy is made and synced
/// first, then `target` goes to the Trash, then the copy takes its name.
///
/// Refuses a `target` that holds `source` ([`OpError::SourceInsideTarget`]).
/// On any failure `target` is where it was (unless the failure came after it
/// went to the Trash, when the error says the name was taken again) and the
/// partial copy is gone.
pub fn replace_with_copy(
    source: &Path,
    target: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Replaced, OpError> {
    let mut send = crate::trash::trash;
    replace_with(
        source,
        target,
        Placement::Copy,
        cancellation,
        progress,
        &mut send,
    )
}

/// Replaces `target` with `source` itself, moved: the move lands under a
/// hidden name first, then `target` goes to the Trash, then the moved entry
/// takes its name.
///
/// Refuses a `target` that holds `source` ([`OpError::SourceInsideTarget`]).
/// On any failure `target` is where it was (with the same exception as
/// [`replace_with_copy`]) and `source` is moved back; if even that fails, the
/// error names where the entry is.
pub fn replace_with_move(
    source: &Path,
    target: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Replaced, OpError> {
    let mut send = crate::trash::trash;
    replace_with(
        source,
        target,
        Placement::Move,
        cancellation,
        progress,
        &mut send,
    )
}

/// Whether the new entry is a copy of the source or the source itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Placement {
    Copy,
    Move,
}

/// Sends an entry to the Trash; the real Trash in production, a scratch one
/// in tests.
type SendToTrash<'a> = &'a mut dyn FnMut(
    &Path,
    &CancellationToken,
    &mut dyn FnMut(Progress),
) -> Result<Trashed, OpError>;

/// The new entry, complete under its hidden name.
struct Staged {
    path: PathBuf,
    /// What a copy created, so that giving it back removes only that.
    journal: Option<Journal>,
    left_behind: Vec<PathBuf>,
}

fn replace_with(
    source: &Path,
    target: &Path,
    placement: Placement,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
    send_to_trash: SendToTrash,
) -> Result<Replaced, OpError> {
    if cancellation.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    let folder = match target.parent() {
        Some(folder) if target.file_name().is_some() => folder,
        _ => {
            return Err(OpError::Io {
                path: target.to_path_buf(),
                kind: io::ErrorKind::InvalidInput,
                message: "the target has no folder to be replaced in".to_owned(),
            })
        }
    };
    match fs::symlink_metadata(source) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::SourceMissing {
                path: source.to_path_buf(),
            })
        }
        Err(error) => return Err(OpError::io(source, &error)),
    }
    if holds(target, source) {
        return Err(OpError::SourceInsideTarget {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
        });
    }

    let staged = stage(source, folder, placement, cancellation, progress)?;

    let trashed = match fs::symlink_metadata(target) {
        Ok(_) => match send_to_trash(target, cancellation, progress) {
            // A Trash across filesystems left part of the old entry in place
            // (entries that arrived or changed while it was copied): the name
            // is not free, and the error says where each part is.
            Ok(trashed) if !trashed.left_behind.is_empty() => {
                let split = OpError::Io {
                    path: target.to_path_buf(),
                    kind: io::ErrorKind::Other,
                    message: format!(
                        "only part of it went to the Trash, to '{}': {} entries that arrived or \
                         changed while it was copied stayed in place, so the replacement was undone",
                        trashed.trashed.display(),
                        trashed.left_behind.len()
                    ),
                };
                return Err(give_back(staged, source, split));
            }
            Ok(trashed) => Some(trashed),
            Err(error) => return Err(give_back(staged, source, error)),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(give_back(staged, source, OpError::io(target, &error))),
    };

    let staged_is_directory = fs::symlink_metadata(&staged.path)
        .map(|data| data.is_dir())
        .unwrap_or(false);
    match rename_without_replacing(&staged.path, target, staged_is_directory) {
        Ok(()) => Ok(Replaced {
            placed: target.to_path_buf(),
            trashed,
            left_behind: staged.left_behind,
        }),
        Err(failure) => Err(give_back(staged, source, failure.into_op_error(target))),
    }
}

/// Places the new entry in `folder` under a hidden name nobody else holds.
fn stage(
    source: &Path,
    folder: &Path,
    placement: Placement,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(Progress),
) -> Result<Staged, OpError> {
    for _ in 0..STAGING_ATTEMPTS {
        let path = folder.join(staging_name());
        let placed = match placement {
            Placement::Copy => {
                guard_not_inside(source, folder, &path)?;
                copy_to(source, &path, cancellation, progress, Durability::Synced).map(|journal| {
                    Staged {
                        path: path.clone(),
                        journal: Some(journal),
                        left_behind: Vec::new(),
                    }
                })
            }
            Placement::Move => move_as(source, &path, cancellation, progress).map(|moved| Staged {
                path: path.clone(),
                journal: None,
                left_behind: moved.left_behind,
            }),
        };
        match placed {
            Err(OpError::AlreadyExists { path: taken }) if taken == path => continue,
            other => return other,
        }
    }
    Err(OpError::Io {
        path: folder.to_path_buf(),
        kind: io::ErrorKind::AlreadyExists,
        message: "could not find a free hidden name to prepare the replacement".to_owned(),
    })
}

/// A hidden name for the new entry while the old one still holds its name:
/// short and fixed in shape, so a long original name cannot push it past the
/// filesystem's limit.
fn staging_name() -> String {
    format!(
        ".siderita-replace-{}-{}",
        std::process::id(),
        NEXT_STAGING.fetch_add(1, Ordering::Relaxed)
    )
}

/// Undoes the placement after a later step failed, and returns the error to
/// report: `error` itself, or, when a moved entry could not be moved back,
/// `error` together with where that entry now is.
fn give_back(staged: Staged, source: &Path, error: OpError) -> OpError {
    if let Some(journal) = staged.journal {
        journal.roll_back();
        return error;
    }
    // Putting the entry back is not something a cancellation may interrupt.
    match move_as(&staged.path, source, &CancellationToken::new(), &mut |_| {}) {
        Ok(_) => error,
        // A move across filesystems that left entries behind leaves the source
        // folder in place, so the moved part cannot go back under its name.
        Err(OpError::AlreadyExists { .. }) if !staged.left_behind.is_empty() => OpError::Io {
            message: format!(
                "{error}; the moved part is kept at this path, because '{}' still holds {} \
                 entries that arrived or changed while it was copied",
                source.display(),
                staged.left_behind.len()
            ),
            path: staged.path,
            kind: io::ErrorKind::Other,
        },
        Err(undo) => OpError::Io {
            path: staged.path,
            kind: io::ErrorKind::Other,
            message: format!(
                "{error}; moving the entry back also failed ({undo}), so it is kept at this path"
            ),
        },
    }
}

/// Whether trashing `target` would take `source` with it: `source` is
/// `target` or lies inside it. Each is compared with its folder resolved but
/// the entry itself not followed, because trashing a symlink trashes the
/// link, never what it points to.
fn holds(target: &Path, source: &Path) -> bool {
    match (entry_path(target), entry_path(source)) {
        (Some(target), Some(source)) => source.starts_with(target),
        _ => source.starts_with(target),
    }
}

/// `path` with its folder canonical and its own name kept as written.
fn entry_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?;
    let folder = match path.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder,
        _ => Path::new("."),
    };
    fs::canonicalize(folder)
        .ok()
        .map(|folder| folder.join(name))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::{replace_with, Placement, Replaced};
    use crate::copy::Progress;
    use crate::error::OpError;
    use crate::trash::{trash_into, Trashed};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-ops-replace-{label}-{}-{nonce}",
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

    /// Replaces through a scratch Trash under `dir`.
    fn replace(
        dir: &Path,
        source: &Path,
        target: &Path,
        placement: Placement,
        cancellation: &CancellationToken,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Replaced, OpError> {
        let trash_root = dir.join("Trash");
        let mut send = |path: &Path,
                        token: &CancellationToken,
                        progress: &mut dyn FnMut(Progress)|
         -> Result<Trashed, OpError> {
            trash_into(path, &trash_root, token, progress)
        };
        replace_with(source, target, placement, cancellation, progress, &mut send)
    }

    /// Names in `folder` that a replacement would have left hidden.
    fn hidden_leftovers(folder: &Path) -> Vec<String> {
        fs::read_dir(folder)
            .expect("list")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".siderita-replace-"))
            .collect()
    }

    fn trashed_count(dir: &Path) -> usize {
        fs::read_dir(dir.join("Trash/files"))
            .map(|entries| entries.count())
            .unwrap_or(0)
    }

    #[test]
    fn a_copy_replaces_the_old_entry_which_goes_to_the_trash() {
        let dir = TestDir::new("copy");
        let source = dir.path().join("from/nota.txt");
        fs::create_dir(dir.path().join("from")).expect("mk from");
        fs::write(&source, b"new").expect("seed source");
        let target = dir.path().join("nota.txt");
        fs::write(&target, b"old").expect("seed target");

        let replaced = replace(
            dir.path(),
            &source,
            &target,
            Placement::Copy,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect("replace");

        assert_eq!(replaced.placed, target);
        assert_eq!(fs::read(&target).expect("new in place"), b"new");
        assert_eq!(fs::read(&source).expect("source kept"), b"new");
        let trashed = replaced.trashed.expect("the old one went to the Trash");
        assert_eq!(fs::read(&trashed.trashed).expect("old in Trash"), b"old");
        assert!(hidden_leftovers(dir.path()).is_empty());
    }

    #[test]
    fn a_move_replaces_the_old_entry_and_leaves_the_source() {
        let dir = TestDir::new("move");
        fs::create_dir(dir.path().join("from")).expect("mk from");
        let source = dir.path().join("from/carpeta");
        fs::create_dir(&source).expect("seed source");
        fs::write(source.join("dentro.txt"), b"new").expect("seed child");
        let target = dir.path().join("carpeta");
        fs::create_dir(&target).expect("seed target");
        fs::write(target.join("viejo.txt"), b"old").expect("seed old child");

        let replaced = replace(
            dir.path(),
            &source,
            &target,
            Placement::Move,
            &CancellationToken::new(),
            &mut |_| {},
        )
        .expect("replace");

        assert!(!source.exists(), "a move takes the source");
        assert_eq!(fs::read(target.join("dentro.txt")).expect("new"), b"new");
        assert!(!target.join("viejo.txt").exists());
        let trashed = replaced.trashed.expect("old folder trashed");
        assert_eq!(
            fs::read(trashed.trashed.join("viejo.txt")).expect("old in Trash"),
            b"old"
        );
    }

    /// SID-15: pasting `a/x/x` into `a` with "replace" would trash `a/x`, and
    /// with it the source. It is refused before anything moves.
    #[test]
    fn replacing_a_folder_that_holds_the_source_is_refused() {
        let dir = TestDir::new("ancestor");
        let target = dir.path().join("x");
        fs::create_dir(&target).expect("mk x");
        let source = target.join("x");
        fs::write(&source, b"inner").expect("seed inner");

        for placement in [Placement::Copy, Placement::Move] {
            let error = replace(
                dir.path(),
                &source,
                &target,
                placement,
                &CancellationToken::new(),
                &mut |_| {},
            )
            .expect_err("must refuse");
            assert!(
                matches!(error, OpError::SourceInsideTarget { .. }),
                "{error:?}"
            );
            assert_eq!(fs::read(&source).expect("source kept"), b"inner");
            assert_eq!(trashed_count(dir.path()), 0, "nothing was trashed");
            assert!(hidden_leftovers(dir.path()).is_empty());
        }
    }

    /// SID-15's ordering: when the new entry cannot be placed, the old one has
    /// not been touched. The placement is cancelled half-way through a large
    /// file here.
    #[test]
    fn a_placement_that_fails_leaves_the_old_entry_in_place() {
        let dir = TestDir::new("cancelled");
        fs::create_dir(dir.path().join("from")).expect("mk from");
        let source = dir.path().join("from/grande.bin");
        fs::write(&source, vec![1u8; 512 * 1024]).expect("seed source");
        let target = dir.path().join("grande.bin");
        fs::write(&target, b"old").expect("seed target");

        let token = CancellationToken::new();
        let cancel = token.clone();
        let error = replace(
            dir.path(),
            &source,
            &target,
            Placement::Copy,
            &token,
            &mut |progress| {
                if progress.bytes > 0 {
                    cancel.cancel();
                }
            },
        )
        .expect_err("cancelled");

        assert!(matches!(error, OpError::Cancelled), "{error:?}");
        assert_eq!(fs::read(&target).expect("old kept"), b"old");
        assert_eq!(trashed_count(dir.path()), 0, "the old one was not trashed");
        assert!(hidden_leftovers(dir.path()).is_empty());
    }

    /// When the old entry cannot go to the Trash, the new one is given back:
    /// a copy is removed, a moved source goes home.
    #[test]
    fn a_trash_that_fails_gives_the_placement_back() {
        let dir = TestDir::new("trash-fails");
        fs::create_dir(dir.path().join("from")).expect("mk from");
        let source = dir.path().join("from/nota.txt");
        let target = dir.path().join("nota.txt");

        for placement in [Placement::Copy, Placement::Move] {
            fs::write(&source, b"new").expect("seed source");
            fs::write(&target, b"old").expect("seed target");
            let mut refuse = |path: &Path,
                              _: &CancellationToken,
                              _: &mut dyn FnMut(Progress)|
             -> Result<Trashed, OpError> {
                Err(OpError::Io {
                    path: path.to_path_buf(),
                    kind: std::io::ErrorKind::PermissionDenied,
                    message: "no Trash here".to_owned(),
                })
            };
            let error = replace_with(
                &source,
                &target,
                placement,
                &CancellationToken::new(),
                &mut |_| {},
                &mut refuse,
            )
            .expect_err("the Trash refused");

            assert!(matches!(error, OpError::Io { .. }), "{error:?}");
            assert_eq!(fs::read(&target).expect("old kept"), b"old");
            assert_eq!(fs::read(&source).expect("source back"), b"new");
            assert!(hidden_leftovers(dir.path()).is_empty(), "{placement:?}");
        }
    }

    /// Review M-6: a Trash across filesystems that left part of the old entry
    /// in place is reported as that split, and the new entry is given back.
    #[test]
    fn a_partial_trash_is_reported_and_undone() {
        let dir = TestDir::new("partial-trash");
        fs::create_dir(dir.path().join("from")).expect("mk from");
        let source = dir.path().join("from/nota.txt");
        fs::write(&source, b"new").expect("seed source");
        let target = dir.path().join("nota.txt");
        fs::write(&target, b"old").expect("seed target");
        let mut partial = |path: &Path,
                           _: &CancellationToken,
                           _: &mut dyn FnMut(Progress)|
         -> Result<Trashed, OpError> {
            Ok(Trashed {
                original: path.to_path_buf(),
                trashed: PathBuf::from("/Trash/files/nota.txt"),
                info: PathBuf::from("/Trash/info/nota.txt.trashinfo"),
                left_behind: vec![path.join("arrived.txt")],
            })
        };

        let error = replace_with(
            &source,
            &target,
            Placement::Copy,
            &CancellationToken::new(),
            &mut |_| {},
            &mut partial,
        )
        .expect_err("a split Trash is not a replacement");

        let text = error.to_string();
        assert!(text.contains("only part of it went to the Trash"), "{text}");
        assert!(text.contains("/Trash/files/nota.txt"), "{text}");
        assert_eq!(fs::read(&target).expect("old kept"), b"old");
        assert!(hidden_leftovers(dir.path()).is_empty());
    }

    /// Review M-6: when a moved entry cannot go back because its source folder
    /// still holds what a move across filesystems left, the error names where
    /// the moved part is and why.
    #[test]
    fn a_move_that_cannot_go_back_says_where_it_is() {
        let dir = TestDir::new("no-way-back");
        let source = dir.path().join("carpeta");
        fs::create_dir(&source).expect("the source folder still stands");
        fs::write(source.join("arrived.txt"), b"late").expect("seed newcomer");
        let staged_path = dir.path().join(".siderita-replace-test");
        fs::create_dir(&staged_path).expect("the moved part");
        let staged = super::Staged {
            path: staged_path.clone(),
            journal: None,
            left_behind: vec![source.join("arrived.txt")],
        };

        let error = super::give_back(
            staged,
            &source,
            OpError::AlreadyExists {
                path: dir.path().join("target"),
            },
        );

        match &error {
            OpError::Io { path, message, .. } => {
                assert_eq!(path, &staged_path);
                assert!(message.contains("still holds 1 entries"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        assert!(staged_path.exists(), "the moved part was not touched");
    }
}
