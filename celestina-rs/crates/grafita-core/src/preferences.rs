//! The few editing choices that outlive a window.
//!
//! Grafita has no settings dialog and does not want one: a preference lands
//! here only when the user can already change it with a key. Today those are
//! the text size, which Ctrl + and Ctrl − move, and whether long lines wrap,
//! which Alt + Z turns off and on. Both would be an irritation to set again on
//! every launch.
//!
//! The file is `key = value`, one per line, in `$XDG_CONFIG_HOME/grafita/
//! preferences`. Like [`crate::recent`], reading a broken one is not an error —
//! an unreadable preference is the default, never a refusal to start — and
//! writing is best-effort, because a preference that cannot be saved must not
//! stop an edit.
//!
//! Writing syncs the file and its folder, and a wheel spun with Ctrl held asks
//! for a new size on every notch. A host therefore hands each change to a
//! [`PreferenceWriter`], which writes on its own thread once the changes stop
//! arriving, instead of calling [`Preferences::store`] from its GUI thread.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use celestina_core::{atomic_file, xdg};

/// The text size Grafita starts at, matching the theme's caption size — the
/// value the editor used before it was adjustable.
pub const DEFAULT_FONT_SIZE: u32 = 11;

/// Below this the caret is larger than the glyphs; above it a line of code
/// stops fitting. Anything outside is clamped rather than refused, so a
/// hand-edited file still opens the editor.
pub const MIN_FONT_SIZE: u32 = 7;
/// See [`MIN_FONT_SIZE`].
pub const MAX_FONT_SIZE: u32 = 42;

/// Long lines wrap unless the user says otherwise. Grafita opens prose as
/// readily as code, and prose with a horizontal scroll bar is unreadable.
pub const DEFAULT_WRAP: bool = true;

/// How long the preferences must stay unchanged before a [`PreferenceWriter`]
/// writes them: long enough that a spun wheel or a held key writes once, short
/// enough that a window closed right afterwards has already written them.
pub const STORE_QUIET: Duration = Duration::from_millis(400);

/// What the editor remembers between launches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Preferences {
    font_size: u32,
    wrap: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            wrap: DEFAULT_WRAP,
        }
    }
}

impl Preferences {
    /// Reads the stored preferences, or the defaults.
    ///
    /// A missing, unreadable or malformed file is the defaults: there is
    /// nothing the user could do about it and nothing is lost.
    #[must_use]
    pub fn load() -> Self {
        storage().map_or_else(Self::default, |store| Self::load_from(&store))
    }

    /// Reads the preferences kept in `store`, or the defaults, by the rule of
    /// [`Preferences::load`].
    #[must_use]
    pub fn load_from(store: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(store) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    #[must_use]
    fn parse(text: &str) -> Self {
        let mut preferences = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            // An unknown key is left alone rather than dropped from the parse:
            // a newer Grafita's preference should survive an older one reading
            // the file — which it does, because writing only ever rewrites the
            // keys this version knows, and the reader ignores the rest.
            match key.trim() {
                "font_size" => {
                    if let Ok(size) = value.trim().parse::<u32>() {
                        preferences.set_font_size(size);
                    }
                }
                // Only the two spellings this file is written with. Anything
                // else is a value nobody wrote, so the default stands rather
                // than a guess being made about what was meant.
                "wrap" => match value.trim() {
                    "true" => preferences.wrap = true,
                    "false" => preferences.wrap = false,
                    _ => {}
                },
                _ => {}
            }
        }
        preferences
    }

    /// The editor's text size, in pixels.
    #[must_use]
    pub const fn font_size(&self) -> u32 {
        self.font_size
    }

    /// Sets the text size, clamped to what stays legible.
    pub const fn set_font_size(&mut self, size: u32) {
        self.font_size = if size < MIN_FONT_SIZE {
            MIN_FONT_SIZE
        } else if size > MAX_FONT_SIZE {
            MAX_FONT_SIZE
        } else {
            size
        };
    }

    /// Moves the text size by `steps` pixels and answers the size now in
    /// effect, so a host can tell a real change from a keypress at the limit.
    pub const fn nudge_font_size(&mut self, steps: i32) -> u32 {
        // Saturating on both sides: the clamp below is what decides the
        // result, and it should never be reached through a wrapped number.
        let wanted = (self.font_size as i64).saturating_add(steps as i64);
        self.set_font_size(if wanted < 0 { 0 } else { wanted as u32 });
        self.font_size
    }

    /// Whether long lines wrap to the width of the surface.
    #[must_use]
    pub const fn wrap(&self) -> bool {
        self.wrap
    }

    /// Turns wrapping off and on.
    pub const fn toggle_wrap(&mut self) {
        self.wrap = !self.wrap;
    }

    /// Writes the preferences back. Best-effort, for the reason at the top.
    ///
    /// Blocking: a GUI thread hands the change to a [`PreferenceWriter`]
    /// instead.
    pub fn store(&self) {
        if let Some(store) = storage() {
            self.store_to(&store);
        }
    }

    /// Writes the preferences to `store`. Best-effort and blocking, like
    /// [`Preferences::store`].
    pub fn store_to(&self, store: &Path) {
        let text = format!("font_size = {}\nwrap = {}\n", self.font_size, self.wrap);
        let _ = atomic_file::replace(store, text.as_bytes());
    }
}

/// Where the preferences live. Config, not data: this is a choice the user
/// made, and it is the kind of file they may reasonably want to edit or copy.
///
/// Resolved from the environment alone; nothing is read.
#[must_use]
pub fn storage() -> Option<PathBuf> {
    Some(xdg::config_home()?.join("grafita").join("preferences"))
}

/// Writes preferences on a thread of its own, once they stop changing.
///
/// Each [`PreferenceWriter::submit`] replaces whatever was waiting, so a burst
/// of changes is one write of the last one, made [`STORE_QUIET`] after the
/// burst ends. Dropping the writer writes whatever is still waiting and waits
/// up to [`CLOSE_WAIT`] for that write, joining the thread when it finishes;
/// a write still stuck in its syncs after that is left to finish on its own,
/// which is safe because the write is an atomic replace. A window that closes
/// mid-burst loses nothing, and a slow disk cannot hold its GUI thread for
/// longer than that bound.
pub struct PreferenceWriter {
    shared: Arc<WriterShared>,
    thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for PreferenceWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreferenceWriter")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct WriterShared {
    state: Mutex<WriterState>,
    wake: Condvar,
}

/// How long dropping a [`PreferenceWriter`] waits for its last write.
pub const CLOSE_WAIT: Duration = Duration::from_secs(2);

/// How the writer thread writes. A function pointer so a test can stand in a
/// write as slow as a busy disk.
type Write = fn(&Preferences, &Path);

#[derive(Debug)]
struct WriterState {
    /// The newest preferences not yet written.
    pending: Option<Preferences>,
    /// When `pending` last changed; the quiet period counts from here.
    changed_at: Instant,
    closing: bool,
    /// The thread has written what it had and returned.
    done: bool,
}

impl PreferenceWriter {
    /// Starts the writer for the file at `store`, writing once the preferences
    /// have been left alone for `quiet`.
    ///
    /// Returns the error the thread could not be created with, so a host can
    /// fall back to not remembering rather than failing to start.
    pub fn new(store: PathBuf, quiet: Duration) -> Result<Self, io::Error> {
        Self::start(store, quiet, Preferences::store_to)
    }

    /// A writer that writes with `write`, so a test can make it slow.
    #[cfg(test)]
    fn with_write(store: PathBuf, quiet: Duration, write: Write) -> Result<Self, io::Error> {
        Self::start(store, quiet, write)
    }

    fn start(store: PathBuf, quiet: Duration, write: Write) -> Result<Self, io::Error> {
        let shared = Arc::new(WriterShared {
            state: Mutex::new(WriterState {
                pending: None,
                changed_at: Instant::now(),
                closing: false,
                done: false,
            }),
            wake: Condvar::new(),
        });
        let writer_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("grafita-preferences".to_owned())
            .spawn(move || {
                writer_loop(&writer_shared, &store, quiet, write);
                lock(&writer_shared.state).done = true;
                writer_shared.wake.notify_all();
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Hands over the preferences now in effect. Never blocks on the disk.
    pub fn submit(&self, preferences: Preferences) {
        let mut state = lock(&self.shared.state);
        state.pending = Some(preferences);
        state.changed_at = Instant::now();
        self.shared.wake.notify_all();
    }
}

impl Drop for PreferenceWriter {
    fn drop(&mut self) {
        let finished = {
            let mut state = lock(&self.shared.state);
            state.closing = true;
            self.shared.wake.notify_all();
            let (state, _) = self
                .shared
                .wake
                .wait_timeout_while(state, CLOSE_WAIT, |state| !state.done)
                .unwrap_or_else(PoisonError::into_inner);
            state.done
        };
        // A thread that has not finished is detached rather than joined: the
        // one thing it can still be doing is an atomic replace.
        if let Some(thread) = self.thread.take() {
            if finished {
                let _ = thread.join();
            }
        }
    }
}

fn writer_loop(shared: &WriterShared, store: &Path, quiet: Duration, write: Write) {
    loop {
        let (due, closing) = {
            let mut state = lock(&shared.state);
            loop {
                if state.closing {
                    break (state.pending.take(), true);
                }
                if state.pending.is_none() {
                    state = shared
                        .wake
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                    continue;
                }
                let waited = state.changed_at.elapsed();
                if waited >= quiet {
                    break (state.pending.take(), false);
                }
                state = shared
                    .wake
                    .wait_timeout(state, quiet - waited)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
        };
        if let Some(preferences) = due {
            write(&preferences, store);
        }
        if closing {
            return;
        }
    }
}

fn lock(mutex: &Mutex<WriterState>) -> MutexGuard<'_, WriterState> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        PreferenceWriter, Preferences, CLOSE_WAIT, DEFAULT_FONT_SIZE, DEFAULT_WRAP, MAX_FONT_SIZE,
        MIN_FONT_SIZE,
    };
    use crate::testing::scratch_directory;

    #[test]
    fn an_unreadable_line_leaves_the_default_standing() {
        let preferences =
            Preferences::parse("rubbish\n# a comment\nfont_size = eight\nwrap = maybe\n");

        assert_eq!(preferences.font_size(), DEFAULT_FONT_SIZE);
        assert_eq!(preferences.wrap(), DEFAULT_WRAP);
    }

    #[test]
    fn a_stored_size_is_read_back() {
        let preferences = Preferences::parse("font_size = 17\n");

        assert_eq!(preferences.font_size(), 17);
    }

    #[test]
    fn wrapping_is_stored_and_read_back_in_both_states() {
        assert!(!Preferences::parse("wrap = false\n").wrap());
        assert!(Preferences::parse("wrap = true\n").wrap());

        // A file this version wrote must read back as itself.
        let mut written = Preferences::default();
        written.toggle_wrap();
        written.set_font_size(23);
        let read_back = Preferences::parse(&format!(
            "font_size = {}\nwrap = {}\n",
            written.font_size(),
            written.wrap()
        ));

        assert_eq!(read_back, written);
    }

    #[test]
    fn a_size_outside_the_legible_range_is_clamped_rather_than_refused() {
        assert_eq!(
            Preferences::parse("font_size = 0").font_size(),
            MIN_FONT_SIZE
        );
        assert_eq!(
            Preferences::parse("font_size = 4000").font_size(),
            MAX_FONT_SIZE
        );
    }

    #[test]
    fn nudging_stops_at_the_limits_and_reports_the_size_in_effect() {
        let mut preferences = Preferences::default();

        assert_eq!(preferences.nudge_font_size(1), DEFAULT_FONT_SIZE + 1);
        assert_eq!(preferences.nudge_font_size(-1), DEFAULT_FONT_SIZE);
        assert_eq!(preferences.nudge_font_size(-1000), MIN_FONT_SIZE);
        assert_eq!(preferences.nudge_font_size(1000), MAX_FONT_SIZE);
    }

    #[test]
    fn a_burst_of_changes_is_written_once_the_burst_is_over() {
        let root = scratch_directory("preferences-quiet");
        let store = root.join("grafita").join("preferences");
        let writer =
            PreferenceWriter::new(store.clone(), Duration::from_millis(50)).expect("a writer");

        let mut preferences = Preferences::default();
        for _ in 0..20 {
            preferences.nudge_font_size(1);
            writer.submit(preferences);
        }

        let deadline = Instant::now() + Duration::from_secs(5);
        while Preferences::load_from(&store) != preferences && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(Preferences::load_from(&store), preferences);

        drop(writer);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nothing_is_written_while_changes_keep_coming_and_closing_writes_the_last() {
        let root = scratch_directory("preferences-close");
        let store = root.join("preferences");
        // A quiet period no test run waits out: only closing can write.
        let writer =
            PreferenceWriter::new(store.clone(), Duration::from_secs(3600)).expect("a writer");

        let mut preferences = Preferences::default();
        preferences.toggle_wrap();
        writer.submit(preferences);
        preferences.set_font_size(19);
        writer.submit(preferences);
        std::thread::sleep(Duration::from_millis(50));
        assert!(
            !store.exists(),
            "nothing is written before the changes stop"
        );

        drop(writer);
        assert_eq!(Preferences::load_from(&store), preferences);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn an_idle_writer_closes_without_writing() {
        let root = scratch_directory("preferences-idle");
        let store = root.join("preferences");
        let writer =
            PreferenceWriter::new(store.clone(), Duration::from_millis(10)).expect("a writer");

        drop(writer);
        assert!(!store.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn closing_waits_a_bounded_time_for_a_write_stuck_on_the_disk() {
        fn stuck(_preferences: &Preferences, _store: &std::path::Path) {
            std::thread::sleep(Duration::from_secs(30));
        }
        let root = scratch_directory("preferences-stuck");
        let writer = PreferenceWriter::with_write(
            root.join("preferences"),
            Duration::from_secs(3600),
            stuck,
        )
        .expect("a writer");
        writer.submit(Preferences::default());

        let started = Instant::now();
        drop(writer);
        let waited = started.elapsed();
        assert!(waited >= CLOSE_WAIT, "{waited:?}");
        assert!(waited < CLOSE_WAIT + Duration::from_secs(1), "{waited:?}");

        let _ = std::fs::remove_dir_all(root);
    }
}
