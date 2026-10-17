//! The two settings the shell reads.
//!
//! - The suite's appearance file (`celestina-settings`): followed on its own
//!   thread for as long as the returned [`Follower`] lives, each change
//!   published as [`Message::Appearance`].
//! - The shell's own `shell.json` (do-not-disturb, the duties, the energy
//!   switches, the wallpaper): read at start through `celestina_shell::settings`, followed
//!   live by [`follow_shell`] (each new value published as
//!   [`Message::ShellSettings`]), and rewritten whole and atomically
//!   (`celestina_core::atomic_file::replace_private`) when the person changes
//!   it. A file that exists but cannot be read is never overwritten.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use celestina_core::atomic_file::{self, Published, WriteError};
use celestina_settings::Follower;
use celestina_shell::settings::{
    self, Energy, Pending, Settings, Store, WallpaperSettings, WriteOutcome, MAX_FILE_BYTES,
};
use celestina_wayland::Sender;
use notify_debouncer_full::notify::{self, EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::message::Message;

/// Follows the appearance file, publishing every value (the current one
/// first) into the loop. Dropping the [`Follower`] stops it.
#[must_use]
pub fn follow_appearance(sender: Sender<Message>) -> Follower {
    celestina_settings::follow(move |appearance| {
        if sender.send(Message::Appearance(appearance)).is_err() {
            tracing::debug!("the appearance changed after the loop stopped");
        }
    })
}

/// How long the follower of `shell.json` gathers events before reading it.
const FOLLOW_COALESCE: Duration = Duration::from_millis(300);

/// A running follower of `shell.json`; dropping it stops the watch and its
/// thread.
pub struct ShellFollower {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl std::fmt::Debug for ShellFollower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellFollower").finish_non_exhaustive()
    }
}

/// Why `shell.json` is not followed.
#[derive(Debug, thiserror::Error)]
pub enum FollowError {
    /// The settings have no file (no configuration directory).
    #[error("the shell settings have no file to follow")]
    NoFile,
    /// The settings directory could not be created, so it cannot be watched.
    #[error("cannot create {}", .path.display())]
    Directory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The watcher could not start.
    #[error("cannot watch {}", .path.display())]
    Watch {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
}

/// One clean read of `shell.json`, tagged with the shell's write count when
/// the read began.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellRead {
    /// What the file held.
    pub settings: Settings,
    /// How many of the shell's own writes had landed before the file was
    /// read: a read that began before a later own write may predate it, and
    /// [`ShellSettings::adopt_read`] ignores it rather than revert that write.
    pub generation: u64,
}

/// What the follower of `shell.json` starts from, taken from the
/// [`ShellSettings`] before they move into the app.
#[derive(Debug)]
pub struct FollowFrom {
    path: Option<PathBuf>,
    start: Settings,
    /// The start value is the defaults standing in for a file that could not
    /// be read: the first clean read is delivered even when equal.
    protected: bool,
    writes: Arc<AtomicU64>,
}

/// Follows `shell.json`, publishing every value that differs from the last
/// one (starting from what the shell read) as [`Message::ShellSettings`].
/// The watch is on the directory, as `celestina-settings` does, because a
/// file published by renaming a sibling over it is a new inode; bursts are
/// coalesced for 300 ms, and the file is read and parsed on the watcher's
/// thread, never on the loop's. The file is read once more right after the
/// watch starts, so an edit made between the start's read and the watch is
/// not missed. A missing file (between the delete and the create of a save)
/// keeps the value in force. The shell's own writes come back as values
/// equal to what it holds, which [`ShellSettings::adopt_read`] ignores.
///
/// # Errors
///
/// [`FollowError`] when there is no file, its directory cannot be created or
/// the watcher cannot start; the shell then keeps the value it read at start.
pub fn follow_shell(
    from: FollowFrom,
    sender: Sender<Message>,
) -> Result<ShellFollower, FollowError> {
    watch_shell(from, move |read| {
        if sender.send(Message::ShellSettings(read)).is_err() {
            tracing::debug!("the shell settings changed after the loop stopped");
        }
    })
}

/// The follower's state, shared by the watcher's thread and the read that
/// follows the watch's start.
struct Follow<D> {
    file: PathBuf,
    last: Settings,
    /// The last read failed (or the start's did): the next clean read is
    /// delivered even when equal to `last`, so the shell lifts its
    /// protection.
    dirty: bool,
    writes: Arc<AtomicU64>,
    deliver: D,
}

impl<D: FnMut(ShellRead)> Follow<D> {
    fn consider(&mut self) {
        // Taken before the read: every own write counted here is on disk.
        let generation = self.writes.load(Ordering::Acquire);
        match reread(&self.file) {
            Reread::Missing => {
                tracing::debug!(
                    "{} is missing; the settings in force stay",
                    self.file.display()
                );
            }
            Reread::Unusable => self.dirty = true,
            Reread::Value(read) => {
                if read != self.last || self.dirty {
                    self.dirty = false;
                    self.last = read.clone();
                    (self.deliver)(ShellRead {
                        settings: read,
                        generation,
                    });
                }
            }
        }
    }
}

/// Locks the follower's state; a panic in another holder leaves it usable
/// (it holds plain values).
fn lock<T>(state: &Mutex<T>) -> MutexGuard<'_, T> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// [`follow_shell`] with its sink given.
fn watch_shell(
    from: FollowFrom,
    deliver: impl FnMut(ShellRead) + Send + 'static,
) -> Result<ShellFollower, FollowError> {
    let file = from.path.ok_or(FollowError::NoFile)?;
    let directory = file
        .parent()
        .map(Path::to_path_buf)
        .ok_or(FollowError::NoFile)?;
    // inotify watches only a directory that exists, and the file must be
    // seen from its first write.
    std::fs::create_dir_all(&directory).map_err(|source| FollowError::Directory {
        path: directory.clone(),
        source,
    })?;
    let name = file.file_name().map(std::ffi::OsStr::to_os_string);
    let state = Arc::new(Mutex::new(Follow {
        file,
        last: from.start,
        dirty: from.protected,
        writes: from.writes,
        deliver,
    }));
    let watched = Arc::clone(&state);
    let mut debouncer = new_debouncer(FOLLOW_COALESCE, None, move |result: DebounceEventResult| {
        let concerned = match &result {
            Ok(events) => events.iter().any(|event| {
                !matches!(event.event.kind, EventKind::Access(_))
                    && event
                        .event
                        .paths
                        .iter()
                        .any(|changed| changed.file_name() == name.as_deref())
            }),
            // Lost events: the file may have changed unseen.
            Err(_) => true,
        };
        if concerned {
            lock(&watched).consider();
        }
    })
    .map_err(|source| FollowError::Watch {
        path: directory.clone(),
        source,
    })?;
    debouncer
        .watch(&directory, RecursiveMode::NonRecursive)
        .map_err(|source| FollowError::Watch {
            path: directory,
            source,
        })?;
    // What changed between the start's read and the watch.
    lock(&state).consider();
    Ok(ShellFollower {
        _debouncer: debouncer,
    })
}

/// What a read of `shell.json` found.
enum Reread {
    /// The file is gone: the value in force stays.
    Missing,
    /// The file cannot be read or is not this shell's JSON: the value in
    /// force stays.
    Unusable,
    Value(Settings),
}

fn reread(file: &Path) -> Reread {
    let limit = u64::try_from(MAX_FILE_BYTES).unwrap_or(u64::MAX);
    match atomic_file::read_bounded(file, limit) {
        Ok(None) => Reread::Missing,
        Ok(Some(bytes)) => match Settings::from_bytes(&bytes) {
            Some(read) => Reread::Value(read),
            None => {
                tracing::warn!(
                    "{} is not a settings file this shell can read; the settings in force stay",
                    file.display()
                );
                Reread::Unusable
            }
        },
        Err(error) => {
            tracing::warn!("the shell settings: {error}; the settings in force stay");
            Reread::Unusable
        }
    }
}

/// Why a change to the shell's settings was not made.
#[derive(Debug, thiserror::Error)]
pub enum ShellSettingsError {
    /// Neither `$XDG_CONFIG_HOME` nor `$HOME` is absolute.
    #[error("no configuration directory: neither XDG_CONFIG_HOME nor HOME is absolute")]
    NoConfigHome,
    /// The file existed at start but could not be read, so it is kept as it
    /// is for the person to inspect rather than replaced with defaults.
    #[error("{} could not be read at start, so it is not overwritten", .path.display())]
    Protected { path: PathBuf },
    #[error("the shell settings could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("the shell settings could not be written")]
    Write(#[source] WriteError),
}

/// The shell's own durable settings and the file they live in.
#[derive(Debug)]
pub struct ShellSettings {
    path: Option<PathBuf>,
    store: Store,
    protected: bool,
    /// How many of its writes landed; shared with the follower, which tags
    /// each read with it (see [`ShellRead::generation`]).
    writes: Arc<AtomicU64>,
}

impl ShellSettings {
    /// Reads `$XDG_CONFIG_HOME/celestina/shell.json` (or
    /// `~/.config/celestina/shell.json`).
    #[must_use]
    pub fn load() -> Self {
        Self::load_from(celestina_core::xdg::config_home().map(|home| settings::path_in(&home)))
    }

    /// Reads the settings at `path`; `None` is a session without a
    /// configuration directory, which runs on the defaults.
    ///
    /// A missing file is the defaults. A file that is too large, not a regular
    /// file, unreadable, not this shell's JSON or from a later schema is
    /// logged, the defaults are used, and the file is protected from being
    /// overwritten.
    #[must_use]
    pub fn load_from(path: Option<PathBuf>) -> Self {
        let Some(file) = path.as_deref() else {
            tracing::warn!("no configuration directory; the shell settings are not kept");
            return Self::fresh(None, false);
        };
        let limit = u64::try_from(MAX_FILE_BYTES).unwrap_or(u64::MAX);
        match atomic_file::read_bounded(file, limit) {
            Ok(None) => Self::fresh(path, false),
            Ok(Some(bytes)) => match Settings::from_bytes(&bytes) {
                Some(read) => Self {
                    path,
                    store: Store::new(read),
                    protected: false,
                    writes: Arc::default(),
                },
                None => {
                    tracing::warn!(
                        "{} is not a settings file this shell can read; using the defaults",
                        file.display()
                    );
                    Self::fresh(path, true)
                }
            },
            Err(error) => {
                tracing::warn!("the shell settings: {error}; using the defaults");
                Self::fresh(path, true)
            }
        }
    }

    fn fresh(path: Option<PathBuf>, protected: bool) -> Self {
        Self {
            path,
            store: Store::default(),
            protected,
            writes: Arc::default(),
        }
    }

    /// What the follower of the file starts from.
    #[must_use]
    pub fn follow_from(&self) -> FollowFrom {
        FollowFrom {
            path: self.path.clone(),
            start: self.store.current().clone(),
            protected: self.protected,
            writes: Arc::clone(&self.writes),
        }
    }

    /// What is in force: always what is on disk, or the defaults.
    #[must_use]
    pub fn current(&self) -> &Settings {
        self.store.current()
    }

    /// Turns do-not-disturb on or off, durably. Returns whether anything
    /// changed; the new value is in force only once the file is written.
    ///
    /// # Errors
    ///
    /// [`ShellSettingsError`] when there is no file to write, the file is
    /// protected, or the write failed; the previous value then stays in force.
    pub fn set_do_not_disturb(&mut self, on: bool) -> Result<bool, ShellSettingsError> {
        let pending = self.store.stage(|settings| settings.do_not_disturb = on);
        self.commit(pending)
    }

    /// Changes the energy switches, durably, the way do-not-disturb is
    /// changed. Returns whether anything changed.
    ///
    /// # Errors
    ///
    /// As [`ShellSettings::set_do_not_disturb`].
    pub fn set_energy(
        &mut self,
        change: impl FnOnce(&mut Energy),
    ) -> Result<bool, ShellSettingsError> {
        let pending = self.store.stage_energy(change);
        self.commit(pending)
    }

    /// Changes the wallpaper choices durably (the folder's files per output,
    /// the mode). Returns whether anything changed; the new value is in force
    /// only once the file is written.
    ///
    /// # Errors
    ///
    /// As [`ShellSettings::set_do_not_disturb`].
    pub fn update_wallpaper(
        &mut self,
        change: impl FnOnce(&mut WallpaperSettings),
    ) -> Result<bool, ShellSettingsError> {
        let pending = self.store.stage(|settings| change(&mut settings.wallpaper));
        self.commit(pending)
    }

    /// The file the settings live in, if there is one.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Takes what the follower read from the file as the value in force: the
    /// file is what is on disk, so it is current by definition, and a file
    /// that reads cleanly is no longer protected. Returns whether anything
    /// changed (the shell's own writes come back equal and change nothing).
    pub fn adopt(&mut self, read: Settings) -> bool {
        self.protected = false;
        if &read == self.store.current() {
            return false;
        }
        self.store = Store::new(read);
        true
    }

    /// [`ShellSettings::adopt`] for a follower's read, unless the read began
    /// before one of the shell's own writes landed: the file then holds that
    /// write, whose own event brings a newer read, and the older one must not
    /// revert it in the meantime.
    pub fn adopt_read(&mut self, read: ShellRead) -> bool {
        let landed = self.writes.load(Ordering::Acquire);
        if read.generation < landed {
            tracing::debug!(
                read = read.generation,
                landed,
                "a read older than the shell's own write is ignored"
            );
            return false;
        }
        self.adopt(read.settings)
    }

    /// Writes `pending` and makes it current only once it is on disk.
    fn commit(&mut self, pending: Pending) -> Result<bool, ShellSettingsError> {
        if pending.wanted() == self.store.current() {
            return Ok(false);
        }
        let path = self
            .path
            .as_deref()
            .ok_or(ShellSettingsError::NoConfigHome)?;
        if self.protected {
            return Err(ShellSettingsError::Protected {
                path: path.to_path_buf(),
            });
        }
        let bytes = pending.bytes().map_err(ShellSettingsError::Encode)?;
        match atomic_file::replace_private(path, &bytes) {
            Ok(published) => {
                if let Published::DirectoryNotSynced(error) = &published {
                    // The bytes are in place; only their survival across a
                    // power cut before the next filesystem commit is not
                    // guaranteed. Every later read sees them, so they are the
                    // value in force.
                    tracing::warn!(
                        "{} written but its directory not synced: {error}",
                        path.display()
                    );
                }
                // Counted once on disk: a read that began before is older.
                self.writes.fetch_add(1, Ordering::AcqRel);
                Ok(self.store.apply(pending, WriteOutcome::Durable))
            }
            Err(error) => {
                self.store.apply(pending, WriteOutcome::Failed);
                Err(ShellSettingsError::Write(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test, removed when the guard drops.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir()
                .join(format!("celestina-settings-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(&directory).expect("a test directory");
            Self(directory)
        }

        fn file(&self) -> PathBuf {
            settings::path_in(&self.0)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_file_is_the_defaults_and_a_change_is_written_and_read_back() {
        let scratch = Scratch::new("round-trip");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        assert!(!shell.current().do_not_disturb);

        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        assert!(shell.current().do_not_disturb);
        // The same value again is not a change and writes nothing.
        assert!(!shell.set_do_not_disturb(true).expect("nothing to write"));

        let again = ShellSettings::load_from(Some(scratch.file()));
        assert!(again.current().do_not_disturb);
        let text = std::fs::read_to_string(scratch.file()).expect("the file exists");
        assert!(text.contains("\"do_not_disturb\": true"));
    }

    #[test]
    fn an_energy_change_is_written_and_read_back() {
        let scratch = Scratch::new("energy");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        assert!(!shell.current().energy.caffeine);
        assert!(shell.set_energy(|e| e.caffeine = true).expect("written"));
        assert!(!shell.set_energy(|e| e.caffeine = true).expect("no change"));
        let again = ShellSettings::load_from(Some(scratch.file()));
        assert!(again.current().energy.caffeine);
    }

    #[test]
    fn the_file_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("private");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        shell.set_do_not_disturb(true).expect("the write succeeds");

        let mode = std::fs::metadata(scratch.file())
            .expect("the file exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn an_unreadable_file_is_kept_and_never_overwritten() {
        let scratch = Scratch::new("protected");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), b"{ half a file").expect("the corrupt file");

        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        assert!(!shell.current().do_not_disturb);

        assert!(matches!(
            shell.set_do_not_disturb(true),
            Err(ShellSettingsError::Protected { .. })
        ));
        assert!(!shell.current().do_not_disturb);
        assert_eq!(
            std::fs::read(scratch.file()).expect("the file is still there"),
            b"{ half a file"
        );
    }

    #[test]
    fn without_a_configuration_directory_nothing_is_written() {
        let mut shell = ShellSettings::load_from(None);

        assert!(matches!(
            shell.set_do_not_disturb(true),
            Err(ShellSettingsError::NoConfigHome)
        ));
        assert!(!shell.current().do_not_disturb);
    }

    #[test]
    fn a_failed_write_leaves_the_previous_value_in_force() {
        let scratch = Scratch::new("failed");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        // After the start, the settings directory becomes a file, so the
        // write cannot create the file inside it.
        std::fs::write(scratch.0.join("celestina"), b"in the way").expect("the obstacle");

        assert!(matches!(
            shell.set_do_not_disturb(true),
            Err(ShellSettingsError::Write(_))
        ));
        assert!(!shell.current().do_not_disturb);
    }

    #[test]
    fn a_followed_value_is_adopted_once_and_unprotects_the_file() {
        let scratch = Scratch::new("adopt");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), b"{ half a file").expect("the corrupt file");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let mut read = Settings::default();
        read.duties.wallpaper = celestina_shell::settings::Duty::On;
        assert!(shell.adopt(read.clone()));
        assert!(!shell.adopt(read), "the same value again is no change");
        // The file now reads cleanly, so the shell may write it.
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        assert_eq!(
            shell.current().duties.wallpaper,
            celestina_shell::settings::Duty::On
        );
    }

    /// A follower of `scratch`'s file started from what a load reads now,
    /// sending each read into a channel.
    fn follow(scratch: &Scratch) -> (ShellFollower, std::sync::mpsc::Receiver<ShellRead>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let from = ShellSettings::load_from(Some(scratch.file())).follow_from();
        let watch = watch_shell(from, move |read| {
            let _ = tx.send(read);
        })
        .expect("the watch starts");
        (watch, rx)
    }

    const DELIVERED: Duration = Duration::from_secs(5);
    const QUIET: Duration = Duration::from_millis(900);

    fn wallpaper_on() -> Settings {
        let mut wanted = Settings::default();
        wanted.duties.wallpaper = celestina_shell::settings::Duty::On;
        wanted
    }

    fn publish(scratch: &Scratch, settings: &Settings) {
        let bytes = settings.to_bytes().expect("bytes");
        let _ = atomic_file::replace_private(&scratch.file(), &bytes).expect("written");
    }

    #[test]
    fn the_follower_delivers_a_changed_file_once_and_ignores_an_equal_one() {
        let scratch = Scratch::new("follow");
        let (watch, rx) = follow(&scratch);
        let wanted = wallpaper_on();
        publish(&scratch, &wanted);
        let read = rx.recv_timeout(DELIVERED).expect("delivered");
        assert_eq!(read.settings, wanted);
        // The same value rewritten (the shell's own write) is not delivered.
        publish(&scratch, &wanted);
        assert!(rx.recv_timeout(QUIET).is_err());
        // A file this shell cannot read keeps the value in force silently.
        std::fs::write(scratch.file(), b"{ broken").expect("written");
        assert!(rx.recv_timeout(QUIET).is_err());
        // The first clean read after it is delivered even though it equals
        // the last value: the shell lifts its protection on it.
        publish(&scratch, &wanted);
        assert_eq!(
            rx.recv_timeout(DELIVERED).map(|read| read.settings),
            Ok(wanted)
        );
        drop(watch);
    }

    #[test]
    fn a_missing_file_keeps_the_value_in_force() {
        let scratch = Scratch::new("missing");
        let (watch, rx) = follow(&scratch);
        let wanted = wallpaper_on();
        publish(&scratch, &wanted);
        assert!(rx.recv_timeout(DELIVERED).is_ok());
        // Deleted (the first half of a delete-then-create save): no reset to
        // the defaults.
        std::fs::remove_file(scratch.file()).expect("removed");
        assert!(rx.recv_timeout(QUIET).is_err());
        let mut next = wanted.clone();
        next.do_not_disturb = true;
        publish(&scratch, &next);
        assert_eq!(
            rx.recv_timeout(DELIVERED).map(|read| read.settings),
            Ok(next)
        );
        drop(watch);
    }

    #[test]
    fn an_edit_between_the_load_and_the_watch_is_read() {
        let scratch = Scratch::new("gap");
        let (tx, rx) = std::sync::mpsc::channel();
        let from = ShellSettings::load_from(Some(scratch.file())).follow_from();
        // Written after the load, before the watch: no event will tell.
        let wanted = wallpaper_on();
        publish(&scratch, &wanted);
        let watch = watch_shell(from, move |read| {
            let _ = tx.send(read);
        })
        .expect("the watch starts");
        assert_eq!(
            rx.recv_timeout(DELIVERED).map(|read| read.settings),
            Ok(wanted)
        );
        drop(watch);
    }

    #[test]
    fn a_protected_start_delivers_the_first_clean_read_even_if_it_is_the_defaults() {
        let scratch = Scratch::new("protected-start");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), b"{ half a file").expect("the corrupt file");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let from = shell.follow_from();
        // The person repairs the file to exactly the defaults.
        publish(&scratch, &Settings::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let watch = watch_shell(from, move |read| {
            let _ = tx.send(read);
        })
        .expect("the watch starts");
        let read = rx.recv_timeout(DELIVERED).expect("delivered");
        assert_eq!(read.settings, Settings::default());
        assert!(!shell.adopt_read(read), "no change in value");
        // But the file is no longer protected.
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        drop(watch);
    }

    #[test]
    fn a_read_older_than_an_own_write_does_not_revert_it() {
        let scratch = Scratch::new("generation");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        // The follower began reading here (no own write landed yet) and saw
        // an external edit.
        let stale = ShellRead {
            settings: wallpaper_on(),
            generation: 0,
        };
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        assert!(!shell.adopt_read(stale));
        assert!(shell.current().do_not_disturb);
        // A read begun after the write is taken.
        let mut newer = wallpaper_on();
        newer.do_not_disturb = true;
        assert!(shell.adopt_read(ShellRead {
            settings: newer.clone(),
            generation: 1,
        }));
        assert_eq!(shell.current(), &newer);
    }

    #[test]
    fn without_a_file_nothing_is_followed() {
        let from = ShellSettings::load_from(None).follow_from();
        assert!(matches!(
            watch_shell(from, |_| {}),
            Err(FollowError::NoFile)
        ));
    }
}
