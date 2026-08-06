//! Per-folder view options: the sort field/direction, the hidden toggle and the
//! list/grid/details mode. Changing any of them reprojects the current view and
//! persists the choice both globally (the default for folders never arranged)
//! and, when it is a real folder, as that folder's own remembered arrangement.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use siderita_core::SortDirection;

use super::qobject;
use super::sort_field_from_index;

impl qobject::SideritaController {
    pub fn toggle_hidden(mut self: Pin<&mut Self>) {
        let show_hidden = !*self.show_hidden();
        self.as_mut().set_show_hidden(show_hidden);
        self.as_mut().rust_mut().get_mut().options.show_hidden = show_hidden;
        self.as_mut().reproject();
        self.as_mut().persist_view_settings();
    }

    pub fn change_sort_field(mut self: Pin<&mut Self>, field: i32) {
        let Some(sort_field) = sort_field_from_index(field) else {
            return;
        };
        if self.rust().options.sort_field == sort_field {
            return;
        }

        self.as_mut().rust_mut().get_mut().options.sort_field = sort_field;
        self.as_mut().set_sort_field(field);
        self.as_mut().reproject();
        self.as_mut().persist_view_settings();
    }

    pub fn toggle_sort_direction(mut self: Pin<&mut Self>) {
        let ascending = !*self.sort_ascending();
        self.as_mut().rust_mut().get_mut().options.sort_direction = if ascending {
            SortDirection::Ascending
        } else {
            SortDirection::Descending
        };
        self.as_mut().set_sort_ascending(ascending);
        self.as_mut().reproject();
        self.as_mut().persist_view_settings();
    }

    /// Saves the current sort field / direction / hidden toggle so they persist
    /// (read fresh, change only these fields, write back — no cross-tab clobber).
    fn persist_view_settings(mut self: Pin<&mut Self>) {
        let mut settings = crate::settings::load();
        settings.sort_field = *self.sort_field();
        settings.sort_ascending = *self.sort_ascending();
        settings.show_hidden = *self.show_hidden();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        // Arranging a folder is a statement about *that* folder, so it is also
        // remembered for it — the global setting stays the default for folders
        // the user has never arranged.
        self.as_mut().remember_folder_view(None);
    }

    /// Records how the current folder is arranged. `view_mode` overrides what
    /// the record should say (the mode lives in the QML, which hands it down
    /// when the user switches); `None` keeps whatever the folder already had,
    /// falling back to the global default.
    fn remember_folder_view(mut self: Pin<&mut Self>, view_mode: Option<String>) {
        let path = self.current_path_key().to_string();
        if path.is_empty() || self.rust().virtual_rows() {
            return;
        }
        let mode = view_mode
            .or_else(|| {
                crate::folder_views::find(&self.rust().folder_views, &path)
                    .map(|record| record.view_mode.clone())
            })
            .unwrap_or_else(|| self.rust().settings.view_mode.clone());
        let record = crate::folder_views::FolderView {
            path,
            view_mode: mode,
            sort_field: *self.sort_field(),
            sort_ascending: *self.sort_ascending(),
        };
        {
            let records = &mut self.as_mut().rust_mut().get_mut().folder_views;
            crate::folder_views::remember(records, record);
        }
        let _ = crate::folder_views::save(&self.rust().folder_views);
        self.as_mut().refresh_folder_view_props();
    }

    /// Applies the record for the folder just opened, if it has one: the sort
    /// takes effect here, and `folder_view_mode` tells the QML which view to
    /// show. A folder with no record leaves both alone, so it inherits whatever
    /// the user last chose.
    pub(crate) fn apply_folder_view(mut self: Pin<&mut Self>) {
        let path = self.current_path_key().to_string();
        let record = crate::folder_views::find(&self.rust().folder_views, &path).cloned();
        let Some(record) = record else {
            self.as_mut().refresh_folder_view_props();
            return;
        };

        let field_changed = *self.sort_field() != record.sort_field;
        let direction_changed = *self.sort_ascending() != record.sort_ascending;
        if let Some(sort_field) = sort_field_from_index(record.sort_field) {
            if field_changed {
                self.as_mut().rust_mut().get_mut().options.sort_field = sort_field;
                self.as_mut().set_sort_field(record.sort_field);
            }
        }
        if direction_changed {
            self.as_mut().rust_mut().get_mut().options.sort_direction = if record.sort_ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            };
            self.as_mut().set_sort_ascending(record.sort_ascending);
        }
        self.as_mut().refresh_folder_view_props();
        if field_changed || direction_changed {
            self.as_mut().reproject();
        }
    }

    fn refresh_folder_view_props(mut self: Pin<&mut Self>) {
        let path = self.current_path_key().to_string();
        let mode = crate::folder_views::find(&self.rust().folder_views, &path)
            .map(|record| record.view_mode.clone())
            .unwrap_or_default();
        self.as_mut().set_folder_view_pinned(!mode.is_empty());
        self.as_mut()
            .set_folder_view_mode(QString::from(mode.as_str()));
    }

    /// Called by the QML when the user picks a view mode: this folder keeps it.
    pub fn remember_view_mode(self: Pin<&mut Self>, mode: &QString) {
        self.remember_folder_view(Some(mode.to_string()));
    }

    /// Drops this folder's record, so it follows the global defaults again.
    pub fn forget_folder_view(mut self: Pin<&mut Self>) {
        let path = self.current_path_key().to_string();
        let dropped = {
            let records = &mut self.as_mut().rust_mut().get_mut().folder_views;
            crate::folder_views::forget(records, &path)
        };
        if dropped {
            let _ = crate::folder_views::save(&self.rust().folder_views);
        }
        self.as_mut().refresh_folder_view_props();
    }
}
