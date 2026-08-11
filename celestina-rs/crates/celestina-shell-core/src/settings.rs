//! What the person chose, and what it takes for that choice to be real.
//!
//! Everything else this shell shows is a reading: the audio device's level, the
//! compositor's workspaces, what an application said. Settings are the opposite
//! — they are the only state the shell itself owns — and that changes what
//! honesty means for them.
//!
//! A reading is published as soon as it is read. A setting is published only
//! once it is **durable**. A control centre that flipped a switch, showed it
//! on, and then failed to write the file would be reporting a choice the next
//! session will not honour; the person would have to discover that by
//! restarting. So the rule here is one way round: write, confirm, then publish.
//! [`Settings::apply`] cannot be called with anything but a confirmed write,
//! because it takes the value back from whoever performed it.
//!
//! Nothing in this module touches a filesystem. It owns the schema, the bounds
//! every value is clamped to, the text that goes to disk and the text that
//! comes back — so the rules are testable without a temporary directory, and
//! the durable write itself stays with the runtime that can actually fsync.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::notifications::MAX_ICON_CHARS;
use crate::session::MAX_LEVEL;

/// The schema version written into the file. A file from the future is not
/// read: an older shell guessing at a newer schema would be inventing the
/// person's preferences.
pub const SCHEMA_VERSION: u32 = 3;
/// A place name is a label, not an address. It is shown, never resolved.
pub const MAX_PLACE_CHARS: usize = 64;
/// The whole file, as a guard against a corrupted or hostile settings path.
pub const MAX_FILE_BYTES: usize = 64 * 1024;
/// A tray host accepts at most 64 live items. Preferences may outlive those
/// items, so a new explicit choice replaces one existing entry at this bound.
pub const MAX_TRAY_ITEM_MODES: usize = 64;
/// SHA-256 written as lowercase hexadecimal by the Qt tray adapter.
pub const TRAY_PREFERENCE_KEY_CHARS: usize = 64;
/// The selected wallpaper library is carried through the same bounded text
/// channel as every other provider field. It is an absolute local path, not a
/// URI; the Qt boundary performs URL-to-path conversion before it reaches the
/// settings owner.
pub const MAX_WALLPAPER_DIRECTORY_UNITS: usize = crate::snapshot::MAX_TEXT_UNITS;

/// Whether a wallpaper-library directory is safe to persist as a preference.
///
/// This is deliberately syntax-only. The non-Qt wallpaper worker owns the
/// filesystem check and may report a temporarily unavailable mounted folder;
/// settings own only whether the value is bounded and unambiguous on disk.
#[must_use]
pub fn wallpaper_directory_is_valid(directory: &str) -> bool {
    !directory.is_empty()
        && directory.encode_utf16().count() <= MAX_WALLPAPER_DIRECTORY_UNITS
        && !directory.contains('\0')
        && Path::new(directory).is_absolute()
}

/// Where one tray item belongs when it is present.
///
/// Absence from [`Settings::tray_items`] is the ordinary visible state. The
/// two stored values are deliberately exclusive: hiding an item also unpins
/// it, and pinning it also makes it visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrayItemMode {
    Pinned,
    Hidden,
}

impl TrayItemMode {
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::Hidden => "hidden",
        }
    }

    #[must_use]
    pub fn from_token(raw: &str) -> Option<Self> {
        match raw {
            "pinned" => Some(Self::Pinned),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

/// Whether a preference key is one the tray adapter could have produced.
#[must_use]
pub fn tray_preference_key_is_valid(key: &str) -> bool {
    key.len() == TRAY_PREFERENCE_KEY_CHARS
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Where the weather reading is for. Coordinates rather than a place lookup:
/// this shell sends one pair of numbers to one service and nothing else, and a
/// name that had to be resolved would be a second thing to send.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// Degrees, clamped to the range the earth actually has.
    pub latitude: f64,
    pub longitude: f64,
    /// What to call it on screen. Never sent anywhere.
    pub label: String,
}

impl Location {
    /// Reads one location, or nothing when it cannot be a place on earth.
    ///
    /// A NaN or out-of-range coordinate is refused rather than clamped: 0°,0°
    /// is a real place in the Atlantic, and silently sending somebody there
    /// would be worse than having no weather.
    #[must_use]
    pub fn new(latitude: f64, longitude: f64, label: &str) -> Option<Self> {
        if !latitude.is_finite() || !longitude.is_finite() {
            return None;
        }
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return None;
        }

        Some(Self {
            latitude,
            longitude,
            label: crate::bounded(label.trim(), MAX_PLACE_CHARS),
        })
    }
}

/// Everything the person chose. Every field has a default that is the quietest
/// possible behaviour: nothing here turns itself on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Written so a later shell can refuse a file it does not understand.
    pub schema: u32,
    /// Whether notifications are being held back. Persisted because a person
    /// who silenced their session did not mean "until the next reboot".
    pub quiet: bool,
    /// Whether the session is being kept awake.
    pub caffeine: bool,
    /// Whether night light is on.
    pub night_light: bool,
    /// The step one wheel notch or one key press takes, in whole percent.
    pub level_step: u8,
    /// Where the weather is for, or nothing — in which case no weather is
    /// shown at all, rather than somebody else's city.
    pub weather: Option<Location>,
    /// The icon name shown beside the weather, when the provider offers one.
    pub weather_icon: String,
    /// Which monitor a workspace belongs to, said by the person rather than
    /// observed. Empty for every session that never needed to correct what the
    /// shell learned by watching.
    ///
    /// This is the repair route for a memory that recorded a layout the person
    /// has since changed: the observed memory lives in the shell's state
    /// directory and is not meant to be hand-edited, while this is a preference
    /// and outranks it. See
    /// [`crate::workspace_groups`], which owns what a home means and what may
    /// teach one.
    pub workspace_homes: BTreeMap<String, String>,
    /// Per-item tray placement, keyed by the stable fingerprint exported by
    /// the StatusNotifierItem adapter. Live D-Bus service/path identities are
    /// intentionally not persisted because unique bus names change whenever
    /// an application restarts.
    pub tray_items: BTreeMap<String, TrayItemMode>,
    /// The person's wallpaper library. The directory survives a shell restart;
    /// its inventory does not, because files may be added, removed or replaced
    /// while the shell is away and must be observed again.
    pub wallpaper_directory: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            quiet: false,
            caffeine: false,
            night_light: false,
            level_step: 5,
            weather: None,
            weather_icon: String::new(),
            workspace_homes: BTreeMap::new(),
            tray_items: BTreeMap::new(),
            wallpaper_directory: None,
        }
    }
}

impl Settings {
    /// Brings every value inside its bounds. Called on the way in *and* on the
    /// way out, so neither a hand-edited file nor a broken caller can put an
    /// unusable number in front of the person.
    fn clamped(mut self) -> Self {
        self.schema = SCHEMA_VERSION;
        // A step of zero moves nothing and a step past the range is a typo;
        // both would make a wheel notch useless rather than dangerous, so they
        // become the default rather than a refusal of the whole file.
        if self.level_step == 0 || self.level_step > MAX_LEVEL {
            self.level_step = Self::default().level_step;
        }
        self.weather = self
            .weather
            .and_then(|place| Location::new(place.latitude, place.longitude, &place.label));
        self.weather_icon = crate::bounded(self.weather_icon.trim(), MAX_ICON_CHARS);
        // Bounded by the module that owns what a home is, so a hand-edited
        // declaration obeys the same limits an observed one does.
        let mut homes = crate::workspace_groups::Homes::new();
        homes.set_declarations(
            self.workspace_homes
                .iter()
                .map(|(label, output)| (label.as_str(), output.as_str())),
        );
        self.workspace_homes = homes.declarations().clone();
        self.tray_items = self
            .tray_items
            .into_iter()
            .filter(|(key, _)| tray_preference_key_is_valid(key))
            .take(MAX_TRAY_ITEM_MODES)
            .collect();
        self.wallpaper_directory = self
            .wallpaper_directory
            .filter(|directory| wallpaper_directory_is_valid(directory));
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
    /// file it could not read.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_FILE_BYTES {
            return None;
        }
        let parsed: Self = serde_json::from_slice(bytes).ok()?;
        if parsed.schema > SCHEMA_VERSION {
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, Default, PartialEq)]
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
    /// it before it happens — a confirmation prompt, a log line. It is
    /// deliberately not a way to publish it.
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
        assert!(!settings.quiet);
        assert!(!settings.caffeine);
        assert!(!settings.night_light);
        assert_eq!(settings.weather, None);
        assert!(settings.tray_items.is_empty());
        assert_eq!(settings.wallpaper_directory, None);
    }

    fn tray_key(index: usize) -> String {
        format!("{index:0width$x}", width = TRAY_PREFERENCE_KEY_CHARS)
    }

    #[test]
    fn tray_item_modes_survive_a_round_trip_and_remain_exclusive() {
        let pinned = tray_key(1);
        let hidden = tray_key(2);
        let settings = Settings {
            tray_items: BTreeMap::from([
                (pinned.clone(), TrayItemMode::Pinned),
                (hidden.clone(), TrayItemMode::Hidden),
            ]),
            ..Settings::default()
        };

        let bytes = settings.to_bytes().expect("settings serialize");
        let read = Settings::from_bytes(&bytes).expect("settings read back");

        assert_eq!(read.tray_items.get(&pinned), Some(&TrayItemMode::Pinned));
        assert_eq!(read.tray_items.get(&hidden), Some(&TrayItemMode::Hidden));
        assert!(String::from_utf8(bytes)
            .expect("settings are UTF-8")
            .contains("\"pinned\""));
    }

    #[test]
    fn tray_item_modes_drop_foreign_keys_and_are_bounded() {
        let mut tray_items = BTreeMap::new();
        tray_items.insert("not-a-fingerprint".to_owned(), TrayItemMode::Pinned);
        for index in 0..(MAX_TRAY_ITEM_MODES + 4) {
            tray_items.insert(tray_key(index), TrayItemMode::Hidden);
        }
        let settings = Settings {
            tray_items,
            ..Settings::default()
        };

        let bytes = settings.to_bytes().expect("settings serialize");
        let read = Settings::from_bytes(&bytes).expect("settings read back");

        assert_eq!(read.tray_items.len(), MAX_TRAY_ITEM_MODES);
        assert!(!read.tray_items.contains_key("not-a-fingerprint"));
        assert!(read
            .tray_items
            .keys()
            .all(|key| tray_preference_key_is_valid(key)));
    }

    #[test]
    fn tray_mode_tokens_are_the_persisted_protocol_vocabulary() {
        assert_eq!(TrayItemMode::Pinned.token(), "pinned");
        assert_eq!(TrayItemMode::Hidden.token(), "hidden");
        assert_eq!(
            TrayItemMode::from_token("pinned"),
            Some(TrayItemMode::Pinned)
        );
        assert_eq!(
            TrayItemMode::from_token("hidden"),
            Some(TrayItemMode::Hidden)
        );
        assert_eq!(TrayItemMode::from_token("visible"), None);
        assert!(!tray_preference_key_is_valid(
            &"A".repeat(TRAY_PREFERENCE_KEY_CHARS)
        ));
    }

    #[test]
    fn a_declared_workspace_home_survives_a_round_trip_and_is_bounded() {
        let long = "x".repeat(crate::workspace_groups::MAX_NAME_CHARS * 2);
        let settings = Settings {
            workspace_homes: BTreeMap::from([
                ("7".to_owned(), "DP-1".to_owned()),
                (long.clone(), long.clone()),
                // Neither half of this names anything, so it declares nothing.
                (String::new(), "DP-2".to_owned()),
            ]),
            ..Settings::default()
        };

        let bytes = settings.to_bytes().expect("the settings serialize");
        let read = Settings::from_bytes(&bytes).expect("a readable file");

        assert_eq!(read.workspace_homes.len(), 2);
        assert_eq!(
            read.workspace_homes.get("7").map(String::as_str),
            Some("DP-1")
        );
        let truncated = "x".repeat(crate::workspace_groups::MAX_NAME_CHARS);
        assert_eq!(
            read.workspace_homes.get(&truncated).map(String::as_str),
            Some(truncated.as_str())
        );
    }

    #[test]
    fn older_settings_files_migrate_to_v3() {
        // Every previous session's file. It must keep working rather than
        // becoming unreadable because tray placement and then the wallpaper
        // library were added. The next durable write protects both fields from
        // an older binary.
        let v1 = br#"{
            "schema": 1,
            "quiet": true,
            "caffeine": false,
            "night_light": true,
            "level_step": 5,
            "workspace_homes": {"7": "DP-1"}
        }"#;
        let v2 = br#"{
            "schema": 2,
            "quiet": false,
            "tray_items": {}
        }"#;

        let read = Settings::from_bytes(v1).expect("a v1 file is still ours");
        let read_v2 = Settings::from_bytes(v2).expect("a v2 file is still ours");

        assert_eq!(read.schema, SCHEMA_VERSION);
        assert_eq!(read_v2.schema, SCHEMA_VERSION);
        assert!(read.quiet);
        assert!(read.night_light);
        assert_eq!(
            read.workspace_homes.get("7").map(String::as_str),
            Some("DP-1")
        );
        assert!(read.tray_items.is_empty());
        assert_eq!(read.wallpaper_directory, None);
        assert_eq!(read_v2.wallpaper_directory, None);

        let migrated = read.to_bytes().expect("the migrated settings serialize");
        assert!(String::from_utf8(migrated.clone())
            .expect("settings are UTF-8")
            .contains("\"schema\": 3"));
        assert_eq!(Settings::from_bytes(&migrated), Some(read));
    }

    #[test]
    fn wallpaper_directory_is_absolute_bounded_and_durable_before_adoption() {
        let chosen = "/home/person/Pictures/Wallpapers".to_owned();
        assert!(wallpaper_directory_is_valid(&chosen));
        assert!(!wallpaper_directory_is_valid("Pictures/Wallpapers"));
        assert!(!wallpaper_directory_is_valid("/tmp/a\0b"));
        assert!(!wallpaper_directory_is_valid(&format!(
            "/{}",
            "x".repeat(MAX_WALLPAPER_DIRECTORY_UNITS)
        )));

        let mut store = Store::new(Settings::default());
        let pending = store.stage(|settings| {
            settings.wallpaper_directory = Some(chosen.clone());
        });
        assert_eq!(store.current().wallpaper_directory, None);
        assert!(!store.apply(pending.clone(), WriteOutcome::Failed));
        assert_eq!(store.current().wallpaper_directory, None);
        assert!(store.apply(pending, WriteOutcome::Durable));
        assert_eq!(
            store.current().wallpaper_directory.as_deref(),
            Some(chosen.as_str())
        );

        let bytes = store.current().to_bytes().expect("settings serialize");
        assert_eq!(
            Settings::from_bytes(&bytes).and_then(|settings| settings.wallpaper_directory),
            Some(chosen)
        );
    }

    #[test]
    fn the_largest_legal_settings_state_fits_and_reads_back() {
        let workspace_homes = (0..crate::workspace_groups::MAX_HOMES)
            .map(|index| {
                (
                    format!(
                        "{index:0width$x}",
                        width = crate::workspace_groups::MAX_NAME_CHARS
                    ),
                    format!(
                        "output-{index:0width$}",
                        width = crate::workspace_groups::MAX_NAME_CHARS - 7
                    ),
                )
            })
            .collect();
        let tray_items = (0..MAX_TRAY_ITEM_MODES)
            .map(|index| (tray_key(index), TrayItemMode::Hidden))
            .collect();
        let settings = Settings {
            weather: Location::new(90.0, 180.0, &"w".repeat(MAX_PLACE_CHARS)),
            weather_icon: "i".repeat(MAX_ICON_CHARS),
            workspace_homes,
            tray_items,
            wallpaper_directory: Some(format!(
                "/{}",
                "w".repeat(MAX_WALLPAPER_DIRECTORY_UNITS - 1)
            )),
            ..Settings::default()
        };

        let bytes = settings.to_bytes().expect("maximum settings serialize");

        assert!(bytes.len() > 8 * 1024);
        assert!(bytes.len() <= MAX_FILE_BYTES);
        assert_eq!(Settings::from_bytes(&bytes), Some(settings));
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let settings = Settings {
            quiet: true,
            level_step: 10,
            weather: Location::new(53.35, -6.26, "Dublin"),
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
        let ahead = format!(r#"{{"schema":{}}}"#, SCHEMA_VERSION + 1);
        assert_eq!(Settings::from_bytes(ahead.as_bytes()), None);
        // A file this shell could write, missing everything optional, is read.
        assert_eq!(Settings::from_bytes(b"{}"), Some(Settings::default()));
    }

    #[test]
    fn an_unusable_number_becomes_the_default_rather_than_a_refusal() {
        let zero = Settings::from_bytes(br#"{"level_step":0}"#).expect("read");
        assert_eq!(zero.level_step, Settings::default().level_step);
        let huge = Settings::from_bytes(br#"{"level_step":200}"#).expect("read");
        assert_eq!(huge.level_step, Settings::default().level_step);
    }

    #[test]
    fn a_coordinate_that_is_not_a_place_on_earth_is_refused() {
        assert_eq!(Location::new(91.0, 0.0, "nowhere"), None);
        assert_eq!(Location::new(0.0, 181.0, "nowhere"), None);
        assert_eq!(Location::new(f64::NAN, 0.0, "nowhere"), None);
        // And a file carrying one loses the weather rather than gaining a
        // location in the Atlantic.
        let broken =
            Settings::from_bytes(br#"{"weather":{"latitude":95.0,"longitude":0.0,"label":"x"}}"#)
                .expect("read");
        assert_eq!(broken.weather, None);
    }

    #[test]
    fn a_place_name_is_bounded_and_never_sent() {
        let place = Location::new(0.0, 0.0, &"x".repeat(500)).expect("a place");
        assert_eq!(place.label.chars().count(), MAX_PLACE_CHARS);
    }

    #[test]
    fn a_change_is_not_in_force_until_its_write_is() {
        let mut store = Store::new(Settings::default());
        let pending = store.stage(|settings| settings.quiet = true);

        // Staging alone changes nothing anybody can see.
        assert!(!store.current().quiet);
        assert!(pending.wanted().quiet);

        assert!(!store.apply(pending.clone(), WriteOutcome::Failed));
        assert!(
            !store.current().quiet,
            "a failed write must leave the previous value in force"
        );

        assert!(store.apply(pending, WriteOutcome::Durable));
        assert!(store.current().quiet);
    }

    #[test]
    fn applying_the_same_value_is_not_a_change() {
        let mut store = Store::new(Settings::default());
        let pending = store.stage(|settings| settings.quiet = false);
        assert!(!store.apply(pending, WriteOutcome::Durable));
    }

    #[test]
    fn what_is_written_is_what_would_be_read_back() {
        let store = Store::new(Settings::default());
        let pending = store.stage(|settings| {
            settings.caffeine = true;
            settings.weather = Location::new(53.35, -6.26, "Dublin");
        });

        let bytes = pending.bytes().expect("bytes to write");
        assert_eq!(
            Settings::from_bytes(&bytes).as_ref(),
            Some(pending.wanted())
        );
    }

    #[test]
    fn a_staged_change_is_clamped_before_it_is_ever_written() {
        let store = Store::new(Settings::default());
        let pending = store.stage(|settings| settings.level_step = 0);
        assert_eq!(pending.wanted().level_step, Settings::default().level_step);
    }
}
