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
            Ok(request) => {
                // Remembered for the answer: only the scan in flight can be
                // published, so this is the one `handle_scan_result` will hear
                // from, and a quiet one may not write a banner.
                self.as_mut().rust_mut().get_mut().quiet_scan = quiet;
                request
            }
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
            self.as_mut().publish_location(&destination);
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
                self.as_mut().publish_location(&location);
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

                // A background watcher refresh that failed says nothing. The
                // folder on screen is the last one that read correctly, the user
                // asked for nothing, and the usual cause is the very change that
                // triggered the rescan — turning that into a banner made an
                // active download flicker "No se pudo leer la carpeta".
                let quiet = self.rust().quiet_scan;
                let message = error.to_string();
                self.as_mut().rollback_pending_nav();
                if quiet {
                    return;
                }
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
        // Identity, not text: a row's path crosses as its key (ADR 0008), so a
        // name that is not valid UTF-8 can still be opened, renamed or trashed.
        // What a person reads is `names` / `subtitles`, published beside it.
        let paths: QStringList = view
            .rows()
            .iter()
            .map(|row| crate::pathkey::publish(row.path()))
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

    /// Publishes the folder being shown twice over, as ADR 0008 requires: the
    /// lossy text a person reads, and the key every verb and every navigation
    /// hands back.
    pub(crate) fn publish_location(mut self: Pin<&mut Self>, location: &Path) {
        let display = location.to_string_lossy().into_owned();
        self.as_mut()
            .set_current_path(QString::from(display.as_str()));
        self.as_mut()
            .set_current_path_key(crate::pathkey::publish(location));
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
            self.as_mut().publish_location(&previous_location);
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
        // A running paste/move is itself the source of these writes, and it
        // already does its own refresh() when it finishes (finish_batch). Quiet
        // rescans in the meantime reset the entry model (beginResetModel), which
        // tears down every delegate — killing the in-flight right-click gesture
        // and starving the progress panel's own queued Qt-thread updates for
        // nothing, since the batch's own refresh will show the final state.
        if !degraded && *self.op_running() {
            return;
        }
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

#[cfg(test)]
mod tests {
    //! The `SID-A2` acceptance path, exercised without Qt: everything below the
    //! invokables is ordinary Rust, and it is where the bytes were being lost.

    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;
    use siderita_core::{scan_directory, ScanCoordinator, ViewOptions};
    use siderita_qt::SnapshotAdapter;

    use crate::pathkey;

    /// A temporary directory holding one file whose name is not valid UTF-8 —
    /// the fixture `siderita-core` already scans for, carried up to the seam.
    struct Fixture(PathBuf);

    impl Fixture {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "siderita-seam-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create fixture directory");
            Self(path)
        }

        fn write_non_utf8(&self) -> PathBuf {
            let name = OsString::from_vec(b"na\xffme".to_vec());
            let file = self.0.join(name);
            fs::write(&file, b"content").expect("write fixture file");
            file
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn rows_of(directory: &PathBuf) -> Vec<(String, String)> {
        let mut coordinator = ScanCoordinator::new();
        let request = coordinator.begin(directory).expect("issue scan request");
        let snapshot = scan_directory(&request).expect("scan the fixture");
        let mut adapter = SnapshotAdapter::new();
        let view = adapter
            .adapt_projected(&snapshot, &ViewOptions::default())
            .expect("project the snapshot");
        view.rows()
            .iter()
            .map(|row| (row.display_name().to_owned(), pathkey::encode(row.path())))
            .collect()
    }

    #[test]
    fn a_non_utf8_name_is_listed_and_its_key_round_trips_byte_for_byte() {
        let fixture = Fixture::new("list");
        let file = fixture.write_non_utf8();

        let rows = rows_of(&fixture.0);
        assert_eq!(rows.len(), 1, "the entry is listed: {rows:?}");
        let (name, key) = &rows[0];
        // What a person reads still carries the replacement character…
        assert_eq!(name, "na\u{fffd}me");
        // …and what comes back to Rust is the file itself, byte for byte.
        assert_eq!(pathkey::decode_str(key), Ok(file));
    }

    #[test]
    fn a_non_utf8_entry_can_be_renamed_through_its_key() {
        let fixture = Fixture::new("rename");
        fixture.write_non_utf8();
        let key = rows_of(&fixture.0)[0].1.clone();

        // Exactly what `rename_path` does once its argument is accepted.
        let path = pathkey::decode_str(&key).expect("the published key decodes");
        siderita_ops::rename(&path, OsStr::new("renombrado"), &CancellationToken::new())
            .expect("rename the entry the key names");

        assert!(fixture.0.join("renombrado").exists());
        assert!(!path.exists(), "the original name is gone");
    }

    #[test]
    fn a_non_utf8_entry_can_be_trashed_through_its_key() {
        let fixture = Fixture::new("trash");
        fixture.write_non_utf8();
        let key = rows_of(&fixture.0)[0].1.clone();

        let path = pathkey::decode_str(&key).expect("the published key decodes");
        let trashed = siderita_ops::trash(&path, &CancellationToken::new(), &mut |_| {})
            .expect("trash the entry the key names");

        assert!(!path.exists(), "the entry left its folder");
        assert!(trashed.info.exists(), "and left a .trashinfo behind");
        // Put the Trash back the way it was found.
        let _ = siderita_ops::purge_from_trash(&trashed.info);
    }

    #[test]
    fn a_key_the_seam_did_not_produce_is_refused_without_panicking() {
        for bad in ["/tmp/bad%2", "/tmp/bad%zz", "relative", ""] {
            assert!(
                pathkey::decode_str(bad).is_err(),
                "'{bad}' must be refused rather than salvaged"
            );
        }
        // And the refusal is typed, so a caller can say why.
        let refusal = pathkey::decode_str("/tmp/bad%2").expect_err("malformed");
        assert_eq!(refusal, pathkey::KeyError::Malformed);
    }

    #[test]
    fn an_ordinary_name_keys_to_the_spelling_the_uri_codec_uses() {
        // The drag payload is `file://` + the key, so the two must agree.
        let path = PathBuf::from(OsStr::from_bytes(b"/home/u/informe#3.pdf"));
        assert_eq!(
            format!("file://{}", pathkey::encode(&path)),
            crate::dbus::path_to_uri(&path)
        );
    }
}
