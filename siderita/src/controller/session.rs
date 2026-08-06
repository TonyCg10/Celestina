//! Session and window state: the settings-store getters/setters the window and
//! sidebar read before any tab exists — the window size, the open-tabs list, the
//! remembered view mode and the four size scales. Every setter re-reads the
//! settings file, changes only its own fields and writes back, so a change made
//! in one tab never clobbers another's.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QStringList};

use super::launch_argument;
use super::qobject;

impl qobject::SideritaController {
    pub fn saved_window_width(&self) -> i32 {
        self.rust().settings.window_width
    }

    pub fn saved_window_height(&self) -> i32 {
        self.rust().settings.window_height
    }

    pub fn save_window_size(mut self: Pin<&mut Self>, width: i32, height: i32) {
        let mut settings = crate::settings::load();
        settings.window_width = width;
        settings.window_height = height;
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
    }

    /// The folders that were open last time, as path keys. Sessions written
    /// before ADR 0008 hold raw paths, so each record is migrated on the way
    /// out rather than rewriting the file behind the user's back.
    pub fn saved_tabs(&self) -> QStringList {
        self.rust()
            .settings
            .tabs
            .iter()
            .map(|stored| QString::from(crate::pathkey::normalize(stored).as_str()))
            .collect()
    }

    pub fn saved_active_tab(&self) -> i32 {
        self.rust().settings.active_tab
    }

    /// Remembers the open tabs. `keys` are path keys, stored marked as such by
    /// `pathkey::persist`, so a folder whose name is not valid UTF-8 reopens
    /// where it was and no reader has to infer which spelling a record holds.
    pub fn save_tabs(mut self: Pin<&mut Self>, keys: &QStringList, active: i32) {
        let tabs: Vec<String> = keys
            .iter()
            .map(ToString::to_string)
            .filter(|key| !key.is_empty())
            .map(|key| crate::pathkey::persist(&key))
            .collect();
        let mut settings = crate::settings::load();
        settings.tabs = tabs;
        settings.active_tab = active;
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
    }

    pub fn launch_path_given(&self) -> bool {
        launch_argument().is_some()
    }

    /// The persisted list/grid mode and size scales, so a new tab / the sidebar
    /// opens the way the user last left it.
    pub fn saved_view_mode(&self) -> QString {
        QString::from(self.rust().settings.view_mode.as_str())
    }

    pub fn saved_content_icon_scale(&self) -> f64 {
        self.rust().settings.content_icon_scale
    }

    pub fn saved_content_text_scale(&self) -> f64 {
        self.rust().settings.content_text_scale
    }

    pub fn saved_interface_icon_scale(&self) -> f64 {
        self.rust().settings.interface_icon_scale
    }

    pub fn saved_interface_text_scale(&self) -> f64 {
        self.rust().settings.interface_text_scale
    }

    pub fn saved_sidebar_icon_scale(&self) -> f64 {
        self.rust().settings.sidebar_icon_scale
    }

    pub fn saved_sidebar_text_scale(&self) -> f64 {
        self.rust().settings.sidebar_text_scale
    }

    /// Persists the current view mode (list / grid).
    pub fn save_view_mode(mut self: Pin<&mut Self>, mode: &QString) {
        let mode = mode.to_string();
        // Read fresh, change only this field, write back — so a sort/hidden,
        // sizing or device change in another tab is not clobbered.
        let mut settings = crate::settings::load();
        settings.view_mode = match mode.as_str() {
            "grid" => "grid".to_owned(),
            "details" => "details".to_owned(),
            _ => "list".to_owned(),
        };
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
    }

    /// Persists the four independent size scales.
    pub fn save_sizing(
        mut self: Pin<&mut Self>,
        content_icon: f64,
        content_text: f64,
        interface_icon: f64,
        interface_text: f64,
        sidebar_icon: f64,
        sidebar_text: f64,
    ) {
        let mut settings = crate::settings::load();
        settings.content_icon_scale = content_icon;
        settings.content_text_scale = content_text;
        settings.interface_icon_scale = interface_icon;
        settings.interface_text_scale = interface_text;
        settings.sidebar_icon_scale = sidebar_icon;
        settings.sidebar_text_scale = sidebar_text;
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
    }
}
