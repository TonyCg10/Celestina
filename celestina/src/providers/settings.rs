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
//! [`follow_shell`] is the one watcher of `shell.json`; every duty shares
//! it (ADR 0011). No bus name is involved: the bar has no observable owner
//! to probe, and the layer namespace it draws in is the shell's own.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use celestina_core::atomic_file::{self, Published, WriteError};
use celestina_settings::Follower;
use celestina_shell::settings::{
    self, Duties, Energy, LauncherView, Pending, Settings, Store, WallpaperSettings, WriteOutcome,
    MAX_FILE_BYTES,
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

/// A running follower of `shell.json`; dropping it stops the watch.
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
        if sender.send(Message::ShellSettings(Box::new(read))).is_err() {
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
                let read = *read;
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
    })
}

/// What a read of `shell.json` found.
enum Reread {
    /// The file is gone: the value in force stays.
    Missing,
    /// The file cannot be read or is not this shell's JSON: the value in
    /// force stays.
    Unusable,
    /// Boxed: the whole settings would size every variant.
    Value(Box<Settings>),
}

fn reread(file: &Path) -> Reread {
    let limit = u64::try_from(MAX_FILE_BYTES).unwrap_or(u64::MAX);
    match atomic_file::read_bounded(file, limit) {
        Ok(None) => Reread::Missing,
        Ok(Some(bytes)) => match Settings::from_bytes(&bytes) {
            Some(read) => Reread::Value(Box::new(read)),
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
    /// logged, the file is protected from being overwritten, and the shell
    /// runs **quiet**, every duty off ([`Duties::all_off`]) over the other
    /// defaults, until the follower's first clean read brings the file's
    /// real values: a hand-edited file with a typo may say `"off"` to a duty
    /// that records (the clipboard) or acts on hardware (idle, DDC), and the
    /// cutover's defaults of on are not its word. This quiet start is the
    /// only time protection empties the duties: a file that breaks later,
    /// while the shell runs, keeps the values in force
    /// ([`ShellSettings::is_protected`]).
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
                        "{} is not a settings file this shell can read; every duty stays off \
                         until it reads cleanly",
                        file.display()
                    );
                    Self::fresh(path, true)
                }
            },
            Err(error) => {
                tracing::warn!(
                    "the shell settings: {error}; every duty stays off until the file reads cleanly"
                );
                Self::fresh(path, true)
            }
        }
    }

    /// The settings without a readable file: the defaults, or, `protected`
    /// (the file exists but cannot be read), the defaults with every duty
    /// off.
    fn fresh(path: Option<PathBuf>, protected: bool) -> Self {
        let start = if protected {
            Settings {
                duties: Duties::all_off(),
                ..Settings::default()
            }
        } else {
            Settings::default()
        };
        Self {
            path,
            store: Store::new(start),
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
        self.set_merged(|settings| settings.do_not_disturb = on)
    }

    /// Hides (`hide`) or shows the tray item whose `Id` is `id` in
    /// `bar.tray_hidden`, durably, the way [`ShellSettings::set_do_not_disturb`]
    /// writes its key: it is appended once or removed, the list stays well
    /// formed and bounded, and a stale read does not revert the write. The
    /// new list is computed over the list being staged, the file's own when
    /// it is re-read, so an edit the follower has not delivered yet is kept.
    /// Returns whether anything changed.
    ///
    /// # Errors
    ///
    /// As [`ShellSettings::set_do_not_disturb`].
    pub fn set_tray_hidden(&mut self, id: &str, hide: bool) -> Result<bool, ShellSettingsError> {
        self.set_merged(|settings| {
            let ids = settings.bar_layout().tray_hidden_with(id, hide);
            settings.bar.set_tray_hidden(ids);
        })
    }

    /// Keeps the launcher's list or grid `view` durably, over the file as it
    /// is now. Returns whether anything changed.
    ///
    /// # Errors
    ///
    /// As [`ShellSettings::set_do_not_disturb`].
    pub fn set_launcher_view(&mut self, view: LauncherView) -> Result<bool, ShellSettingsError> {
        self.set_merged(move |settings| settings.launcher.view = view)
    }

    /// Stages `change` over the values in force and writes it over the file
    /// as it is now (see [`ShellSettings::set_do_not_disturb`]).
    fn set_merged(&mut self, change: impl Fn(&mut Settings)) -> Result<bool, ShellSettingsError> {
        let pending = self.store.stage(&change);
        if pending.wanted() == self.store.current() || self.protected {
            return self.commit(pending);
        }
        let written = match self.path.as_deref().map(reread) {
            Some(Reread::Value(on_disk)) => Store::new(*on_disk).stage(&change),
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

    /// Whether the file exists but could not be read, so the values in force
    /// are not the person's current word and nothing irreversible (the
    /// clipboard duty's deletion of its history) may be done on them. What
    /// is in force then depends on when the file broke: at start, the quiet
    /// set, every duty off ([`Duties::all_off`]) over the other defaults;
    /// mid-run (a [`ShellSettings::set_do_not_disturb`] that found the file
    /// unreadable), the values that were in force, kept as they are, running
    /// duties included. A clean read lifts it either way.
    #[must_use]
    pub fn is_protected(&self) -> bool {
        self.protected
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
    use celestina_shell::settings::Duty;

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
        assert!(shell.is_protected());
        assert_eq!(
            shell.current().duties,
            Duties::all_off(),
            "a protected start is quiet"
        );

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
    fn a_protected_start_is_quiet_until_a_clean_read_and_the_rest_is_the_defaults() {
        // Every way a file that exists fails to read starts every duty off,
        // the rest of the settings being the defaults; a missing file and no
        // configuration directory are not protected and run on the cutover's
        // defaults, duties included.
        let quiet = Settings {
            duties: Duties::all_off(),
            ..Settings::default()
        };
        for (name, bytes) in [
            (
                "trailing comma",
                br#"{"schema_version":1,"duties":{"clipboard":"off",}}"#.as_slice(),
            ),
            ("half written", b"{ half a file".as_slice()),
            ("a later schema", br#"{"schema_version":2}"#.as_slice()),
            ("not JSON", b"clipboard=off".as_slice()),
        ] {
            let scratch = Scratch::new("quiet");
            std::fs::create_dir_all(scratch.file().parent().expect("a parent"))
                .expect("the directory");
            std::fs::write(scratch.file(), bytes).expect("the file");
            let shell = ShellSettings::load_from(Some(scratch.file()));
            assert!(shell.is_protected(), "{name}");
            assert_eq!(shell.current(), &quiet, "{name}");
        }
        let missing = Scratch::new("quiet-missing");
        let shell = ShellSettings::load_from(Some(missing.file()));
        assert!(!shell.is_protected());
        assert_eq!(shell.current(), &Settings::default());
        let none = ShellSettings::load_from(None);
        assert!(!none.is_protected());
        assert_eq!(none.current(), &Settings::default());

        // The clean read of a file that says `"idle":"on"` brings the idle
        // duty (and the other shipped duties) on only then, and lifts the
        // protection.
        let scratch = Scratch::new("quiet-then-clean");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), br#"{"duties":{"idle":"on",}}"#).expect("the typo");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        assert_eq!(shell.current().duties.idle, Duty::Off);
        let repaired = Settings::from_bytes(br#"{"duties":{"idle":"on"}}"#).expect("read");
        assert!(shell.adopt_read(ShellRead {
            settings: repaired.clone(),
            generation: 0,
        }));
        assert!(!shell.is_protected());
        assert_eq!(shell.current(), &repaired);
        assert_eq!(shell.current().duties.idle, Duty::On);
    }

    #[test]
    fn a_file_that_breaks_mid_run_protects_the_values_in_force_without_emptying_them() {
        // m-4 of the cutover re-review: a clean start with the idle duty on,
        // then the file corrupted behind the shell's back. The next
        // do-not-disturb toggle finds it unreadable: the store is protected
        // again (nothing is written over the broken file), but the values in
        // force stay what they were, idle still on, do-not-disturb unchanged.
        // Only a protected *start* is the quiet set.
        let scratch = Scratch::new("broken-mid-run");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(
            scratch.file(),
            br#"{"schema_version":1,"duties":{"idle":"on","clipboard":"on"}}"#,
        )
        .expect("the clean file");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        assert!(!shell.is_protected());
        assert_eq!(shell.current().duties.idle, Duty::On);
        let in_force = shell.current().clone();

        std::fs::write(scratch.file(), b"{ broken while running").expect("the corrupt file");
        assert!(matches!(
            shell.set_do_not_disturb(true),
            Err(ShellSettingsError::Protected { .. })
        ));
        assert!(shell.is_protected());
        assert_eq!(shell.current(), &in_force, "the values in force are kept");
        assert_eq!(shell.current().duties.idle, Duty::On, "idle is still on");
        assert_eq!(shell.current().duties.clipboard, Duty::On);
        assert!(!shell.current().do_not_disturb);
        assert_eq!(
            std::fs::read(scratch.file()).expect("the file is still there"),
            b"{ broken while running",
            "nothing was written over the broken file"
        );
        // A clean read lifts the protection and brings the repaired file.
        let repaired = Settings::from_bytes(br#"{"duties":{"idle":"off"}}"#).expect("read");
        assert!(shell.adopt_read(ShellRead {
            settings: repaired.clone(),
            generation: 0,
        }));
        assert!(!shell.is_protected());
        assert_eq!(shell.current(), &repaired);
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

    /// The defaults with the clock placed beside the Island: an edit of the
    /// bar's layout, the kind the person makes by hand.
    fn clock_centred() -> Settings {
        let mut settings = Settings::default();
        settings.bar.centre = Some(vec!["clock".to_owned()]);
        settings
    }

    #[test]
    fn a_do_not_disturb_write_keeps_an_edit_the_follower_has_not_delivered() {
        let scratch = Scratch::new("merge");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        // The person edits the bar's layout by hand; the follower has not
        // read it.
        publish(&scratch, &clock_centred());

        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        let written = std::fs::read(scratch.file()).expect("the file");
        let on_disk = Settings::from_bytes(&written).expect("the shell's own bytes");
        assert!(on_disk.do_not_disturb);
        assert_eq!(on_disk.bar, clock_centred().bar, "the edit survives");
        // In force: the switch, not yet the edit, which the follower brings
        // (its read begins after the write) so the app applies it.
        assert!(shell.current().do_not_disturb);
        assert_eq!(shell.current().bar, Settings::default().bar);
        assert!(shell.adopt_read(ShellRead {
            settings: on_disk,
            generation: 1,
        }));
        assert_eq!(shell.current().bar, clock_centred().bar);

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
        assert!(shell.adopt(clock_centred()));
        // Between the delete and the create of someone else's save.
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        let on_disk = ShellSettings::load_from(Some(scratch.file()));
        assert_eq!(on_disk.current().bar, clock_centred().bar);
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
        let mut edit = clock_centred();
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

    #[test]
    fn a_followed_value_is_adopted_once_and_unprotects_the_file() {
        let scratch = Scratch::new("adopt");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), b"{ half a file").expect("the corrupt file");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let read = wallpaper_off();
        // The file now reads cleanly (do-not-disturb reads it before it
        // writes), so the shell may write it.
        publish(&scratch, &read);
        assert!(shell.adopt(read.clone()));
        assert!(!shell.adopt(read), "the same value again is no change");
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        assert_eq!(shell.current().duties.wallpaper, Duty::Off);
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

    /// An edit the person makes by hand: the wallpaper duty, on by default
    /// since the cutover (CEL-2-M), turned off. It differs from the defaults
    /// a fresh load holds, so a follower delivers it.
    fn wallpaper_off() -> Settings {
        let mut wanted = Settings::default();
        wanted.duties.wallpaper = Duty::Off;
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
        let wanted = wallpaper_off();
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
        let wanted = wallpaper_off();
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
        let wanted = wallpaper_off();
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
        // The protected start ran quiet, so the defaults are a change: the
        // duties come on now, and the file is no longer protected.
        assert_eq!(shell.current().duties, Duties::all_off());
        assert!(shell.adopt_read(read), "the duties come on");
        assert_eq!(shell.current(), &Settings::default());
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
            settings: wallpaper_off(),
            generation: 0,
        };
        assert!(shell.set_do_not_disturb(true).expect("the write succeeds"));
        assert!(!shell.adopt_read(stale));
        assert!(shell.current().do_not_disturb);
        // A read begun after the write is taken.
        let mut newer = wallpaper_off();
        newer.do_not_disturb = true;
        assert!(shell.adopt_read(ShellRead {
            settings: newer.clone(),
            generation: 1,
        }));
        assert_eq!(shell.current(), &newer);
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| (*id).to_owned()).collect()
    }

    /// The default hidden list followed by `more`.
    fn defaults_and(more: &[&str]) -> Vec<String> {
        let mut list = Settings::default().bar_layout().tray_hidden;
        list.extend(ids(more));
        list
    }

    #[test]
    fn set_tray_hidden_writes_the_key_and_keeps_others() {
        let scratch = Scratch::new("tray-hidden");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        // An edit the follower has not delivered: the bar's end zone.
        let mut edit = Settings::default();
        edit.bar.end = Some(ids(&["tray", "clipboard"]));
        edit.do_not_disturb = true;
        publish(&scratch, &edit);

        for id in ["a", "a", "b"] {
            shell.set_tray_hidden(id, true).expect("the write succeeds");
        }
        let written = std::fs::read(scratch.file()).expect("the file");
        let on_disk = Settings::from_bytes(&written).expect("the shell's own bytes");
        assert_eq!(on_disk.bar.tray_hidden, Some(defaults_and(&["a", "b"])));
        assert_eq!(on_disk.bar.end, edit.bar.end, "the other keys survive");
        assert!(on_disk.do_not_disturb);
        assert_eq!(
            shell.current().bar_layout().tray_hidden,
            defaults_and(&["a", "b"])
        );
        assert!(
            !shell.set_tray_hidden("b", true).expect("nothing to write"),
            "hiding a hidden id changes nothing"
        );
    }

    #[test]
    fn set_launcher_view_writes_the_key_and_keeps_others() {
        let scratch = Scratch::new("launcher-view");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let mut edit = Settings::default();
        edit.bar.end = Some(ids(&["tray", "clipboard"]));
        edit.do_not_disturb = true;
        publish(&scratch, &edit);

        assert!(shell
            .set_launcher_view(LauncherView::Grid)
            .expect("the write succeeds"));
        let written = std::fs::read(scratch.file()).expect("the file");
        let on_disk = Settings::from_bytes(&written).expect("the shell's own bytes");
        assert_eq!(on_disk.launcher.view, LauncherView::Grid);
        assert_eq!(on_disk.bar.end, edit.bar.end, "the other keys survive");
        assert!(on_disk.do_not_disturb);
        assert_eq!(shell.current().launcher.view, LauncherView::Grid);
        assert!(
            !shell
                .set_launcher_view(LauncherView::Grid)
                .expect("nothing to write"),
            "choosing the grid again changes nothing"
        );
        assert!(shell
            .set_launcher_view(LauncherView::List)
            .expect("back to the list"));
        let back = std::fs::read(scratch.file()).expect("the file");
        let text = String::from_utf8(back).expect("utf8");
        assert!(!text.contains("launcher"), "{text}");
    }

    #[test]
    fn hiding_an_app_keeps_a_hand_edit_the_follower_has_not_delivered() {
        let scratch = Scratch::new("tray-hand-edit");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let mut edit = Settings::default();
        edit.bar.set_tray_hidden(ids(&["x"]));
        publish(&scratch, &edit);

        assert!(shell
            .set_tray_hidden("steam", true)
            .expect("the write succeeds"));
        let written = std::fs::read(scratch.file()).expect("the file");
        let on_disk = Settings::from_bytes(&written).expect("the shell's own bytes");
        assert_eq!(on_disk.bar.tray_hidden, Some(ids(&["x", "steam"])));
        // Showing removes only that id, again over the file's own list.
        assert!(shell
            .set_tray_hidden("steam", false)
            .expect("the write succeeds"));
        let written = std::fs::read(scratch.file()).expect("the file");
        let on_disk = Settings::from_bytes(&written).expect("the shell's own bytes");
        assert_eq!(on_disk.bar.tray_hidden, Some(ids(&["x"])));
    }

    #[test]
    fn own_write_is_not_reverted_by_a_stale_read() {
        let scratch = Scratch::new("tray-generation");
        let mut shell = ShellSettings::load_from(Some(scratch.file()));
        let stale = ShellRead {
            settings: wallpaper_off(),
            generation: 0,
        };
        assert!(shell
            .set_tray_hidden("a", true)
            .expect("the write succeeds"));
        assert!(!shell.adopt_read(stale));
        assert_eq!(
            shell.current().bar_layout().tray_hidden,
            defaults_and(&["a"])
        );
        let mut newer = wallpaper_off();
        newer.bar.tray_hidden = Some(ids(&["a"]));
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
