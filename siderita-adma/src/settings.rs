//! Small persisted UI settings — the view mode, the independent size scales
//! (content / interface / sidebar × icons / text), sort and hidden-toggle
//! state, and the removable devices the user hid from the sidebar. Stored as a
//! `key=value` file
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

/// The window size a first run opens at, and the bounds a remembered one is
/// clamped to — a stale config must never reopen a window too small to use or
/// larger than any real display.
const DEFAULT_WINDOW_WIDTH: i32 = 1120;
const DEFAULT_WINDOW_HEIGHT: i32 = 720;
const MIN_WINDOW_WIDTH: i32 = 680;
const MIN_WINDOW_HEIGHT: i32 = 480;
const MAX_WINDOW_SIDE: i32 = 16384;
/// Cap on the restored session, so a runaway config cannot open tabs forever.
const MAX_TABS: usize = 32;

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
    /// Sidebar place keys in the order the user arranged them. Keys the file
    /// does not mention keep their catalogue order behind the ones it does, so
    /// a place added by a later version simply appears at the end instead of
    /// disappearing.
    pub place_order: Vec<String>,
    /// Place keys the user hid from the sidebar.
    pub hidden_places: Vec<String>,
    /// The window size to reopen at. Only the size: a Wayland client cannot
    /// place its own window, so there is no honest position to remember.
    pub window_width: i32,
    pub window_height: i32,
    /// The folders that were open in tabs, in order, and which one was active.
    pub tabs: Vec<String>,
    pub active_tab: i32,
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
            place_order: Vec::new(),
            hidden_places: Vec::new(),
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            tabs: Vec::new(),
            active_tab: 0,
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
            "view_mode" if value == "list" || value == "grid" || value == "details" => {
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
            "place_order" if !value.is_empty() => {
                settings.place_order = value
                    .split(',')
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            "hidden_place" if !value.is_empty() => {
                settings.hidden_places.push(value.to_owned());
            }
            "window_width" => {
                if let Ok(width) = value.parse::<i32>() {
                    settings.window_width = width.clamp(MIN_WINDOW_WIDTH, MAX_WINDOW_SIDE);
                }
            }
            "window_height" => {
                if let Ok(height) = value.parse::<i32>() {
                    settings.window_height = height.clamp(MIN_WINDOW_HEIGHT, MAX_WINDOW_SIDE);
                }
            }
            "tab" if !value.is_empty() && settings.tabs.len() < MAX_TABS => {
                settings.tabs.push(value.to_owned());
            }
            "active_tab" => {
                if let Ok(index) = value.parse::<i32>() {
                    settings.active_tab = index.max(0);
                }
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
        match settings.view_mode.as_str() {
            "grid" => "grid",
            "details" => "details",
            _ => "list",
        },
        settings
            .content_icon_scale
            .clamp(SCALE_MIN, CONTENT_ICON_SCALE_MAX),
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
    // Place keys are a closed vocabulary (HOME, DOCUMENTS, TRASH …), so a comma
    // list is safe here in a way a path list would not be.
    let order: Vec<&str> = settings
        .place_order
        .iter()
        .map(|key| key.trim())
        .filter(|key| !key.is_empty() && !key.contains([',', '\n', '\r']))
        .collect();
    if !order.is_empty() {
        text.push_str("place_order=");
        text.push_str(&order.join(","));
        text.push('\n');
    }
    for place in &settings.hidden_places {
        let place = place.replace(['\n', '\r'], "");
        if !place.is_empty() {
            text.push_str("hidden_place=");
            text.push_str(&place);
            text.push('\n');
        }
    }
    text.push_str(&format!(
        "window_width={}\nwindow_height={}\n",
        settings
            .window_width
            .clamp(MIN_WINDOW_WIDTH, MAX_WINDOW_SIDE),
        settings
            .window_height
            .clamp(MIN_WINDOW_HEIGHT, MAX_WINDOW_SIDE),
    ));
    for tab in settings.tabs.iter().take(MAX_TABS) {
        let tab = tab.replace(['\n', '\r'], "");
        if !tab.is_empty() {
            text.push_str("tab=");
            text.push_str(&tab);
            text.push('\n');
        }
    }
    text.push_str(&format!("active_tab={}\n", settings.active_tab.max(0)));
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
            place_order: vec!["TRASH".to_owned(), "HOME".to_owned()],
            hidden_places: vec!["MUSIC".to_owned()],
            window_width: 1400,
            window_height: 900,
            tabs: vec!["/home/u".to_owned(), "/etc".to_owned()],
            active_tab: 1,
        };
        save_to(&file, &settings).expect("save");
        assert_eq!(load_from(&file), settings);
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn a_silly_window_size_is_clamped_and_the_session_is_capped() {
        let file = temp_file("window");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let mut text = "window_width=10\nwindow_height=99999\n".to_owned();
        for i in 0..MAX_TABS + 5 {
            text.push_str(&format!("tab=/f{i}\n"));
        }
        fs::write(&file, text).unwrap();
        let loaded = load_from(&file);
        assert_eq!(loaded.window_width, MIN_WINDOW_WIDTH);
        assert_eq!(loaded.window_height, MAX_WINDOW_SIDE);
        assert_eq!(loaded.tabs.len(), MAX_TABS);
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn a_config_without_place_keys_loads_the_defaults() {
        let file = temp_file("noplaces");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "view_mode=grid\n").unwrap();
        let loaded = load_from(&file);
        assert!(loaded.place_order.is_empty());
        assert!(loaded.hidden_places.is_empty());
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
