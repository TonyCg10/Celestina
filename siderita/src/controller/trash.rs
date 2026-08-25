//! language-contract: product-copy
//!
//! The two locations that are listed rather than scanned: the freedesktop
//! Trash and the desktop's own recently-used list.
//!
//! Neither is a folder. There is no directory to read, no snapshot and no
//! projection — the rows come from records other programs keep, plus one look
//! at each body to learn what it is and how big. They still ride the folder's
//! entry model, so list, grid, details and thumbnails render them without
//! knowing the difference.
//!
//! Both read on a worker thread, and that is the rule this module exists to
//! keep. The Trash asks every mounted volume whether it holds a trash of its
//! own, and the recently-used list names files that may live anywhere the
//! desktop has ever opened something — a phone, a share, a drive that has been
//! unplugged. Any one of those questions can block for as long as that
//! filesystem takes to give up, and while it did, the window was frozen: the
//! author saw the whole application stop on opening Papelera or Recientes.
//! Nothing here reads the filesystem on the Qt thread, and an answer that
//! arrives after the person has left is dropped rather than painted.

use core::pin::Pin;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use siderita_ops::TrashEntry;

use super::qobject;
use super::{display_name, search_hit_parent, RECENT_LIMIT};
use crate::pathkey;

/// One finished row of a listed location.
///
/// Every field is an answer, not a question: the worker that read the
/// filesystem decided all of them, so publishing costs nothing but the
/// conversion to `QString` and can happen on the Qt thread safely.
struct ListedRow {
    name: String,
    /// Identity (ADR 0008): the body in the Trash, or the recent file itself.
    path: PathBuf,
    is_dir: bool,
    subtitle: String,
    size: String,
    date: String,
}

/// What a row's body is, and how big — one question instead of four.
///
/// The columns used to ask `is_dir()` for the kind, again for the size branch,
/// and once more for the row lookup, then `metadata()` for the size itself:
/// four round trips per row to learn two facts, and on a volume that has
/// stopped answering, four chances to block.
fn body_facts(path: &Path) -> (bool, String) {
    match std::fs::metadata(path) {
        // A folder shows no size, matching the folder view.
        Ok(metadata) if metadata.is_dir() => (true, "—".to_owned()),
        Ok(metadata) => (false, crate::format::size(metadata.len())),
        // Unreadable is not a folder and has no size to state — the same thing
        // the four separate questions each concluded on their own.
        Err(_) => (false, String::new()),
    }
}

/// Reads the Trash: the entries themselves, which carry the identity restore
/// and purge resolve, and the row each one shows.
fn gather_trash() -> Result<(Vec<TrashEntry>, Vec<ListedRow>), String> {
    let entries = siderita_ops::list_trash().map_err(|error| error.to_string())?;
    let rows = entries
        .iter()
        .map(|entry| {
            let (is_dir, size) = body_facts(&entry.trashed);
            ListedRow {
                name: entry.name.clone(),
                path: entry.trashed.clone(),
                is_dir,
                // Where it was, and when it went to the Trash.
                subtitle: entry.original.to_string_lossy().into_owned(),
                size,
                date: crate::format::trash_date(&entry.deletion_date),
            }
        })
        .collect();
    Ok((entries, rows))
}

/// Reads the desktop's recently-used list. Siderita only ever reads that file —
/// the applications that open things are what write it.
fn gather_recent() -> Vec<ListedRow> {
    crate::recent::load(RECENT_LIMIT)
        .into_iter()
        .map(|item| {
            let (is_dir, size) = body_facts(&item.path);
            ListedRow {
                name: item.name,
                // Where it lives, and the day it was last touched — the same
                // two facts a Trash row carries.
                subtitle: search_hit_parent(&item.path),
                date: crate::format::date_only(&item.stamp).to_owned(),
                path: item.path,
                is_dir,
                size,
            }
        })
        .collect()
}

impl qobject::SideritaController {
    /// Leaves Trash and repaints the current folder.
    pub fn close_trash(mut self: Pin<&mut Self>) {
        self.as_mut().exit_trash();
        self.as_mut().reproject();
    }

    /// Opens Trash as a content-view location: its entries ride the same entry
    /// model a folder's rows do, so list / grid / details / thumbnails render
    /// them exactly like a folder.
    pub fn open_trash(mut self: Pin<&mut Self>) {
        self.as_mut().exit_search();
        self.as_mut().exit_recent();
        // The location is entered now, not when its rows land: the click has to
        // reach a window that is already showing Papelera. `trash_active` is
        // also what a listing still in flight asks to find out whether anyone
        // is still waiting for it.
        self.as_mut().set_trash_active(true);
        self.as_mut().publish_marked_key();
        self.as_mut().load_trash();
    }

    /// Reads the freedesktop Trash on a worker thread and publishes it when it
    /// lands. Also the refresh after every restore, purge and empty.
    pub fn load_trash(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        // Nobody is showing it, so nobody would be allowed to see the answer:
        // `trash_listed` would drop it, and the reading state raised here would
        // have no one left to lower it.
        if !self.rust().trash_active {
            return;
        }
        self.as_mut().set_loading(true);
        self.as_mut()
            .set_status_text(QString::from("Leyendo la papelera…"));
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let listed = gather_trash();
            let _ = qt.queue(move |controller: Pin<&mut qobject::SideritaController>| {
                controller.trash_listed(listed);
            });
        });
    }

    /// A Trash listing that has arrived.
    fn trash_listed(
        mut self: Pin<&mut Self>,
        listed: Result<(Vec<TrashEntry>, Vec<ListedRow>), String>,
    ) {
        // A slow volume can hold a listing back for as long as it likes, and by
        // then the person may be reading a folder. An answer nobody is waiting
        // for is dropped: it must never pull the window back into Papelera.
        if !self.rust().trash_active {
            return;
        }
        self.as_mut().set_loading(false);
        self.as_mut().set_status_text(QString::default());
        let (entries, rows) = match listed {
            Ok(listed) => listed,
            Err(error) => {
                self.as_mut().set_op_error(QString::from(error.as_str()));
                return;
            }
        };

        let names: QStringList = rows
            .iter()
            .map(|row| QString::from(row.name.as_str()))
            .collect();
        let origins: QStringList = rows
            .iter()
            .map(|row| QString::from(row.subtitle.as_str()))
            .collect();
        let dates: QStringList = rows
            .iter()
            .map(|row| QString::from(row.date.as_str()))
            .collect();
        self.as_mut().rust_mut().get_mut().trash_entries = entries;
        self.as_mut().set_trash_names(names);
        self.as_mut().set_trash_origins(origins);
        self.as_mut().set_trash_dates(dates);
        self.as_mut().publish_listing(&rows);
    }

    /// Opens Recientes as a content-view location: the desktop's own
    /// recently-used list (`recently-used.xbel`), published onto the shared
    /// entry model so the list / grid / details render it like a folder.
    pub fn open_recent(mut self: Pin<&mut Self>) {
        self.as_mut().exit_search();
        self.as_mut().exit_trash();
        // Entered now, for the same two reasons Papelera is.
        self.as_mut().set_recent_active(true);
        self.as_mut().publish_marked_key();
        self.as_mut().load_recent();
    }

    /// Reads the recently-used list on a worker thread and publishes it when it
    /// lands.
    pub(crate) fn load_recent(mut self: Pin<&mut Self>) {
        if !self.rust().recent_active {
            return;
        }
        self.as_mut().set_loading(true);
        self.as_mut()
            .set_status_text(QString::from("Leyendo Recientes…"));
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let rows = gather_recent();
            let _ = qt.queue(move |controller: Pin<&mut qobject::SideritaController>| {
                controller.recent_listed(rows);
            });
        });
    }

    /// A Recientes listing that has arrived. Dropped, like the Trash's, if the
    /// person has already left.
    fn recent_listed(mut self: Pin<&mut Self>, rows: Vec<ListedRow>) {
        if !self.rust().recent_active {
            return;
        }
        self.as_mut().set_loading(false);
        self.as_mut().set_status_text(QString::default());
        let count = rows.len().min(i32::MAX as usize) as i32;
        self.as_mut().set_recent_count(count);
        self.as_mut().publish_listing(&rows);
    }

    /// Publishes a finished listing onto the shared entry model: the same
    /// parallel role columns and the same signal a folder's rows travel on, so
    /// a trashed file behaves like any other row — single-click selects,
    /// double-click opens, the keyboard and the selection all work.
    fn publish_listing(mut self: Pin<&mut Self>, rows: &[ListedRow]) {
        let names: QStringList = rows
            .iter()
            .map(|row| QString::from(row.name.as_str()))
            .collect();
        // Identity, not text (ADR 0008): a row is opened, revealed and restored
        // through the key published here.
        let paths: QStringList = rows.iter().map(|row| pathkey::publish(&row.path)).collect();
        let kinds: QStringList = rows
            .iter()
            .map(|row| QString::from(if row.is_dir { "directory" } else { "file" }))
            .collect();
        let tokens: QStringList = (0..rows.len())
            .map(|index| QString::from(index.to_string().as_str()))
            .collect();
        let subtitles: QStringList = rows
            .iter()
            .map(|row| QString::from(row.subtitle.as_str()))
            .collect();
        let sizes: QStringList = rows
            .iter()
            .map(|row| QString::from(row.size.as_str()))
            .collect();
        let dates: QStringList = rows
            .iter()
            .map(|row| QString::from(row.date.as_str()))
            .collect();
        let sections: QStringList = rows.iter().map(|_| QString::default()).collect();

        // Every row lookup a verb makes goes through `search_hits`, which all
        // three virtual locations share.
        let hits: Vec<crate::search::SearchHit> = rows
            .iter()
            .map(|row| crate::search::SearchHit {
                name: row.name.clone(),
                path: row.path.clone(),
                is_dir: row.is_dir,
            })
            .collect();

        self.as_mut().rust_mut().get_mut().search_hits = hits;
        // A fresh listing drops any selection carried over from the folder.
        self.as_mut().set_selected_token(QString::default());
        self.as_mut().set_entry_names(names.clone());
        self.as_mut().invalidate_published_rows();
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
            self.as_mut().clear_listing_state();
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
            self.as_mut().clear_listing_state();
            self.as_mut().publish_marked_key();
        }
    }

    /// Takes down the reading state a listing put up.
    ///
    /// Leaving is what cancels a listing — the answer will find its location
    /// gone and drop itself — so the state it set has no other owner left to
    /// clear it. Without this, walking away from a Papelera that is still
    /// reading a sleeping phone left the window busy for good. A navigation
    /// that follows raises `loading` again for its own scan.
    fn clear_listing_state(mut self: Pin<&mut Self>) {
        self.as_mut().set_loading(false);
        self.as_mut().set_status_text(QString::default());
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
            Ok(_) => self.as_mut().load_trash(),
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
            Ok(_) => self.as_mut().after_trash_write(),
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
        self.as_mut().after_trash_write();
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

    /// Re-reads the Trash after a restore, and repaints whatever the window is
    /// actually showing: the Trash listing, or the folder an entry has just
    /// been restored into.
    fn after_trash_write(mut self: Pin<&mut Self>) {
        if self.rust().trash_active {
            self.as_mut().load_trash();
        } else {
            self.as_mut().refresh();
        }
    }
}
