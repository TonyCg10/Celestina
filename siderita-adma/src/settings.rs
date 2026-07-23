//! Small persisted UI settings — the view mode and item scale the user last
//! chose, and the removable devices they hid from the sidebar. Stored as a
//! `key=value` file under the XDG config home; like bookmarks, it is a
//! convenience that never fails the app when absent or unreadable.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The persisted view configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// `"list"` or `"grid"`.
    pub view_mode: String,
    /// Item scale (the zoom slider), clamped to a sane range on load.
    pub scale: f64,
    /// UDisks2 device names the user hid from the "Dispositivos" list.
    pub hidden_devices: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            view_mode: "list".to_owned(),
            scale: 1.0,
            hidden_devices: Vec::new(),
        }
    }
}

fn config_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|value| value.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("siderita").join("settings.conf"))
}

pub fn load() -> Settings {
    match config_file() {
        Some(path) => load_from(&path),
        None => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> io::Result<()> {
    match config_file() {
        Some(path) => save_to(&path, settings),
        None => Ok(()),
    }
}

fn load_from(path: &Path) -> Settings {
    let Ok(content) = fs::read_to_string(path) else {
        return Settings::default();
    };
    let mut settings = Settings::default();
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "view_mode" if value == "list" || value == "grid" => {
                settings.view_mode = value.to_owned();
            }
            "scale" => {
                if let Ok(scale) = value.parse::<f64>() {
                    settings.scale = scale.clamp(0.8, 1.9);
                }
            }
            "hidden_device" if !value.is_empty() => {
                settings.hidden_devices.push(value.to_owned());
            }
            _ => {}
        }
    }
    settings
}

fn save_to(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = format!(
        "view_mode={}\nscale={:.2}\n",
        if settings.view_mode == "grid" {
            "grid"
        } else {
            "list"
        },
        settings.scale.clamp(0.8, 1.9),
    );
    for device in &settings.hidden_devices {
        let device = device.replace(['\n', '\r'], "");
        if !device.is_empty() {
            text.push_str("hidden_device=");
            text.push_str(&device);
            text.push('\n');
        }
    }
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "siderita-set-{label}-{}-{nonce}/settings.conf",
            std::process::id()
        ))
    }

    #[test]
    fn round_trips_view_mode_scale_and_hidden_devices() {
        let file = temp_file("rt");
        let settings = Settings {
            view_mode: "grid".to_owned(),
            scale: 1.3,
            hidden_devices: vec!["MI USB".to_owned(), "sdb1".to_owned()],
        };
        save_to(&file, &settings).expect("save");
        assert_eq!(load_from(&file), settings);
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn scale_is_clamped_and_bad_values_fall_back() {
        let file = temp_file("clamp");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "view_mode=weird\nscale=99\n").unwrap();
        let loaded = load_from(&file);
        assert_eq!(loaded.view_mode, "list"); // invalid → default
        assert_eq!(loaded.scale, 1.9); // clamped
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn missing_file_is_defaults() {
        assert_eq!(
            load_from(Path::new("/nonexistent/siderita/settings.conf")),
            Settings::default()
        );
    }
}
