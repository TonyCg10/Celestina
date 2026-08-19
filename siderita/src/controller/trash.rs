use core::pin::Pin;
use std::path::PathBuf;

use celestina_core::CancellationToken;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QStringList};

use super::qobject;
use super::{display_name, search_hit_parent, RECENT_LIMIT};
use crate::pathkey;

impl qobject::SideritaController {
    /// Leaves Trash and repaints the current folder.
    pub fn close_trash(mut self: Pin<&mut Self>) {
        self.as_mut().exit_trash();
        self.as_mut().reproject();
    }

    /// Reads the freedesktop Trash and publishes it to the Trash view (parallel
    /// name / origin / date lists), keeping the info paths for restore-by-index.
    pub fn load_trash(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let entries = match siderita_ops::list_trash() {
            Ok(entries) => entries,
            Err(error) => {
                self.as_mut()
                    .set_op_error(QString::from(error.to_string().as_str()));
                return;
            }
        };

        let names: QStringList = entries
            .iter()
            .map(|entry| QString::from(entry.name.as_str()))
            .collect();
        let origins: QStringList = entries
            .iter()
            .map(|entry| QString::from(entry.original.to_string_lossy().as_ref()))
            .collect();
        let dates: QStringList = entries
            .iter()
            .map(|entry| QString::from(crate::format::trash_date(&entry.deletion_date).as_str()))
            .collect();
        self.as_mut().rust_mut().get_mut().trash_entries = entries;
        self.as_mut().set_trash_names(names);
        self.as_mut().set_trash_origins(origins);
        self.as_mut().set_trash_dates(dates);
    }
    /// Opens Trash as a content-view location: loads the entries and publishes
    /// them onto the shared entry model (so list / grid / details / thumbnails
    /// render them exactly like a folder), flipping `trash_active`.
    pub fn open_trash(mut self: Pin<&mut Self>) {
        self.as_mut().exit_search();
        self.as_mut().exit_recent();
        self.as_mut().load_trash();
        self.as_mut().publish_trash();
    }
    /// Builds the entry-model columns from the loaded trash entries. Reuses the
    /// `search_hits` rendering path (so the lookups resolve) while `trash_entries`
    /// keeps the restore/purge identity.
    pub(crate) fn publish_trash(mut self: Pin<&mut Self>) {
        let entries = self.rust().trash_entries.clone();
        let names: QStringList = entries
            .iter()
            .map(|e| QString::from(e.name.as_str()))
            .collect();
        let paths: QStringList = entries
            .iter()
            .map(|e| pathkey::publish(&e.trashed))
            .collect();
        let kinds: QStringList = entries
            .iter()
            .map(|e| {
                QString::from(if e.trashed.is_dir() {
                    "directory"
                } else {
                    "file"
                })
            })
            .collect();
        let tokens: QStringList = (0..entries.len())
            .map(|i| QString::from(i.to_string().as_str()))
            .collect();
        // Subtitle = where it was; date = when it went to trash; size from the
        // trashed body (folders show "—", matching the folder view).
        let subtitles: QStringList = entries
            .iter()
            .map(|e| QString::from(e.original.to_string_lossy().as_ref()))
            .collect();
        let dates: QStringList = entries
            .iter()
            .map(|e| QString::from(crate::format::trash_date(&e.deletion_date).as_str()))
            .collect();
        let sizes: QStringList = entries
            .iter()
            .map(|e| {
                if e.trashed.is_dir() {
                    QString::from("—")
                } else {
                    QString::from(
                        std::fs::metadata(&e.trashed)
                            .map(|m| crate::format::size(m.len()))
                            .unwrap_or_default()
                            .as_str(),
                    )
                }
            })
            .collect();
        let sections: QStringList = entries.iter().map(|_| QString::default()).collect();

        let hits: Vec<crate::search::SearchHit> = entries
            .iter()
            .map(|e| crate::search::SearchHit {
                name: e.name.clone(),
                path: e.trashed.clone(),
                is_dir: e.trashed.is_dir(),
            })
            .collect();
        self.as_mut().rust_mut().get_mut().search_hits = hits;
        self.as_mut().set_trash_active(true);
        self.as_mut().publish_marked_key();
        self.as_mut().set_selected_token(QString::default());
        self.as_mut().set_entry_names(names.clone());
        self.as_mut().rows_ready(
            names, tokens, kinds, subtitles, paths, sections, sizes, dates,
        );
    }
    /// Opens Recientes as a content-view location: the desktop's own
    /// recently-used list (`recently-used.xbel`), read and published onto the
    /// shared entry model so the list / grid / details render it like a folder.
    /// Siderita only reads that file — the applications that open things are
    /// what write it.
    pub fn open_recent(mut self: Pin<&mut Self>) {
        self.as_mut().exit_search();
        self.as_mut().exit_trash();

        let items = crate::recent::load(RECENT_LIMIT);
        let names: QStringList = items
            .iter()
            .map(|item| QString::from(item.name.as_str()))
            .collect();
        let paths: QStringList = items
            .iter()
            .map(|item| pathkey::publish(&item.path))
            .collect();
        let kinds: QStringList = items
            .iter()
            .map(|item| {
                QString::from(if item.path.is_dir() {
                    "directory"
                } else {
                    "file"
                })
            })
            .collect();
        let tokens: QStringList = (0..items.len())
            .map(|i| QString::from(i.to_string().as_str()))
            .collect();
        // Where it lives, and the day it was last touched — the same two facts
        // the Trash rows carry.
        let subtitles: QStringList = items
            .iter()
            .map(|item| QString::from(search_hit_parent(&item.path).as_str()))
            .collect();
        let dates: QStringList = items
            .iter()
            .map(|item| QString::from(crate::format::date_only(&item.stamp)))
            .collect();
        let sizes: QStringList = items
            .iter()
            .map(|item| {
                if item.path.is_dir() {
                    QString::from("—")
                } else {
                    QString::from(
                        std::fs::metadata(&item.path)
                            .map(|meta| crate::format::size(meta.len()))
                            .unwrap_or_default()
                            .as_str(),
                    )
                }
            })
            .collect();
        let sections: QStringList = items.iter().map(|_| QString::default()).collect();

        let hits: Vec<crate::search::SearchHit> = items
            .iter()
            .map(|item| crate::search::SearchHit {
                name: item.name.clone(),
                path: item.path.clone(),
                is_dir: item.path.is_dir(),
            })
            .collect();

        let count = hits.len().min(i32::MAX as usize) as i32;
        self.as_mut().rust_mut().get_mut().search_hits = hits;
        self.as_mut().set_recent_active(true);
        self.as_mut().publish_marked_key();
        self.as_mut().set_recent_count(count);
        self.as_mut().set_selected_token(QString::default());
        self.as_mut().set_entry_names(names.clone());
        self.as_mut().rows_ready(
            names, tokens, kinds, subtitles, paths, sections, sizes, dates,
        );
    }
    /// Leaves Recientes (a no-op when it is not shown, so any navigation can
    /// call it) without repainting.
    pub(crate) fn exit_recent(mut self: Pin<&mut Self>) {
        if self.rust().recent_active {
            self.as_mut().rust_mut().get_mut().search_hits.clear();
            self.as_mut().set_recent_active(false);
            self.as_mut().publish_marked_key();
        }
    }
    /// Publishes which location the sidebar should mark.
    ///
    /// One question with one owner. Every sidebar row used to compare its own
    /// path against `current_path_key`, which keeps naming the folder
    /// underneath while Papelera or Recientes are shown — so entering either
    /// left two rows lit at once. The rule lives here instead of in the five
    /// QML files that would otherwise have to agree.
    pub(crate) fn publish_marked_key(mut self: Pin<&mut Self>) {
        let marked = if *self.trash_active() || *self.recent_active() {
            QString::default()
        } else {
            self.current_path_key().clone()
        };
        self.as_mut().set_marked_key(marked);
    }

    /// Leaves Recientes and repaints the folder underneath.
    pub fn close_recent(mut self: Pin<&mut Self>) {
        self.as_mut().exit_recent();
        self.as_mut().reproject();
    }
    /// Leaves the Trash location (only clears state if it is actually shown, so
    /// it is safe to call on any navigation) and returns to the folder.
    pub(crate) fn exit_trash(mut self: Pin<&mut Self>) {
        if self.rust().trash_active {
            self.as_mut().rust_mut().get_mut().search_hits.clear();
            self.as_mut().set_trash_active(false);
            self.as_mut().publish_marked_key();
        }
    }
    /// The `.trashinfo` record of the trashed entry whose body the key
    /// `trashed` names, if that entry is still in the loaded list and its
    /// record still exists.
    ///
    /// The identity of a trashed entry is its own path, never its position: the
    /// list is reloaded after every restore, purge and empty, so a row index
    /// captured when a menu opened can name a different entry by the time the
    /// menu item is clicked — and "Eliminar permanentemente" is irreversible.
    fn trash_record(&self, trashed: &QString) -> Option<PathBuf> {
        let trashed = pathkey::decode(trashed).ok()?;
        let info = self
            .rust()
            .trash_entries
            .iter()
            .find(|entry| entry.trashed == trashed)
            .map(|entry| entry.info.clone())?;
        info.exists().then_some(info)
    }

    /// Permanently deletes the trashed entry whose body sits at `trashed`, then
    /// refreshes the view.
    pub fn purge_trash(mut self: Pin<&mut Self>, trashed: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(info) = self.trash_record(trashed) else {
            return;
        };
        match siderita_ops::purge_from_trash(&info) {
            Ok(_) => {
                self.as_mut().load_trash();
                if self.rust().trash_active {
                    self.as_mut().publish_trash();
                }
            }
            Err(error) => self
                .as_mut()
                .set_op_error(QString::from(error.to_string().as_str())),
        }
    }
    /// Restores the trashed entry whose body sits at `trashed`, then refreshes
    /// both the Trash view and the current folder (the entry may reappear
    /// there). A refusal (its origin is taken) surfaces as `op_error`.
    pub fn restore_trash(mut self: Pin<&mut Self>, trashed: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(info) = self.trash_record(trashed) else {
            return;
        };
        match siderita_ops::restore_from_trash(&info, &CancellationToken::new()) {
            Ok(_) => {
                self.as_mut().load_trash();
                if self.rust().trash_active {
                    self.as_mut().publish_trash();
                } else {
                    self.as_mut().refresh();
                }
            }
            Err(error) => self
                .as_mut()
                .set_op_error(QString::from(error.to_string().as_str())),
        }
    }
    /// Restores every entry currently in the Trash view. Each is attempted
    /// independently; failures (e.g. an origin now occupied) are reported
    /// together after the list and the folder are refreshed.
    pub fn restore_all_trash(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let infos: Vec<PathBuf> = self
            .rust()
            .trash_entries
            .iter()
            .map(|e| e.info.clone())
            .collect();
        if infos.is_empty() {
            return;
        }
        let cancellation = CancellationToken::new();
        let mut failures = Vec::new();
        for info in &infos {
            if let Err(error) = siderita_ops::restore_from_trash(info, &cancellation) {
                failures.push(format!("{}: {error}", display_name(info)));
            }
        }
        // Refresh first (both clear op_error), then report any failures last.
        self.as_mut().load_trash();
        if self.rust().trash_active {
            self.as_mut().publish_trash();
        } else {
            self.as_mut().refresh();
        }
        if !failures.is_empty() {
            let total = infos.len();
            let summary = if failures.len() == total {
                failures.join("\n")
            } else {
                format!(
                    "{} de {} restauraciones fallaron:\n{}",
                    failures.len(),
                    total,
                    failures.join("\n")
                )
            };
            self.as_mut().set_op_error(QString::from(summary.as_str()));
        }
    }
    /// Permanently deletes every entry in the Trash view. Irreversible — the QML
    /// gates this behind a confirmation. Each is purged independently; failures
    /// are reported together after the list is refreshed. The current folder is
    /// untouched (trashed entries live in the Trash, not here), so unlike
    /// restore there is nothing to refresh but the Trash list itself.
    pub fn empty_trash(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let infos: Vec<PathBuf> = self
            .rust()
            .trash_entries
            .iter()
            .map(|e| e.info.clone())
            .collect();
        if infos.is_empty() {
            return;
        }
        let mut failures = Vec::new();
        for info in &infos {
            if let Err(error) = siderita_ops::purge_from_trash(info) {
                failures.push(format!("{}: {error}", display_name(info)));
            }
        }
        self.as_mut().load_trash();
        if self.rust().trash_active {
            self.as_mut().publish_trash();
        }
        if !failures.is_empty() {
            let total = infos.len();
            let summary = if failures.len() == total {
                failures.join("\n")
            } else {
                format!(
                    "{} de {} no se pudieron borrar:\n{}",
                    failures.len(),
                    total,
                    failures.join("\n")
                )
            };
            self.as_mut().set_op_error(QString::from(summary.as_str()));
        }
    }
}
