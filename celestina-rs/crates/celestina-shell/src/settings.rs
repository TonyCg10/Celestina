// Copied from `celestina-rs/crates/celestina-shell-core/src/settings.rs` at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9, adapted: `Settings` keeps only
// `do_not_disturb` (the halted shell's `quiet`) and `schema_version`, with
// `SCHEMA_VERSION = 1` for this shell's own file `celestina/shell.json`; the
// tray, wallpaper, weather, night-light, level-step and workspace-home fields,
// their bounds and their tests are dropped. `Store`, `Pending`, `WriteOutcome`
// and the durable-before-published rule are unchanged. CEL-2-A then added
// what the copy did not have: `bar` (the layout of the bar's zones and the
// tray ids kept off it) and `duties.bar` (whether the shell has taken the bar
// over from Noctalia), both optional so a 1.0.0 file still reads, plus their
// bounds in `clamped` and their tests. Unlike the other duties, whose `off`
// is written once the file is, a bar value the person never set is never
// written, so a later default still reaches them.

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

use crate::nightlight::Whitepoint;
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

/// A duty that stays unset until the person switches it: `"on"` or `"off"` is
/// their choice, anything else is no choice (the default, which is off).
fn lenient_chosen_duty<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Duty>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(deserializer)?).ok())
}

/// A chosen id list: an array keeps its string entries (the shape checks in
/// `clamped` come after), anything else is no choice.
fn lenient_ids<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Array(entries) => Some(
            entries
                .into_iter()
                .filter_map(|entry| match entry {
                    Value::String(id) => Some(id),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    })
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

/// The shell's optional duties, one line per duty in name order. Later units
/// add fields; each defaults to off, and a value that is not `"on"` or `"off"`
/// degrades to off instead of failing the file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Duties {
    /// Whether this shell draws the bar; [`Duties::bar`] is the state in
    /// force. `None` until the person switches it, and then never written, so
    /// a later default (CEL-2-M switching the bar on) reaches every file that
    /// did not choose.
    #[serde(
        deserialize_with = "lenient_chosen_duty",
        skip_serializing_if = "Option::is_none"
    )]
    pub bar: Option<Duty>,
    /// Whether this shell holds the session awake while caffeine is on.
    #[serde(deserialize_with = "lenient_duty")]
    pub caffeine: Duty,
    /// Whether this shell turns the screens off, locks and suspends when idle.
    #[serde(deserialize_with = "lenient_duty")]
    pub idle: Duty,
    /// Whether this shell drives the night-light gamma ramps.
    #[serde(deserialize_with = "lenient_duty")]
    pub nightlight: Duty,
    /// Whether this shell paints the wallpaper.
    #[serde(deserialize_with = "lenient_duty")]
    pub wallpaper: Duty,
}

impl Duties {
    /// Whether the bar duty is on: what the person chose, or the default.
    #[must_use]
    pub fn bar(&self) -> Duty {
        self.bar.unwrap_or_default()
    }
}

/// Most piece ids one zone keeps.
pub const MAX_ZONE_IDS: usize = 16;
/// Most tray ids the bar keeps hidden.
pub const MAX_HIDDEN_IDS: usize = 32;
/// Longest piece id, in bytes.
pub const MAX_PIECE_ID_BYTES: usize = 64;
/// Longest hidden tray id, in bytes.
pub const MAX_HIDDEN_ID_BYTES: usize = 128;

/// The bar's three zones, as piece ids in order, and the tray items it hides:
/// the layout in force, what the person chose with today's defaults for the
/// rest ([`Settings::bar_layout`]).
///
/// This crate cannot know which piece ids exist; the product logs the ones it
/// does not recognise. Here an id is only checked for shape and uniqueness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarLayout {
    /// Left zone.
    pub start: Vec<String>,
    /// Zone either side of the Island.
    pub centre: Vec<String>,
    /// Right zone.
    pub end: Vec<String>,
    /// Tray item ids the bar does not show (they still appear on the Tray page).
    pub tray_hidden: Vec<String>,
}

impl Default for BarLayout {
    fn default() -> Self {
        let ids = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect();
        Self {
            start: ids(&["workspaces", "notifications"]),
            centre: Vec::new(),
            end: ids(&["tray"]),
            // Blueman's item `Id` is expected to be `blueman` (the halted
            // shell's fixtures of this session); `blueman-tray` is its
            // process. Both until the live row confirms the `Id`.
            tray_hidden: ids(&["nm-applet", "blueman", "blueman-tray"]),
        }
    }
}

/// The ids of `zone` that are well formed and not in `seen`, in order, at
/// most `MAX_ZONE_IDS`; each kept id joins `seen`.
fn kept_pieces(zone: Vec<String>, seen: &mut Vec<String>) -> Vec<String> {
    let mut kept = Vec::new();
    for id in zone {
        if is_piece_id(&id) && !seen.contains(&id) {
            seen.push(id.clone());
            kept.push(id);
            if kept.len() == MAX_ZONE_IDS {
                break;
            }
        }
    }
    kept
}

/// The well-formed hidden ids, once each, at most `MAX_HIDDEN_IDS`.
fn kept_hidden(ids: Vec<String>) -> Vec<String> {
    let mut hidden: Vec<String> = Vec::new();
    for id in ids {
        if is_hidden_id(&id) && !hidden.contains(&id) {
            hidden.push(id);
            if hidden.len() == MAX_HIDDEN_IDS {
                break;
            }
        }
    }
    hidden
}

/// What the person chose of the bar, as the file stores it: a zone or the
/// hidden list is `None` until it is set, and an unset one is never written,
/// so it follows the default of the shell that reads it (a later unit adds
/// its piece to the default end zone and it reaches every such file). A list
/// of the wrong shape is no choice; an entry that is not a string is dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BarChoice {
    /// Left zone.
    #[serde(
        deserialize_with = "lenient_ids",
        skip_serializing_if = "Option::is_none"
    )]
    pub start: Option<Vec<String>>,
    /// Zone either side of the Island.
    #[serde(
        deserialize_with = "lenient_ids",
        skip_serializing_if = "Option::is_none"
    )]
    pub centre: Option<Vec<String>>,
    /// Right zone.
    #[serde(
        deserialize_with = "lenient_ids",
        skip_serializing_if = "Option::is_none"
    )]
    pub end: Option<Vec<String>>,
    /// Tray item ids the bar does not show (they still appear on the Tray page).
    #[serde(
        deserialize_with = "lenient_ids",
        skip_serializing_if = "Option::is_none"
    )]
    pub tray_hidden: Option<Vec<String>>,
}

impl BarChoice {
    /// Nothing was chosen.
    #[must_use]
    pub fn is_unset(&self) -> bool {
        *self == Self::default()
    }

    /// Each chosen list inside its bounds; a piece listed twice keeps its
    /// first place among the chosen zones.
    fn clamped(mut self) -> Self {
        let mut seen = Vec::new();
        for zone in [&mut self.start, &mut self.centre, &mut self.end] {
            if let Some(ids) = zone.take() {
                *zone = Some(kept_pieces(ids, &mut seen));
            }
        }
        self.tray_hidden = self.tray_hidden.take().map(kept_hidden);
        self
    }

    /// The layout in force: each chosen list, and the default for the rest.
    /// A chosen zone claims its pieces first, so a default zone drops a piece
    /// the person placed elsewhere; then a piece keeps its first place.
    fn layout(&self) -> BarLayout {
        let defaults = BarLayout::default();
        let mut seen: Vec<String> = [&self.start, &self.centre, &self.end]
            .into_iter()
            .flatten()
            .flatten()
            .cloned()
            .collect();
        let mut zone = |chosen: &Option<Vec<String>>, default: Vec<String>| match chosen {
            Some(ids) => ids.clone(),
            None => kept_pieces(default, &mut seen),
        };
        let start = zone(&self.start, defaults.start);
        let centre = zone(&self.centre, defaults.centre);
        let end = zone(&self.end, defaults.end);
        let mut placed = Vec::new();
        BarLayout {
            start: kept_pieces(start, &mut placed),
            centre: kept_pieces(centre, &mut placed),
            end: kept_pieces(end, &mut placed),
            tray_hidden: self.tray_hidden.clone().unwrap_or(defaults.tray_hidden),
        }
    }
}

fn is_piece_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_PIECE_ID_BYTES
        && id
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
}

fn is_hidden_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_HIDDEN_ID_BYTES && id.chars().all(|c| !c.is_control())
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
    /// What the person chose of the bar's zones and of the tray items it keeps
    /// off; [`Settings::bar_layout`] is the layout in force. Written only for
    /// what was chosen.
    #[serde(
        deserialize_with = "lenient",
        skip_serializing_if = "BarChoice::is_unset"
    )]
    pub bar: BarChoice,
    /// Which optional duties this shell performs. Absent from a 1.0.0 file:
    /// all off.
    #[serde(deserialize_with = "lenient")]
    pub duties: Duties,
    /// The switches the person flips for the energy duties. Absent: all off.
    #[serde(deserialize_with = "lenient")]
    pub energy: Energy,
    /// The wallpaper choices.
    #[serde(deserialize_with = "lenient")]
    pub wallpaper: WallpaperSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            do_not_disturb: false,
            bar: BarChoice::default(),
            duties: Duties::default(),
            energy: Energy::default(),
            wallpaper: WallpaperSettings::default(),
        }
    }
}

/// What the person switched on for energy. The duty decides whether the shell
/// acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Energy {
    pub caffeine: bool,
    pub night_light: bool,
    pub night_light_kelvin: u32,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            caffeine: false,
            night_light: false,
            night_light_kelvin: Whitepoint::DEFAULT_KELVIN,
        }
    }
}

impl Settings {
    /// Brings every value inside its bounds. Called on the way in *and* on the
    /// way out, so neither a hand-edited file nor a broken caller can put an
    /// unusable value in front of the person.
    fn clamped(mut self) -> Self {
        self.schema_version = SCHEMA_VERSION;
        self.bar = self.bar.clamped();
        self.wallpaper = self.wallpaper.clamped();
        // The whitepoint's own range, so the file and the light never disagree.
        self.energy.night_light_kelvin = self
            .energy
            .night_light_kelvin
            .clamp(Whitepoint::MINIMUM_KELVIN, Whitepoint::MAXIMUM_KELVIN);
        self
    }

    /// The bar's layout in force: what the person chose, today's defaults
    /// for the rest.
    #[must_use]
    pub fn bar_layout(&self) -> BarLayout {
        self.bar.layout()
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

impl Store {
    /// Stages a change to the energy switches, the way do-not-disturb is
    /// staged: nothing is in force until its write is confirmed.
    #[must_use]
    pub fn stage_energy(&self, change: impl FnOnce(&mut Energy)) -> Pending {
        self.stage(|settings| change(&mut settings.energy))
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
    fn the_energy_defaults_turn_nothing_on() {
        let settings = Settings::default();
        assert_eq!(settings.duties, Duties::default());
        assert_eq!(settings.duties.idle, Duty::Off);
        assert!(!settings.energy.caffeine);
        assert!(!settings.energy.night_light);
        assert_eq!(settings.energy.night_light_kelvin, 2700);
    }

    #[test]
    fn energy_settings_survive_a_round_trip() {
        let settings = Settings {
            duties: Duties {
                bar: None,
                idle: Duty::On,
                caffeine: Duty::On,
                nightlight: Duty::Off,
                wallpaper: Duty::On,
            },
            energy: Energy {
                caffeine: true,
                night_light: true,
                night_light_kelvin: 3300,
            },
            ..Settings::default()
        };
        let bytes = settings.to_bytes().expect("settings serialize");
        let text = String::from_utf8(bytes.clone()).expect("utf8");
        assert!(text.contains("\"on\"") && text.contains("\"off\""));
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

    #[test]
    fn a_1_0_0_file_keeps_its_value_and_defaults_the_rest() {
        let read = Settings::from_bytes(br#"{"schema_version":1,"do_not_disturb":true}"#)
            .expect("an old file is read");
        assert!(read.do_not_disturb);
        assert_eq!(read.duties, Duties::default());
        assert_eq!(read.energy, Energy::default());
    }

    #[test]
    fn an_out_of_range_kelvin_is_clamped() {
        let low = Settings::from_bytes(br#"{"energy":{"night_light_kelvin":10}}"#).expect("read");
        assert_eq!(low.energy.night_light_kelvin, 2000);
        let high =
            Settings::from_bytes(br#"{"energy":{"night_light_kelvin":99999}}"#).expect("read");
        assert_eq!(high.energy.night_light_kelvin, 6500);
        let store = Store::new(Settings::default());
        let pending = store.stage_energy(|energy| energy.night_light_kelvin = 1);
        assert_eq!(pending.wanted().energy.night_light_kelvin, 2000);
    }

    #[test]
    fn an_unknown_duty_value_degrades_to_off_without_losing_the_rest() {
        let read = Settings::from_bytes(
            br#"{"do_not_disturb":true,
                 "duties":{"idle":"on","caffeine":"maybe","nightlight":7},
                 "energy":{"caffeine":true}}"#,
        )
        .expect("the file is still read");
        assert!(read.do_not_disturb);
        assert_eq!(read.duties.idle, Duty::On);
        assert_eq!(read.duties.caffeine, Duty::Off);
        assert_eq!(read.duties.nightlight, Duty::Off);
        assert!(read.energy.caffeine);
    }

    #[test]
    fn an_energy_change_is_durable_before_published() {
        let mut store = Store::new(Settings::default());
        let pending = store.stage_energy(|energy| energy.caffeine = true);
        assert!(!store.current().energy.caffeine);
        assert!(!store.apply(pending.clone(), WriteOutcome::Failed));
        assert!(!store.current().energy.caffeine);
        let bytes = pending.bytes().expect("bytes");
        assert!(store.apply(pending, WriteOutcome::Durable));
        assert!(store.current().energy.caffeine);
        assert_eq!(Settings::from_bytes(&bytes).as_ref(), Some(store.current()));
    }

    #[test]
    fn hostile_energy_input_is_still_refused() {
        let mut huge = br#"{"energy":{"night_light_kelvin":3000}}"#.to_vec();
        huge.resize(MAX_FILE_BYTES + 1, b' ');
        assert_eq!(Settings::from_bytes(&huge), None);
        // A wrong-typed table degrades to its default, like every other field.
        assert_eq!(
            Settings::from_bytes(br#"{"energy":"x"}"#),
            Some(Settings::default())
        );
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    /// The top-level keys of a written file, and the keys of its `duties`.
    fn written_keys(settings: &Settings) -> (Vec<String>, Vec<String>) {
        let bytes = settings.to_bytes().expect("settings serialize");
        let value: Value = serde_json::from_slice(&bytes).expect("JSON");
        let keys = |value: &Value| {
            value
                .as_object()
                .map(|object| object.keys().cloned().collect())
                .unwrap_or_default()
        };
        (keys(&value), keys(&value["duties"]))
    }

    #[test]
    fn the_bar_defaults_are_todays_layout() {
        let settings = Settings::default();
        let layout = settings.bar_layout();
        assert_eq!(layout.start, ids(&["workspaces", "notifications"]));
        assert!(layout.centre.is_empty());
        assert_eq!(layout.end, ids(&["tray"]));
        assert_eq!(
            layout.tray_hidden,
            ids(&["nm-applet", "blueman", "blueman-tray"])
        );
        assert_eq!(layout, BarLayout::default());
        assert_eq!(settings.duties.bar(), Duty::Off);
        assert!(settings.bar.is_unset() && settings.duties.bar.is_none());
    }

    #[test]
    fn bar_and_duties_survive_a_round_trip() {
        let mut settings = Settings::default();
        settings.bar.centre = Some(ids(&["clock"]));
        settings.bar.tray_hidden = Some(ids(&["x y"]));
        settings.duties.bar = Some(Duty::On);
        let bytes = settings.to_bytes().expect("settings serialize");
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
    }

    #[test]
    fn a_choice_that_equals_the_default_is_kept_as_a_choice() {
        // "off" written by the person stays "off" when the default turns on.
        let read = Settings::from_bytes(br#"{"duties":{"bar":"off"},"bar":{"end":["tray"]}}"#)
            .expect("ours");
        assert_eq!(read.duties.bar, Some(Duty::Off));
        assert_eq!(read.bar.end, Some(ids(&["tray"])));
        let (top, duties) = written_keys(&read);
        assert!(top.contains(&"bar".to_owned()));
        assert!(duties.contains(&"bar".to_owned()));
        let again = Settings::from_bytes(&read.to_bytes().expect("serialize")).expect("ours");
        assert_eq!(again, read);
    }

    #[test]
    fn an_unchosen_zone_follows_the_default_and_a_chosen_one_claims_its_pieces() {
        // Only the end zone was chosen: the start zone is the default's, less
        // the piece the person moved to the end.
        let read =
            Settings::from_bytes(br#"{"bar":{"end":["notifications","tray"]}}"#).expect("ours");
        let layout = read.bar_layout();
        assert_eq!(layout.start, ids(&["workspaces"]));
        assert!(layout.centre.is_empty());
        assert_eq!(layout.end, ids(&["notifications", "tray"]));
        assert_eq!(layout.tray_hidden, BarLayout::default().tray_hidden);
        // An empty chosen list is a choice too.
        let read = Settings::from_bytes(br#"{"bar":{"tray_hidden":[]}}"#).expect("ours");
        assert!(read.bar_layout().tray_hidden.is_empty());
        assert_eq!(read.bar_layout().start, BarLayout::default().start);
    }

    #[test]
    fn a_1_0_0_file_reads_with_the_new_defaults_and_keeps_the_choice() {
        let read = Settings::from_bytes(br#"{"schema_version":1,"do_not_disturb":true}"#)
            .expect("a 1.0.0 file is ours");
        assert!(read.do_not_disturb);
        assert_eq!(read.bar_layout(), BarLayout::default());
        assert_eq!(read.duties.bar(), Duty::Off);

        let written = read.to_bytes().expect("settings serialize");
        let again = Settings::from_bytes(&written).expect("written file is ours");
        assert!(again.do_not_disturb);
        assert_eq!(again.bar_layout(), BarLayout::default());
        let (top, duties) = written_keys(&again);
        assert!(!top.contains(&"bar".to_owned()));
        assert!(!duties.contains(&"bar".to_owned()));
    }

    #[test]
    fn unknown_bar_keys_are_ignored_and_dropped_on_write() {
        let read = Settings::from_bytes(
            br#"{"duties":{"bar":"on","future":"on"},"bar":{"wat":1},"extra":true}"#,
        )
        .expect("unknown keys do not make a file foreign");
        assert_eq!(read.duties.bar(), Duty::On);
        let written = String::from_utf8(read.to_bytes().expect("serialize")).expect("utf8");
        assert!(!written.contains("future") && !written.contains("extra"));
        assert!(
            !written.contains("\"bar\": {"),
            "an empty bar choice is not written"
        );
    }

    #[test]
    fn a_piece_listed_twice_keeps_its_first_place() {
        let read = Settings::from_bytes(
            br#"{"bar":{"start":["a","b","a"],"centre":["b","c"],"end":["c","d"],"tray_hidden":["x","x"]}}"#,
        )
        .expect("ours");
        assert_eq!(read.bar.start, Some(ids(&["a", "b"])));
        assert_eq!(read.bar.centre, Some(ids(&["c"])));
        assert_eq!(read.bar.end, Some(ids(&["d"])));
        assert_eq!(read.bar.tray_hidden, Some(ids(&["x"])));
        let layout = read.bar_layout();
        assert_eq!(
            (layout.start, layout.centre, layout.end),
            (ids(&["a", "b"]), ids(&["c"]), ids(&["d"]))
        );
    }

    #[test]
    fn malformed_ids_are_dropped() {
        let long_piece = "a".repeat(MAX_PIECE_ID_BYTES + 1);
        let long_hidden = "h".repeat(MAX_HIDDEN_ID_BYTES + 1);
        let json = serde_json::json!({"bar": {
            "start": ["Upper", "ok-1.x_y", "", "sp ace", long_piece],
            "tray_hidden": ["fine app", "", "tab\there", long_hidden],
        }});
        let read = Settings::from_bytes(json.to_string().as_bytes()).expect("ours");
        assert_eq!(read.bar.start, Some(ids(&["ok-1.x_y"])));
        assert_eq!(read.bar.tray_hidden, Some(ids(&["fine app"])));
    }

    #[test]
    fn thousands_of_ids_are_truncated() {
        // Ten thousand ids do not fit the file bound at all (see the oversize
        // test), so the largest list that does is split across the two bounds.
        let many: Vec<String> = (0..5_000).map(|n| format!("p{n}")).collect();
        let json = serde_json::json!({"bar": {"start": many, "tray_hidden": &many[..2_000]}});
        let read = Settings::from_bytes(json.to_string().as_bytes()).expect("ours");
        let layout = read.bar_layout();
        assert_eq!(layout.start.len(), MAX_ZONE_IDS);
        assert_eq!(layout.tray_hidden.len(), MAX_HIDDEN_IDS);
        assert_eq!(layout.start[0], "p0");
    }

    #[test]
    fn a_wrong_typed_bar_value_never_fails_the_file() {
        // Like every other field: a typo degrades that value, not the file.
        let read = Settings::from_bytes(
            br#"{"do_not_disturb":true,"duties":{"bar":"maybe","idle":"on"},
                 "bar":{"start":[1,"clock",null],"end":"tray","centre":7}}"#,
        )
        .expect("the file is still read");
        assert!(read.do_not_disturb);
        assert_eq!(read.duties.bar, None, "not a choice: the default holds");
        assert_eq!(read.duties.idle, Duty::On, "the other duties are kept");
        assert_eq!(read.bar.start, Some(ids(&["clock"])));
        assert_eq!(read.bar.end, None);
        assert_eq!(read.bar.centre, None);
        assert_eq!(read.bar_layout().end, BarLayout::default().end);
        // A bar table of the wrong shape is no choice at all.
        let read = Settings::from_bytes(br#"{"bar":[1],"duties":{"bar":5}}"#).expect("read");
        assert_eq!(read, Settings::default());
    }

    #[test]
    fn a_hostile_oversize_bar_is_refused() {
        let blob = "a".repeat(MAX_FILE_BYTES);
        let json = format!(r#"{{"bar":{{"start":["{blob}"]}}}}"#);
        assert_eq!(Settings::from_bytes(json.as_bytes()), None);
    }

    #[test]
    fn dnd_and_bar_changes_do_not_touch_each_other() {
        let mut store = Store::new(Settings::default());
        let bar_before = store.current().bar.clone();
        let pending = store.stage(|s| s.do_not_disturb = true);
        assert!(store.apply(pending, WriteOutcome::Durable));
        assert_eq!(store.current().bar, bar_before);
        assert_eq!(store.current().duties.bar, None);

        let pending = store.stage(|s| {
            s.bar.centre = Some(ids(&["clock"]));
            s.duties.bar = Some(Duty::On);
        });
        assert!(store.apply(pending, WriteOutcome::Durable));
        assert!(store.current().do_not_disturb);
        assert_eq!(store.current().bar_layout().centre, ids(&["clock"]));
    }

    #[test]
    fn a_do_not_disturb_toggle_writes_no_bar_choice_the_person_did_not_make() {
        let read = Settings::from_bytes(br#"{"schema_version":1,"do_not_disturb":false}"#)
            .expect("a 1.0.0 file is ours");
        let store = Store::new(read);
        let pending = store.stage(|s| s.do_not_disturb = true);
        for settings in [pending.wanted(), &Settings::default()] {
            let (top, duties) = written_keys(settings);
            assert!(!top.contains(&"bar".to_owned()), "{top:?}");
            assert!(!duties.contains(&"bar".to_owned()), "{duties:?}");
        }
    }

    #[test]
    fn a_staged_bar_is_clamped_like_a_read_one() {
        let store = Store::new(Settings::default());
        let pending = store.stage(|s| s.bar.start = Some(ids(&["a", "a", "BAD"])));
        assert_eq!(pending.wanted().bar.start, Some(ids(&["a"])));
    }
}
