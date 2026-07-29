//! User-curated marks and places for the sidebar: custom per-entry icons,
//! favourites (starred paths), bookmarks (named, reorderable) and the places
//! row (the XDG/Trash locations, hideable and reorderable). Each list is small,
//! persisted to its own file, and — because every tab holds its own copy — is
//! re-read on tab activation so a change made in one tab reaches the others.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QStringList};

use super::qobject;
use super::{favorite_entry_list, icon_override_entries};

impl qobject::SideritaController {
    pub fn set_custom_icon(mut self: Pin<&mut Self>, path: &QString, icon: &QString) {
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let previous = self.rust().custom_icons.clone();
        let icon = icon.to_string();
        {
            let map = &mut self.as_mut().rust_mut().get_mut().custom_icons;
            let empty = {
                let appearance = map.entry(path.clone()).or_default();
                appearance.icon = icon;
                appearance.icon.is_empty() && appearance.accent.is_empty()
            };
            if empty {
                map.remove(&path);
            }
        }
        self.as_mut().persist_custom_icons(previous);
    }

    pub fn set_custom_icon_accent(mut self: Pin<&mut Self>, path: &QString, accent: &QString) {
        let path = path.to_string();
        let accent = accent.to_string();
        if path.is_empty() || !crate::icons::valid_accent(&accent) {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let previous = self.rust().custom_icons.clone();
        {
            let map = &mut self.as_mut().rust_mut().get_mut().custom_icons;
            let empty = {
                let appearance = map.entry(path.clone()).or_default();
                appearance.accent = accent;
                appearance.icon.is_empty() && appearance.accent.is_empty()
            };
            if empty {
                map.remove(&path);
            }
        }
        self.as_mut().persist_custom_icons(previous);
    }

    fn persist_custom_icons(
        mut self: Pin<&mut Self>,
        previous: std::collections::HashMap<String, crate::icons::IconAppearance>,
    ) {
        if let Err(error) = crate::icons::save(&self.rust().custom_icons) {
            self.as_mut().rust_mut().get_mut().custom_icons = previous;
            let message = format!("No se pudo guardar la apariencia del icono: {error}");
            self.as_mut().set_op_error(QString::from(message.as_str()));
            return;
        }
        self.as_mut().refresh_custom_icon_props();
    }

    /// Re-reads the saved overrides from disk. Each tab owns its own controller
    /// and its own copy of the map, so after one tab writes an override the
    /// others are told to reload — otherwise they keep the old icon until the
    /// next start.
    pub fn reload_custom_icons(mut self: Pin<&mut Self>) {
        let loaded = crate::icons::load();
        self.as_mut().rust_mut().get_mut().custom_icons = loaded;
        self.as_mut().refresh_custom_icon_props();
    }

    fn refresh_custom_icon_props(mut self: Pin<&mut Self>) {
        let entries = icon_override_entries(&self.rust().custom_icons);
        self.as_mut().set_custom_icon_entries(entries);
    }

    pub fn toggle_favorite(mut self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        {
            let set = &mut self.as_mut().rust_mut().get_mut().favorites;
            if !set.remove(&path) {
                set.insert(path);
            }
        }
        let _ = crate::favorites::save(&self.rust().favorites);
        self.as_mut().refresh_favorite_props();
    }

    /// Like the icon overrides: each tab holds its own copy, so a tab re-reads
    /// the file when it is activated rather than trusting what it loaded at
    /// start.
    pub fn reload_favorites(mut self: Pin<&mut Self>) {
        let loaded = crate::favorites::load();
        self.as_mut().rust_mut().get_mut().favorites = loaded;
        self.as_mut().refresh_favorite_props();
    }

    fn refresh_favorite_props(mut self: Pin<&mut Self>) {
        let entries = favorite_entry_list(&self.rust().favorites);
        self.as_mut().set_favorite_entries(entries);
    }

    pub fn add_bookmark(mut self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        if path.is_empty() || self.rust().bookmarks.iter().any(|entry| entry.path == path) {
            return;
        }
        let name = crate::bookmarks::name_for(&path);
        self.as_mut()
            .rust_mut()
            .get_mut()
            .bookmarks
            .push(crate::bookmarks::Bookmark { name, path });
        self.as_mut().refresh_bookmark_properties();
        let _ = crate::bookmarks::save(&self.rust().bookmarks);
    }

    pub fn remove_bookmark(mut self: Pin<&mut Self>, index: i32) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if index >= self.rust().bookmarks.len() {
            return;
        }
        self.as_mut().rust_mut().get_mut().bookmarks.remove(index);
        self.as_mut().refresh_bookmark_properties();
        let _ = crate::bookmarks::save(&self.rust().bookmarks);
    }

    pub fn rename_bookmark(mut self: Pin<&mut Self>, index: i32, name: &QString) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let name = name.to_string();
        if name.is_empty() || index >= self.rust().bookmarks.len() {
            return;
        }
        self.as_mut().rust_mut().get_mut().bookmarks[index].name = name;
        self.as_mut().refresh_bookmark_properties();
        let _ = crate::bookmarks::save(&self.rust().bookmarks);
    }

    pub fn move_bookmark(mut self: Pin<&mut Self>, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let moved = {
            let list = &mut self.as_mut().rust_mut().get_mut().bookmarks;
            crate::bookmarks::move_item(list, from, to)
        };
        if !moved {
            return;
        }
        self.as_mut().refresh_bookmark_properties();
        let _ = crate::bookmarks::save(&self.rust().bookmarks);
    }

    /// Re-reads the bookmark file into this controller and republishes the
    /// name/path properties. Called on tab activation so a bookmark added in one
    /// tab becomes visible in the others, and once as part of `start_common`.
    pub fn reload_bookmarks(mut self: Pin<&mut Self>) {
        let loaded = crate::bookmarks::load();
        self.as_mut().rust_mut().get_mut().bookmarks = loaded;
        self.as_mut().refresh_bookmark_properties();
    }

    pub(crate) fn refresh_bookmark_properties(mut self: Pin<&mut Self>) {
        let (names, paths): (QStringList, QStringList) = {
            let bookmarks = &self.rust().bookmarks;
            (
                bookmarks
                    .iter()
                    .map(|entry| QString::from(entry.name.as_str()))
                    .collect(),
                bookmarks
                    .iter()
                    .map(|entry| QString::from(entry.path.as_str()))
                    .collect(),
            )
        };
        self.as_mut().set_bookmark_names(names);
        self.as_mut().set_bookmark_paths(paths);
    }

    pub fn place_path(&self, key: &QString) -> QString {
        self.rust()
            .places
            .get(&key.to_string())
            .map(|path| QString::from(path.as_str()))
            .unwrap_or_default()
    }

    /// Republishes the sidebar's places: the keys that exist here, in the
    /// user's order, minus the ones they hid — plus how many are hidden, so the
    /// sidebar can offer them back.
    pub(crate) fn refresh_place_props(mut self: Pin<&mut Self>) {
        let (visible, hidden_count) = {
            let rust = self.rust();
            let existing: Vec<&str> = PLACE_CATALOGUE
                .iter()
                .copied()
                // TRASH and RECENT are not XDG directories but locations the
                // app always offers; the rest exist only if the folder does.
                .filter(|key| matches!(*key, "TRASH" | "RECENT") || rust.places.contains_key(*key))
                .collect();

            // The saved order first (only keys that still exist), then anything
            // it never mentioned, in catalogue order.
            let mut ordered: Vec<&str> = Vec::with_capacity(existing.len());
            for key in &rust.settings.place_order {
                if let Some(found) = existing.iter().find(|candidate| *candidate == key) {
                    if !ordered.contains(found) {
                        ordered.push(found);
                    }
                }
            }
            for key in &existing {
                if !ordered.contains(key) {
                    ordered.push(key);
                }
            }

            let hidden = &rust.settings.hidden_places;
            let visible: QStringList = ordered
                .iter()
                .filter(|key| !hidden.iter().any(|h| h == *key))
                .map(|key| QString::from(*key))
                .collect();
            let hidden_count = ordered
                .iter()
                .filter(|key| hidden.iter().any(|h| h == *key))
                .count();
            (visible, hidden_count)
        };
        self.as_mut().set_place_keys(visible);
        self.as_mut()
            .set_hidden_place_count(hidden_count.min(i32::MAX as usize) as i32);
    }

    /// Moves the place at `from` so it sits at `to` among the *visible* places,
    /// and persists the whole order.
    pub fn move_place(mut self: Pin<&mut Self>, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let mut keys: Vec<String> = self
            .place_keys()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if from == to || from >= keys.len() || to >= keys.len() {
            return;
        }
        let moved = keys.remove(from);
        keys.insert(to, moved);

        let mut settings = crate::settings::load();
        // Hidden places keep their relative place at the end, so un-hiding one
        // does not scramble the order the user just set.
        let hidden: Vec<String> = settings.hidden_places.clone();
        settings.place_order = keys.into_iter().chain(hidden).collect();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }

    /// Re-reads the sidebar settings — how a tab picks up an order or a hide
    /// another tab just set (the bookmarks and icons do the same).
    pub fn reload_places(mut self: Pin<&mut Self>) {
        let settings = crate::settings::load();
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }

    pub fn hide_place(mut self: Pin<&mut Self>, key: &QString) {
        let key = key.to_string();
        if key.is_empty() {
            return;
        }
        let mut settings = crate::settings::load();
        if !settings.hidden_places.contains(&key) {
            settings.hidden_places.push(key);
            let _ = crate::settings::save(&settings);
        }
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }

    /// Un-hides every previously-hidden place.
    pub fn unhide_all_places(mut self: Pin<&mut Self>) {
        let mut settings = crate::settings::load();
        settings.hidden_places.clear();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }
}

/// Every sidebar place Siderita knows how to offer, in the order it offers them
/// before the user rearranges anything. The keys are the vocabulary the QML
/// maps to a label and an icon; `TRASH` is Siderita's own, the rest are XDG.
const PLACE_CATALOGUE: &[&str] = &[
    "HOME",
    "DESKTOP",
    "DOCUMENTS",
    "DOWNLOAD",
    "MUSIC",
    "PICTURES",
    "VIDEOS",
    "RECENT",
    "TRASH",
];
