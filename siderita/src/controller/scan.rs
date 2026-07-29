use core::pin::Pin;
use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use notify_debouncer_full::notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use siderita_core::{DirectorySnapshot, EntryKind, PublishOutcome, ScanResult, WatchState};
use siderita_qt::RowKind;

use super::qobject;
use super::{kind_key, row_subtitle, PendingNav};

struct FolderMetadata {
    total: usize,
    directories: usize,
    files: usize,
    hidden: usize,
    size: u64,
    modified: String,
    accessed: String,
    created: String,
}

impl FolderMetadata {
    fn from_snapshot(snapshot: &DirectorySnapshot) -> Self {
        let entries = snapshot.entries();
        Self {
            total: entries.len(),
            directories: entries
                .iter()
                .filter(|entry| entry.kind() == EntryKind::Directory)
                .count(),
            files: entries
                .iter()
                .filter(|entry| entry.kind() != EntryKind::Directory)
                .count(),
            hidden: entries.iter().filter(|entry| entry.is_hidden()).count(),
            size: entries
                .iter()
                .filter(|entry| entry.kind() != EntryKind::Directory)
                .map(|entry| entry.size())
                .sum(),
            modified: snapshot
                .modified()
                .map(crate::format::system_time)
                .unwrap_or_default(),
            accessed: snapshot
                .accessed()
                .map(crate::format::system_time)
                .unwrap_or_default(),
            created: snapshot
                .created()
                .map(crate::format::system_time)
                .unwrap_or_default(),
        }
    }
}

fn qml_count(value: usize) -> i32 {
    value.min(i32::MAX as usize) as i32
}

impl qobject::SideritaController {
    /// Rescans the current location without a history change (refresh, initial).
    pub(crate) fn request_scan(mut self: Pin<&mut Self>, destination: PathBuf) {
        self.as_mut().rust_mut().get_mut().pending_nav = None;
        self.as_mut().request_scan_inner(destination, false);
    }

    /// A background rescan (the filesystem watcher) that must not disturb the UI:
    /// it keeps the current list and selection on screen and never flashes the
    /// "Leyendo carpeta…" loading state — the new snapshot simply replaces the old
    /// when it lands. This is what keeps an actively-changing folder from
    /// flickering.
    pub(crate) fn refresh_quiet(mut self: Pin<&mut Self>) {
        let Some(location) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        self.as_mut().rust_mut().get_mut().pending_nav = None;
        self.as_mut().request_scan_inner(location, true);
    }

    /// Scans a navigation's destination and holds the history change back until
    /// it succeeds — so a failed navigation never strands the path bar on an
    /// unreadable directory. All of back / forward / up / home / activate / typed
    /// path go through here.
    pub(crate) fn request_nav_scan(mut self: Pin<&mut Self>, nav: PendingNav) {
        // Any explicit navigation leaves the Trash / Recientes locations
        // (no-ops otherwise).
        self.as_mut().exit_trash();
        self.as_mut().exit_recent();
        let destination = nav.destination().to_path_buf();
        self.as_mut().rust_mut().get_mut().pending_nav = Some(nav);
        self.as_mut().request_scan_inner(destination, false);
    }

    /// `quiet` = a background refresh (watcher): leave the list, selection and
    /// status untouched and let the fresh snapshot swap in on success.
    pub(crate) fn request_scan_inner(mut self: Pin<&mut Self>, destination: PathBuf, quiet: bool) {
        let request = match self
            .as_mut()
            .rust_mut()
            .get_mut()
            .coordinator
            .begin(&destination)
        {
            Ok(request) => request,
            Err(error) => {
                self.as_mut().rust_mut().get_mut().pending_nav = None;
                if !quiet {
                    self.as_mut().set_loading(false);
                    self.as_mut()
                        .set_error_text(QString::from(error.to_string().as_str()));
                }
                return;
            }
        };

        if !quiet {
            let display_path = destination.to_string_lossy();
            self.as_mut()
                .set_current_path(QString::from(display_path.as_ref()));
            self.as_mut().set_selected_token(QString::default());
            self.as_mut().set_error_text(QString::default());
            self.as_mut().set_op_error(QString::default());
            self.as_mut().set_loading(true);
            self.as_mut()
                .set_status_text(QString::from("Leyendo carpeta…"));
            self.as_mut().update_navigation_state();
        }

        let submitted = self
            .rust()
            .executor
            .as_ref()
            .ok_or("el ejecutor de escaneo no está iniciado")
            .and_then(|executor| {
                executor
                    .submit(request)
                    .map_err(|_| "el ejecutor de escaneo se detuvo")
            });

        if let Err(message) = submitted {
            self.as_mut().rollback_pending_nav();
            if !quiet {
                self.as_mut().set_loading(false);
                self.as_mut().set_error_text(QString::from(message));
            }
        }
    }

    pub(crate) fn handle_scan_result(mut self: Pin<&mut Self>, result: ScanResult) {
        match result {
            Ok(snapshot) => {
                let accepted = match self
                    .as_mut()
                    .rust_mut()
                    .get_mut()
                    .coordinator
                    .publish(snapshot)
                {
                    PublishOutcome::Accepted(snapshot) => Some(snapshot),
                    PublishOutcome::Stale(_) => None,
                };

                let Some(snapshot) = accepted else {
                    return;
                };

                let display_path = snapshot.location().to_string_lossy().into_owned();
                let location = snapshot.location().to_path_buf();

                // Commit the deferred navigation now that its scan succeeded —
                // but only if it is still the one we are waiting for.
                {
                    let state = self.as_mut().rust_mut();
                    let state = state.get_mut();
                    let commits = state
                        .pending_nav
                        .as_ref()
                        .is_some_and(|nav| nav.destination() == location);
                    if commits {
                        if let Some(nav) = state.pending_nav.take() {
                            nav.commit(&mut state.history);
                        }
                    }
                }

                self.as_mut().rust_mut().get_mut().snapshot = Some(snapshot);
                self.as_mut()
                    .set_current_path(QString::from(display_path.as_str()));
                self.as_mut().set_loading(false);
                self.as_mut().set_error_text(QString::default());
                self.as_mut().update_navigation_state();
                self.as_mut().update_watch(&location);
                self.as_mut().reproject();
                // After the projection, so a folder that remembers a different
                // sort re-projects once with it rather than twice on arrival.
                self.as_mut().apply_folder_view();
            }
            Err(error) => {
                let is_current = self
                    .as_mut()
                    .rust_mut()
                    .get_mut()
                    .coordinator
                    .publish_error(&error);
                if !is_current {
                    return;
                }

                let message = error.to_string();
                self.as_mut().rollback_pending_nav();
                self.as_mut().set_loading(false);
                self.as_mut()
                    .set_error_text(QString::from(message.as_str()));
                self.as_mut()
                    .set_status_text(QString::from("No se pudo leer la carpeta"));
            }
        }
    }

    pub(crate) fn reproject(mut self: Pin<&mut Self>) {
        // While search results occupy the content box, folder reprojections
        // (a watcher tick, a sort toggle) must not overwrite them; `close_search`
        // drops the flag first, then reprojects to restore the folder.
        if self.rust().virtual_rows() {
            return;
        }
        let projected = {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            let Some(snapshot) = state.snapshot.as_ref() else {
                return;
            };
            let metadata = FolderMetadata::from_snapshot(snapshot);
            state
                .adapter
                .adapt_projected(snapshot, &state.options)
                .map(|view| (view, metadata))
        };

        let (view, metadata) = match projected {
            Ok(projected) => projected,
            Err(error) => {
                self.as_mut()
                    .set_error_text(QString::from(error.to_string().as_str()));
                return;
            }
        };

        let names: QStringList = view
            .rows()
            .iter()
            .map(|row| QString::from(row.display_name()))
            .collect();
        // Parallel role columns for the native SideritaEntryModel.
        let tokens: QStringList = view
            .rows()
            .iter()
            .map(|row| QString::from(row.token().to_string().as_str()))
            .collect();
        let kinds: QStringList = view
            .rows()
            .iter()
            .map(|row| QString::from(kind_key(row.kind())))
            .collect();
        let subtitles: QStringList = view
            .rows()
            .iter()
            .map(|row| QString::from(row_subtitle(row).as_str()))
            .collect();
        let paths: QStringList = view
            .rows()
            .iter()
            .map(|row| QString::from(row.path().to_string_lossy().as_ref()))
            .collect();
        // A plain folder listing has no section headers.
        let sections: QStringList = view.rows().iter().map(|_| QString::default()).collect();
        // Per-column text for the details view. A folder has no meaningful entry
        // size in the listing (recursive size is a Properties action), so it
        // shows a dash.
        let sizes: QStringList = view
            .rows()
            .iter()
            .map(|row| {
                if row.kind() == RowKind::Directory {
                    QString::from("—")
                } else {
                    QString::from(crate::format::size(row.size()).as_str())
                }
            })
            .collect();
        let dates: QStringList = view
            .rows()
            .iter()
            .map(|row| {
                QString::from(
                    row.modified()
                        .map(crate::format::system_time)
                        .unwrap_or_default()
                        .as_str(),
                )
            })
            .collect();
        let visible = view.rows().len();
        let selected_is_visible = {
            let selected = self.selected_token().to_string();
            !selected.is_empty()
                && view
                    .rows()
                    .iter()
                    .any(|row| row.token().to_string() == selected)
        };

        // A hit opened from search asks us to select a specific path once its
        // folder lands (one-shot).
        let select_token = {
            let pending = self
                .as_mut()
                .rust_mut()
                .get_mut()
                .pending_select_path
                .take();
            pending.and_then(|path| {
                view.rows()
                    .iter()
                    .find(|row| row.path() == path.as_path())
                    .map(|row| row.token().to_string())
            })
        };

        self.as_mut().rust_mut().get_mut().view = Some(view);
        self.as_mut().set_entry_names(names.clone());
        if let Some(token) = select_token {
            self.as_mut()
                .set_selected_token(QString::from(token.as_str()));
        } else if !selected_is_visible {
            self.as_mut().set_selected_token(QString::default());
        }

        // The item count and per-item detail live in the sidebar info box now;
        // the bottom status line only carries transient state. Keep a filtered
        // "N de M" hint there, but stay blank when nothing is filtered out.
        let status = if visible == metadata.total {
            String::new()
        } else {
            format!("{visible} de {}", metadata.total)
        };
        self.as_mut()
            .set_status_text(QString::from(status.as_str()));

        // Stable folder metadata comes from the unfiltered snapshot, while the
        // visible count follows the current projection. This keeps the heading
        // truthful when hidden entries or a local filter are active.
        self.as_mut().set_folder_visible_count(qml_count(visible));
        self.as_mut()
            .set_folder_total_count(qml_count(metadata.total));
        self.as_mut()
            .set_folder_directory_count(qml_count(metadata.directories));
        self.as_mut()
            .set_folder_file_count(qml_count(metadata.files));
        self.as_mut()
            .set_folder_hidden_count(qml_count(metadata.hidden));
        let folder_size = crate::format::size(metadata.size);
        self.as_mut()
            .set_folder_size(QString::from(folder_size.as_str()));
        self.as_mut()
            .set_folder_modified(QString::from(metadata.modified.as_str()));
        self.as_mut()
            .set_folder_accessed(QString::from(metadata.accessed.as_str()));
        self.as_mut()
            .set_folder_created(QString::from(metadata.created.as_str()));

        // Hand the projected rows to the native model.
        self.as_mut().rows_ready(
            names, tokens, kinds, subtitles, paths, sections, sizes, dates,
        );
    }

    pub(crate) fn update_navigation_state(mut self: Pin<&mut Self>) {
        let history = &self.rust().history;
        let can_go_back = history.can_go_back();
        let can_go_forward = history.can_go_forward();
        let can_go_up = history.current().and_then(Path::parent).is_some();

        self.as_mut().set_can_go_back(can_go_back);
        self.as_mut().set_can_go_forward(can_go_forward);
        self.as_mut().set_can_go_up(can_go_up);
    }

    /// A deferred navigation failed (or could not be submitted): drop it and
    /// restore the path bar to where the history still is, so nothing is stranded
    /// on the unreadable destination.
    pub(crate) fn rollback_pending_nav(mut self: Pin<&mut Self>) {
        let previous_location = {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            let had_pending = state.pending_nav.take().is_some();
            had_pending
                .then(|| state.history.current().map(Path::to_path_buf))
                .flatten()
        };

        if let Some(previous_location) = previous_location {
            let display_path = previous_location.to_string_lossy();
            self.as_mut()
                .set_current_path(QString::from(display_path.as_ref()));
            self.as_mut().update_navigation_state();
        }
    }

    /// Creates the filesystem debouncer once. Its callback runs on the notify
    /// thread and only marshals a coalesced "something changed" back to the Qt
    /// thread — it never touches Qt state directly.
    pub(crate) fn ensure_debouncer(mut self: Pin<&mut Self>) {
        if self.rust().debouncer.is_some() {
            return;
        }
        let qt = self.qt_thread();
        let created = new_debouncer(
            std::time::Duration::from_millis(200),
            None,
            move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        // Ignore Access events (open/close/read) — our own scan
                        // opens the directory, which notify reports as IN_OPEN;
                        // reacting to that would loop scan → open → scan. Only a
                        // real content change (create/modify/remove/rename) counts.
                        let content_changed = events
                            .iter()
                            .any(|event| !matches!(event.event.kind, EventKind::Access(_)));
                        if content_changed {
                            let _ = qt.queue(
                                move |controller: Pin<&mut qobject::SideritaController>| {
                                    controller.on_fs_change(false);
                                },
                            );
                        }
                    }
                    Err(_errors) => {
                        let _ =
                            qt.queue(move |controller: Pin<&mut qobject::SideritaController>| {
                                controller.on_fs_change(true);
                            });
                    }
                }
            },
        );
        if let Ok(debouncer) = created {
            self.as_mut().rust_mut().get_mut().debouncer = Some(debouncer);
        }
    }

    /// Points the watch at `location`: a rescan of the already-watched folder
    /// just marks the snapshot fresh again; a new folder moves the (non-recursive)
    /// watch there. Called after every successful scan.
    pub(crate) fn update_watch(mut self: Pin<&mut Self>, location: &Path) {
        if self.rust().watched.as_deref() == Some(location) {
            if let Some(watch) = self.as_mut().rust_mut().get_mut().watch.as_mut() {
                watch.mark_rescanned(location);
            }
            return;
        }

        self.as_mut().ensure_debouncer();

        let established = {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            let Some(debouncer) = state.debouncer.as_mut() else {
                return;
            };
            if let Some(old) = state.watched.take() {
                let _ = debouncer.unwatch(&old);
            }
            match debouncer.watch(location, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    state.watched = Some(location.to_path_buf());
                    state.watch = Some(WatchState::active(location));
                    true
                }
                Err(_) => {
                    state.watched = None;
                    state.watch = None;
                    false
                }
            }
        };
        self.as_mut().set_watch_degraded(!established);
    }

    /// A coalesced filesystem change (or watcher error) arrived for the watched
    /// folder: invalidate the snapshot and let a fresh rescan win.
    pub(crate) fn on_fs_change(mut self: Pin<&mut Self>, degraded: bool) {
        let Some(watched) = self.rust().watched.clone() else {
            return;
        };
        let became_stale = {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            let Some(watch) = state.watch.as_mut() else {
                return;
            };
            if degraded {
                watch.degrade(&watched, "se perdió la vigilancia de la carpeta")
            } else {
                watch.observe_change(&watched)
            }
        };
        if degraded {
            self.as_mut().set_watch_degraded(true);
        }
        if became_stale {
            // Quiet: a watched folder changing must never flash the loading
            // state or clear the list — it just swaps in the fresh snapshot.
            self.as_mut().refresh_quiet();
        }
    }
}
