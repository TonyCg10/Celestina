//! User-curated marks and places for the sidebar: custom per-entry icons,
//! favourites (starred paths), bookmarks (named, reorderable) and the places
//! row (the XDG/Trash locations, hideable and reorderable). Each list is small,
//! persisted to its own file, and — because every tab holds its own copy — is
//! re-read on tab activation so a change made in one tab reaches the others.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QStringList};

use super::display::display_name;
use super::qobject;
use crate::pathkey;

/// One stable, atomic `key\ticon\taccent` line per appearance. QML therefore
/// sees a complete record at once and never mixes fields from different edits.
pub(crate) fn icon_override_entries(
    map: &std::collections::HashMap<String, crate::icons::IconAppearance>,
) -> QStringList {
    let mut entries: Vec<(&String, &crate::icons::IconAppearance)> = map.iter().collect();
    entries.sort_by_key(|(key, _)| *key);
    entries
        .iter()
        .map(|(key, appearance)| {
            QString::from(format!("{key}\t{}\t{}", appearance.icon, appearance.accent).as_str())
        })
        .collect()
}

/// The starred entries as `key\tkind` lines. The kind is resolved here, once
/// per refresh, so the sidebar can show a folder as a folder and say plainly
/// when a favourite's target is gone rather than offering a row that leads
/// nowhere. A key that will not decode is dropped: it names nothing.
pub(crate) fn favorite_entry_list(keys: &std::collections::BTreeSet<String>) -> QStringList {
    keys.iter()
        .filter_map(|key| {
            let path = pathkey::decode_str(key).ok()?;
            let kind = match std::fs::metadata(&path) {
                Ok(meta) if meta.is_dir() => "directory",
                Ok(_) => "file",
                Err(_) => "missing",
            };
            Some(QString::from(format!("{key}\t{kind}").as_str()))
        })
        .collect()
}

impl qobject::SideritaController {
    /// Sets the custom icon for the entry `key` names. A key that is not well
    /// formed is refused: nothing on disk answers to it.
    pub fn set_custom_icon(mut self: Pin<&mut Self>, key: &QString, icon: &QString) {
        let Some(key) = self.as_mut().accept_mark(key) else {
            return;
        };
        self.as_mut().set_op_error(QString::default());
        let previous = self.rust().custom_icons.clone();
        let icon = icon.to_string();
        {
            let map = &mut self.as_mut().rust_mut().get_mut().custom_icons;
            let empty = {
                let appearance = map.entry(key.clone()).or_default();
                appearance.icon = icon;
                appearance.icon.is_empty() && appearance.accent.is_empty()
            };
            if empty {
                map.remove(&key);
            }
        }
        self.as_mut().persist_custom_icons(previous);
    }

    pub fn set_custom_icon_accent(mut self: Pin<&mut Self>, key: &QString, accent: &QString) {
        let accent = accent.to_string();
        if !crate::icons::valid_accent(&accent) {
            return;
        }
        let Some(key) = self.as_mut().accept_mark(key) else {
            return;
        };
        self.as_mut().set_op_error(QString::default());
        let previous = self.rust().custom_icons.clone();
        {
            let map = &mut self.as_mut().rust_mut().get_mut().custom_icons;
            let empty = {
                let appearance = map.entry(key.clone()).or_default();
                appearance.accent = accent;
                appearance.icon.is_empty() && appearance.accent.is_empty()
            };
            if empty {
                map.remove(&key);
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

    pub fn toggle_favorite(mut self: Pin<&mut Self>, key: &QString) {
        let Some(key) = self.as_mut().accept_mark(key) else {
            return;
        };
        {
            let set = &mut self.as_mut().rust_mut().get_mut().favorites;
            if !set.remove(&key) {
                set.insert(key);
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

    pub fn add_bookmark(mut self: Pin<&mut Self>, key: &QString) {
        let Some(location) = self.as_mut().accept_key(key) else {
            return;
        };
        let key = pathkey::encode(&location);
        if self.rust().bookmarks.iter().any(|entry| entry.path == key) {
            return;
        }
        let name = display_name(&location);
        self.as_mut()
            .rust_mut()
            .get_mut()
            .bookmarks
            .push(crate::bookmarks::Bookmark { name, path: key });
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

    /// The path key of the sidebar place `key` names (`HOME`, `DOWNLOAD`, …),
    /// or an empty string when this machine has no such folder.
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

impl qobject::SideritaController {
    /// Records a section's collapsed state and republishes the list QML reads.
    ///
    /// Collapsing is a preference like any other: it survived only until the
    /// window closed, which for someone who keeps a section shut is a setting
    /// that does not work.
    pub fn set_section_collapsed(mut self: Pin<&mut Self>, section: &QString, collapsed: bool) {
        let section = section.to_string();
        if section.is_empty() {
            return;
        }
        let mut settings = crate::settings::load();
        let already = settings.collapsed_sections.contains(&section);
        if collapsed == already {
            return;
        }
        if collapsed {
            settings.collapsed_sections.push(section);
        } else {
            settings.collapsed_sections.retain(|s| *s != section);
        }
        let _ = crate::settings::save(&settings);
        self.as_mut().publish_collapsed_sections(&settings);
    }

    /// Publishes the stored list, so a binding can read it without asking the
    /// disk on every evaluation.
    pub(crate) fn publish_collapsed_sections(
        mut self: Pin<&mut Self>,
        settings: &crate::settings::Settings,
    ) {
        self.as_mut()
            .set_collapsed_sections(folded_list(&settings.collapsed_sections));
    }
}

/// The stored section names as the list QML binds to.
pub(crate) fn folded_list(sections: &[String]) -> QStringList {
    sections
        .iter()
        .fold(QStringList::default(), |mut list, section| {
            list.append(QString::from(section.as_str()));
            list
        })
}
