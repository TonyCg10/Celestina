//! The two settings the shell reads.
//!
//! - The suite's appearance file (`celestina-settings`): followed on its own
//!   thread for as long as the returned [`Follower`] lives, each change
//!   published as [`Message::Appearance`].
//! - The shell's own `shell.json` (do-not-disturb, the bar's layout, the
//!   duties, the energy switches, the wallpaper): read at start through
//!   `celestina_shell::settings`, followed live by [`follow_shell`] (each new
//!   value published as [`Message::ShellSettings`]), and rewritten whole and
//!   atomically (`celestina_core::atomic_file::replace_private`) when the
//!   person changes it. A file that exists but cannot be read is never
//!   overwritten.
//!
//! [`follow_shell`] is the one watcher of `shell.json`. A value that flips
//! the `bar` duty moves every Island to a new surface, so its deliveries are
//! paced ([`RESHAPE_GAP`]) on a second thread of the follower that only
//! delays; it never reads the file. No bus name is involved: the `bar` duty
//! has no observable owner to probe, and the layer namespace it draws in is
//! the shell's own.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use celestina_core::atomic_file::{self, Published, WriteError};
use celestina_settings::Follower;
use celestina_shell::settings::{
    self, Duty, Energy, Pending, Settings, Store, WallpaperSettings, WriteOutcome, MAX_FILE_BYTES,
};
use celestina_wayland::Sender;
use notify_debouncer_full::notify::{self, EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::message::Message;
use crate::providers::Worker;

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
/// The shortest time between two delivered flips of the `bar` duty: each one
/// gives every output a new surface, so a file flipped back and forth (a
/// script, a sync tool, an editor saving as the person types) moves them at
/// most once a second, to the latest value.
pub const RESHAPE_GAP: Duration = Duration::from_secs(1);
/// How often the pacing thread looks at the shutdown flag: the bound on its
/// join.
const PACE_POLL: Duration = Duration::from_millis(100);

/// A running follower of `shell.json`; dropping it stops the watch and its
/// threads (the watch first, then the pacing, which is joined).
pub struct ShellFollower {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    /// The thread that paces deliveries ([`RESHAPE_GAP`]); absent for a
    /// follower that delivers every read at once.
    _pace: Option<Worker>,
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
    /// The thread that paces the deliveries could not start.
    #[error("cannot start the thread that paces the shell settings")]
    Pace(#[source] std::io::Error),
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
/// Every read goes through [`Pace`] on a thread named `shell-settings-pace`
/// before it reaches the loop: a flip of the `bar` duty within
/// [`RESHAPE_GAP`] of the last one waits, at its latest value. The thread
/// looks at `shutdown` every [`PACE_POLL`] and is joined when the follower
/// drops.
///
/// # Errors
///
/// [`FollowError`] when there is no file, its directory cannot be created,
/// the watcher cannot start or the pacing thread cannot start; the shell then
/// keeps the value it read at start.
pub fn follow_shell(
    from: FollowFrom,
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> Result<ShellFollower, FollowError> {
    let pace = Pace::new(&from.start);
    let (reads, paced) = mpsc::channel();
    let mut follower = watch_shell(from, move |read| {
        // Gone only once the pacing stopped, which is the shell's exit; a
        // read sent before the pacing starts waits in the queue.
        let _ = reads.send(read);
    })?;
    // Started once the watch is: a worker dropped raises the shell's
    // shutdown flag, so one is never made only to be thrown away.
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("shell-settings-pace", shutdown, move || {
        pace_reads(&paced, pace, &flag, |read| {
            let delivered = sender.send(Message::ShellSettings(Box::new(read))).is_ok();
            if !delivered {
                tracing::debug!("the shell settings changed after the loop stopped");
            }
            delivered
        });
    })
    .map_err(FollowError::Pace)?;
    follower._pace = Some(worker);
    Ok(follower)
}

/// The pace of delivered flips ([`RESHAPE_GAP`]). A read that flips the
/// `bar` duty within the gap of the last delivered flip is held, and so is
/// every read after it until the gap has passed, when the latest one is
/// delivered; the rest go at once.
#[derive(Debug)]
struct Pace {
    /// The duty of the settings delivered last.
    bar: Duty,
    /// When the last delivered flip went.
    flipped: Option<Instant>,
    /// The latest read held back.
    held: Option<ShellRead>,
}

impl Pace {
    fn new(start: &Settings) -> Self {
        Self {
            bar: start.duties.bar(),
            flipped: None,
            held: None,
        }
    }

    fn within_gap(&self, now: Instant) -> bool {
        self.flipped.is_some_and(|at| now < at + RESHAPE_GAP)
    }

    /// `read` to deliver now, or `None` while it is held.
    fn offer(&mut self, read: ShellRead, now: Instant) -> Option<ShellRead> {
        if self.held.is_some() || (read.settings.duties.bar() != self.bar && self.within_gap(now)) {
            self.held = Some(read);
            return None;
        }
        Some(self.delivered(read, now))
    }

    /// The read held back, once the gap has passed.
    fn due(&mut self, now: Instant) -> Option<ShellRead> {
        if self.within_gap(now) {
            return None;
        }
        let read = self.held.take()?;
        Some(self.delivered(read, now))
    }

    fn delivered(&mut self, read: ShellRead, now: Instant) -> ShellRead {
        if read.settings.duties.bar() != self.bar {
            self.bar = read.settings.duties.bar();
            self.flipped = Some(now);
        }
        read
    }
}

/// The pacing thread's body: each read from the watcher goes through `pace`
/// into `deliver`, which answers whether anyone still listens.
fn pace_reads(
    reads: &mpsc::Receiver<ShellRead>,
    mut pace: Pace,
    shutdown: &AtomicBool,
    mut deliver: impl FnMut(ShellRead) -> bool,
) {
    while !shutdown.load(Ordering::Acquire) {
        let fresh = match reads.recv_timeout(PACE_POLL) {
            Ok(read) => pace.offer(read, Instant::now()),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let held = pace.due(Instant::now());
        if !fresh.into_iter().chain(held).all(&mut deliver) {
            break;
        }
    }
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
    /// The write count of the read delivered last.
    delivered: u64,
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
                // A read after an own write is delivered even when equal to
                // the last one: the last one may have been ignored as older
                // than that write, and the write may have kept an edit it
                // carried (`set_do_not_disturb` merges onto the file).
                if read != self.last || self.dirty || generation > self.delivered {
                    self.dirty = false;
                    self.delivered = generation;
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
        delivered: from.writes.load(Ordering::Acquire),
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
        _pace: None,
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
    /// The file is read again first and the switch is written on top of what
    /// it holds, so an edit the follower has not delivered yet (the person
    /// turning the bar on by hand a moment earlier) is kept rather than
    /// replaced by the values in force. Only do-not-disturb changes in force
    /// here: the follower then delivers the edit and the app applies it like
    /// any other. A missing file is written from the values in force, as the
    /// follower keeps them; a file that cannot be read now is protected, as
    /// at start.
    ///
    /// # Errors
    ///
    /// [`ShellSettingsError`] when there is no file to write, the file is
    /// protected or cannot be read now, or the write failed; the previous
    /// value then stays in force.
    pub fn set_do_not_disturb(&mut self, on: bool) -> Result<bool, ShellSettingsError> {
        let pending = self.store.stage(|settings| settings.do_not_disturb = on);
        if pending.wanted() == self.store.current() || self.protected {
            return self.commit(pending);
        }
        let written = match self.path.as_deref().map(reread) {
            Some(Reread::Value(on_disk)) => {
                Store::new(on_disk).stage(|settings| settings.do_not_disturb = on)
            }
            Some(Reread::Unusable) => {
                // Broken since the last read: kept for the person, as at start.
                self.protected = true;
                return self.commit(pending);
            }
            Some(Reread::Missing) | None => pending.clone(),
        };
        self.commit_writing(pending, &written)
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
        let written = pending.clone();
        self.commit_writing(pending, &written)
    }

    /// Writes `written` and makes `pending` current only once it is on disk:
    /// the two differ by what the file held that is not in force yet.
    fn commit_writing(
        &mut self,
        pending: Pending,
        written: &Pending,
    ) -> Result<bool, ShellSettingsError> {
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
        let bytes = written.bytes().map_err(ShellSettingsError::Encode)?;
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
        use std::os::unix::fs::PermissionsExt;

        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        // After the start, the settings directory exists but cannot be
        // written: the file reads as missing, and the write fails.
        let directory = scratch.0.join("celestina");
        std::fs::create_dir(&directory).expect("the directory");
        let mode = |m| std::fs::Permissions::from_mode(m);
        std::fs::set_permissions(&directory, mode(0o500)).expect("read-only");

        let outcome = shell.set_do_not_disturb(true);
        std::fs::set_permissions(&directory, mode(0o700)).expect("writable again");
        assert!(
            matches!(outcome, Err(ShellSettingsError::Write(_))),
            "{outcome:?}"
        );
        assert!(!shell.current().do_not_disturb);
        // The settings directory becomes a file: an energy write cannot
        // create the file inside it, and do-not-disturb, which reads the
        // file first, finds it unreadable and does not write.
        std::fs::remove_dir(&directory).expect("removed");
        std::fs::write(&directory, b"in the way").expect("the obstacle");
        assert!(matches!(
            shell.set_energy(|energy| energy.caffeine = true),
            Err(ShellSettingsError::Write(_))
        ));
        assert!(matches!(
            shell.set_do_not_disturb(true),
            Err(ShellSettingsError::Protected { .. })
        ));
        assert!(!shell.current().do_not_disturb);
    }

    fn bar_on() -> Settings {
        let mut settings = Settings::default();
        settings.duties.bar = Some(Duty::On);
        settings
    }

    #[test]
    fn a_do_not_disturb_write_keeps_an_edit_the_follower_has_not_delivered() {
        let scratch = Scratch::new("merge");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        // The person turns the bar on by hand; the follower has not read it.
        publish(&scratch, &bar_on());

        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        let written = std::fs::read(scratch.file()).expect("the file");
        let on_disk = Settings::from_bytes(&written).expect("the shell's own bytes");
        assert!(on_disk.do_not_disturb);
        assert_eq!(on_disk.duties.bar(), Duty::On, "the edit survives");
        // In force: the switch, not yet the edit, which the follower brings
        // (its read begins after the write) so the app applies it.
        assert!(shell.current().do_not_disturb);
        assert_eq!(shell.current().duties.bar(), Duty::Off);
        assert!(shell.adopt_read(ShellRead {
            settings: on_disk,
            generation: 1,
        }));
        assert_eq!(shell.current().duties.bar(), Duty::On);

        // A file broken since the last read is not overwritten either.
        std::fs::write(scratch.file(), b"{ not json").expect("a broken file");
        assert!(matches!(
            shell.set_do_not_disturb(false),
            Err(ShellSettingsError::Protected { .. })
        ));
        assert_eq!(
            std::fs::read(scratch.file()).expect("the file"),
            b"{ not json"
        );
        assert!(shell.current().do_not_disturb, "the value in force is kept");
    }

    #[test]
    fn a_do_not_disturb_write_over_a_missing_file_keeps_the_values_in_force() {
        let scratch = Scratch::new("merge-missing");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        assert!(shell.adopt(bar_on()));
        // Between the delete and the create of someone else's save.
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        let on_disk = ShellSettings::load_from(Some(scratch.file()));
        assert_eq!(on_disk.current().duties.bar(), Duty::On);
        assert!(on_disk.current().do_not_disturb);
    }

    #[test]
    fn a_read_after_an_own_write_is_delivered_even_when_equal_to_the_last() {
        let scratch = Scratch::new("own-write-echo");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let (watch, rx) = {
            let (tx, rx) = std::sync::mpsc::channel();
            let watch = watch_shell(shell.follow_from(), move |read| {
                let _ = tx.send(read);
            })
            .expect("the watch starts");
            (watch, rx)
        };
        // The person's edit is read (and, say, ignored by the app as older
        // than the write that follows it)...
        let mut edit = bar_on();
        edit.do_not_disturb = true;
        publish(&scratch, &edit);
        assert_eq!(
            rx.recv_timeout(DELIVERED).map(|read| read.settings),
            Ok(edit.clone())
        );
        // ...then the shell writes the same switch onto it: the file is the
        // edit again, and it comes back so the app can adopt it.
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        let echo = rx.recv_timeout(DELIVERED).expect("delivered again");
        assert_eq!(echo.settings, edit);
        assert_eq!(echo.generation, 1);
        assert!(shell.adopt_read(echo));
        assert_eq!(shell.current(), &edit);
        drop(watch);
    }

    /// `settings` as a read begun before any own write.
    fn read_of(settings: Settings) -> ShellRead {
        ShellRead {
            settings,
            generation: 0,
        }
    }

    #[test]
    fn duty_flips_are_delivered_a_second_apart_at_their_latest_value() {
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        let off = Settings::default();
        let mut quiet_on = bar_on();
        quiet_on.do_not_disturb = true;
        let mut pace = Pace::new(&off);
        // The first flip goes at once.
        assert_eq!(pace.offer(read_of(bar_on()), t0), Some(read_of(bar_on())));
        // Flipped back and forth within the second: held, the latest kept.
        assert_eq!(pace.offer(read_of(off.clone()), t0 + ms(300)), None);
        assert_eq!(pace.offer(read_of(quiet_on.clone()), t0 + ms(500)), None);
        assert_eq!(pace.due(t0 + ms(900)), None);
        assert_eq!(pace.due(t0 + RESHAPE_GAP), Some(read_of(quiet_on.clone())));
        assert_eq!(pace.due(t0 + RESHAPE_GAP), None, "delivered once");
        // That was no flip (on, then on), so the next one goes at once; a
        // change that flips nothing never waits.
        assert_eq!(
            pace.offer(read_of(off.clone()), t0 + ms(1100)),
            Some(read_of(off.clone()))
        );
        let mut quiet_off = off.clone();
        quiet_off.do_not_disturb = true;
        assert_eq!(
            pace.offer(read_of(quiet_off.clone()), t0 + ms(1200)),
            Some(read_of(quiet_off))
        );
        assert_eq!(pace.offer(read_of(bar_on()), t0 + ms(1300)), None);
        assert_eq!(
            pace.due(t0 + ms(1100) + RESHAPE_GAP),
            Some(read_of(bar_on()))
        );
    }

    #[test]
    fn the_pacing_thread_delivers_a_held_flip_late_and_stops_on_shutdown() {
        let (reads, paced) = std::sync::mpsc::channel();
        let (deliver, delivered) = std::sync::mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&shutdown);
        let worker = Worker::spawn("test-shell-settings-pace", &shutdown, move || {
            pace_reads(&paced, Pace::new(&Settings::default()), &flag, |read| {
                deliver.send(read).is_ok()
            });
        })
        .expect("the thread starts");
        let started = Instant::now();
        reads.send(read_of(bar_on())).expect("sent");
        reads.send(read_of(Settings::default())).expect("sent");
        let first = delivered.recv_timeout(DELIVERED).expect("the first flip");
        assert_eq!(first.settings, bar_on());
        let second = delivered.recv_timeout(DELIVERED).expect("the held flip");
        assert_eq!(second.settings, Settings::default());
        assert!(started.elapsed() >= RESHAPE_GAP, "{:?}", started.elapsed());
        let joining = Instant::now();
        worker.join();
        assert!(
            joining.elapsed() < Duration::from_secs(1),
            "joined promptly"
        );
    }

    #[test]
    fn a_followed_value_is_adopted_once_and_unprotects_the_file() {
        let scratch = Scratch::new("adopt");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), b"{ half a file").expect("the corrupt file");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let mut read = Settings::default();
        read.duties.wallpaper = celestina_shell::settings::Duty::On;
        // The file now reads cleanly (do-not-disturb reads it before it
        // writes), so the shell may write it.
        publish(&scratch, &read);
        assert!(shell.adopt(read.clone()));
        assert!(!shell.adopt(read), "the same value again is no change");
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
