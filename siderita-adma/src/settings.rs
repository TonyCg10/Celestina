//! Small persisted UI settings — the view mode, the four independent size
//! scales (content/sidebar × icons/text), sort and hidden-toggle state, and the
//! removable devices the user hid from the sidebar. Stored as a `key=value` file
//! under the XDG config home; like bookmarks, it is a convenience that never
//! fails the app when absent or unreadable.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The inclusive range every size scale is clamped to on load and save. The UI
/// shows this as 10 %–100 % (a fraction of the 2.0 maximum); 1.0 is the
/// historical default and reads as 50 %.
const SCALE_MIN: f64 = 0.2;
const SCALE_MAX: f64 = 2.0;
/// Content icons alone may go larger — up to 150 % (factor 3.0).
const CONTENT_ICON_SCALE_MAX: f64 = 3.0;

/// The persisted view configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// `"list"` or `"grid"`.
    pub view_mode: String,
    /// Content-view icon scale (the glyph tiles), clamped on load.
    pub content_icon_scale: f64,
    /// Content-view text scale (name + subtitle), clamped on load.
    pub content_text_scale: f64,
    /// Chrome icon scale (top bar + tabs + bottom bar controls), clamped on load.
    pub interface_icon_scale: f64,
    /// Chrome text scale (breadcrumb, search, tabs, bottom bar), clamped on load.
    pub interface_text_scale: f64,
    /// Sidebar icon scale (place / bookmark / device icons), clamped on load.
    pub sidebar_icon_scale: f64,
    /// Sidebar text scale (labels + the info box), clamped on load.
    pub sidebar_text_scale: f64,
    /// Sort field index (0 name, 1 size, 2 date, 3 kind).
    pub sort_field: i32,
    /// Ascending vs descending.
    pub sort_ascending: bool,
    /// Whether hidden (dotfile) entries are shown.
    pub show_hidden: bool,
    /// UDisks2 device names the user hid from the "Dispositivos" list.
    pub hidden_devices: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            view_mode: "list".to_owned(),
            content_icon_scale: 1.0,
            content_text_scale: 1.0,
            interface_icon_scale: 1.0,
            interface_text_scale: 1.0,
            sidebar_icon_scale: 1.0,
            sidebar_text_scale: 1.0,
            sort_field: 0,
            sort_ascending: true,
            show_hidden: false,
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

fn parse_scale(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .map(|s| s.clamp(SCALE_MIN, SCALE_MAX))
}

fn load_from(path: &Path) -> Settings {
    let Ok(content) = fs::read_to_string(path) else {
        return Settings::default();
    };
    let mut settings = Settings::default();
    // A pre-granular config held one `scale` for the whole content view; adopt
    // it for both content scales unless the granular keys override.
    let mut legacy_scale: Option<f64> = None;
    let mut content_icon_seen = false;
    let mut content_text_seen = false;
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "view_mode" if value == "list" || value == "grid" => {
                settings.view_mode = value.to_owned();
            }
            "scale" => legacy_scale = parse_scale(value),
            "content_icon_scale" => {
                if let Ok(scale) = value.parse::<f64>() {
                    settings.content_icon_scale = scale.clamp(SCALE_MIN, CONTENT_ICON_SCALE_MAX);
                    content_icon_seen = true;
                }
            }
            "content_text_scale" => {
                if let Some(scale) = parse_scale(value) {
                    settings.content_text_scale = scale;
                    content_text_seen = true;
                }
            }
            "interface_icon_scale" => {
                if let Some(scale) = parse_scale(value) {
                    settings.interface_icon_scale = scale;
                }
            }
            "interface_text_scale" => {
                if let Some(scale) = parse_scale(value) {
                    settings.interface_text_scale = scale;
                }
            }
            "sidebar_icon_scale" => {
                if let Some(scale) = parse_scale(value) {
                    settings.sidebar_icon_scale = scale;
                }
            }
            "sidebar_text_scale" => {
                if let Some(scale) = parse_scale(value) {
                    settings.sidebar_text_scale = scale;
                }
            }
            "sort_field" => {
                if let Ok(field) = value.parse::<i32>() {
                    if (0..=3).contains(&field) {
                        settings.sort_field = field;
                    }
                }
            }
            "sort_ascending" => settings.sort_ascending = value != "false",
            "show_hidden" => settings.show_hidden = value == "true",
            "hidden_device" if !value.is_empty() => {
                settings.hidden_devices.push(value.to_owned());
            }
            _ => {}
        }
    }
    if let Some(scale) = legacy_scale {
        if !content_icon_seen {
            settings.content_icon_scale = scale;
        }
        if !content_text_seen {
            settings.content_text_scale = scale;
        }
    }
    settings
}

fn save_to(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = format!(
        "view_mode={}\ncontent_icon_scale={:.2}\ncontent_text_scale={:.2}\n\
         interface_icon_scale={:.2}\ninterface_text_scale={:.2}\n\
         sidebar_icon_scale={:.2}\nsidebar_text_scale={:.2}\n\
         sort_field={}\nsort_ascending={}\nshow_hidden={}\n",
        if settings.view_mode == "grid" {
            "grid"
        } else {
            "list"
        },
        settings.content_icon_scale.clamp(SCALE_MIN, CONTENT_ICON_SCALE_MAX),
        settings.content_text_scale.clamp(SCALE_MIN, SCALE_MAX),
        settings.interface_icon_scale.clamp(SCALE_MIN, SCALE_MAX),
        settings.interface_text_scale.clamp(SCALE_MIN, SCALE_MAX),
        settings.sidebar_icon_scale.clamp(SCALE_MIN, SCALE_MAX),
        settings.sidebar_text_scale.clamp(SCALE_MIN, SCALE_MAX),
        settings.sort_field.clamp(0, 3),
        settings.sort_ascending,
        settings.show_hidden,
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
    fn round_trips_view_mode_scales_and_hidden_devices() {
        let file = temp_file("rt");
        let settings = Settings {
            view_mode: "grid".to_owned(),
            content_icon_scale: 1.3,
            content_text_scale: 0.9,
            interface_icon_scale: 1.2,
            interface_text_scale: 0.8,
            sidebar_icon_scale: 1.5,
            sidebar_text_scale: 1.1,
            sort_field: 2,
            sort_ascending: false,
            show_hidden: true,
            hidden_devices: vec!["MI USB".to_owned(), "sdb1".to_owned()],
        };
        save_to(&file, &settings).expect("save");
        assert_eq!(load_from(&file), settings);
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn scales_are_clamped_and_bad_values_fall_back() {
        let file = temp_file("clamp");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "view_mode=weird\ncontent_icon_scale=99\n").unwrap();
        let loaded = load_from(&file);
        assert_eq!(loaded.view_mode, "list"); // invalid → default
        assert_eq!(loaded.content_icon_scale, 3.0); // clamped to the 150% max
        assert_eq!(loaded.content_text_scale, 1.0); // untouched default
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn a_legacy_scale_migrates_to_both_content_scales() {
        let file = temp_file("legacy");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        // The old single-scale key seeds both content scales, but an explicit
        // granular key still wins.
        fs::write(&file, "scale=1.4\ncontent_text_scale=1.1\n").unwrap();
        let loaded = load_from(&file);
        assert_eq!(loaded.content_icon_scale, 1.4); // from legacy scale
        assert_eq!(loaded.content_text_scale, 1.1); // explicit override
        assert_eq!(loaded.sidebar_icon_scale, 1.0); // legacy never touched sidebar
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
