//! Placing new bytes beside, or instead of, a person's file.
//!
//! Every edit, metadata write and batch item ends here, so the order of
//! operations that decides whether a person can lose a picture has one owner:
//!
//! 1. **Stage.** The complete new bytes go into a hidden sibling of the
//!    destination through [`atomic_file::stage_media`], which gives them the
//!    source's permission bits (a `0600` photograph stays `0600`) and syncs the
//!    bytes and the sibling's name before anything else moves.
//! 2. **Move the original, when asked to.** A replacement sends the original to
//!    the Trash through the suite's one Trash implementation. Nothing here
//!    unlinks.
//! 3. **Publish.** The sibling takes its name through
//!    [`atomic_file::publish_without_replacing`], so a file that appeared under
//!    that name in the meantime is refused, never overwritten.
//!
//! Steps 2 and 3 swap when they can: a replacement whose result has a
//! different name (the container changed) publishes first and trashes second,
//! so the result exists before the original moves. A replacement that keeps
//! the original's name cannot publish first — the name is taken by the very
//! file being replaced — so it first lands the result as an ordinary file
//! under a hidden name of its own, then trashes, then moves the result into
//! the freed name. If that move fails the result stays at its hidden name.
//! Either way the person loses neither file: what is not at its name is in the
//! Trash or beside it.
//!
//! [ADR 0009](../../../../docs/decisions/0009-editing-without-an-encoder.md)
//! states the contract: a copy lands beside the original, a replacement sends
//! the original to the desktop Trash.

use std::path::{Path, PathBuf};

use celestina_core::atomic_file::{self, WriteError};
use celestina_core::CancellationToken;
use fluorita_core::SaveChoice;

use crate::edit::Bin;
use crate::error::{EngineError, EngineResult};

/// What a landing did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Landed {
    /// The name now holding the new bytes.
    pub(crate) written: PathBuf,
    /// Where the original went. `Some` for every replacement, `None` for a
    /// copy.
    pub(crate) trashed_original: Option<PathBuf>,
}

/// The order a landing takes, decided before anything touches the disk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Order {
    /// A new file beside the original; the original is not touched.
    Copy,
    /// A replacement under a different name: publish, then trash.
    PublishThenTrash,
    /// A replacement under the original's own name: trash, then publish into
    /// the name the original left.
    TrashThenPublish,
}

impl Order {
    pub(crate) fn of(source: &Path, destination: &Path, choice: SaveChoice) -> Self {
        match choice {
            SaveChoice::Copy => Self::Copy,
            SaveChoice::Replace if destination == source => Self::TrashThenPublish,
            SaveChoice::Replace => Self::PublishThenTrash,
        }
    }
}

/// Lands `bytes` at `destination`, a sibling name of the person's file
/// `source`, under `choice`. `operation` names the write in errors.
///
/// # Errors
///
/// - [`EngineError::Landing`] when the bytes could not be staged or could not
///   take their name (a name that appeared meanwhile is
///   [`atomic_file::WriteError::TargetExists`]); nothing moved.
/// - [`EngineError::Cancelled`] before the original is touched.
/// - [`EngineError::NotReplaced`] when a same-name replacement could not trash
///   the original; nothing was written.
/// - [`EngineError::Trash`] when a replacement under a new name could not
///   trash the original; the result is written and the original stays.
/// - [`EngineError::ReplacementNotPublished`] when the original is in the Trash
///   and the result could not take its name; the result is kept, under a
///   hidden name the error gives.
pub(crate) fn land(
    source: &Path,
    destination: &Path,
    bytes: &[u8],
    choice: SaveChoice,
    bin: &dyn Bin,
    cancellation: &CancellationToken,
    operation: &'static str,
) -> EngineResult<Landed> {
    let landing = |source| EngineError::Landing { operation, source };
    let order = Order::of(source, destination, choice);
    if order == Order::TrashThenPublish {
        return replace_in_place(source, bytes, bin, cancellation, operation);
    }

    let staged = atomic_file::stage_media(destination, bytes, source).map_err(landing)?;
    // The last point at which stopping leaves every file as it was: dropping
    // the staged sibling removes it.
    if cancellation.is_cancelled() {
        return Err(EngineError::Cancelled);
    }
    // `Published` only says whether the directory sync also held; the bytes
    // are at their name either way.
    let _published = staged.publish().map_err(landing)?;
    if order == Order::Copy {
        return Ok(Landed {
            written: destination.to_path_buf(),
            trashed_original: None,
        });
    }
    let trashed = bin
        .send(source, cancellation)
        .map_err(|error| EngineError::Trash {
            path: source.to_path_buf(),
            source: error,
        })?;
    Ok(Landed {
        written: destination.to_path_buf(),
        trashed_original: Some(trashed),
    })
}

/// How many hidden result names are tried before a same-name replacement
/// gives up; each collision means another process holds that exact name.
const RESULT_NAME_ATTEMPTS: u32 = 16;

/// A same-name replacement. The result is first *published* under a hidden
/// name this function owns — synced, with the source's mode, never replacing
/// anything — so it is an ordinary file that survives any later failure. Only
/// then does the original go to the Trash, and the hidden file is moved into
/// the name the original left. If that last move fails, the result stays at
/// its hidden name and the error names both files: the edit is never lost.
fn replace_in_place(
    source: &Path,
    bytes: &[u8],
    bin: &dyn Bin,
    cancellation: &CancellationToken,
    operation: &'static str,
) -> EngineResult<Landed> {
    let hidden = land_hidden(source, bytes, operation)?;
    let discard = |error| {
        let _ = std::fs::remove_file(&hidden);
        error
    };
    if cancellation.is_cancelled() {
        return Err(discard(EngineError::Cancelled));
    }
    let trashed = bin.send(source, cancellation).map_err(|error| {
        discard(EngineError::NotReplaced {
            path: source.to_path_buf(),
            source: error,
        })
    })?;
    match atomic_file::publish_without_replacing(&hidden, source) {
        Ok(()) => Ok(Landed {
            written: source.to_path_buf(),
            trashed_original: Some(trashed),
        }),
        Err(error) => Err(EngineError::ReplacementNotPublished {
            path: source.to_path_buf(),
            trashed,
            kept: hidden,
            source: error,
        }),
    }
}

/// The most bytes of the original's name the hidden name keeps.
///
/// A file name may be 255 bytes (`NAME_MAX`), and two names are built on this
/// stem: the hidden name `.<stem>.fluorita-result-<pid>-<n>`, and the
/// temporary `atomic_file` writes it through, `.<hidden>.<pid>-<seq>.tmp`.
/// With a pid of up to 10 digits, `n` of 2 and a sequence of up to 20, those
/// add 68 bytes to the stem, so 180 leaves room. Uniqueness comes from the pid
/// and the counter, not from the stem.
const HIDDEN_STEM_BYTES: usize = 180;

/// `name` cut to at most `limit` bytes, on a character boundary.
fn truncated(name: &str, limit: usize) -> &str {
    let mut end = name.len().min(limit);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

/// Lands `bytes` beside `source` under a fresh hidden name.
fn land_hidden(source: &Path, bytes: &[u8], operation: &'static str) -> EngineResult<PathBuf> {
    let name = source.file_name().unwrap_or_default().to_string_lossy();
    let name = truncated(&name, HIDDEN_STEM_BYTES);
    let mut last = None;
    for attempt in 0..RESULT_NAME_ATTEMPTS {
        let hidden = source.with_file_name(format!(
            ".{name}.fluorita-result-{}-{attempt}",
            std::process::id()
        ));
        match atomic_file::land_media(&hidden, bytes, source) {
            Ok(_published) => return Ok(hidden),
            Err(error @ WriteError::TargetExists { .. }) => last = Some(error),
            Err(error) => {
                return Err(EngineError::Landing {
                    operation,
                    source: error,
                })
            }
        }
    }
    Err(EngineError::Landing {
        operation,
        source: last.unwrap_or(WriteError::TargetExists {
            path: source.to_path_buf(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::atomic_file::WriteError;
    use celestina_core::CancellationToken;
    use fluorita_core::SaveChoice;
    use siderita_ops::OpError;

    use super::{land, Order};
    use crate::edit::Bin;
    use crate::error::EngineError;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fluorita-landing-{label}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(path.join("bin")).expect("test directory");
            Self(path)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).expect("test file");
            path
        }

        /// The names in the directory, apart from the bin.
        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&self.0)
                .expect("the directory")
                .map(|entry| entry.expect("an entry").file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| name != "bin")
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Moves what it is sent into `bin/`, and records what the directory held
    /// at that moment — the only way to see the order of operations.
    struct WatchingBin {
        directory: PathBuf,
        seen_at_send: RefCell<Vec<(String, Vec<u8>)>>,
        /// A file another process creates under the original's name right
        /// after it leaves, to prove the publish refuses to replace it.
        intruder: Option<Vec<u8>>,
    }

    impl WatchingBin {
        fn new(directory: &TestDir) -> Self {
            Self {
                directory: directory.0.clone(),
                seen_at_send: RefCell::new(Vec::new()),
                intruder: None,
            }
        }
    }

    impl Bin for WatchingBin {
        fn send(&self, path: &Path, _cancellation: &CancellationToken) -> Result<PathBuf, OpError> {
            for entry in std::fs::read_dir(&self.directory).expect("the directory") {
                let entry = entry.expect("an entry");
                if entry.file_type().expect("a type").is_file() {
                    self.seen_at_send.borrow_mut().push((
                        entry.file_name().to_string_lossy().into_owned(),
                        std::fs::read(entry.path()).expect("readable"),
                    ));
                }
            }
            let kept = self
                .directory
                .join("bin")
                .join(path.file_name().unwrap_or_default());
            std::fs::rename(path, &kept).map_err(|error| OpError::io(path, &error))?;
            if let Some(bytes) = &self.intruder {
                std::fs::write(path, bytes).expect("the intruder");
            }
            Ok(kept)
        }
    }

    struct RefusingBin;

    impl Bin for RefusingBin {
        fn send(&self, path: &Path, _cancellation: &CancellationToken) -> Result<PathBuf, OpError> {
            Err(OpError::io(
                path,
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ))
        }
    }

    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .expect("a file")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn the_order_follows_the_choice_and_whether_the_name_is_kept() {
        let source = Path::new("/m/foto.jpg");
        assert_eq!(
            Order::of(source, Path::new("/m/foto (editado).jpg"), SaveChoice::Copy),
            Order::Copy
        );
        assert_eq!(
            Order::of(source, source, SaveChoice::Replace),
            Order::TrashThenPublish
        );
        assert_eq!(
            Order::of(source, Path::new("/m/foto.png"), SaveChoice::Replace),
            Order::PublishThenTrash
        );
    }

    #[test]
    fn a_same_name_replacement_stages_the_result_before_the_original_moves() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new("in-place");
        let source = directory.file("foto.jpg", b"original");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o640))
            .expect("a group-readable photograph");
        let bin = WatchingBin::new(&directory);

        let landed = land(
            &source,
            &source,
            b"result",
            SaveChoice::Replace,
            &bin,
            &CancellationToken::new(),
            "writing the test result",
        )
        .expect("the replacement lands");

        let seen = bin.seen_at_send.borrow();
        assert!(
            seen.iter()
                .any(|(name, bytes)| name.starts_with(".foto.jpg.") && bytes == b"result"),
            "the result was not staged beside the original when it moved: {seen:?}"
        );
        assert_eq!(std::fs::read(&source).expect("the result"), b"result");
        assert_eq!(mode(&source), 0o640, "the result keeps the original's mode");
        let trashed = landed.trashed_original.expect("the original is trashed");
        assert_eq!(std::fs::read(trashed).expect("the original"), b"original");
        assert_eq!(directory.names(), vec!["foto.jpg".to_owned()]);
    }

    #[test]
    fn a_name_as_long_as_the_filesystem_allows_is_still_replaced() {
        let directory = TestDir::new("long-name");
        // 250 bytes, with a two-byte character across the stem's cut.
        let name = format!("{}\u{e9}{}.jpg", "a".repeat(179), "b".repeat(65));
        assert_eq!(name.len(), 250);
        let source = directory.file(&name, b"original");

        let landed = land(
            &source,
            &source,
            b"result",
            SaveChoice::Replace,
            &WatchingBin::new(&directory),
            &CancellationToken::new(),
            "writing the test result",
        )
        .expect("a long name is replaced like any other");

        assert_eq!(std::fs::read(&source).expect("the result"), b"result");
        assert!(landed.trashed_original.is_some());
        assert_eq!(directory.names(), vec![name]);
    }

    #[test]
    fn a_same_name_replacement_the_trash_refuses_writes_nothing() {
        let directory = TestDir::new("in-place-refused");
        let source = directory.file("foto.jpg", b"original");

        let failure = land(
            &source,
            &source,
            b"result",
            SaveChoice::Replace,
            &RefusingBin,
            &CancellationToken::new(),
            "writing the test result",
        )
        .expect_err("refused");

        assert!(matches!(failure, EngineError::NotReplaced { .. }));
        assert_eq!(std::fs::read(&source).expect("the original"), b"original");
        assert_eq!(directory.names(), vec!["foto.jpg".to_owned()]);
    }

    #[test]
    fn a_name_taken_after_the_original_left_is_refused_and_the_original_stays_in_the_bin() {
        let directory = TestDir::new("in-place-intruder");
        let source = directory.file("foto.jpg", b"original");
        let mut bin = WatchingBin::new(&directory);
        bin.intruder = Some(b"another program's file".to_vec());

        let failure = land(
            &source,
            &source,
            b"result",
            SaveChoice::Replace,
            &bin,
            &CancellationToken::new(),
            "writing the test result",
        )
        .expect_err("the name was taken");

        let EngineError::ReplacementNotPublished {
            trashed,
            kept,
            source,
            ..
        } = failure
        else {
            panic!("unexpected failure: {failure:?}");
        };
        assert_eq!(source.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(directory.0.join("foto.jpg")).expect("the other file"),
            b"another program's file",
            "a file that appeared under the name is never overwritten"
        );
        assert_eq!(std::fs::read(trashed).expect("the original"), b"original");
        assert_eq!(
            std::fs::read(&kept).expect("the result survives"),
            b"result",
            "the person's edit is kept beside the name it could not take"
        );
    }

    #[test]
    fn a_copy_refuses_a_name_that_appeared_meanwhile() {
        let directory = TestDir::new("copy-taken");
        let source = directory.file("foto.jpg", b"original");
        let taken = directory.file("foto (editado).jpg", b"created meanwhile");

        let failure = land(
            &source,
            &taken,
            b"result",
            SaveChoice::Copy,
            &RefusingBin,
            &CancellationToken::new(),
            "writing the test result",
        )
        .expect_err("the name was taken");

        assert!(matches!(
            failure,
            EngineError::Landing {
                source: WriteError::TargetExists { .. },
                ..
            }
        ));
        assert_eq!(std::fs::read(&taken).expect("kept"), b"created meanwhile");
        assert_eq!(std::fs::read(&source).expect("kept"), b"original");
        assert_eq!(
            directory.names(),
            vec!["foto (editado).jpg".to_owned(), "foto.jpg".to_owned()],
            "no staged sibling is left behind"
        );
    }

    #[test]
    fn a_copy_takes_the_originals_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new("copy-mode");
        let source = directory.file("foto.jpg", b"original");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600))
            .expect("a private photograph");
        let destination = directory.0.join("foto (editado).jpg");

        let landed = land(
            &source,
            &destination,
            b"result",
            SaveChoice::Copy,
            &RefusingBin,
            &CancellationToken::new(),
            "writing the test result",
        )
        .expect("the copy lands");

        assert_eq!(landed.trashed_original, None);
        assert_eq!(mode(&destination), 0o600);
        assert_eq!(std::fs::read(&source).expect("kept"), b"original");
    }

    #[test]
    fn a_cancelled_landing_leaves_every_file_as_it_was() {
        let directory = TestDir::new("cancelled");
        let source = directory.file("foto.jpg", b"original");
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let failure = land(
            &source,
            &source,
            b"result",
            SaveChoice::Replace,
            &WatchingBin::new(&directory),
            &cancellation,
            "writing the test result",
        )
        .expect_err("cancelled");

        assert!(matches!(failure, EngineError::Cancelled));
        assert_eq!(std::fs::read(&source).expect("kept"), b"original");
        assert_eq!(directory.names(), vec!["foto.jpg".to_owned()]);
    }
}
