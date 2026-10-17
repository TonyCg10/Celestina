//! The clipboard history's file: `$XDG_STATE_HOME/celestina/clipboard.json`
//! (design §4).
//!
//! What may be written and what a file may bring back is the pure domain's
//! (`celestina_shell::clipboard::{encode, decode}`): text only, nothing
//! expired, nothing beyond [`MAX_PERSISTED_BYTES`]. This module only moves
//! those bytes to and from the disk, and treats the file as hostile, since any
//! process of the person could have written it:
//!
//! - it is opened with `O_NOFOLLOW` (a symlink is refused, not followed) and
//!   `O_NONBLOCK` (a FIFO cannot stall the worker before `fstat` refuses it);
//! - on the open descriptor it must be a regular file, owned by this user,
//!   with no group or other permission bits, and no larger than the bound;
//! - it is read through a cap one byte past the bound, so a file that grows
//!   while it is read is still refused.
//!
//! A file that fails any of this is not loaded: [`Store::load`] answers an
//! empty history and a [`Refusal`] naming why, and the next [`Store::save`]
//! replaces it. Writes go through the suite's atomic private write
//! (`celestina_core::atomic_file::replace_private`): a `0600` sibling created
//! with `create_new` in the same directory, synced, then renamed over the
//! target, so the old file stays whole until the new one is complete and a
//! failed attempt removes only its own sibling. The directory is the suite's
//! private directory (`celestina_core::xdg::ensure_private_dir`), created
//! `0700`.
//!
//! An interrupted write (a crash between creating the sibling and renaming
//! it) leaves a plaintext sibling `.clipboard.json.<pid>-<n>.tmp` behind.
//! [`Store::load`] and [`Store::save`] sweep such strays, and
//! [`Store::delete`] removes them with the file: only regular files of this
//! user whose names have exactly that shape, never a symlink.
//!
//! [`Keeper`] is the store as one worker may use it: a lock around it and a
//! revocation. Turning the duty off revokes the worker's keeper before it
//! deletes the file, under the same lock a save takes, so a worker that is
//! still running (or has not yet seen its stop) can never write the file
//! again once the deletion has begun.
//!
//! No error here carries a byte of clipboard content: the file's metadata and
//! the operating system's error are all they hold.

use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use celestina_core::atomic_file::{self, Published, WriteError};
use celestina_core::xdg::{self, PrivateDirError};
use celestina_shell::clipboard::{self, DecodeError, EncodeError, History, MAX_PERSISTED_BYTES};
use rustix::fs::OFlags;

/// The directory under `$XDG_STATE_HOME` the shell keeps its state in.
const STATE_DIR: &str = "celestina";
/// The history file's name inside [`STATE_DIR`].
const FILE_NAME: &str = "clipboard.json";
/// The most directory entries one sweep for stray temporaries looks at; the
/// state directory holds a handful.
const MAX_SWEEP_ENTRIES: usize = 4096;

/// The history file under a state home: `<state_home>/celestina/clipboard.json`.
#[must_use]
pub fn path_in(state_home: &Path) -> PathBuf {
    state_home.join(STATE_DIR).join(FILE_NAME)
}

/// Why a history file that exists was not loaded. None of these carries
/// content; the file is replaced on the next save.
#[derive(Debug, thiserror::Error)]
pub enum Refusal {
    /// The name is a symbolic link, which is never followed.
    #[error("the clipboard history is a symbolic link")]
    Symlink,
    /// The name is a directory, a FIFO, a socket or a device.
    #[error("the clipboard history is not a regular file")]
    NotRegular,
    /// The file belongs to another user.
    #[error("the clipboard history belongs to uid {owner}, not to uid {user}")]
    NotOwned { owner: u32, user: u32 },
    /// The file grants its group or other users some access.
    #[error("the clipboard history has mode {mode:04o}; it must be 0600")]
    Shared { mode: u32 },
    /// The file is larger than [`MAX_PERSISTED_BYTES`].
    #[error("the clipboard history is over the {MAX_PERSISTED_BYTES}-byte bound")]
    TooLarge,
    /// This process's effective user id could not be read, so ownership
    /// could not be checked.
    #[error("cannot read the effective user id")]
    UnknownUser(#[source] io::Error),
    /// Opening, inspecting or reading the file failed.
    #[error("cannot read the clipboard history")]
    Unreadable(#[source] io::Error),
    /// The bytes are not a history the domain accepts (malformed, or another
    /// schema). The domain's error keeps no content either.
    #[error("the clipboard history is not loadable")]
    Undecodable(#[source] DecodeError),
}

/// What [`Store::load`] found.
#[derive(Debug)]
pub struct Loaded {
    /// The history to start from: empty when there was no file or it was
    /// refused.
    pub history: History,
    /// Why a file that exists was not loaded; `None` when it loaded or did
    /// not exist.
    pub refusal: Option<Refusal>,
    /// How many entries of a loaded file were left out (expired, not
    /// recordable, duplicated, past the cap).
    pub dropped: usize,
}

impl Loaded {
    fn empty(refusal: Option<Refusal>) -> Self {
        Self {
            history: History::new(),
            refusal,
            dropped: 0,
        }
    }

    /// Whether the file on disk holds something the history does not: a
    /// refused file, or entries the load left out. Such a file is rewritten
    /// (or deleted, when nothing is left) at once rather than at the next
    /// change, so an expired secret does not wait on disk for a copy.
    #[must_use]
    pub fn stale(&self) -> bool {
        self.refusal.is_some() || self.dropped > 0
    }
}

/// Why [`Store::save`] did not write. The previous file is unchanged.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The history file has no parent directory to keep it in.
    #[error("the clipboard history path has no directory")]
    NoDirectory,
    /// The state directory could not be created, or is not a private
    /// directory of this user.
    #[error("the clipboard state directory is not usable")]
    Directory(#[source] PrivateDirError),
    /// The domain refused to serialise an entry; only its position is kept.
    #[error("the clipboard history did not serialise")]
    Encode(#[source] EncodeError),
    /// The atomic write failed at one of its steps.
    #[error("the clipboard history was not written")]
    Write(#[source] WriteError),
}

/// Why [`Store::delete`] did not remove the file.
#[derive(Debug, thiserror::Error)]
#[error("cannot delete the clipboard history")]
pub struct DeleteError(#[source] io::Error);

/// The clipboard history's file. Every call does blocking IO, so it belongs
/// on the clipboard worker, never on the loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// The store under `$XDG_STATE_HOME` (else `$HOME/.local/state`), or
    /// `None` when neither is an absolute path.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        xdg::state_home().map(|home| Self::in_state_home(&home))
    }

    /// The store under an explicit state home.
    #[must_use]
    pub fn in_state_home(state_home: &Path) -> Self {
        Self::at(path_in(state_home))
    }

    /// The store at an explicit file path, whose directory must be absolute.
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// Where the history lives.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the history back as it stands at `now` (seconds of the clock the
    /// entries were captured with): expired entries are dropped by the
    /// domain's `decode`. A missing file is an empty history without a
    /// refusal; a refused file is an empty history with one.
    #[must_use]
    pub fn load(&self, now: u64) -> Loaded {
        self.sweep();
        let refused = |refusal| Loaded::empty(Some(refusal));
        let bytes = match read_private(&self.path) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Loaded::empty(None),
            Err(refusal) => return refused(refusal),
        };
        match clipboard::decode(&bytes, now) {
            Ok(decoded) => Loaded {
                history: decoded.history,
                refusal: None,
                dropped: decoded.dropped,
            },
            Err(DecodeError::TooLarge { .. }) => refused(Refusal::TooLarge),
            Err(error) => refused(Refusal::Undecodable(error)),
        }
    }

    /// Writes `history` as it stands at `now`: text only and nothing expired
    /// (the domain's `encode`), into a private directory, atomically. On any
    /// error the previous file is untouched and no temporary is left behind.
    ///
    /// # Errors
    ///
    /// A [`SaveError`] naming the failed step.
    pub fn save(&self, history: &History, now: u64) -> Result<Published, SaveError> {
        let directory = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or(SaveError::NoDirectory)?;
        xdg::ensure_private_dir(directory).map_err(SaveError::Directory)?;
        self.sweep();
        let bytes = clipboard::encode(history.entries(), now).map_err(SaveError::Encode)?;
        atomic_file::replace_private(&self.path, &bytes).map_err(SaveError::Write)
    }

    /// Removes the file (a symlink in its place is removed, never followed)
    /// and every stray temporary of an interrupted write. Removing a file
    /// that is not there succeeds.
    ///
    /// # Errors
    ///
    /// [`DeleteError`] when the name exists and could not be removed.
    pub fn delete(&self) -> Result<(), DeleteError> {
        self.sweep();
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(DeleteError(error)),
        }
    }

    /// Removes the plaintext temporaries an interrupted write of this file
    /// left behind (see the module's head). Best effort: a sibling that
    /// cannot be inspected or removed is left, and logged by count only.
    ///
    /// Every write of this process runs under the [`Keeper`]'s lock, and the
    /// shell runs once per session, so no sibling of a write in progress is
    /// ever swept from under it.
    fn sweep(&self) {
        let (Some(directory), Some(name)) = (
            self.path.parent(),
            self.path.file_name().and_then(|name| name.to_str()),
        ) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        let Ok(user) = xdg::effective_uid() else {
            return;
        };
        let mut left = 0_usize;
        for entry in entries.take(MAX_SWEEP_ENTRIES).flatten() {
            let file_name = entry.file_name();
            let Some(candidate) = file_name.to_str() else {
                continue;
            };
            if !is_stray_temporary(name, candidate) {
                continue;
            }
            // `symlink_metadata` never follows: a link of that name is not
            // ours and is left alone.
            let path = entry.path();
            let ours = std::fs::symlink_metadata(&path)
                .is_ok_and(|metadata| metadata.file_type().is_file() && metadata.uid() == user);
            if ours && std::fs::remove_file(&path).is_err() {
                left += 1;
            }
        }
        if left > 0 {
            tracing::warn!(
                left,
                "clipboard: stray history temporaries were not removed"
            );
        }
    }
}

/// Whether `candidate` is the name the atomic write gives a sibling of
/// `name`: `.<name>.<pid>-<sequence>.tmp`, both numbers decimal.
fn is_stray_temporary(name: &str, candidate: &str) -> bool {
    let Some(middle) = candidate
        .strip_prefix('.')
        .and_then(|rest| rest.strip_prefix(name))
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|rest| rest.strip_suffix(".tmp"))
    else {
        return false;
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    middle
        .split_once('-')
        .is_some_and(|(pid, sequence)| digits(pid) && digits(sequence))
}

/// The store as one clipboard worker may use it: every call holds one lock,
/// and once [`Keeper::revoke_and_delete`] has run, nothing more is read or
/// written through this keeper (or any clone of it).
///
/// A worker gets a fresh keeper each time the duty turns on; the duty keeps a
/// clone of it. Turning the duty off revokes and deletes under the lock: a
/// save already under way finishes first (and its file is then deleted), and
/// a save that comes later finds the revocation and writes nothing.
#[derive(Debug, Clone)]
pub struct Keeper(Arc<Mutex<Kept>>);

#[derive(Debug)]
struct Kept {
    store: Store,
    revoked: bool,
}

impl Keeper {
    #[must_use]
    pub fn new(store: Store) -> Self {
        Self(Arc::new(Mutex::new(Kept {
            store,
            revoked: false,
        })))
    }

    fn lock(&self) -> MutexGuard<'_, Kept> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// [`Store::load`], or an empty history once revoked.
    #[must_use]
    pub fn load(&self, now: u64) -> Loaded {
        let kept = self.lock();
        if kept.revoked {
            return Loaded::empty(None);
        }
        kept.store.load(now)
    }

    /// [`Store::save`] unless revoked. Returns whether it wrote.
    ///
    /// # Errors
    ///
    /// As [`Store::save`].
    pub fn save(&self, history: &History, now: u64) -> Result<bool, SaveError> {
        let kept = self.lock();
        if kept.revoked {
            return Ok(false);
        }
        kept.store.save(history, now).map(|_| true)
    }

    /// [`Store::delete`] unless revoked (the revoker deletes). Returns
    /// whether it ran.
    ///
    /// # Errors
    ///
    /// As [`Store::delete`].
    pub fn delete(&self) -> Result<bool, DeleteError> {
        let kept = self.lock();
        if kept.revoked {
            return Ok(false);
        }
        kept.store.delete().map(|()| true)
    }

    /// Revokes this keeper for good and deletes the file and its strays,
    /// under the lock every save takes.
    ///
    /// # Errors
    ///
    /// As [`Store::delete`]; the keeper is revoked either way.
    pub fn revoke_and_delete(&self) -> Result<(), DeleteError> {
        let mut kept = self.lock();
        kept.revoked = true;
        kept.store.delete()
    }

    /// Whether [`Keeper::revoke_and_delete`] has run.
    #[must_use]
    pub fn revoked(&self) -> bool {
        self.lock().revoked
    }
}

/// The bytes of the file at `path` when it passes every check, `None` when it
/// does not exist.
fn read_private(path: &Path) -> Result<Option<Vec<u8>>, Refusal> {
    let descriptor = match rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY,
        rustix::fs::Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(rustix::io::Errno::LOOP) => return Err(Refusal::Symlink),
        Err(errno) => return Err(Refusal::Unreadable(io::Error::from(errno))),
    };
    let file = File::from(descriptor);
    let metadata = file.metadata().map_err(Refusal::Unreadable)?;
    let user = xdg::effective_uid().map_err(Refusal::UnknownUser)?;
    inspect(
        metadata.is_file(),
        metadata.uid(),
        metadata.permissions().mode(),
        metadata.len(),
        user,
    )?;
    read_capped(file).map(Some)
}

/// The checks made on the open descriptor's metadata, apart from the
/// filesystem so each refusal is testable.
fn inspect(is_file: bool, owner: u32, mode: u32, len: u64, user: u32) -> Result<(), Refusal> {
    if !is_file {
        return Err(Refusal::NotRegular);
    }
    if owner != user {
        return Err(Refusal::NotOwned { owner, user });
    }
    if mode & 0o077 != 0 {
        return Err(Refusal::Shared {
            mode: mode & 0o7777,
        });
    }
    if len > MAX_PERSISTED_BYTES {
        return Err(Refusal::TooLarge);
    }
    Ok(())
}

/// Reads at most one byte past [`MAX_PERSISTED_BYTES`], so a file that grew
/// after `fstat` is refused rather than read whole.
fn read_capped(file: File) -> Result<Vec<u8>, Refusal> {
    let mut bytes = Vec::new();
    file.take(MAX_PERSISTED_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(Refusal::Unreadable)?;
    if bytes.len() as u64 > MAX_PERSISTED_BYTES {
        return Err(Refusal::TooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::clipboard::{Content, Kind, MAX_AGE_SECS};
    use std::os::unix::fs::{symlink, OpenOptionsExt};

    const NOW: u64 = 1_800_000_000;
    const PNG: [u8; 12] = [
        0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13,
    ];

    /// An empty state home for one test, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "celestina-clipboard-store-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a test directory");
            Self(dir)
        }

        fn store(&self) -> Store {
            Store::in_state_home(&self.0)
        }

        fn state_dir(&self) -> PathBuf {
            self.0.join(STATE_DIR)
        }

        fn private_dir(&self) {
            xdg::ensure_private_dir(&self.state_dir()).expect("state directory");
        }

        /// Writes `bytes` at the store's path with `mode`.
        fn plant(&self, bytes: &[u8], mode: u32) {
            self.private_dir();
            let path = path_in(&self.0);
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(mode)
                .open(&path)
                .and_then(|mut file| io::Write::write_all(&mut file, bytes))
                .expect("planted file");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("mode");
        }

        /// The names in the state directory other than the history itself.
        fn strays(&self) -> Vec<String> {
            std::fs::read_dir(self.state_dir())
                .expect("state directory")
                .map(|entry| {
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .filter(|name| name != FILE_NAME)
                .collect()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Undo a test's read-only directory so the tree can be removed.
            let _ =
                std::fs::set_permissions(self.state_dir(), std::fs::Permissions::from_mode(0o700));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn texts(history: &History) -> Vec<&str> {
        history
            .entries()
            .iter()
            .filter_map(|entry| entry.content().as_text())
            .collect()
    }

    fn history_of(items: &[(&str, u64)]) -> History {
        let mut history = History::new();
        for (text, at) in items {
            history.record(Content::text((*text).to_owned()).expect("recordable"), *at);
        }
        history
    }

    fn mode_of(path: &Path) -> u32 {
        std::fs::symlink_metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o7777
    }

    fn save_ok(store: &Store, history: &History, now: u64) {
        if let Err(error) = store.save(history, now) {
            panic!("save failed: {error}");
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn the_path_is_under_the_state_home() {
        assert_eq!(
            path_in(Path::new("/state")),
            Path::new("/state/celestina/clipboard.json")
        );
    }

    #[test]
    fn a_missing_file_is_an_empty_history_without_a_refusal() {
        let scratch = Scratch::new("missing");
        let loaded = scratch.store().load(NOW);
        assert!(loaded.history.is_empty());
        assert!(loaded.refusal.is_none());
    }

    #[test]
    fn a_saved_history_loads_back_in_order() {
        let scratch = Scratch::new("round-trip");
        let store = scratch.store();
        let history = history_of(&[
            ("first", NOW - 30),
            ("second", NOW - 20),
            ("third", NOW - 10),
        ]);
        save_ok(&store, &history, NOW);

        let loaded = store.load(NOW);
        assert!(loaded.refusal.is_none(), "{:?}", loaded.refusal);
        assert_eq!(texts(&loaded.history), ["third", "second", "first"]);
        assert_eq!(loaded.history.entries()[0].captured_at(), NOW - 10);
        assert!(scratch.strays().is_empty(), "{:?}", scratch.strays());
    }

    #[test]
    fn the_file_is_0600_and_its_directory_0700() {
        let scratch = Scratch::new("modes");
        let store = scratch.store();
        save_ok(&store, &history_of(&[("x", NOW)]), NOW);
        assert_eq!(mode_of(store.path()), 0o600);
        assert_eq!(mode_of(&scratch.state_dir()), 0o700);
    }

    #[test]
    fn a_wider_directory_of_this_user_is_tightened_on_save() {
        let scratch = Scratch::new("wide-dir");
        std::fs::create_dir(scratch.state_dir()).expect("dir");
        std::fs::set_permissions(scratch.state_dir(), std::fs::Permissions::from_mode(0o755))
            .expect("mode");
        save_ok(&scratch.store(), &history_of(&[("x", NOW)]), NOW);
        assert_eq!(mode_of(&scratch.state_dir()), 0o700);
    }

    #[test]
    fn images_never_reach_the_file() {
        let scratch = Scratch::new("images");
        let store = scratch.store();
        let mut history = History::new();
        history.record(Content::text("words".to_owned()).expect("text"), NOW - 2);
        history.record(Content::png(PNG.to_vec()).expect("png"), NOW - 1);
        assert_eq!(history.len(), 2);
        save_ok(&store, &history, NOW);

        let raw = std::fs::read(store.path()).expect("file");
        assert!(!contains(&raw, b"PNG"));
        let loaded = store.load(NOW);
        assert_eq!(texts(&loaded.history), ["words"]);
        assert!(loaded
            .history
            .entries()
            .iter()
            .all(|entry| entry.content().kind() == Kind::Text));
    }

    #[test]
    fn expired_entries_are_not_saved() {
        let scratch = Scratch::new("expired-save");
        let store = scratch.store();
        let history = history_of(&[("stale", NOW - MAX_AGE_SECS), ("fresh", NOW - 1)]);
        save_ok(&store, &history, NOW);
        let raw = std::fs::read(store.path()).expect("file");
        assert!(!contains(&raw, b"stale"));
        assert_eq!(texts(&store.load(NOW).history), ["fresh"]);
    }

    #[test]
    fn expired_entries_are_dropped_on_load() {
        let scratch = Scratch::new("expired-load");
        let store = scratch.store();
        save_ok(
            &store,
            &history_of(&[("aging", NOW - 10), ("young", NOW)]),
            NOW,
        );
        let later = NOW - 10 + MAX_AGE_SECS;
        let loaded = store.load(later);
        assert!(loaded.refusal.is_none());
        assert_eq!(texts(&loaded.history), ["young"]);
    }

    #[test]
    fn a_symlink_is_refused_and_replaced_by_a_file_on_save() {
        let scratch = Scratch::new("symlink");
        let store = scratch.store();
        // A loadable history elsewhere, so only the link itself is at fault.
        let target = scratch.0.join("elsewhere.json");
        let planted =
            clipboard::encode(history_of(&[("linked", NOW)]).entries(), NOW).expect("encodable");
        std::fs::write(&target, &planted).expect("target");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).expect("mode");
        scratch.private_dir();
        symlink(&target, store.path()).expect("symlink");

        let loaded = store.load(NOW);
        assert!(loaded.history.is_empty());
        assert!(
            matches!(loaded.refusal, Some(Refusal::Symlink)),
            "{:?}",
            loaded.refusal
        );

        save_ok(&store, &history_of(&[("mine", NOW)]), NOW);
        let metadata = std::fs::symlink_metadata(store.path()).expect("metadata");
        assert!(metadata.file_type().is_file());
        assert_eq!(texts(&store.load(NOW).history), ["mine"]);
        // The link's target was never written through.
        assert_eq!(std::fs::read(&target).expect("target"), planted);
    }

    #[test]
    fn a_shared_mode_is_refused_and_replaced_on_save() {
        let scratch = Scratch::new("wrong-mode");
        let store = scratch.store();
        save_ok(&store, &history_of(&[("readable", NOW)]), NOW);
        std::fs::set_permissions(store.path(), std::fs::Permissions::from_mode(0o644))
            .expect("mode");

        let loaded = store.load(NOW);
        assert!(loaded.history.is_empty());
        assert!(
            matches!(loaded.refusal, Some(Refusal::Shared { mode: 0o644 })),
            "{:?}",
            loaded.refusal
        );

        save_ok(&store, &history_of(&[("private", NOW)]), NOW);
        assert_eq!(mode_of(store.path()), 0o600);
        assert_eq!(texts(&store.load(NOW).history), ["private"]);
    }

    #[test]
    fn garbage_is_refused_without_content_and_replaced_on_save() {
        let scratch = Scratch::new("garbage");
        let store = scratch.store();
        scratch.plant(b"{\"entries\": hunter2-not-json", 0o600);

        let loaded = store.load(NOW);
        assert!(loaded.history.is_empty());
        let refusal = loaded.refusal.expect("a refusal");
        assert!(matches!(refusal, Refusal::Undecodable(_)), "{refusal:?}");
        assert!(!format!("{refusal} {refusal:?}").contains("hunter2"));

        save_ok(&store, &history_of(&[("clean", NOW)]), NOW);
        assert_eq!(texts(&store.load(NOW).history), ["clean"]);
    }

    #[test]
    fn another_schema_is_refused() {
        let scratch = Scratch::new("schema");
        scratch.plant(br#"{"schema_version":99,"entries":[]}"#, 0o600);
        let loaded = scratch.store().load(NOW);
        assert!(matches!(loaded.refusal, Some(Refusal::Undecodable(_))));
        assert!(loaded.history.is_empty());
    }

    #[test]
    fn an_oversized_file_is_refused_and_replaced_on_save() {
        let scratch = Scratch::new("oversized");
        let store = scratch.store();
        scratch.plant(b"{}", 0o600);
        // Sparse: the length passes the bound without writing the bytes.
        std::fs::OpenOptions::new()
            .write(true)
            .open(store.path())
            .and_then(|file| file.set_len(MAX_PERSISTED_BYTES + 1))
            .expect("grown file");

        let loaded = store.load(NOW);
        assert!(loaded.history.is_empty());
        assert!(
            matches!(loaded.refusal, Some(Refusal::TooLarge)),
            "{:?}",
            loaded.refusal
        );

        save_ok(&store, &history_of(&[("small", NOW)]), NOW);
        assert_eq!(texts(&store.load(NOW).history), ["small"]);
    }

    #[test]
    fn a_directory_in_its_place_is_refused() {
        let scratch = Scratch::new("directory");
        std::fs::create_dir_all(path_in(&scratch.0)).expect("dir");
        let loaded = scratch.store().load(NOW);
        assert!(loaded.history.is_empty());
        assert!(
            matches!(loaded.refusal, Some(Refusal::NotRegular)),
            "{:?}",
            loaded.refusal
        );
    }

    #[test]
    fn metadata_checks_refuse_each_hostile_case() {
        let user = 1000;
        assert!(inspect(true, user, 0o100_600, 10, user).is_ok());
        assert!(matches!(
            inspect(false, user, 0o600, 10, user),
            Err(Refusal::NotRegular)
        ));
        assert!(matches!(
            inspect(true, 0, 0o600, 10, user),
            Err(Refusal::NotOwned {
                owner: 0,
                user: 1000
            })
        ));
        assert!(matches!(
            inspect(true, user, 0o100_640, 10, user),
            Err(Refusal::Shared { mode: 0o640 })
        ));
        assert!(matches!(
            inspect(true, user, 0o602, 10, user),
            Err(Refusal::Shared { mode: 0o602 })
        ));
        assert!(matches!(
            inspect(true, user, 0o600, MAX_PERSISTED_BYTES + 1, user),
            Err(Refusal::TooLarge)
        ));
        assert!(inspect(true, user, 0o600, MAX_PERSISTED_BYTES, user).is_ok());
    }

    #[test]
    fn a_failed_write_leaves_the_old_file_and_no_temporary() {
        let scratch = Scratch::new("failed-write");
        let store = scratch.store();
        save_ok(&store, &history_of(&[("kept", NOW)]), NOW);
        let before = std::fs::read(store.path()).expect("file");

        // A directory its owner cannot write into: the temporary cannot be
        // created. Owner-only bits, so the private-directory check accepts it.
        std::fs::set_permissions(scratch.state_dir(), std::fs::Permissions::from_mode(0o500))
            .expect("mode");
        let result = store.save(&history_of(&[("lost", NOW)]), NOW);
        std::fs::set_permissions(scratch.state_dir(), std::fs::Permissions::from_mode(0o700))
            .expect("mode");

        assert!(matches!(result, Err(SaveError::Write(_))), "{result:?}");
        assert_eq!(std::fs::read(store.path()).expect("file"), before);
        assert!(scratch.strays().is_empty(), "{:?}", scratch.strays());
        assert_eq!(texts(&store.load(NOW).history), ["kept"]);
    }

    #[test]
    fn a_save_error_carries_no_content() {
        let scratch = Scratch::new("save-error");
        // A file where the directory should be.
        std::fs::write(scratch.state_dir(), b"x").expect("file");
        let error = scratch
            .store()
            .save(&history_of(&[("s3cret-token", NOW)]), NOW)
            .expect_err("no directory");
        assert!(matches!(error, SaveError::Directory(_)), "{error:?}");
        assert!(!format!("{error} {error:?}").contains("s3cret"));
    }

    #[test]
    fn deletion_removes_the_file_and_is_idempotent() {
        let scratch = Scratch::new("delete");
        let store = scratch.store();
        save_ok(&store, &history_of(&[("gone", NOW)]), NOW);
        assert!(store.path().exists());

        assert!(store.delete().is_ok());
        assert!(!store.path().exists());
        assert!(store.delete().is_ok());
        let loaded = store.load(NOW);
        assert!(loaded.history.is_empty());
        assert!(loaded.refusal.is_none());
    }

    #[test]
    fn deletion_removes_a_symlink_not_its_target() {
        let scratch = Scratch::new("delete-link");
        let target = scratch.0.join("target");
        std::fs::write(&target, b"untouched").expect("target");
        scratch.private_dir();
        symlink(&target, scratch.store().path()).expect("symlink");

        assert!(scratch.store().delete().is_ok());
        assert!(std::fs::symlink_metadata(scratch.store().path()).is_err());
        assert_eq!(std::fs::read(&target).expect("target"), b"untouched");
    }

    /// Plants a stray temporary of an interrupted write.
    fn plant_stray(scratch: &Scratch, name: &str) -> PathBuf {
        scratch.private_dir();
        let path = scratch.state_dir().join(name);
        std::fs::write(&path, b"{\"entries\":[{\"text\":\"hunter2\"}]}").expect("stray");
        path
    }

    #[test]
    fn only_names_of_the_atomic_writes_shape_are_strays() {
        let name = FILE_NAME;
        assert!(is_stray_temporary(name, ".clipboard.json.1234-0.tmp"));
        assert!(is_stray_temporary(name, ".clipboard.json.1-99999.tmp"));
        for other in [
            "clipboard.json",
            ".clipboard.json.tmp",
            ".clipboard.json.-1.tmp",
            ".clipboard.json.12-.tmp",
            ".clipboard.json.12-3.tmp.bak",
            ".clipboard.json.a-3.tmp",
            ".shell.json.12-3.tmp",
            ".clipboard.jsonx.12-3.tmp",
        ] {
            assert!(!is_stray_temporary(name, other), "{other}");
        }
    }

    #[test]
    fn delete_removes_stray_temporaries_but_not_links_or_other_files() {
        let scratch = Scratch::new("strays-delete");
        let store = scratch.store();
        save_ok(&store, &history_of(&[("gone", NOW)]), NOW);
        let stray = plant_stray(&scratch, ".clipboard.json.4242-7.tmp");
        let other = plant_stray(&scratch, "notes.txt");
        let target = scratch.0.join("target");
        std::fs::write(&target, b"untouched").expect("target");
        let link = scratch.state_dir().join(".clipboard.json.4242-8.tmp");
        symlink(&target, &link).expect("symlink");

        assert!(store.delete().is_ok());
        assert!(!store.path().exists());
        assert!(!stray.exists(), "the plaintext stray is gone");
        assert!(other.exists(), "another file is not ours to remove");
        assert!(std::fs::symlink_metadata(&link).is_ok(), "a link is left");
        assert_eq!(std::fs::read(&target).expect("target"), b"untouched");
    }

    #[test]
    fn load_and_save_sweep_stray_temporaries() {
        let scratch = Scratch::new("strays-sweep");
        let store = scratch.store();
        let stray = plant_stray(&scratch, ".clipboard.json.1-1.tmp");
        let _ = store.load(NOW);
        assert!(!stray.exists(), "load swept it");
        let stray = plant_stray(&scratch, ".clipboard.json.1-2.tmp");
        save_ok(&store, &history_of(&[("x", NOW)]), NOW);
        assert!(!stray.exists(), "save swept it");
        assert!(scratch.strays().is_empty(), "{:?}", scratch.strays());
    }

    #[test]
    fn a_load_reports_what_it_left_out_as_stale() {
        let scratch = Scratch::new("stale");
        let store = scratch.store();
        save_ok(
            &store,
            &history_of(&[("aging", NOW - 10), ("young", NOW)]),
            NOW,
        );
        let fresh = store.load(NOW);
        assert_eq!((fresh.dropped, fresh.stale()), (0, false));
        let later = store.load(NOW - 10 + MAX_AGE_SECS);
        assert_eq!((later.dropped, later.stale()), (1, true));
        std::fs::set_permissions(store.path(), std::fs::Permissions::from_mode(0o644))
            .expect("mode");
        assert!(store.load(NOW).stale(), "a refused file is stale");
        assert!(!Scratch::new("stale-missing").store().load(NOW).stale());
    }

    #[test]
    fn a_revoked_keeper_never_writes_or_reads_again() {
        let scratch = Scratch::new("revoked");
        let keeper = Keeper::new(scratch.store());
        let worker_side = keeper.clone();
        assert!(matches!(
            worker_side.save(&history_of(&[("one", NOW)]), NOW),
            Ok(true)
        ));
        assert!(scratch.store().path().exists());

        assert!(keeper.revoke_and_delete().is_ok());
        assert!(keeper.revoked() && worker_side.revoked());
        assert!(!scratch.store().path().exists(), "deleted");
        assert!(
            matches!(
                worker_side.save(&history_of(&[("late", NOW)]), NOW),
                Ok(false)
            ),
            "a late save of the stopped worker writes nothing"
        );
        assert!(!scratch.store().path().exists(), "and the file stays gone");
        assert!(matches!(worker_side.delete(), Ok(false)));

        // A file that appears later (another keeper's) is not read through it.
        save_ok(&scratch.store(), &history_of(&[("new", NOW)]), NOW);
        assert!(worker_side.load(NOW).history.is_empty());
        assert_eq!(
            texts(&Keeper::new(scratch.store()).load(NOW).history),
            ["new"]
        );
    }

    #[test]
    fn a_save_in_progress_finishes_before_the_revocation_deletes() {
        let scratch = Scratch::new("revoke-race");
        let keeper = Keeper::new(scratch.store());
        // Hold the lock as a save in progress would, then revoke from another
        // thread: the revocation waits, and its delete comes after the write.
        let guard = keeper.lock();
        let revoker = {
            let keeper = keeper.clone();
            std::thread::spawn(move || keeper.revoke_and_delete().is_ok())
        };
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!revoker.is_finished(), "the revocation waits for the lock");
        save_ok(&guard.store, &history_of(&[("in flight", NOW)]), NOW);
        drop(guard);
        assert!(revoker.join().expect("revoker"));
        assert!(
            !scratch.store().path().exists(),
            "the write was deleted after"
        );
    }
}
