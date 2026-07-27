//! Magnetita's plugin settings — which plugins the daemon acts on.
//!
//! A small, file-backed set of on/off flags, one per daily plugin, that the
//! app's Settings surface toggles and the daemon honours: a disabled plugin is
//! one the daemon does not act on — it stops mirroring the phone's
//! notifications, syncing the clipboard, showing battery, and so on. Defaults
//! are all-on, so a fresh install behaves exactly as it did before this existed.
//!
//! Persisted as JSON in Magnetita's config dir — the suite's own config space,
//! not a hidden private store — and loaded at boot. A missing or unknown key
//! falls back to on, so the file stays forward-compatible as plugins are added.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The per-plugin on/off flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "on")]
    pub battery: bool,
    #[serde(default = "on")]
    pub notifications: bool,
    #[serde(default = "on")]
    pub clipboard: bool,
    #[serde(default = "on")]
    pub share: bool,
    #[serde(default = "on")]
    pub findmyphone: bool,
    #[serde(default = "on")]
    pub media: bool,
}

fn on() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            battery: true,
            notifications: true,
            clipboard: true,
            share: true,
            findmyphone: true,
            media: true,
        }
    }
}

impl Settings {
    /// Load from `path`, or all-on defaults if it is absent. A corrupt file also
    /// falls back to defaults rather than erroring — losing a toggle is not worth
    /// refusing to start the whole daemon.
    pub fn load(path: &Path) -> Settings {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    /// Persist to `path`, creating the parent directory.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, text)
    }

    /// Set the flag named `plugin`; returns whether the name was a known plugin.
    pub fn set(&mut self, plugin: &str, enabled: bool) -> bool {
        match plugin {
            "battery" => self.battery = enabled,
            "notifications" => self.notifications = enabled,
            "clipboard" => self.clipboard = enabled,
            "share" => self.share = enabled,
            "findmyphone" => self.findmyphone = enabled,
            "media" => self.media = enabled,
            _ => return false,
        }
        true
    }

    /// The flags as `(name, enabled)` pairs, in a stable order for the UI.
    pub fn entries(&self) -> [(&'static str, bool); 6] {
        [
            ("battery", self.battery),
            ("notifications", self.notifications),
            ("clipboard", self.clipboard),
            ("share", self.share),
            ("findmyphone", self.findmyphone),
            ("media", self.media),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn defaults_are_all_on() {
        let settings = Settings::default();
        assert!(settings.entries().iter().all(|(_, on)| *on));
    }

    #[test]
    fn set_toggles_a_known_plugin_and_rejects_an_unknown_one() {
        let mut settings = Settings::default();
        assert!(settings.set("clipboard", false));
        assert!(!settings.clipboard);
        assert!(!settings.set("teleport", false));
    }

    #[test]
    fn a_partial_file_fills_missing_plugins_with_on() {
        // Only `media` is written; every other plugin must default to on.
        let dir = std::env::temp_dir();
        let path = dir.join(format!("mag-settings-{}.json", std::process::id()));
        std::fs::write(&path, r#"{"media":false}"#).unwrap();

        let settings = Settings::load(&path);
        assert!(!settings.media);
        assert!(settings.battery);
        assert!(settings.clipboard);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_missing_file_is_all_on_and_a_corrupt_one_falls_back() {
        let missing = std::env::temp_dir().join("mag-settings-nope-xyz.json");
        let _ = std::fs::remove_file(&missing);
        assert_eq!(Settings::load(&missing), Settings::default());

        let corrupt =
            std::env::temp_dir().join(format!("mag-settings-bad-{}.json", std::process::id()));
        std::fs::write(&corrupt, "{ not json").unwrap();
        assert_eq!(Settings::load(&corrupt), Settings::default());
        std::fs::remove_file(&corrupt).unwrap();
    }

    #[test]
    fn it_round_trips_through_a_file() {
        let path =
            std::env::temp_dir().join(format!("mag-settings-rt-{}.json", std::process::id()));
        let mut settings = Settings::default();
        settings.set("notifications", false);
        settings.set("media", false);
        settings.save(&path).unwrap();

        assert_eq!(Settings::load(&path), settings);
        std::fs::remove_file(&path).unwrap();
    }
}
