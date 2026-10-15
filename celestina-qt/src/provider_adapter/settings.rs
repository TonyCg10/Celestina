//! Where the person's choices live between sessions.
//!
//! [`celestina_shell_core::settings`] owns the schema, the bounds and the rule
//! that a choice is only in force once it is durable. This module is the half
//! that can actually make it durable: it finds the file, reads it at startup,
//! writes through the suite's existing atomic replacement — temporary sibling,
//! fsync, rename, fsync of the directory — and only then publishes.
//!
//! It is also the one writer. Night light, caffeine and do-not-disturb are live
//! states owned by the providers that hold them, but the *choice* to have them
//! on is one fact and is recorded here, by whoever just changed it, so the file
//! can never disagree with itself.
//!
//! A file this shell cannot read is left exactly as it is. Overwriting it would
//! destroy a hand-edit or a newer schema in the name of tidiness, and the person
//! would never learn what happened.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use celestina_core::atomic_file;
use celestina_core::xdg;
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::settings::{
    tray_preference_key_is_valid, Settings, Store, TrayItemMode, WriteOutcome, MAX_TRAY_ITEM_MODES,
};
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use super::tools::lock_runtime;

pub const NAME: &str = "settings";

/// The file, and the settings currently in force. One process, one file.
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
/// Set once the provider is registered, so a change made by another module can
/// republish without being handed the runtime.
static PUBLISHER: OnceLock<(Arc<Mutex<ProviderRuntime>>, ProviderId)> = OnceLock::new();

fn path() -> Option<PathBuf> {
    Some(xdg::config_home()?.join("celestina").join("settings.json"))
}

fn store() -> &'static Mutex<Store> {
    STORE.get_or_init(|| {
        let read = path().and_then(|path| std::fs::read(path).ok());
        // No file yet is not a problem: the defaults are what a session starts
        // with, and the first change writes them. A file that exists but is not
        // understood is different — the defaults run this session and the file
        // is left exactly as it is; see this module's own note.
        let settings = match read.as_deref().map(Settings::from_bytes) {
            Some(Some(settings)) => settings,
            Some(None) => {
                eprintln!(
                    "celestina-provider-adapter: settings: the settings file could not be \
                     read and was left untouched; this session runs on defaults"
                );
                Settings::default()
            }
            None => Settings::default(),
        };
        Mutex::new(Store::new(settings))
    })
}

fn lock_store() -> std::sync::MutexGuard<'static, Store> {
    match store().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn payload_of(settings: &Settings) -> Payload {
    let mut payload = Payload::new();
    payload.insert("quiet".to_owned(), Value::from(settings.quiet));
    payload.insert("caffeine".to_owned(), Value::from(settings.caffeine));
    payload.insert("nightLight".to_owned(), Value::from(settings.night_light));
    payload.insert("levelStep".to_owned(), Value::from(settings.level_step));
    // Absent rather than zeroed: no location means no weather at all, not a
    // reading for a point in the Atlantic.
    if let Some(place) = &settings.weather {
        payload.insert("weatherLabel".to_owned(), Value::from(place.label.clone()));
        payload.insert("weatherLatitude".to_owned(), Value::from(place.latitude));
        payload.insert("weatherLongitude".to_owned(), Value::from(place.longitude));
    }
    payload.insert(
        "trayItems".to_owned(),
        Value::Array(
            settings
                .tray_items
                .iter()
                .map(|(key, mode)| {
                    Value::Object(
                        [
                            ("key".to_owned(), Value::from(key.clone())),
                            ("mode".to_owned(), Value::from(mode.token())),
                        ]
                        .into_iter()
                        .collect(),
                    )
                })
                .collect(),
        ),
    );
    if let Some(directory) = &settings.wallpaper_directory {
        payload.insert(
            "wallpaperDirectory".to_owned(),
            Value::from(directory.clone()),
        );
    }
    payload
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestedTrayItemMode {
    Visible,
    Stored(TrayItemMode),
}

fn tray_item_mode_request(options: &Payload) -> Result<(String, RequestedTrayItemMode), String> {
    let key = options
        .get("key")
        .and_then(Value::as_str)
        .filter(|key| tray_preference_key_is_valid(key))
        .ok_or_else(|| format!("'{NAME}' needs a valid tray preference 'key'"))?;
    let mode = options
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("'{NAME}' needs a tray item 'mode'"))?;
    let requested = if mode == "visible" {
        RequestedTrayItemMode::Visible
    } else {
        TrayItemMode::from_token(mode)
            .map(RequestedTrayItemMode::Stored)
            .ok_or_else(|| format!("'{NAME}' cannot use the tray item mode '{mode}'"))?
    };
    Ok((key.to_owned(), requested))
}

fn apply_tray_item_mode(settings: &mut Settings, key: String, requested: RequestedTrayItemMode) {
    match requested {
        RequestedTrayItemMode::Visible => {
            settings.tray_items.remove(&key);
        }
        RequestedTrayItemMode::Stored(mode) => {
            // Preferences can outlive the applications that created them. Once
            // the bounded map is full, the current choice must still win: evict
            // the lexicographically first existing fingerprint so replacement
            // is deterministic without pretending the map records recency.
            if !settings.tray_items.contains_key(&key)
                && settings.tray_items.len() >= MAX_TRAY_ITEM_MODES
            {
                settings.tray_items.pop_first();
            }
            settings.tray_items.insert(key, mode);
        }
    }
}

fn publish() {
    let Some((runtime, id)) = PUBLISHER.get() else {
        return;
    };
    let payload = payload_of(lock_store().current());
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: settings: {error}");
    }
}

/// Records a choice durably, then publishes it.
///
/// Called by whoever just changed the live state that this choice describes, so
/// the file and the session cannot disagree. Returns whether anything changed;
/// a write that failed changes nothing and says so.
///
/// # Errors
///
/// Returns the sentence the requester should be shown when the choice could not
/// be made durable. The live state the caller already changed is deliberately
/// *not* rolled back: what the person asked for is happening, and what failed
/// is only its survival past this session.
pub fn remember(change: impl FnOnce(&mut Settings)) -> Result<bool, String> {
    let mut held = lock_store();
    let pending = held.stage(change);
    let bytes = pending
        .bytes()
        .map_err(|error| format!("the settings could not be written out: {error}"))?;
    let Some(path) = path() else {
        return Err("there is no XDG config directory to save settings in".to_owned());
    };

    let outcome = match atomic_file::replace(&path, &bytes) {
        Ok(()) => WriteOutcome::Durable,
        Err(error) => {
            let message = format!("the settings file could not be saved: {error}");
            drop(held);
            return Err(message);
        }
    };
    let changed = held.apply(pending, outcome);
    drop(held);

    if changed {
        publish();
    }
    Ok(changed)
}

/// What is in force right now.
#[must_use]
pub fn current() -> Settings {
    lock_store().current().clone()
}

/// Registers the provider and publishes what was read from disk.
///
/// Unlike every other provider this returns nothing: there is no thread to fail
/// to start. Settings change only when somebody changes them, so a poll would
/// be asking a file that cannot have moved on its own.
pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: settings: unusable provider name");
        return;
    };

    lock_runtime(runtime).register(id.clone());
    let _ = PUBLISHER.set((Arc::clone(runtime), id));
    publish();
}

/// The verbs the control centre uses to change a stored preference.
///
/// Only values with no live state of their own are set here. Night light,
/// caffeine and do-not-disturb are asked for through the providers that hold
/// them, which record the choice through [`remember`] once the session really
/// changed — a preference that persisted while the change itself failed would
/// be a promise nothing kept.
///
/// # Errors
///
/// Returns the requester's sentence for an unknown verb, an unusable option or
/// a write that did not survive.
pub fn action(verb: &str, options: &Payload) -> Result<(), String> {
    match verb {
        "level-step" => {
            let step = options
                .get("by")
                .and_then(Value::as_u64)
                .and_then(|step| u8::try_from(step).ok())
                .filter(|step| *step > 0)
                .ok_or_else(|| format!("'{NAME}' needs a 'by' step of at least 1"))?;
            remember(|settings| settings.level_step = step)?;
        }
        "weather-clear" => {
            remember(|settings| settings.weather = None)?;
        }
        "tray-item-mode" => {
            let (key, requested) = tray_item_mode_request(options)?;
            remember(move |settings| apply_tray_item_mode(settings, key, requested))?;
        }
        _ => return Err(format!("'{NAME}' does not serve the verb '{verb}'")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_sits_under_the_shell_s_own_config_directory() {
        let Some(path) = path() else {
            // A session with no config home has nowhere to save; the module
            // reports that rather than inventing a path.
            return;
        };
        assert!(path.ends_with("celestina/settings.json"));
    }

    #[test]
    fn an_absent_location_publishes_no_coordinates_at_all() {
        let payload = payload_of(&Settings::default());
        assert!(!payload.contains_key("weatherLatitude"));
        assert!(!payload.contains_key("weatherLongitude"));
        assert!(!payload.contains_key("weatherLabel"));
        assert_eq!(payload.get("trayItems"), Some(&Value::Array(Vec::new())));
        assert!(!payload.contains_key("wallpaperDirectory"));
    }

    #[test]
    fn what_is_published_is_what_the_person_chose() {
        let pinned = "1".repeat(64);
        let hidden = "2".repeat(64);
        let settings = Settings {
            quiet: true,
            level_step: 10,
            wallpaper_directory: Some("/home/person/Pictures/Wallpapers".to_owned()),
            tray_items: [
                (pinned.clone(), TrayItemMode::Pinned),
                (hidden.clone(), TrayItemMode::Hidden),
            ]
            .into_iter()
            .collect(),
            ..Settings::default()
        };
        let payload = payload_of(&settings);

        assert_eq!(payload.get("quiet"), Some(&Value::from(true)));
        assert_eq!(payload.get("levelStep"), Some(&Value::from(10u8)));
        assert_eq!(
            payload.get("wallpaperDirectory"),
            Some(&Value::from("/home/person/Pictures/Wallpapers"))
        );
        assert_eq!(
            payload.get("trayItems"),
            Some(&Value::Array(vec![
                Value::Object(
                    [
                        ("key".to_owned(), Value::from(pinned)),
                        ("mode".to_owned(), Value::from("pinned")),
                    ]
                    .into_iter()
                    .collect(),
                ),
                Value::Object(
                    [
                        ("key".to_owned(), Value::from(hidden)),
                        ("mode".to_owned(), Value::from("hidden")),
                    ]
                    .into_iter()
                    .collect(),
                ),
            ]))
        );
    }

    #[test]
    fn tray_item_mode_requests_accept_only_host_keys_and_known_modes() {
        let key = "a".repeat(64);
        let options = |mode: &str| {
            [
                ("key".to_owned(), Value::from(key.clone())),
                ("mode".to_owned(), Value::from(mode)),
            ]
            .into_iter()
            .collect()
        };

        assert_eq!(
            tray_item_mode_request(&options("visible")),
            Ok((key.clone(), RequestedTrayItemMode::Visible))
        );
        assert_eq!(
            tray_item_mode_request(&options("pinned")),
            Ok((
                key.clone(),
                RequestedTrayItemMode::Stored(TrayItemMode::Pinned)
            ))
        );
        assert_eq!(
            tray_item_mode_request(&options("hidden")),
            Ok((
                key.clone(),
                RequestedTrayItemMode::Stored(TrayItemMode::Hidden)
            ))
        );
        assert!(tray_item_mode_request(&options("floating")).is_err());

        let invalid = [
            ("key".to_owned(), Value::from(":1.42/item")),
            ("mode".to_owned(), Value::from("pinned")),
        ]
        .into_iter()
        .collect();
        assert!(tray_item_mode_request(&invalid).is_err());
    }

    #[test]
    fn a_new_tray_choice_replaces_one_deterministic_old_choice_at_capacity() {
        let mut settings = Settings {
            tray_items: (0..MAX_TRAY_ITEM_MODES)
                .map(|index| (format!("{index:064x}"), TrayItemMode::Pinned))
                .collect(),
            ..Settings::default()
        };
        let replaced = settings
            .tray_items
            .first_key_value()
            .map(|(key, _)| key.clone())
            .expect("the bounded map is full");
        let newcomer = "f".repeat(64);

        apply_tray_item_mode(
            &mut settings,
            newcomer.clone(),
            RequestedTrayItemMode::Stored(TrayItemMode::Hidden),
        );

        assert_eq!(settings.tray_items.len(), MAX_TRAY_ITEM_MODES);
        assert!(!settings.tray_items.contains_key(&replaced));
        assert_eq!(
            settings.tray_items.get(&newcomer),
            Some(&TrayItemMode::Hidden)
        );

        // Updating the same item consumes no second slot, and ordinary visible
        // state removes its stored override again.
        apply_tray_item_mode(
            &mut settings,
            newcomer.clone(),
            RequestedTrayItemMode::Stored(TrayItemMode::Pinned),
        );
        assert_eq!(settings.tray_items.len(), MAX_TRAY_ITEM_MODES);
        apply_tray_item_mode(
            &mut settings,
            newcomer.clone(),
            RequestedTrayItemMode::Visible,
        );
        assert!(!settings.tray_items.contains_key(&newcomer));
        assert_eq!(settings.tray_items.len(), MAX_TRAY_ITEM_MODES - 1);
    }
}
