// Copied from `celestina-rs/crates/celestina-shell-core/src/settings.rs` at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9, adapted: `Settings` keeps only
// `do_not_disturb` (the halted shell's `quiet`) and `schema_version`, with
// `SCHEMA_VERSION = 1` for this shell's own file `celestina/shell.json`; the
// tray, wallpaper, weather, night-light, level-step and workspace-home fields,
// their bounds and their tests are dropped. `Store`, `Pending`, `WriteOutcome`
// and the durable-before-published rule are unchanged. CEL-2-A then added
// what the copy did not have: `bar` (the layout of the bar's zones and the
// tray ids kept off it), optional so a 1.0.0 file still reads, plus its
// bounds in `clamped` and its tests; the `duties.bar` switch it also added
// (whether the shell had taken the bar over from Noctalia) is gone since the
// cutover (CEL-2-M): the bar is always on, and a `"bar"` key left in `duties`
// by an older file is an unknown key, read past and dropped on the next
// write. CEL-2-K then added `weather`
// (where the weather reading is for), copied in shape from the core's
// `Location` and stored in whole micro-degrees so `Settings` stays `Eq`; it is
// optional, lenient and written only once chosen. CEL-2-C adds `duties.osd`
// and the `osd` table (timeout, position, kinds), all defaulted per field.
// CEL-2-H adds `duties.lock`, off like the other switches. CEL-2-M, the
// cutover, flips the default of every shipped duty to on (`lenient_duty_on`,
// `impl Default for Duties`) and removes the `bar` duty; the lock keeps off
// until its live rows pass, and the clipboard, a privacy duty, fails closed:
// a value that is not `"on"` or `"off"` reads as off (`lenient_duty`), while
// an absent key still reads as on. `Duties::all_off` is the quiet set a
// protected start (an unreadable file) runs on until a clean read.
// CEL-2-M-LOCK-POLKIT flips `duties.lock` and `duties.polkit` to on by
// default once their live rows passed; both keep `lenient_duty` and so fail
// closed like the clipboard, and `Duties::unclear` names all three off.

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

/// A switch that is on unless the file says `false`: a wrong-typed value keeps
/// the default instead of silently turning the switch off.
fn lenient_true<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(Value::deserialize(deserializer)?.as_bool().unwrap_or(true))
}

/// A timeout in milliseconds; anything that is not a non-negative integer is
/// the default (the bounds are applied afterwards by `clamped`).
fn lenient_timeout<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    Ok(Value::deserialize(deserializer)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(DEFAULT_OSD_TIMEOUT_MS))
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

/// A duty whose written value is off unless it says `"on"`: a value of the
/// wrong type or spelling is read as off. For the three duties that fail
/// closed, the clipboard ([`Duties::clipboard`]), the session lock
/// ([`Duties::lock`]) and the polkit agent ([`Duties::polkit`]): each is on
/// when its key is absent, which serde never brings here (the struct's
/// default fills the field), and off when the key is there but unreadable.
fn lenient_duty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duty, D::Error> {
    lenient(deserializer)
}

/// A duty that is on unless the file says `"off"`. Since the cutover
/// (CEL-2-M) the shipped duties default to on, so a wrong-typed value keeps
/// that default instead of silently turning the duty off, the way
/// [`lenient_true`] keeps a switch on. For every shipped duty but the three
/// that fail closed ([`lenient_duty`]): a typo that leaves one of these on
/// costs nothing a person cannot see and switch off.
fn lenient_duty_on<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duty, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(deserializer)?).unwrap_or(Duty::On))
}

/// The `duties` table. An object is read field by field with each duty's
/// own leniency ([`lenient_duty`], [`lenient_duty_on`]). Anything else
/// (`"off"`, `false`, `null`, a number, an array, a string) is a file that
/// does not clearly say what it wants: the defaults, with the clipboard,
/// the session lock and the polkit agent **off** ([`Duties::unclear`]).
/// Those three fail closed whenever the file names them, or the whole
/// table, without clearly saying "on" ([`Duties::clipboard`],
/// [`Duties::lock`], [`Duties::polkit`]), and an array is never read
/// positionally into the fields, which serde would otherwise do.
fn lenient_duties<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duties, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        object @ Value::Object(_) => {
            serde_json::from_value(object).unwrap_or_else(|_| Duties::unclear())
        }
        _ => Duties::unclear(),
    })
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

/// A lenient `weather` value: a well-formed location, or no choice. A wrong
/// type or an out-of-range coordinate is none of the file's business to fail
/// on; an oversize label is shortened, never a reason to drop the place.
fn lenient_location<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Location>, D::Error> {
    let raw = Value::deserialize(deserializer)?;
    let Ok(wire) = serde_json::from_value::<LocationWire>(raw) else {
        return Ok(None);
    };
    Ok(Location::new(wire.latitude, wire.longitude, &wire.label))
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

/// Whether a background duty is switched on. The type's own default is off:
/// it is what a duty that fails closed reads an unreadable value as
/// (`lenient_duty`). What each duty is when a file does not name it is
/// [`Duties::default`], not this.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Duty {
    #[default]
    Off,
    On,
}

/// The shell's optional duties, one line per duty in name order. While
/// Noctalia held them every duty shipped off (ADR 0011); since the cutover
/// (CEL-2-M) the duties the author validated live default to **on**, and a
/// value that is not `"on"` or `"off"` degrades to the field's default
/// instead of failing the file, with three exceptions that fail closed, so
/// such a value reads as off: the clipboard, a privacy duty
/// ([`Duties::clipboard`]), and the two privileged duties, the session lock
/// ([`Duties::lock`]) and the polkit agent ([`Duties::polkit`]). Those two
/// stayed `off` through the cutover until their live rows passed (the lock
/// design's §8) and default to on since CEL-2-M-LOCK-POLKIT; an explicit
/// `"off"` in a file is honoured, and the shell writes every duty, so the
/// new default reaches only a file with no such key. The bar is not a duty
/// any more: the shell always draws it (its layout is [`Settings::bar`]),
/// and a `"bar"` key an older file left here is an unknown key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Duties {
    /// Whether this shell groups minimized windows into bubbles.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub bubbles: Duty,
    /// Whether this shell keeps the session from idling while caffeine is on.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub caffeine: Duty,
    /// Whether this shell keeps a clipboard history. On when the key is
    /// absent, like the other shipped duties, but it fails closed: a value
    /// that is not `"on"` or `"off"` (`false`, `"OFF"`, `"disabled"`, a
    /// number) reads as **off**, because a person who tried to write "off"
    /// and got it wrong must not have every selection recorded to disk
    /// (privacy, CEL-2-G); the other duties cost nothing when a typo leaves
    /// them on.
    #[serde(deserialize_with = "lenient_duty")]
    pub clipboard: Duty,
    /// Whether this shell turns the screens off, locks and suspends when idle.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub idle: Duty,
    /// Whether this shell's own session lock runs: the `lock` verbs, the idle
    /// chain's second step and the sleep inhibitor (CEL-2-H). On when the
    /// key is absent, since the lock's live rows passed
    /// (CEL-2-M-LOCK-POLKIT); a person whose session another locker owns
    /// writes `"off"`. It fails closed, like [`Duties::clipboard`]: a value
    /// that is not `"on"` or `"off"` reads as **off**, because this is a
    /// privileged duty and a person who tried to write "off" and got it
    /// wrong must not get a lock that can cover the screen and, on a
    /// machine with no PAM service installed for it, cannot be unlocked.
    /// Off by mistake costs a session that does not lock when idle, which
    /// the person can see and fix; on by mistake could lock them out.
    #[serde(deserialize_with = "lenient_duty")]
    pub lock: Duty,
    /// Whether this shell listens to MPRIS players and shows the media piece
    /// and page.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub media: Duty,
    /// Whether this shell drives the night-light gamma ramps.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub nightlight: Duty,
    /// Whether this shell shows the on-screen display for volume,
    /// microphone and brightness changes.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub osd: Duty,
    /// Whether this shell shows the phone's battery and media.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub phone: Duty,
    /// Whether this shell is the session's polkit authentication agent
    /// (CEL-2-I). On when the key is absent, since the agent's live row
    /// passed (CEL-2-M-LOCK-POLKIT): the shell asks to be the session's
    /// agent when it starts, and while another agent holds the session that
    /// is refused, never forced. It fails closed, like
    /// [`Duties::clipboard`]: a value that is not `"on"` or `"off"` reads
    /// as **off**, because this is a privileged duty and a person who tried
    /// to write "off" and got it wrong must not get an agent that registers
    /// for the session and takes the keyboard for password prompts. Off by
    /// mistake costs a prompt that never appears, which polkit answers by
    /// refusing: its safe baseline, and one the person can see and fix.
    #[serde(deserialize_with = "lenient_duty")]
    pub polkit: Duty,
    /// Whether this shell paints the wallpaper.
    #[serde(deserialize_with = "lenient_duty_on")]
    pub wallpaper: Duty,
}

impl Default for Duties {
    /// The cutover's defaults: every shipped duty on, the session lock and
    /// the polkit agent included since their live rows passed
    /// (CEL-2-M-LOCK-POLKIT).
    fn default() -> Self {
        Self::defaults()
    }
}

impl Duties {
    /// What a `duties` value that is not an object reads as (`"off"`,
    /// `false`, `null`, `5`, `[]`), and what an object serde cannot read
    /// falls back to: the defaults with the clipboard, the lock and the
    /// polkit agent off. A person who wrote `"duties": "off"` meant
    /// something this shell cannot read as a table, and the three duties
    /// that fail closed must not run on a guess: the one that records to
    /// disk ([`Duties::clipboard`]) and the two privileged ones
    /// ([`Duties::lock`], [`Duties::polkit`]), which default to on and so
    /// have to be named off here.
    #[must_use]
    pub const fn unclear() -> Self {
        Self {
            clipboard: Duty::Off,
            lock: Duty::Off,
            polkit: Duty::Off,
            ..Self::defaults()
        }
    }

    /// [`Duties::default`] as a `const fn`, so [`Duties::unclear`] can build
    /// on it.
    const fn defaults() -> Self {
        Self {
            bubbles: Duty::On,
            caffeine: Duty::On,
            clipboard: Duty::On,
            idle: Duty::On,
            lock: Duty::On,
            media: Duty::On,
            nightlight: Duty::On,
            osd: Duty::On,
            phone: Duty::On,
            polkit: Duty::On,
            wallpaper: Duty::On,
        }
    }

    /// Every duty off: what the shell runs on while its settings file exists
    /// but cannot be read (a protected start), so a typo in a hand-edited
    /// file never starts a worker the file may say `"off"` to, the session
    /// lock and the polkit agent included, whatever their default is; the
    /// first clean read then applies the real values. Also the set a test
    /// builds when it wants to start exactly the duties it names. Adding a
    /// duty adds a line here, so the quiet set can never miss one.
    #[must_use]
    pub const fn all_off() -> Self {
        Self {
            bubbles: Duty::Off,
            caffeine: Duty::Off,
            clipboard: Duty::Off,
            idle: Duty::Off,
            lock: Duty::Off,
            media: Duty::Off,
            nightlight: Duty::Off,
            osd: Duty::Off,
            phone: Duty::Off,
            polkit: Duty::Off,
            wallpaper: Duty::Off,
        }
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
            // Noctalia's order: tray, network, bluetooth (CEL-2-F), the
            // volume and the brightness (CEL-2-C; both absent until a reading
            // exists), then the clipboard (CEL-2-G) and the session menu
            // (CEL-I-006) at the very end. `battery` and `power-profile` are
            // registered pieces but not placed.
            end: ids(&[
                "tray",
                "network",
                "bluetooth",
                "volume",
                "brightness",
                "clipboard",
                "session",
            ]),
            // Blueman's item `Id` is expected to be `blueman` (the halted
            // shell's fixtures of this session); `blueman-tray` is its
            // process. Both until the live row confirms the `Id`.
            tray_hidden: ids(&["nm-applet", "blueman", "blueman-tray"]),
        }
    }
}

impl BarLayout {
    /// Whether hiding `id` would change the list: the id can be listed and is
    /// not listed yet while the list has room.
    #[must_use]
    pub fn can_hide(&self, id: &str) -> bool {
        is_hidden_id(id)
            && !self.tray_hidden.iter().any(|have| have == id)
            && self.tray_hidden.len() < MAX_HIDDEN_IDS
    }

    /// The hidden list with `id` hidden or shown: hiding appends it once
    /// (at the end), showing removes it. An id that cannot be listed (empty,
    /// too long, control characters) leaves the list as it is.
    #[must_use]
    pub fn tray_hidden_with(&self, id: &str, hide: bool) -> Vec<String> {
        let mut ids = self.tray_hidden.clone();
        if hide {
            if is_hidden_id(id) && !ids.iter().any(|have| have == id) {
                ids.push(id.to_owned());
            }
        } else {
            ids.retain(|have| have != id);
        }
        kept_hidden(ids)
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

    /// Chooses the tray items the bar hides, kept well formed, once each and
    /// bounded.
    pub fn set_tray_hidden(&mut self, ids: Vec<String>) {
        self.tray_hidden = Some(kept_hidden(ids));
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

/// The longest place label kept, in characters.
pub const MAX_PLACE_CHARS: usize = 64;
/// Micro-degrees in a degree.
const MICRO: f64 = 1_000_000.0;

/// Where the weather reading is for. Coordinates rather than a place lookup:
/// this shell sends one pair of numbers to one service and nothing else, and a
/// name that had to be resolved would be a second thing to send.
///
/// Coordinates are whole micro-degrees (about 0.11 m), so the type is `Eq` and
/// a round trip through the file is exact; the file holds plain degrees.
#[derive(Clone, PartialEq, Eq)]
pub struct Location {
    latitude_micro: i32,
    longitude_micro: i32,
    /// What to call it on screen. Never sent anywhere.
    pub label: String,
}

/// Where a person is is not a diagnostic: the formatter names the label's
/// length and nothing else, so no log or assertion message carries a position.
impl std::fmt::Debug for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Location")
            .field("label_chars", &self.label.chars().count())
            .finish_non_exhaustive()
    }
}

/// The one rule for a place label: control characters removed, the ends
/// trimmed, then cut to [`MAX_PLACE_CHARS`] Unicode scalar values. A label is
/// shortened, never a reason to refuse the location it names.
fn clean_label(label: &str) -> String {
    let stripped: String = label.chars().filter(|c| !c.is_control()).collect();
    let cut: String = stripped.trim().chars().take(MAX_PLACE_CHARS).collect();
    cut.trim_end().to_owned()
}

/// The file's shape for a location.
#[derive(Serialize, Deserialize)]
struct LocationWire {
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    label: String,
}

impl Location {
    /// Reads one location, or nothing when it cannot be a place on earth.
    ///
    /// A NaN or out-of-range coordinate is refused rather than clamped: 0°,0°
    /// is a real place in the Atlantic, and silently sending somebody there
    /// would be worse than having no weather. The label goes through
    /// [`clean_label`].
    #[must_use]
    pub fn new(latitude: f64, longitude: f64, label: &str) -> Option<Self> {
        if !latitude.is_finite() || !longitude.is_finite() {
            return None;
        }
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return None;
        }

        Some(Self {
            // In range, so the products fit an i32 exactly.
            latitude_micro: (latitude * MICRO).round() as i32,
            longitude_micro: (longitude * MICRO).round() as i32,
            label: clean_label(label),
        })
    }

    /// Degrees north, as the file and the request carry them.
    #[must_use]
    pub fn latitude(&self) -> f64 {
        f64::from(self.latitude_micro) / MICRO
    }

    /// Degrees east.
    #[must_use]
    pub fn longitude(&self) -> f64 {
        f64::from(self.longitude_micro) / MICRO
    }
}

impl Serialize for Location {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        LocationWire {
            latitude: self.latitude(),
            longitude: self.longitude(),
            label: self.label.clone(),
        }
        .serialize(serializer)
    }
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

/// Everything the person chose. Outside [`Settings::duties`], which default
/// to on since the cutover ([`Duties::default`]), every field has a default
/// that is the quietest possible behaviour: nothing else here turns itself
/// on.
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
    /// the cutover's defaults, every shipped duty on ([`Duties::default`]).
    /// Present but not an object: the same with the clipboard, the lock and
    /// the polkit agent off ([`Duties::unclear`], read by `lenient_duties`).
    #[serde(deserialize_with = "lenient_duties")]
    pub duties: Duties,
    /// The switches the person flips for the energy duties. Absent: all off.
    #[serde(deserialize_with = "lenient")]
    pub energy: Energy,
    /// The wallpaper choices.
    #[serde(deserialize_with = "lenient")]
    pub wallpaper: WallpaperSettings,
    /// Where the weather reading is for. Absent: no weather, and no request
    /// ever leaves the machine. Written only once chosen.
    #[serde(
        deserialize_with = "lenient_location",
        skip_serializing_if = "Option::is_none"
    )]
    pub weather: Option<Location>,
    /// How the on-screen display looks and which changes it shows.
    #[serde(deserialize_with = "lenient")]
    pub osd: OsdSettings,
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
            weather: None,
            osd: OsdSettings::default(),
        }
    }
}

/// The shortest on-screen display timeout kept, in milliseconds.
pub const MIN_OSD_TIMEOUT_MS: u32 = 500;
/// The longest on-screen display timeout kept, in milliseconds.
pub const MAX_OSD_TIMEOUT_MS: u32 = 5000;
/// How long the on-screen display stays when the person did not choose.
pub const DEFAULT_OSD_TIMEOUT_MS: u32 = 1500;

/// Where the on-screen display sits on its output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsdPosition {
    /// Centred along the bottom edge.
    #[default]
    #[serde(rename = "bottom")]
    Bottom,
    /// In the bottom-right corner.
    #[serde(rename = "bottom-right")]
    BottomRight,
}

/// Which kinds of change the on-screen display shows. All on by default; the
/// duty decides whether any is shown at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OsdKinds {
    #[serde(deserialize_with = "lenient_true")]
    pub volume: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub microphone: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub brightness: bool,
}

impl Default for OsdKinds {
    fn default() -> Self {
        Self {
            volume: true,
            microphone: true,
            brightness: true,
        }
    }
}

/// The on-screen display's own choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OsdSettings {
    /// How long it stays after the last change, in milliseconds, kept between
    /// [`MIN_OSD_TIMEOUT_MS`] and [`MAX_OSD_TIMEOUT_MS`].
    #[serde(deserialize_with = "lenient_timeout")]
    pub timeout_ms: u32,
    #[serde(deserialize_with = "lenient")]
    pub position: OsdPosition,
    #[serde(deserialize_with = "lenient")]
    pub kinds: OsdKinds,
}

impl Default for OsdSettings {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_OSD_TIMEOUT_MS,
            position: OsdPosition::default(),
            kinds: OsdKinds::default(),
        }
    }
}

impl OsdSettings {
    fn clamped(mut self) -> Self {
        self.timeout_ms = self
            .timeout_ms
            .clamp(MIN_OSD_TIMEOUT_MS, MAX_OSD_TIMEOUT_MS);
        self
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
        self.weather = self.weather.map(|place| Location {
            label: clean_label(&place.label),
            ..place
        });
        self.osd = self.osd.clamped();
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
        assert_eq!(read.duties.wallpaper, Duty::On);
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
    fn the_defaults_turn_the_shipped_duties_on_and_the_energy_switches_off() {
        let settings = Settings::default();
        assert_eq!(settings.duties, Duties::default());
        assert_eq!(settings.duties.idle, Duty::On);
        assert_eq!(settings.duties.lock, Duty::On);
        assert_eq!(settings.duties.polkit, Duty::On);
        assert!(!settings.energy.caffeine);
        assert!(!settings.energy.night_light);
        assert_eq!(settings.energy.night_light_kelvin, 2700);
    }

    #[test]
    fn energy_settings_survive_a_round_trip() {
        let settings = Settings {
            duties: Duties {
                bubbles: Duty::On,
                caffeine: Duty::On,
                clipboard: Duty::Off,
                idle: Duty::On,
                lock: Duty::On,
                media: Duty::On,
                nightlight: Duty::Off,
                osd: Duty::Off,
                phone: Duty::On,
                polkit: Duty::Off,
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
        assert_eq!(read.duties.wallpaper, Duty::On);
        assert_eq!(read.wallpaper.mode, Mode::Fill);
        assert_eq!(read.wallpaper.folder, None);
        assert_eq!(read.wallpaper.outputs.len(), 1);
        assert_eq!(read.wallpaper.outputs["HDMI-A-1"], "b.png");

        for bytes in [
            &br#"{"do_not_disturb":"yes"}"#[..],
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
        // A `duties` that is not an object is the one wrong-typed value that
        // does not read as the plain defaults: the clipboard, the lock and
        // the polkit agent fail closed (`Duties::unclear`), the rest of the
        // file is still read.
        assert_eq!(
            Settings::from_bytes(br#"{"duties":5,"wallpaper":"x"}"#),
            Some(Settings {
                duties: Duties::unclear(),
                ..Settings::default()
            })
        );
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
    fn the_cutover_defaults_reach_a_file_that_chose_nothing_and_keep_every_off() {
        // A file from before the cutover with no `duties` key: every shipped
        // duty is on (CEL-2-M), the lock and the polkit agent included since
        // their live rows passed (CEL-2-M-LOCK-POLKIT).
        let read = Settings::from_bytes(br#"{"schema_version":1,"do_not_disturb":false}"#)
            .expect("an older file is read");
        for (name, duty) in [
            ("bubbles", read.duties.bubbles),
            ("caffeine", read.duties.caffeine),
            ("clipboard", read.duties.clipboard),
            ("idle", read.duties.idle),
            ("lock", read.duties.lock),
            ("media", read.duties.media),
            ("nightlight", read.duties.nightlight),
            ("osd", read.duties.osd),
            ("phone", read.duties.phone),
            ("polkit", read.duties.polkit),
            ("wallpaper", read.duties.wallpaper),
        ] {
            assert_eq!(duty, Duty::On, "{name}");
        }
        assert_eq!(read.duties, Duties::default());
        // An explicit "off" written while Noctalia held a duty, or by a shell
        // from before a default flipped, is honoured.
        let chosen = Settings::from_bytes(
            br#"{"duties":{"idle":"off","osd":"off","lock":"off","polkit":"off","wallpaper":"off"}}"#,
        )
        .expect("read");
        assert_eq!(chosen.duties.idle, Duty::Off);
        assert_eq!(chosen.duties.osd, Duty::Off);
        assert_eq!(chosen.duties.wallpaper, Duty::Off);
        assert_eq!(chosen.duties.lock, Duty::Off);
        assert_eq!(chosen.duties.polkit, Duty::Off);
        assert_eq!(chosen.duties.media, Duty::On);
        // The clipboard, the lock and the polkit agent are on only for an
        // absent key or an explicit "on": a value that is neither reads as
        // off, where the other shipped duties keep their default of on.
        let typo = Settings::from_bytes(
            br#"{"duties":{"clipboard":"Off","idle":"Off","lock":"Off","polkit":"Off"}}"#,
        )
        .expect("read");
        assert_eq!(typo.duties.clipboard, Duty::Off);
        assert_eq!(typo.duties.lock, Duty::Off);
        assert_eq!(typo.duties.polkit, Duty::Off);
        assert_eq!(typo.duties.idle, Duty::On);
        assert_eq!(chosen.duties.clipboard, Duty::On, "absent: on");
    }

    #[test]
    fn all_off_names_every_duty_and_differs_from_the_defaults_in_the_duties_alone() {
        let quiet = Duties::all_off();
        let (_, written) = written_keys(&Settings {
            duties: quiet,
            ..Settings::default()
        });
        let (_, defaults) = written_keys(&Settings::default());
        assert_eq!(written, defaults, "the same keys, so no duty is missed");
        let value = serde_json::to_value(quiet).expect("duties serialize");
        for (name, duty) in value.as_object().expect("an object") {
            assert_eq!(duty, "off", "{name}");
        }
        // A protected start runs on this set: the two privileged duties are
        // off in it whatever their default is.
        assert_eq!(quiet.lock, Duty::Off);
        assert_eq!(quiet.polkit, Duty::Off);
        assert_eq!(Duties::default().lock, Duty::On);
        assert_eq!(Duties::default().polkit, Duty::On);
        assert_ne!(quiet, Duties::default());
        // The round trip keeps every off.
        let settings = Settings {
            duties: quiet,
            ..Settings::default()
        };
        let bytes = settings.to_bytes().expect("serialize");
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
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
    fn an_unknown_duty_value_degrades_to_its_default_without_losing_the_rest() {
        let read = Settings::from_bytes(
            br#"{"do_not_disturb":true,
                 "duties":{"idle":"on","caffeine":"maybe","nightlight":7},
                 "energy":{"caffeine":true}}"#,
        )
        .expect("the file is still read");
        assert!(read.do_not_disturb);
        assert_eq!(read.duties.idle, Duty::On);
        assert_eq!(read.duties.caffeine, Duty::On);
        assert_eq!(read.duties.nightlight, Duty::On);
        assert!(read.energy.caffeine);
    }

    #[test]
    fn the_polkit_duty_is_on_by_default_and_in_a_file_without_the_key() {
        // Default: on, for a fresh file, for a file with no `duties` key and
        // for a `duties` object that predates the key (CEL-2-M-LOCK-POLKIT).
        assert_eq!(Settings::default().duties.polkit, Duty::On);
        let bare = Settings::from_bytes(br#"{"schema_version":1,"do_not_disturb":true}"#)
            .expect("an old file is read");
        assert_eq!(bare.duties.polkit, Duty::On);
        let without = Settings::from_bytes(br#"{"duties":{"idle":"on"}}"#).expect("read");
        assert_eq!(without.duties.polkit, Duty::On);
        assert_eq!(without.duties.idle, Duty::On);
        // The person's choice is kept, either way.
        let on = Settings::from_bytes(br#"{"duties":{"polkit":"on"}}"#).expect("read");
        assert_eq!(on.duties.polkit, Duty::On);
        let off = Settings::from_bytes(br#"{"duties":{"polkit":"off"}}"#).expect("read");
        assert_eq!(off.duties.polkit, Duty::Off);
        assert_eq!(
            off.duties.lock,
            Duty::On,
            "one duty's off is not the other's"
        );
    }

    #[test]
    fn a_hostile_polkit_duty_degrades_to_off_without_losing_the_rest() {
        // The polkit agent fails closed, like the clipboard: a value that is
        // not "on" or "off" reads as off, so a person who tried to write
        // "off" and got it wrong never has this shell register as the
        // session's agent and take the keyboard for a password. Garbage of
        // any shape costs the rest of the file nothing.
        for garbage in GARBAGE_DUTY_VALUES {
            let bytes =
                format!(r#"{{"do_not_disturb":true,"duties":{{"polkit":{garbage},"idle":"on"}}}}"#);
            let read = Settings::from_bytes(bytes.as_bytes()).expect("the file is still read");
            assert_eq!(read.duties.polkit, Duty::Off, "{garbage}");
            assert_eq!(read.duties.lock, Duty::On, "{garbage}");
            assert_eq!(read.duties.idle, Duty::On, "{garbage}");
            assert!(read.do_not_disturb, "{garbage}");
        }
        // A change is durable before it is published, like every duty.
        let store = Store::new(Settings::default());
        let pending = store.stage(|s| s.duties.polkit = Duty::Off);
        assert_eq!(store.current().duties.polkit, Duty::On);
        assert_eq!(pending.wanted().duties.polkit, Duty::Off);
    }

    #[test]
    fn the_polkit_duty_survives_a_round_trip_in_name_order() {
        // Between nightlight and wallpaper, on and off alike.
        for polkit in [Duty::On, Duty::Off] {
            let mut settings = Settings::default();
            settings.duties.polkit = polkit;
            let bytes = settings.to_bytes().expect("settings serialize");
            assert_eq!(Settings::from_bytes(&bytes), Some(settings), "{polkit:?}");
            let text = String::from_utf8(bytes).expect("utf8");
            let at = |key: &str| text.find(key).expect(key);
            assert!(at("\"nightlight\"") < at("\"polkit\""));
            assert!(at("\"polkit\"") < at("\"wallpaper\""));
        }
    }

    #[test]
    fn the_lock_and_the_polkit_agent_are_written_on_and_an_explicit_off_survives_a_write() {
        // What the store writes for the defaults names both duties "on", and
        // reads back as the defaults: a file this version wrote carries the
        // value, so a later change of default cannot move it.
        let store = Store::new(Settings::default());
        let untouched = store.stage(|_| {});
        let bytes = untouched.bytes().expect("bytes to write");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains(r#""lock": "on""#), "{text}");
        assert!(text.contains(r#""polkit": "on""#), "{text}");
        assert_eq!(Settings::from_bytes(&bytes), Some(Settings::default()));

        // A file an earlier version wrote carries explicit "off" values. They
        // are read as off, and a write for an unrelated change keeps them:
        // the flip reaches only a file that has no key.
        let earlier = br#"{"schema_version":1,"do_not_disturb":false,
            "duties":{"bubbles":"on","caffeine":"on","clipboard":"on","idle":"on",
                      "lock":"off","media":"on","nightlight":"on","osd":"on",
                      "phone":"on","polkit":"off","wallpaper":"on"}}"#;
        let read = Settings::from_bytes(earlier).expect("an earlier file is read");
        assert_eq!(read.duties.lock, Duty::Off);
        assert_eq!(read.duties.polkit, Duty::Off);
        let mut store = Store::new(read);
        let pending = store.stage(|s| s.do_not_disturb = true);
        let bytes = pending.bytes().expect("bytes to write");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains(r#""lock": "off""#), "{text}");
        assert!(text.contains(r#""polkit": "off""#), "{text}");
        assert!(store.apply(pending, WriteOutcome::Durable));
        let again = Settings::from_bytes(&bytes).expect("read back");
        assert_eq!(&again, store.current());
        assert_eq!(again.duties.lock, Duty::Off);
        assert_eq!(again.duties.polkit, Duty::Off);
        assert!(again.do_not_disturb);
    }

    #[test]
    fn phone_and_bubbles_default_on_and_read_on_from_older_files() {
        let defaults = Settings::default();
        assert_eq!(defaults.duties.phone, Duty::On);
        assert_eq!(defaults.duties.bubbles, Duty::On);
        // A 1.3.0 file knows neither duty.
        let older = Settings::from_bytes(
            br#"{"schema_version":1,"duties":{"idle":"on","wallpaper":"on"}}"#,
        )
        .expect("read");
        assert_eq!(older.duties.phone, Duty::On);
        assert_eq!(older.duties.bubbles, Duty::On);
        assert_eq!(older.duties.idle, Duty::On);
        // An unknown value is the default, on, and costs the other duties nothing.
        let odd = Settings::from_bytes(br#"{"duties":{"phone":"maybe","bubbles":7,"idle":"on"}}"#)
            .expect("read");
        assert_eq!(odd.duties.phone, Duty::On);
        assert_eq!(odd.duties.bubbles, Duty::On);
        assert_eq!(odd.duties.idle, Duty::On);
    }

    #[test]
    fn phone_and_bubbles_survive_a_round_trip() {
        let mut settings = Settings::default();
        settings.duties.phone = Duty::On;
        settings.duties.bubbles = Duty::Off;
        let bytes = settings.to_bytes().expect("serialize");
        let read = Settings::from_bytes(&bytes).expect("read");
        assert_eq!(read.duties.phone, Duty::On);
        assert_eq!(read.duties.bubbles, Duty::Off);
        let text = String::from_utf8(bytes).expect("utf8");
        assert!(text.contains("\"phone\"") && text.contains("\"bubbles\""));
    }

    #[test]
    fn the_media_duty_defaults_on_and_a_bad_value_degrades_without_losing_the_rest() {
        assert_eq!(Settings::default().duties.media, Duty::On);
        let read = Settings::from_bytes(
            br#"{"do_not_disturb":true,"duties":{"media":"yes","idle":"on"}}"#,
        )
        .expect("the file is still read");
        assert_eq!(read.duties.media, Duty::On);
        assert_eq!(read.duties.idle, Duty::On);
        assert!(read.do_not_disturb);
        let off = Settings::from_bytes(br#"{"duties":{"media":"off"}}"#).expect("read");
        assert_eq!(off.duties.media, Duty::Off);
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

    #[test]
    fn the_clipboard_duty_is_on_by_default_and_in_an_old_file() {
        assert_eq!(Settings::default().duties.clipboard, Duty::On);
        let read = Settings::from_bytes(br#"{"schema_version":1,"do_not_disturb":true}"#)
            .expect("an old file is read");
        assert_eq!(read.duties.clipboard, Duty::On);
        let (_, duties) = written_keys(&Settings::default());
        assert_eq!(
            duties,
            [
                "bubbles",
                "caffeine",
                "clipboard",
                "idle",
                "lock",
                "media",
                "nightlight",
                "osd",
                "phone",
                "polkit",
                "wallpaper"
            ]
        );
    }

    #[test]
    fn the_clipboard_duty_survives_a_round_trip() {
        let mut settings = Settings::default();
        settings.duties.clipboard = Duty::Off;
        let bytes = settings.to_bytes().expect("serialize");
        assert!(String::from_utf8_lossy(&bytes).contains(r#""clipboard": "off""#));
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
    }

    #[test]
    fn a_hostile_clipboard_duty_degrades_to_off_without_losing_the_rest() {
        // The clipboard fails closed (privacy): a value that is not "on" or
        // "off" is read as off, where the other shipped duties would keep
        // their default of on. A person who wrote `false` or "OFF" meant
        // off; a person who wrote "yes" or "ON" gets the duty off and
        // nothing recorded. This crate logs nothing, so the typo itself is
        // not reported anywhere: the only sign is the duty being off. Only
        // an absent key is on.
        for value in GARBAGE_DUTY_VALUES {
            let json = format!(
                r#"{{"do_not_disturb":true,"duties":{{"clipboard":{value},"idle":"on"}}}}"#
            );
            let read = Settings::from_bytes(json.as_bytes()).expect("the file is still read");
            assert_eq!(read.duties.clipboard, Duty::Off, "{value}");
            assert_eq!(read.duties.idle, Duty::On, "{value}");
            assert!(read.do_not_disturb, "{value}");
        }
        let absent = Settings::from_bytes(br#"{"duties":{"idle":"on"}}"#).expect("read");
        assert_eq!(absent.duties.clipboard, Duty::On, "absent: the default");
        let store = Store::new(Settings::default());
        let pending = store.stage(|s| s.duties.clipboard = Duty::Off);
        assert_eq!(store.current().duties.clipboard, Duty::On);
        assert_eq!(pending.wanted().duties.clipboard, Duty::Off);
    }

    #[test]
    fn a_duties_value_that_is_not_an_object_reads_as_the_defaults_with_the_fail_closed_duties_off()
    {
        // I-1 of the cutover re-review: `"duties": "off"` or `false`, meant
        // as "all off", must not turn the clipboard on, and since the lock
        // and the polkit agent default to on (CEL-2-M-LOCK-POLKIT) it must
        // not start either of them. Whatever is not an object reads as the
        // defaults with the clipboard, the lock and the polkit agent off,
        // and the rest of the file is kept.
        for value in [
            r#""off""#,
            r#""on""#,
            r#""all off""#,
            "false",
            "true",
            "null",
            "5",
            "[]",
        ] {
            let json = format!(
                r#"{{"do_not_disturb":true,"duties":{value},"energy":{{"caffeine":true}}}}"#
            );
            let read = Settings::from_bytes(json.as_bytes()).expect("the file is still read");
            assert_eq!(read.duties, Duties::unclear(), "{value}");
            assert_eq!(read.duties.clipboard, Duty::Off, "{value}");
            assert_eq!(read.duties.lock, Duty::Off, "{value}");
            assert_eq!(read.duties.polkit, Duty::Off, "{value}");
            for (name, duty) in [
                ("bubbles", read.duties.bubbles),
                ("caffeine", read.duties.caffeine),
                ("idle", read.duties.idle),
                ("media", read.duties.media),
                ("nightlight", read.duties.nightlight),
                ("osd", read.duties.osd),
                ("phone", read.duties.phone),
                ("wallpaper", read.duties.wallpaper),
            ] {
                assert_eq!(duty, Duty::On, "{name} for {value}");
            }
            assert!(read.do_not_disturb, "{value}");
            assert!(read.energy.caffeine, "{value}");
        }
        // The unclear set is the defaults in everything but the three duties
        // that fail closed, and it is not the quiet set of a protected start.
        assert_eq!(
            Duties {
                clipboard: Duty::On,
                lock: Duty::On,
                polkit: Duty::On,
                ..Duties::unclear()
            },
            Duties::default()
        );
        assert_ne!(Duties::unclear(), Duties::default());
        assert_ne!(Duties::unclear(), Duties::all_off());
    }

    #[test]
    fn a_duties_array_is_never_read_positionally() {
        // serde would read `["off","off","off"]` into the first three fields
        // (bubbles, caffeine, clipboard) and leave idle on, and eleven "on"
        // into every duty, the lock and the polkit agent included: an
        // accidental format. An array is not an object, so it reads as the
        // unclear set, whatever its length or content.
        for value in [
            r#"["off","off","off"]"#,
            r#"["on","on","on","on","on","on","on","on","on","on","on"]"#,
            r#"[{"idle":"off"}]"#,
        ] {
            let json = format!(r#"{{"duties":{value}}}"#);
            let read = Settings::from_bytes(json.as_bytes()).expect("read");
            assert_eq!(read.duties, Duties::unclear(), "{value}");
            assert_eq!(read.duties.bubbles, Duty::On, "{value}");
            assert_eq!(read.duties.idle, Duty::On, "{value}");
            assert_eq!(read.duties.clipboard, Duty::Off, "{value}");
            assert_eq!(read.duties.lock, Duty::Off, "{value}");
            assert_eq!(read.duties.polkit, Duty::Off, "{value}");
        }
    }

    #[test]
    fn a_duties_object_keeps_its_per_field_leniency_and_round_trips() {
        // The object path is unchanged: a known key is read, an unknown value
        // degrades to that duty's own default, an absent key is the default,
        // and the whole table survives a round trip.
        let read = Settings::from_bytes(
            br#"{"duties":{"idle":"off","clipboard":"maybe","nightlight":7,"lock":"maybe","stray":1}}"#,
        )
        .expect("read");
        assert_eq!(read.duties.idle, Duty::Off);
        assert_eq!(read.duties.clipboard, Duty::Off, "fails closed");
        assert_eq!(read.duties.nightlight, Duty::On, "keeps its default");
        assert_eq!(read.duties.lock, Duty::Off, "fails closed");
        assert_eq!(read.duties.osd, Duty::On, "absent: the default");
        assert_eq!(read.duties.polkit, Duty::On, "absent: the default");
        let empty = Settings::from_bytes(br#"{"duties":{}}"#).expect("read");
        assert_eq!(
            empty.duties,
            Duties::default(),
            "an empty object is the defaults"
        );
        assert_eq!(empty.duties.lock, Duty::On);
        assert_eq!(empty.duties.polkit, Duty::On);
        for duties in [
            Duties::default(),
            Duties::all_off(),
            Duties::unclear(),
            Duties {
                idle: Duty::Off,
                lock: Duty::Off,
                polkit: Duty::On,
                ..Duties::default()
            },
            Duties {
                lock: Duty::On,
                polkit: Duty::Off,
                ..Duties::default()
            },
        ] {
            let settings = Settings {
                duties,
                ..Settings::default()
            };
            let bytes = settings.to_bytes().expect("serialize");
            assert_eq!(Settings::from_bytes(&bytes), Some(settings), "{duties:?}");
        }
    }

    #[test]
    fn the_lock_duty_is_on_by_default_and_in_a_file_without_the_key() {
        // Default: on, for a fresh file, for a file with no `duties` key and
        // for a `duties` object from before the lock (CEL-2-M-LOCK-POLKIT).
        assert_eq!(Settings::default().duties.lock, Duty::On);
        let bare = Settings::from_bytes(br#"{"schema_version":1,"do_not_disturb":true}"#)
            .expect("an old file is read");
        assert_eq!(bare.duties.lock, Duty::On);
        let older = Settings::from_bytes(br#"{"schema_version":1,"duties":{"idle":"on"}}"#)
            .expect("an old file is read");
        assert_eq!(older.duties.lock, Duty::On);
        assert_eq!(older.duties.idle, Duty::On);
        // The duty is written, like the other switches.
        let (_, duties) = written_keys(&Settings::default());
        assert!(duties.contains(&"lock".to_owned()));
        // The person's choice is kept, either way.
        let on = Settings::from_bytes(br#"{"duties":{"lock":"on"}}"#).expect("read");
        assert_eq!(on.duties.lock, Duty::On);
        let off = Settings::from_bytes(br#"{"duties":{"lock":"off"}}"#).expect("read");
        assert_eq!(off.duties.lock, Duty::Off);
        assert_eq!(
            off.duties.polkit,
            Duty::On,
            "one duty's off is not the other's"
        );
    }

    #[test]
    fn the_lock_duty_survives_a_round_trip_and_a_hostile_value_degrades_to_off() {
        for lock in [Duty::On, Duty::Off] {
            let mut settings = Settings::default();
            settings.duties.lock = lock;
            let bytes = settings.to_bytes().expect("serialize");
            let written = if lock == Duty::On { "on" } else { "off" };
            assert!(
                String::from_utf8_lossy(&bytes).contains(&format!(r#""lock": "{written}""#)),
                "{lock:?}"
            );
            assert_eq!(Settings::from_bytes(&bytes), Some(settings), "{lock:?}");
        }

        // The lock fails closed, like the clipboard: a value that is not
        // "on" or "off" reads as off, so a person who tried to write "off"
        // and got it wrong never has the session covered by a lock they did
        // not ask for. Garbage costs the rest of the file nothing.
        for value in GARBAGE_DUTY_VALUES {
            let json =
                format!(r#"{{"do_not_disturb":true,"duties":{{"lock":{value},"idle":"on"}}}}"#);
            let read = Settings::from_bytes(json.as_bytes()).expect("the file is still read");
            assert_eq!(read.duties.lock, Duty::Off, "{value}");
            assert_eq!(read.duties.polkit, Duty::On, "{value}");
            assert_eq!(read.duties.idle, Duty::On, "{value}");
            assert!(read.do_not_disturb, "{value}");
        }

        // A change is durable before it is published, like every duty.
        let store = Store::new(Settings::default());
        let pending = store.stage(|s| s.duties.lock = Duty::Off);
        assert_eq!(store.current().duties.lock, Duty::On);
        assert_eq!(pending.wanted().duties.lock, Duty::Off);
    }

    /// Values that are neither `"on"` nor `"off"`: what the duties that fail
    /// closed (the clipboard, the lock, the polkit agent) must read as off.
    const GARBAGE_DUTY_VALUES: [&str; 12] = [
        r#""yes""#,
        r#""ON""#,
        r#""OFF""#,
        r#""On ""#,
        r#""disabled""#,
        "1",
        "true",
        "false",
        "null",
        "[]",
        "[1]",
        r#"{"on":true}"#,
    ];

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
        // The capsule shows the date and the weather: neither piece is in a
        // default zone.
        for zone in [&layout.start, &layout.centre, &layout.end] {
            assert!(!zone.iter().any(|id| id == "clock" || id == "weather"));
        }
        assert_eq!(
            layout.end,
            ids(&[
                "tray",
                "network",
                "bluetooth",
                "volume",
                "brightness",
                "clipboard",
                "session"
            ])
        );
        assert_eq!(
            layout.tray_hidden,
            ids(&["nm-applet", "blueman", "blueman-tray"])
        );
        assert_eq!(layout, BarLayout::default());
        assert!(settings.bar.is_unset());
    }

    #[test]
    fn hiding_appends_an_id_once_and_showing_removes_it() {
        let layout = BarLayout {
            tray_hidden: ids(&["a", "b"]),
            ..BarLayout::default()
        };
        assert_eq!(layout.tray_hidden_with("c", true), ids(&["a", "b", "c"]));
        assert_eq!(layout.tray_hidden_with("a", true), ids(&["a", "b"]));
        assert_eq!(layout.tray_hidden_with("a", false), ids(&["b"]));
        assert_eq!(layout.tray_hidden_with("z", false), ids(&["a", "b"]));
        assert_eq!(layout.tray_hidden_with("", true), ids(&["a", "b"]));
        assert!(layout.can_hide("c"));
        assert!(!layout.can_hide("a"), "already hidden");
        assert!(!layout.can_hide(""));
        assert!(!layout.can_hide(&"h".repeat(MAX_HIDDEN_ID_BYTES + 1)));
        assert!(!layout.can_hide("x\ty"));
        let full = BarLayout {
            tray_hidden: (0..MAX_HIDDEN_IDS).map(|i| format!("id{i}")).collect(),
            ..BarLayout::default()
        };
        assert!(!full.can_hide("one-more"), "no room");
        assert_eq!(layout.tray_hidden_with("x\ty", true), ids(&["a", "b"]));
    }

    #[test]
    fn a_bar_layout_choice_survives_a_round_trip() {
        let mut settings = Settings::default();
        settings.bar.centre = Some(ids(&["clock"]));
        settings.bar.tray_hidden = Some(ids(&["x y"]));
        let bytes = settings.to_bytes().expect("settings serialize");
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
    }

    #[test]
    fn an_older_files_bar_duty_is_read_past_and_not_written_back() {
        // Before the cutover `duties.bar` switched the bar on; since it
        // (CEL-2-M) the bar is always on, so a file that still carries the
        // key reads as if it were not there, whatever its value, the layout
        // beside it is kept, and the next write drops the key.
        for value in [r#""off""#, r#""on""#, r#""maybe""#, "5", "null"] {
            let json = format!(r#"{{"duties":{{"bar":{value}}},"bar":{{"end":["tray"]}}}}"#);
            let read = Settings::from_bytes(json.as_bytes()).expect("ours");
            assert_eq!(read.duties, Duties::default(), "{value}");
            assert_eq!(read.bar.end, Some(ids(&["tray"])));
            let (top, duties) = written_keys(&read);
            assert!(top.contains(&"bar".to_owned()));
            assert!(!duties.contains(&"bar".to_owned()), "{value}");
            let again = Settings::from_bytes(&read.to_bytes().expect("serialize")).expect("ours");
            assert_eq!(again, read);
        }
    }

    #[test]
    fn network_and_bluetooth_reach_files_that_never_chose_an_end_zone() {
        // An old file with a bar section but no end zone follows the default,
        // so it gains the two connectivity pieces after the tray.
        let old = Settings::from_bytes(br#"{"bar":{"tray_hidden":[]}}"#).expect("ours");
        assert_eq!(
            old.bar_layout().end,
            ids(&[
                "tray",
                "network",
                "bluetooth",
                "volume",
                "brightness",
                "clipboard",
                "session"
            ])
        );
        // A person who chose their end zone keeps exactly that choice.
        let chosen = Settings::from_bytes(br#"{"bar":{"end":["tray"]}}"#).expect("ours");
        assert_eq!(chosen.bar_layout().end, ids(&["tray"]));
        // A person who placed the piece elsewhere is not given a second copy.
        let moved = Settings::from_bytes(br#"{"bar":{"start":["network"]}}"#).expect("ours");
        let layout = moved.bar_layout();
        assert_eq!(layout.start, ids(&["network"]));
        assert_eq!(
            layout.end,
            ids(&[
                "tray",
                "bluetooth",
                "volume",
                "brightness",
                "clipboard",
                "session"
            ])
        );
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
        assert_eq!(read.duties, Duties::default());
        let written = String::from_utf8(read.to_bytes().expect("serialize")).expect("utf8");
        assert!(!written.contains("future") && !written.contains("extra"));
        let (_, duties) = written_keys(&read);
        assert!(!duties.contains(&"bar".to_owned()));
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
        assert_eq!(read.duties.idle, Duty::On, "the duties are kept");
        assert_eq!(read.bar.start, Some(ids(&["clock"])));
        assert_eq!(read.bar.end, None);
        assert_eq!(read.bar.centre, None);
        assert_eq!(read.bar_layout().end, BarLayout::default().end);
        // A bar table of the wrong shape is no choice at all, and the old
        // `duties.bar` key means nothing whatever it holds.
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

        let pending = store.stage(|s| s.bar.centre = Some(ids(&["clock"])));
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

    #[test]
    fn the_weather_location_is_unset_by_default_and_never_written_unset() {
        assert_eq!(Settings::default().weather, None);
        let bytes = Settings::default().to_bytes().expect("serialize");
        assert!(!String::from_utf8_lossy(&bytes).contains("weather"));
    }

    #[test]
    fn the_weather_location_survives_a_round_trip_as_plain_degrees() {
        let settings = Settings {
            weather: Location::new(53.35, -6.26, "Dublin"),
            ..Settings::default()
        };
        let bytes = settings.to_bytes().expect("serialize");
        let value: Value = serde_json::from_slice(&bytes).expect("JSON");
        assert_eq!(value["weather"]["latitude"], 53.35);
        assert_eq!(value["weather"]["longitude"], -6.26);
        assert_eq!(value["weather"]["label"], "Dublin");
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn an_invalid_weather_location_is_none_and_the_rest_of_the_file_holds() {
        let hostile = [
            r#""weather":{"latitude":90.5,"longitude":0,"label":"a"}"#.to_owned(),
            r#""weather":{"latitude":0,"longitude":-180.5,"label":"a"}"#.to_owned(),
            r#""weather":{"latitude":"53","longitude":0,"label":"a"}"#.to_owned(),
            r#""weather":{"latitude":53.0,"longitude":-6.0,"label":7}"#.to_owned(),
            r#""weather":{"latitude":53.0}"#.to_owned(),
            r#""weather":"Dublin""#.to_owned(),
            r#""weather":5"#.to_owned(),
            r#""weather":null"#.to_owned(),
        ];
        for fragment in hostile {
            let text = format!(r#"{{"do_not_disturb":true,{fragment}}}"#);
            let read = Settings::from_bytes(text.as_bytes()).expect("the file still reads");
            assert!(read.do_not_disturb, "{fragment}");
            assert_eq!(read.weather, None, "{fragment}");
        }
    }

    #[test]
    fn an_oversize_or_dirty_label_is_cleaned_never_a_reason_to_drop_the_place() {
        let long = "x".repeat(MAX_PLACE_CHARS + 1);
        let text = format!(r#"{{"weather":{{"latitude":1.0,"longitude":2.0,"label":"{long}"}}}}"#);
        let place = Settings::from_bytes(text.as_bytes())
            .expect("reads")
            .weather
            .expect("the place is kept");
        assert_eq!(place.label, "x".repeat(MAX_PLACE_CHARS));
        // Scalar values, not UTF-16 units: astral characters count once.
        let astral = "\u{1F600}".repeat(MAX_PLACE_CHARS + 5);
        let built = Location::new(1.0, 2.0, &astral).expect("a place");
        assert_eq!(built.label.chars().count(), MAX_PLACE_CHARS);
        // The file path and the constructor share the rule.
        let dirty = "  Du\u{7}bl\nin\u{0}  ";
        let json = serde_json::to_string(dirty).expect("JSON");
        let wire = format!(r#"{{"weather":{{"latitude":1.0,"longitude":2.0,"label":{json}}}}}"#);
        let read = Settings::from_bytes(wire.as_bytes()).expect("reads");
        assert_eq!(read.weather.expect("place").label, "Dublin");
        assert_eq!(
            Location::new(1.0, 2.0, dirty).expect("place").label,
            "Dublin"
        );
        // Trimming comes after the control strip and the cut leaves no tail space.
        let spaced = format!("{} tail", "a".repeat(MAX_PLACE_CHARS - 1));
        assert_eq!(
            Location::new(1.0, 2.0, &spaced).expect("place").label,
            "a".repeat(MAX_PLACE_CHARS - 1)
        );
    }

    #[test]
    fn debugging_a_location_never_prints_where_it_is() {
        let place = Location::new(53.349_805, -6.260_31, "Dublin").expect("place");
        let shown = format!("{place:?}");
        let settings = format!(
            "{:?}",
            Settings {
                weather: Some(place),
                ..Settings::default()
            }
        );
        for text in [shown, settings] {
            assert!(!text.contains("53.3") && !text.contains("6.26"), "{text}");
            assert!(!text.contains("Dublin"), "{text}");
            assert!(text.contains("label_chars"), "{text}");
        }
    }

    #[test]
    fn the_weather_boundaries_are_kept_and_a_label_at_the_cap_survives() {
        let label = "w".repeat(MAX_PLACE_CHARS);
        let text = format!(r#"{{"weather":{{"latitude":-90,"longitude":180,"label":"{label}"}}}}"#);
        let read = Settings::from_bytes(text.as_bytes()).expect("reads");
        let place = read.weather.expect("a place");
        assert_eq!(place.latitude(), -90.0);
        assert_eq!(place.longitude(), 180.0);
        assert_eq!(place.label.chars().count(), MAX_PLACE_CHARS);
        // A caller-built label is bounded, and non-finite coordinates are refused.
        let built = Location::new(0.0, 0.0, &"y".repeat(500)).expect("a place");
        assert_eq!(built.label.chars().count(), MAX_PLACE_CHARS);
        assert_eq!(Location::new(f64::NAN, 0.0, "x"), None);
        assert_eq!(Location::new(0.0, f64::INFINITY, "x"), None);
    }

    #[test]
    fn a_staged_weather_location_is_written_and_clearing_it_removes_the_key() {
        let mut store = Store::new(Settings::default());
        let pending = store.stage(|s| s.weather = Location::new(40.4, -3.7, "Madrid"));
        assert!(store.apply(pending, WriteOutcome::Durable));
        assert!(store.current().weather.is_some());
        let pending = store.stage(|s| s.weather = None);
        let bytes = pending.bytes().expect("bytes");
        assert!(!String::from_utf8_lossy(&bytes).contains("weather"));
    }

    #[test]
    fn the_osd_defaults_are_on_with_every_kind_on() {
        let settings = Settings::default();
        assert_eq!(settings.duties.osd, Duty::On);
        assert_eq!(settings.osd.timeout_ms, 1500);
        assert_eq!(settings.osd.position, OsdPosition::Bottom);
        assert!(settings.osd.kinds.volume);
        assert!(settings.osd.kinds.microphone);
        assert!(settings.osd.kinds.brightness);
        // A file that predates the unit reads as the defaults.
        let read = Settings::from_bytes(br#"{"duties":{"idle":"on"}}"#).expect("read");
        assert_eq!(read.duties.osd, Duty::On);
        assert_eq!(read.osd, OsdSettings::default());
    }

    #[test]
    fn osd_settings_survive_a_round_trip() {
        let settings = Settings {
            duties: Duties {
                osd: Duty::On,
                ..Duties::default()
            },
            osd: OsdSettings {
                timeout_ms: 2500,
                position: OsdPosition::BottomRight,
                kinds: OsdKinds {
                    volume: true,
                    microphone: false,
                    brightness: false,
                },
            },
            ..Settings::default()
        };
        let bytes = settings.to_bytes().expect("settings serialize");
        let text = String::from_utf8(bytes.clone()).expect("utf8");
        assert!(text.contains(r#""position": "bottom-right""#));
        assert!(text.contains(r#""osd": "on""#));
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
    }

    #[test]
    fn the_osd_timeout_is_bounded() {
        for (raw, kept) in [
            ("0", 500),
            ("499", 500),
            ("500", 500),
            ("5000", 5000),
            ("5001", 5000),
            ("4294967295", 5000),
            // Not a non-negative integer that fits: the default.
            ("-5", 1500),
            ("99999999999", 1500),
            ("1.5", 1500),
            ("\"fast\"", 1500),
            ("null", 1500),
        ] {
            let bytes = format!(r#"{{"osd":{{"timeout_ms":{raw}}}}}"#);
            let read = Settings::from_bytes(bytes.as_bytes()).expect("read");
            assert_eq!(read.osd.timeout_ms, kept, "{raw}");
        }
        let pending = Store::new(Settings::default()).stage(|s| s.osd.timeout_ms = 1);
        assert_eq!(pending.wanted().osd.timeout_ms, 500);
    }

    #[test]
    fn a_hostile_osd_table_degrades_field_by_field() {
        let read = Settings::from_bytes(
            br#"{"duties":{"osd":"yes"},
                 "osd":{"position":"top","timeout_ms":900,
                        "kinds":{"volume":false,"microphone":"no","brightness":7,"extra":1}}}"#,
        )
        .expect("read");
        assert_eq!(read.duties.osd, Duty::On);
        assert_eq!(read.osd.position, OsdPosition::Bottom);
        assert_eq!(read.osd.timeout_ms, 900);
        assert!(!read.osd.kinds.volume);
        // A wrong-typed switch keeps its default (on), not off.
        assert!(read.osd.kinds.microphone);
        assert!(read.osd.kinds.brightness);

        for bytes in [
            &br#"{"osd":"x"}"#[..],
            br#"{"osd":7}"#,
            br#"{"osd":null}"#,
            br#"{"osd":{"kinds":5}}"#,
        ] {
            let read = Settings::from_bytes(bytes).expect("a bad table never fails the file");
            assert_eq!(read.osd, OsdSettings::default());
        }
        let mut huge = br#"{"osd":{"timeout_ms":900}}"#.to_vec();
        huge.resize(MAX_FILE_BYTES + 1, b' ');
        assert_eq!(Settings::from_bytes(&huge), None);
    }
}
