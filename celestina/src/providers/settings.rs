//! The two settings the shell reads.
//!
//! - The suite's appearance file (`celestina-settings`): followed on its own
//!   thread for as long as the returned [`Follower`] lives, each change
//!   published as [`Message::Appearance`].
//! - The shell's own `shell.json` (do-not-disturb): read once at start
//!   through `celestina_shell::settings`, and rewritten whole and atomically
//!   (`celestina_core::atomic_file::replace_private`) when the person changes
//!   it. A file that exists but cannot be read is never overwritten.

use std::path::PathBuf;

use celestina_core::atomic_file::{self, Published, WriteError};
use celestina_settings::Follower;
use celestina_shell::settings::{self, Settings, Store, WriteOutcome, MAX_FILE_BYTES};
use celestina_wayland::Sender;

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
}
