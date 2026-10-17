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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::wallpaper::{lenient_mode, Mode};

/// Reads any field of a settings file without ever failing the file: a value of
/// the wrong type or shape is the field's default, so one hand-edit typo cannot
/// make the shell treat the whole file as unreadable.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let raw = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(raw).unwrap_or_default())
}

fn lenient_folder<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<PathBuf>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => Some(PathBuf::from(text)),
        _ => None,
    })
}

fn lenient_outputs<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Object(entries) => entries
            .into_iter()
            .filter_map(|(key, value)| match value {
                Value::String(file) => Some((key, file)),
                _ => None,
            })
            .collect(),
        _ => BTreeMap::new(),
    })
}

fn lenient_duty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duty, D::Error> {
    lenient(deserializer)
}

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

/// The longest folder path kept, in bytes.
pub const MAX_FOLDER_BYTES: usize = 4096;
/// The most per-output wallpaper choices kept.
pub const MAX_OUTPUTS: usize = 16;
/// The longest output name or file name kept, in bytes.
pub const MAX_NAME_BYTES: usize = 255;

/// Whether a background duty is switched on. Off unless the person said so.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Duty {
    #[default]
    Off,
    On,
}

/// The shell's optional duties. Later units add fields; each defaults to off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Duties {
    /// Whether this shell paints the wallpaper.
    #[serde(deserialize_with = "lenient_duty")]
    pub wallpaper: Duty,
}

/// What the person chose for the wallpaper.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallpaperSettings {
    /// The folder images are taken from; absolute, or absent.
    #[serde(deserialize_with = "lenient_folder")]
    pub folder: Option<PathBuf>,
    /// How an image is laid onto an output.
    #[serde(deserialize_with = "lenient_mode")]
    pub mode: Mode,
    /// Output name to file name inside the folder.
    #[serde(deserialize_with = "lenient_outputs")]
    pub outputs: BTreeMap<String, String>,
}

impl WallpaperSettings {
    fn clamped(mut self) -> Self {
        self.folder = self.folder.filter(|folder| {
            let bytes = folder.as_os_str().as_encoded_bytes();
            folder.is_absolute() && bytes.len() <= MAX_FOLDER_BYTES && !bytes.contains(&0)
        });
        // Truncation here only bounds a file read from disk; a staged change is
        // held to the cap by `Store::stage`, which keeps existing entries.
        self.outputs = self
            .outputs
            .into_iter()
            .filter(|(key, file)| plain_name(key) && plain_name(file))
            .take(MAX_OUTPUTS)
            .collect();
        self
    }
}

/// A name that cannot escape its directory or hide itself: non-empty, bounded,
/// no leading dot, no separator, no control character. The one predicate the
/// settings clamp and the command verbs share.
#[must_use]
pub fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && !name.starts_with('.')
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
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
    #[serde(deserialize_with = "lenient")]
    pub do_not_disturb: bool,
    /// Which optional duties this shell performs.
    #[serde(deserialize_with = "lenient")]
    pub duties: Duties,
    /// The wallpaper choices.
    #[serde(deserialize_with = "lenient")]
    pub wallpaper: WallpaperSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            do_not_disturb: false,
            duties: Duties::default(),
            wallpaper: WallpaperSettings::default(),
        }
    }
}

impl Settings {
    /// Brings every value inside its bounds. Called on the way in *and* on the
    /// way out, so neither a hand-edited file nor a broken caller can put an
    /// unusable value in front of the person.
    fn clamped(mut self) -> Self {
        self.schema_version = SCHEMA_VERSION;
        self.wallpaper = self.wallpaper.clamped();
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
        // A new output past the cap is the one refused; entries the person
        // already has are never evicted to make room for it.
        let current = &self.current.wallpaper.outputs;
        let mut room = MAX_OUTPUTS.saturating_sub(
            wanted
                .wallpaper
                .outputs
                .keys()
                .filter(|key| current.contains_key(*key))
                .count(),
        );
        wanted.wallpaper.outputs.retain(|key, file| {
            if current.contains_key(key) {
                return true;
            }
            if room > 0 && plain_name(key) && plain_name(file) {
                room -= 1;
                return true;
            }
            false
        });
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

    #[test]
    fn a_file_without_the_new_keys_reads_as_before() {
        let old = br#"{"schema_version":1,"do_not_disturb":true}"#;
        let read = Settings::from_bytes(old).expect("old file");
        assert!(read.do_not_disturb);
        assert_eq!(read.duties.wallpaper, Duty::Off);
        assert_eq!(read.wallpaper, WallpaperSettings::default());
    }

    #[test]
    fn wallpaper_settings_survive_a_round_trip() {
        let mut settings = Settings::default();
        settings.duties.wallpaper = Duty::On;
        settings.wallpaper.folder = Some(PathBuf::from("/home/p/Pictures"));
        settings.wallpaper.mode = Mode::Fit;
        settings
            .wallpaper
            .outputs
            .insert("DP-1".into(), "a.png".into());
        let bytes = settings.to_bytes().expect("serialize");
        assert!(String::from_utf8_lossy(&bytes).contains(r#""wallpaper": "on""#));
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
    }

    #[test]
    fn hostile_wallpaper_values_are_clamped() {
        let mut outputs = BTreeMap::new();
        for index in 0..20 {
            outputs.insert(format!("O{index:02}"), "ok.png".to_owned());
        }
        for bad in ["../x.png", ".hidden", "a/b", "a\\b", "a\0b", ""] {
            outputs.insert("BAD".into(), bad.into());
            let kept = WallpaperSettings {
                outputs: outputs.clone(),
                ..WallpaperSettings::default()
            }
            .clamped();
            assert!(!kept.outputs.contains_key("BAD"), "{bad:?}");
        }
        outputs.remove("BAD");
        outputs.insert("a/b".into(), "x.png".into());
        outputs.insert("n".repeat(256), "x.png".into());
        outputs.insert("F".into(), "n".repeat(256));
        let kept = WallpaperSettings {
            folder: Some(PathBuf::from("relative/dir")),
            outputs,
            ..WallpaperSettings::default()
        }
        .clamped();
        assert_eq!(kept.folder, None);
        assert_eq!(kept.outputs.len(), MAX_OUTPUTS);
        assert!(kept.outputs.keys().all(|key| key.starts_with('O')));
        assert!(kept.outputs.contains_key("O00"));
        assert!(!kept.outputs.contains_key("O16"));

        let long = PathBuf::from(format!("/{}", "d".repeat(MAX_FOLDER_BYTES)));
        let kept = WallpaperSettings {
            folder: Some(long),
            ..WallpaperSettings::default()
        }
        .clamped();
        assert_eq!(kept.folder, None);
        let kept = WallpaperSettings {
            folder: Some(PathBuf::from("/ok/dir")),
            ..WallpaperSettings::default()
        }
        .clamped();
        assert_eq!(kept.folder, Some(PathBuf::from("/ok/dir")));
    }

    #[test]
    fn clamping_applies_on_read_and_stage() {
        let bytes = br#"{"wallpaper":{"folder":"rel","outputs":{"A":"../b"}}}"#;
        let read = Settings::from_bytes(bytes).expect("parses");
        assert_eq!(read.wallpaper, WallpaperSettings::default());
        let store = Store::new(Settings::default());
        let pending = store.stage(|s| {
            s.wallpaper
                .outputs
                .insert("A".into(), ".x".into())
                .map(drop)
                .unwrap_or(())
        });
        assert!(pending.wanted().wallpaper.outputs.is_empty());
    }

    #[test]
    fn unknown_keys_and_an_old_shells_bytes_still_parse() {
        let bytes = br#"{"schema_version":1,"do_not_disturb":false,"duties":{"wallpaper":"on","future":"on"},"wallpaper":{"mode":"center","extra":1},"mystery":[1]}"#;
        let read = Settings::from_bytes(bytes).expect("parses");
        assert_eq!(read.duties.wallpaper, Duty::On);
        assert_eq!(read.wallpaper.mode, Mode::Center);
    }

    #[test]
    fn a_wrong_typed_value_never_fails_the_file() {
        let read = Settings::from_bytes(br#"{"mode":"tile","do_not_disturb":true}"#).expect("read");
        assert!(read.do_not_disturb);
        assert_eq!(read.wallpaper.mode, Mode::Fill);

        let bytes = br#"{"do_not_disturb":true,"duties":{"wallpaper":"yes"},
            "wallpaper":{"mode":"tile","folder":42,"outputs":{"DP-1":5,"HDMI-A-1":"b.png"}}}"#;
        let read = Settings::from_bytes(bytes).expect("read");
        assert!(read.do_not_disturb);
        assert_eq!(read.duties.wallpaper, Duty::Off);
        assert_eq!(read.wallpaper.mode, Mode::Fill);
        assert_eq!(read.wallpaper.folder, None);
        assert_eq!(read.wallpaper.outputs.len(), 1);
        assert_eq!(read.wallpaper.outputs["HDMI-A-1"], "b.png");

        for bytes in [
            &br#"{"do_not_disturb":"yes"}"#[..],
            br#"{"duties":5,"wallpaper":"x"}"#,
            br#"{"wallpaper":{"outputs":[1],"mode":7,"folder":null}}"#,
            br#"{"duties":{"wallpaper":null}}"#,
        ] {
            assert_eq!(
                Settings::from_bytes(bytes),
                Some(Settings::default()),
                "{}",
                String::from_utf8_lossy(bytes)
            );
        }
    }

    #[test]
    fn a_new_output_past_the_cap_is_refused_not_an_old_one_evicted() {
        let mut store = Store::new(Settings::default());
        for index in 0..MAX_OUTPUTS {
            let pending = store.stage(|s| {
                s.wallpaper
                    .outputs
                    .insert(format!("Z{index:02}"), "ok.png".into());
            });
            assert!(store.apply(pending, WriteOutcome::Durable));
        }
        assert_eq!(store.current().wallpaper.outputs.len(), MAX_OUTPUTS);
        // "A-new" sorts before every kept key: key order must not matter.
        let pending = store.stage(|s| {
            s.wallpaper.outputs.insert("A-new".into(), "ok.png".into());
        });
        assert!(!pending.wanted().wallpaper.outputs.contains_key("A-new"));
        assert_eq!(pending.wanted().wallpaper.outputs.len(), MAX_OUTPUTS);
        assert!(!store.apply(pending, WriteOutcome::Durable));
        // Changing or removing an existing entry still works at the cap.
        let pending = store.stage(|s| {
            s.wallpaper.outputs.insert("Z00".into(), "other.png".into());
            s.wallpaper.outputs.remove("Z01");
        });
        assert_eq!(pending.wanted().wallpaper.outputs["Z00"], "other.png");
        assert!(store.apply(pending, WriteOutcome::Durable));
        let pending = store.stage(|s| {
            s.wallpaper.outputs.insert("A-new".into(), "ok.png".into());
        });
        assert!(pending.wanted().wallpaper.outputs.contains_key("A-new"));
    }

    #[test]
    fn plain_names_reject_control_characters_and_separators() {
        for good in ["DP-1", "a b.png", "sea.png"] {
            assert!(plain_name(good), "{good:?}");
        }
        for bad in ["", ".", "..", ".sea.png", "a\\b.png", "a/b", "a\nb", "a\0b"] {
            assert!(!plain_name(bad), "{bad:?}");
        }
    }
}
