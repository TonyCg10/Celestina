// Copied from `celestina-rs/crates/celestina-shell-core/src/settings.rs` at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9, adapted: `Settings` keeps only
// `do_not_disturb` (the halted shell's `quiet`) and `schema_version`, with
// `SCHEMA_VERSION = 1` for this shell's own file `celestina/shell.json`; the
// tray, wallpaper, weather, night-light, level-step and workspace-home fields,
// their bounds and their tests are dropped. `Store`, `Pending`, `WriteOutcome`
// and the durable-before-published rule are unchanged.

//! What the person chose, and what it takes for that choice to be real.
//!
//! Everything else this shell shows is a reading: the compositor's workspaces,
//! what an application said. Settings are the opposite — they are the only
//! state the shell itself owns — and that changes what honesty means for them.
//!
//! A reading is published as soon as it is read. A setting is published only
//! once it is **durable**. A switch that showed do-not-disturb on and then
//! failed to write the file would be reporting a choice the next session will
//! not honour; the person would have to discover that by restarting. So the
//! rule here is one way round: write, confirm, then publish. [`Store::apply`]
//! cannot adopt anything but a confirmed write, because it takes the outcome
//! back from whoever performed it.
//!
//! Nothing in this module touches a filesystem. It owns the schema, the text
//! that goes to disk and the text that comes back — so the rules are testable
//! without a temporary directory, and the durable write itself stays with the
//! provider that can actually fsync.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The schema version written into the file. A file from the future is not
/// read: an older shell guessing at a newer schema would be inventing the
/// person's preferences.
pub const SCHEMA_VERSION: u32 = 1;
/// The whole file, as a guard against a corrupted or hostile settings path.
pub const MAX_FILE_BYTES: usize = 64 * 1024;

/// Where the file lives under a configuration directory:
/// `<config_home>/celestina/shell.json`.
///
/// The directory itself comes from `celestina_core::xdg::config_home`, which
/// the caller resolves; this crate only owns the name.
#[must_use]
pub fn path_in(config_home: &Path) -> PathBuf {
    config_home.join("celestina").join("shell.json")
}

/// Everything the person chose. Every field has a default that is the quietest
/// possible behaviour: nothing here turns itself on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Written so a later shell can refuse a file it does not understand.
    pub schema_version: u32,
    /// Whether notification banners are being held back. Persisted because a
    /// person who silenced their session did not mean "until the next reboot".
    pub do_not_disturb: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            do_not_disturb: false,
        }
    }
}

impl Settings {
    /// Brings every value inside its bounds. Called on the way in *and* on the
    /// way out, so neither a hand-edited file nor a broken caller can put an
    /// unusable value in front of the person.
    fn clamped(mut self) -> Self {
        self.schema_version = SCHEMA_VERSION;
        self
    }

    /// The exact bytes to write. Pretty-printed because this file is meant to
    /// be readable by the person whose preferences it holds.
    ///
    /// # Errors
    ///
    /// Returns the serializer's own error, which cannot happen for this shape
    /// but is not worth an `unwrap` at a call site that can report it.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec_pretty(&self.clone().clamped())?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Reads a settings file.
    ///
    /// Returns `None` for anything that is not one this shell wrote: a file too
    /// large, unreadable text, invalid JSON, or a schema from a version that
    /// knew things this one does not. The caller then keeps the defaults rather
    /// than a half-understood file — and, importantly, must not overwrite the
    /// file it could not read. Keys this schema does not know are ignored, and
    /// the next write drops them.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_FILE_BYTES {
            return None;
        }
        let parsed: Self = serde_json::from_slice(bytes).ok()?;
        if parsed.schema_version > SCHEMA_VERSION {
            return None;
        }
        Some(parsed.clamped())
    }
}

/// A change that has not happened yet.
///
/// It exists so a caller cannot publish a preference it only tried to save: the
/// only way to get the new [`Settings`] out of it is to hand back the outcome
/// of the write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    wanted: Settings,
}

/// What a durable write ended in. The caller reports this; this module never
/// assumes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The bytes are on disk and the directory entry survives a power cut.
    Durable,
    /// Nothing usable was written, so nothing changed.
    Failed,
}

/// The settings this shell is running with, and the one way they change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    current: Settings,
}

impl Store {
    #[must_use]
    pub fn new(current: Settings) -> Self {
        Self {
            current: current.clamped(),
        }
    }

    /// What is in force right now — which is always what is on disk, because
    /// nothing else ever becomes current.
    #[must_use]
    pub fn current(&self) -> &Settings {
        &self.current
    }

    /// Describes a change without making it. The bytes to write come from
    /// [`Pending::bytes`]; the change only becomes real through
    /// [`Store::apply`].
    #[must_use]
    pub fn stage(&self, change: impl FnOnce(&mut Settings)) -> Pending {
        let mut wanted = self.current.clone();
        change(&mut wanted);
        Pending {
            wanted: wanted.clamped(),
        }
    }

    /// Adopts a staged change, but only when its write really happened.
    ///
    /// Returns whether anything is now different. A failed write leaves the
    /// previous value in force, which is what the person's next session will
    /// see and therefore what this one must keep showing.
    pub fn apply(&mut self, pending: Pending, outcome: WriteOutcome) -> bool {
        if outcome == WriteOutcome::Failed {
            return false;
        }
        if pending.wanted == self.current {
            return false;
        }
        self.current = pending.wanted;
        true
    }
}

impl Pending {
    /// The exact bytes whose durable write authorizes [`Store::apply`].
    ///
    /// # Errors
    ///
    /// Returns the serializer's own error.
    pub fn bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        self.wanted.to_bytes()
    }

    /// What this change would make current, for a caller that needs to describe
    /// it before it happens — a log line. It is deliberately not a way to
    /// publish it.
    #[must_use]
    pub fn wanted(&self) -> &Settings {
        &self.wanted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_turn_nothing_on() {
        let settings = Settings::default();
        assert!(!settings.do_not_disturb);
        assert_eq!(settings.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn the_file_lives_under_the_suite_directory() {
        assert_eq!(
            path_in(Path::new("/home/person/.config")),
            PathBuf::from("/home/person/.config/celestina/shell.json")
        );
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let settings = Settings {
            do_not_disturb: true,
            ..Settings::default()
        };

        let bytes = settings.to_bytes().expect("settings serialize");
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
    }

    #[test]
    fn a_file_this_shell_did_not_write_is_not_read() {
        assert_eq!(Settings::from_bytes(b"not json"), None);
        assert_eq!(Settings::from_bytes(&vec![b'x'; MAX_FILE_BYTES + 1]), None);
        // A schema from the future knew things this shell does not.
        let ahead = format!(r#"{{"schema_version":{}}}"#, SCHEMA_VERSION + 1);
        assert_eq!(Settings::from_bytes(ahead.as_bytes()), None);
        // A file this shell could write, missing everything optional, is read.
        assert_eq!(Settings::from_bytes(b"{}"), Some(Settings::default()));
    }

    #[test]
    fn a_change_is_not_in_force_until_its_write_is() {
        let mut store = Store::new(Settings::default());
        let pending = store.stage(|settings| settings.do_not_disturb = true);

        // Staging alone changes nothing anybody can see.
        assert!(!store.current().do_not_disturb);
        assert!(pending.wanted().do_not_disturb);

        assert!(!store.apply(pending.clone(), WriteOutcome::Failed));
        assert!(
            !store.current().do_not_disturb,
            "a failed write must leave the previous value in force"
        );

        assert!(store.apply(pending, WriteOutcome::Durable));
        assert!(store.current().do_not_disturb);
    }

    #[test]
    fn applying_the_same_value_is_not_a_change() {
        let mut store = Store::new(Settings::default());
        let pending = store.stage(|settings| settings.do_not_disturb = false);
        assert!(!store.apply(pending, WriteOutcome::Durable));
    }

    #[test]
    fn what_is_written_is_what_would_be_read_back() {
        let store = Store::new(Settings::default());
        let pending = store.stage(|settings| settings.do_not_disturb = true);

        let bytes = pending.bytes().expect("bytes to write");
        assert_eq!(
            Settings::from_bytes(&bytes).as_ref(),
            Some(pending.wanted())
        );
    }

    #[test]
    fn a_staged_change_cannot_forge_the_schema() {
        let store = Store::new(Settings::default());
        let pending = store.stage(|settings| settings.schema_version = SCHEMA_VERSION + 7);
        assert_eq!(pending.wanted().schema_version, SCHEMA_VERSION);
    }
}
