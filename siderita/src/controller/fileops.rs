//! Loss-free file write operations: the clipboard (copy/cut/paste with conflict
//! resolution), drag-and-drop moves/copies, undo, and the direct creations,
//! renames and trashings the menus trigger. Every write follows the suite's
//! zero-loss rule — verify the destination before removing any source, roll back
//! partial destinations on failure or cancellation — and long copies run on a
//! worker thread with cancellable progress.
use core::pin::Pin;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use siderita_ops::{OpError, Progress};

use super::qobject;
use super::{display_name, ConflictStrategy, PasteOutcome, PendingPaste, UndoAction};
use crate::pathkey;

/// The paths behind the system clipboard's `file://` URIs, skipping anything
/// that does not decode to one so a stray entry never becomes a filesystem
/// operation on `""`.
///
/// Not keys: this list comes from Qt's own clipboard, shared with every other
/// application on the desktop, and what the desktop exchanges there is a
/// percent-encoded URI. It is decoded here, byte by byte, by the same codec
/// that wrote it — so a name that is not valid UTF-8 survives a copy to another
/// application and back, which is precisely what the older path-shaped seam
/// could not do.
///
/// A URI naming another host is not a local file and is skipped too: the
/// clipboard is shared with every application on the desktop, and
/// `file://otherhost/etc/passwd` used to become the local `/etc/passwd`.
fn clipboard_paths(list: &QStringList) -> Vec<PathBuf> {
    list.iter()
        .map(QString::to_string)
        .filter_map(|uri| celestina_core::file_uri::to_path(&uri).ok())
        .collect()
}

/// Announces the entry a job reached on the Qt thread, and answers the
/// throttled byte read-out for it: at most about one update per 60 ms, so a
/// large file animates without flooding the Qt event loop. Paste, trash and
/// undo report the same way.
fn entry_progress(
    qt: &cxx_qt::CxxQtThread<qobject::SideritaController>,
    job: u64,
    index: usize,
    path: &Path,
) -> impl FnMut(Progress) {
    let done = i32::try_from(index).unwrap_or(i32::MAX);
    let announced = display_name(path);
    let _ = qt.queue(move |controller| {
        controller.job_reached(job, done, Some(announced), Some(String::new()));
    });
    let qt = qt.clone();
    let mut last = std::time::Instant::now();
    move |progress: Progress| {
        if last.elapsed().as_millis() < 60 {
            return;
        }
        last = std::time::Instant::now();
        let detail = format!("{} copiados", crate::format::size(progress.bytes));
        let _ = qt.queue(move |controller| {
            controller.job_reached(job, done, None, Some(detail));
        });
    }
}

/// Applies an undo record on a worker as job `job`, announcing each entry and
/// the bytes a move back across disks copies, and answering one line per
/// entry that could not be put back.
fn reverse(
    action: UndoAction,
    cancellation: &CancellationToken,
    qt: &cxx_qt::CxxQtThread<qobject::SideritaController>,
    job: u64,
) -> Vec<String> {
    let mut failures = Vec::new();
    match action {
        UndoAction::Rename { renamed, old_name } => {
            let _ = entry_progress(qt, job, 0, &renamed);
            if let Err(error) = siderita_ops::rename(&renamed, old_name.as_os_str(), cancellation) {
                failures.push(format!("{}: {error}", display_name(&renamed)));
            }
        }
        UndoAction::Move { entries } => {
            for (index, (moved_to, original_parent)) in entries.iter().enumerate() {
                if cancellation.is_cancelled() {
                    break;
                }
                let mut on_progress = entry_progress(qt, job, index, moved_to);
                if let Err(error) = siderita_ops::move_entry(
                    moved_to,
                    original_parent,
                    cancellation,
                    &mut on_progress,
                ) {
                    failures.push(format!("{}: {error}", display_name(moved_to)));
                }
            }
        }
        UndoAction::Trash { infos } => {
            for (index, info) in infos.iter().enumerate() {
                if cancellation.is_cancelled() {
                    break;
                }
                // Restoring reports no bytes; the entry reached is announced.
                let _ = entry_progress(qt, job, index, info);
                match siderita_ops::restore_from_trash(info, cancellation) {
                    Ok(restored) if !restored.left_behind.is_empty() => {
                        failures.push(super::display::left_behind_line(
                            &restored.to,
                            restored.left_behind.len(),
                        ));
                    }
                    Ok(_) => {}
                    Err(error) => failures.push(format!("{}: {error}", display_name(info))),
                }
            }
        }
    }
    failures
}

impl qobject::SideritaController {
    /// Runs a quick write — a creation or a rename — on a worker, and `done`
    /// back on the Qt thread with its outcome.
    ///
    /// Quick is not free: one `mkdir` or `rename` on a phone that stopped
    /// answering, a sleeping share or a dying disk blocks for as long as that
    /// filesystem likes, and on the Qt thread that froze the window. The
    /// worker is kept in the job register, so quitting waits for it.
    fn write_off_thread<T: Send + 'static>(
        mut self: Pin<&mut Self>,
        work: impl FnOnce() -> T + Send + 'static,
        done: impl FnOnce(Pin<&mut Self>, T) + Send + 'static,
    ) {
        let qt = self.qt_thread();
        let started = super::jobs::spawn_worker(move || {
            let outcome = work();
            let _ = qt.queue(move |controller| done(controller, outcome));
        });
        if let Err(error) = started {
            self.as_mut()
                .set_op_error(QString::from(error.to_string().as_str()));
        }
    }

    pub fn new_folder(mut self: Pin<&mut Self>, name: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(parent) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        let name = name.to_string();
        self.write_off_thread(
            move || {
                siderita_ops::create_directory(
                    &parent,
                    OsStr::new(&name),
                    &CancellationToken::new(),
                )
                .map(|_| ())
            },
            |mut controller, outcome| {
                // Creating is not undoable; a success supersedes the last
                // undoable op.
                if outcome.is_ok() {
                    controller.as_mut().set_undo(None);
                }
                controller.finish_op(outcome);
            },
        );
    }

    pub fn new_file(mut self: Pin<&mut Self>, name: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(parent) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        let name = name.to_string();
        self.write_off_thread(
            move || {
                siderita_ops::create_file(&parent, OsStr::new(&name), &CancellationToken::new())
                    .map(|_| ())
            },
            |mut controller, outcome| {
                if outcome.is_ok() {
                    controller.as_mut().set_undo(None);
                }
                controller.finish_op(outcome);
            },
        );
    }

    /// Renames the entry `key` names. `new_name` is the text a person typed,
    /// not a key: the domain validates it and refuses a separator, `.`, `..`
    /// and NUL.
    pub fn rename_path(mut self: Pin<&mut Self>, key: &QString, new_name: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.as_mut().accept_key(key) else {
            return;
        };
        let new_name = new_name.to_string();
        self.write_off_thread(
            move || {
                siderita_ops::rename(&path, OsStr::new(&new_name), &CancellationToken::new()).map(
                    |renamed| {
                        path.file_name().map(|old_name| UndoAction::Rename {
                            renamed: renamed.to,
                            old_name: old_name.to_os_string(),
                        })
                    },
                )
            },
            |mut controller, outcome| {
                let outcome = outcome.map(|undo| {
                    controller.as_mut().set_undo(undo);
                });
                controller.finish_op(outcome);
            },
        );
    }

    /// Renames a whole selection in one pass: `paths[i]` becomes `names[i]`.
    /// Each rename is attempted independently and refuses to overwrite (the
    /// domain guarantees that), so a name that collides fails alone and is
    /// reported — nothing else in the batch is rolled back or lost.
    pub fn rename_paths(mut self: Pin<&mut Self>, keys: &QStringList, names: &QStringList) {
        self.as_mut().set_op_error(QString::default());
        let Some(paths) = self.as_mut().accept_keys(keys) else {
            return;
        };
        let names: Vec<String> = names.iter().map(ToString::to_string).collect();
        if paths.is_empty() || paths.len() != names.len() {
            return;
        }
        let total = paths.len();
        self.write_off_thread(
            move || {
                let cancellation = CancellationToken::new();
                let mut failures = Vec::new();
                for (path, name) in paths.iter().zip(names.iter()) {
                    // The name is validated by `siderita_ops::rename` (it
                    // refuses an empty name, a separator, `.`/`..` and NUL),
                    // exactly as for a single rename.
                    if let Err(error) = siderita_ops::rename(path, OsStr::new(name), &cancellation)
                    {
                        failures.push(format!("{}: {error}", display_name(path)));
                    }
                }
                failures
            },
            move |mut controller, failures| {
                // Deliberately no undo: a batch rename is many renames, and
                // the single undo slot can only honestly reverse one.
                controller.as_mut().set_undo(None);
                controller.finish_batch(total, &failures);
            },
        );
    }

    pub fn trash_path(mut self: Pin<&mut Self>, key: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.as_mut().accept_key(key) else {
            return;
        };
        self.as_mut().spawn_trash(vec![path]);
    }

    /// Sends every path in a multi-selection to Trash. Each entry is attempted
    /// independently; the view is refreshed once so successes appear, and any
    /// failures are reported together without hiding the ones that did land.
    pub fn trash_paths(mut self: Pin<&mut Self>, keys: &QStringList) {
        self.as_mut().set_op_error(QString::default());
        let Some(paths) = self.as_mut().accept_keys(keys) else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        self.as_mut().spawn_trash(paths);
    }

    /// Trashes `paths` on a worker thread. Same filesystem is a cheap rename, but
    /// an entry on another mount (an external drive without its own Trash yet)
    /// falls back to a full copy → verify → remove — that can take a while for a
    /// large file, and running it on the Qt thread would freeze all of Siderita
    /// for as long as it lasts. Reuses the paste operation's progress surface and
    /// cancellation, since it is the same shape of long write.
    ///
    /// Registered as its own job, so it runs alongside whatever else is
    /// writing: each job carries its own cancellation and its own counters.
    /// Two writers aiming at one name do not overwrite each other: the Trash
    /// reserves its record name and places the body with a no-replace rename,
    /// so the loser is told the name is taken.
    fn spawn_trash(mut self: Pin<&mut Self>, paths: Vec<PathBuf>) {
        if *self.conflict_pending() {
            return;
        }
        let (job, token) = self.as_mut().start_job(
            "Enviando a la papelera…",
            super::jobs::JobKind::Trash,
            paths.len(),
        );

        let qt = self.qt_thread();
        self.as_mut().run_job(job, move || {
            let total = paths.len();
            let mut failures = Vec::new();
            let mut infos = Vec::new();

            for (index, path) in paths.iter().enumerate() {
                if token.is_cancelled() {
                    break;
                }

                let mut on_progress = entry_progress(&qt, job, index, path);

                match siderita_ops::trash(path, &token, &mut on_progress) {
                    Ok(trashed) => {
                        // Trashing into another disk's Trash copies; what
                        // arrived or changed meanwhile stayed, and is said.
                        if !trashed.left_behind.is_empty() {
                            failures.push(super::display::left_behind_line(
                                path,
                                trashed.left_behind.len(),
                            ));
                        }
                        infos.push(trashed.info);
                    }
                    Err(error) => failures.push(format!("{}: {error}", display_name(path))),
                }
            }

            let cancelled = token.is_cancelled();
            let _ = qt.queue(move |controller| {
                controller.finish_trash(job, total, infos, failures, cancelled);
            });
            super::jobs::JobEnd::Done
        });
    }

    /// Finalises a trashed batch back on the Qt thread, mirroring `finish_paste`.
    fn finish_trash(
        mut self: Pin<&mut Self>,
        job: u64,
        total: usize,
        infos: Vec<PathBuf>,
        failures: Vec<String>,
        cancelled: bool,
    ) {
        self.as_mut().end_job(job);

        if !infos.is_empty() {
            self.as_mut().set_undo(Some(UndoAction::Trash { infos }));
        }
        self.as_mut().finish_batch(total, &failures);
        if failures.is_empty() && cancelled {
            self.as_mut().notice_cancelled();
        }
    }

    pub fn copy_to_clipboard(mut self: Pin<&mut Self>, key: &QString, cut: bool) {
        let Some(path) = self.as_mut().accept_key(key) else {
            return;
        };
        self.as_mut().set_clipboard(vec![path], cut);
    }

    /// Loads a multi-selection into the internal clipboard for a later paste,
    /// as either a copy (`cut = false`) or a move (`cut = true`).
    pub fn copy_paths_to_clipboard(mut self: Pin<&mut Self>, keys: &QStringList, cut: bool) {
        let Some(paths) = self.as_mut().accept_keys(keys) else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        self.as_mut().set_clipboard(paths, cut);
    }

    pub(crate) fn set_clipboard(mut self: Pin<&mut Self>, paths: Vec<PathBuf>, cut: bool) {
        // Publish to the system clipboard too, so other file managers can paste
        // what Siderita copied or cut (text/uri-list + gnome-copied-files).
        // That shim speaks `file://` URIs to the rest of the desktop, and they
        // are built here by the same byte-exact codec the drag payload and the
        // portal answers use — one spelling, so a name that is not valid UTF-8
        // reaches the other application intact. The view's ghosting list is
        // keyed instead, since it is compared against the keys the rows
        // publish.
        let uris: QStringList = paths
            .iter()
            .filter_map(|path| celestina_core::file_uri::from_path(path))
            .map(|uri| QString::from(uri.as_str()))
            .collect();
        qobject::system_clipboard_set_uris(&uris, cut);
        let keys: QStringList = paths.iter().map(|path| pathkey::publish(path)).collect();
        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.clipboard = paths;
            state.clipboard_cut = cut;
        }
        self.as_mut().set_can_paste(true);
        self.as_mut().set_op_error(QString::default());
        // A cut marks its sources for a ghosted style in the view; a copy leaves
        // no such mark and clears any earlier one.
        self.as_mut()
            .set_cut_paths(if cut { keys } else { QStringList::default() });
    }

    /// Recomputes whether a paste is available from either clipboard. Called when
    /// the folder menu opens so "Pegar" also lights up for content another
    /// manager copied, without polling for clipboard changes.
    pub fn refresh_paste_state(mut self: Pin<&mut Self>) {
        let available = !self.rust().clipboard.is_empty() || qobject::system_clipboard_has_uris();
        self.as_mut().set_can_paste(available);
    }

    pub fn clear_clipboard(mut self: Pin<&mut Self>) {
        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.clipboard.clear();
            state.clipboard_cut = false;
        }
        self.as_mut().set_can_paste(false);
        self.as_mut().set_cut_paths(QStringList::default());
    }

    /// Pastes the clipboard into the current folder. If any entry's destination
    /// already exists the paste is held back and a conflict choice is requested
    /// (see `resolve_conflicts`); otherwise it starts straight away on a worker
    /// thread. A paste is refused while one is running or a conflict is pending.
    pub fn paste(mut self: Pin<&mut Self>) {
        if *self.conflict_pending() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(destination) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };

        // The system clipboard is the source of truth shared with other managers;
        // fall back to the internal one only when the system clipboard holds no
        // file URIs (e.g. it is unavailable).
        let (sources, cut) = if qobject::system_clipboard_has_uris() {
            (
                clipboard_paths(&qobject::system_clipboard_read_uris()),
                qobject::system_clipboard_is_cut(),
            )
        } else {
            (self.rust().clipboard.clone(), self.rust().clipboard_cut)
        };
        self.begin_paste(sources, destination, cut);
    }

    /// Moves or copies dropped file URIs into `destination` (or the current
    /// folder when it is empty) — the drag-and-drop entry point, sharing the same
    /// conflict-detection and worker as paste. `move_entries` chooses move vs copy.
    pub fn drop_uris(
        mut self: Pin<&mut Self>,
        keys: &QStringList,
        destination: &QString,
        move_entries: bool,
    ) {
        let Some(sources) = self.as_mut().accept_keys(keys) else {
            return;
        };
        self.as_mut().drop_paths(sources, destination, move_entries);
    }

    /// The same drop, given the raw `text/uri-list` the drag carried.
    ///
    /// Decoding belongs here rather than in QML: `decodeURIComponent` throws on
    /// any `%XX` sequence that is not valid UTF-8 — which is exactly how another
    /// file manager spells a non-UTF-8 name — and one such URI in a batch used
    /// to abort the whole drop with nothing said. The shared byte-wise codec has
    /// no such failure mode, and a URI it cannot make a path of is skipped
    /// alone. QML does not decide what a dropped URI means.
    pub fn drop_uri_list(
        mut self: Pin<&mut Self>,
        uris: &QStringList,
        destination: &QString,
        move_entries: bool,
    ) {
        let sources: Vec<PathBuf> = uris
            .iter()
            .map(QString::to_string)
            .filter_map(|uri| celestina_core::file_uri::to_path(&uri).ok())
            .collect();
        self.as_mut().drop_paths(sources, destination, move_entries);
    }

    fn drop_paths(
        mut self: Pin<&mut Self>,
        sources: Vec<PathBuf>,
        destination: &QString,
        move_entries: bool,
    ) {
        if *self.conflict_pending() {
            return;
        }
        self.as_mut().set_op_error(QString::default());

        // An empty destination key means "the folder being shown" — a drop on
        // empty space, which has no row to name.
        let destination = if destination.is_empty() {
            self.rust().history.current().map(Path::to_path_buf)
        } else {
            self.as_mut().accept_key(destination)
        };
        let Some(destination) = destination else {
            return;
        };

        // A drop onto a folder that is itself one of the dragged entries, or into
        // the folder an entry already lives in, is a no-op rather than an error.
        let sources: Vec<PathBuf> = sources
            .into_iter()
            .filter(|source| {
                source != &destination && source.parent() != Some(destination.as_path())
            })
            .collect();

        self.begin_paste(sources, destination, move_entries);
    }

    /// Shared tail of paste / drop: refuse an empty set, then detect
    /// destination collisions up front — on a reader, because that is an
    /// `lstat` of every source and every destination name, any of which can
    /// sit on a mount that stopped answering — and hand the plan to
    /// `paste_planned`.
    pub(crate) fn begin_paste(
        mut self: Pin<&mut Self>,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        cut: bool,
    ) {
        if sources.is_empty() {
            return;
        }
        let qt = self.qt_thread();
        let started = super::jobs::spawn_reader(move || {
            // Sorted out before any write: free names, real collisions, and the
            // entry that would collide with itself (see `plan_paste`).
            let plan = super::paste::plan_paste(sources, &destination, cut);
            let _ = qt.queue(move |controller| controller.paste_planned(plan, destination, cut));
        });
        if let Err(error) = started {
            self.as_mut()
                .set_op_error(QString::from(error.to_string().as_str()));
        }
    }

    /// Starts a planned paste straight away, or holds it back for a conflict
    /// choice.
    fn paste_planned(
        mut self: Pin<&mut Self>,
        plan: super::paste::PastePlan,
        destination: PathBuf,
        cut: bool,
    ) {
        // A second paste planned while the first waits on its conflict
        // question must not replace it: that question is still on screen.
        if self.rust().pending_paste.is_some() && !plan.colliding.is_empty() {
            return;
        }
        if plan.sources.is_empty() {
            // Nothing left to write. When the whole paste was a cut into the
            // folder its entries already live in, that is not a reason to say
            // nothing: the person pressed Ctrl+V and the cut is still marked.
            // Settle it — same clipboard convention as a consumed move — and
            // report it, so the shortcut is never a silent no-op.
            if !plan.same_folder_cuts.is_empty() {
                self.as_mut().settle_same_folder_cut(&plan.same_folder_cuts);
            }
            return;
        }

        if plan.colliding.is_empty() {
            let strategies = plan
                .decisions
                .iter()
                .map(|choice| choice.unwrap_or(ConflictStrategy::Skip))
                .collect();
            self.as_mut()
                .spawn_paste(plan.sources, destination, cut, strategies);
            return;
        }

        self.as_mut().rust_mut().get_mut().pending_paste = Some(PendingPaste {
            sources: plan.sources,
            destination,
            cut,
            decisions: plan.decisions,
            colliding: plan.colliding,
            cursor: 0,
        });
        self.as_mut().publish_conflict();
    }

    /// Closes a paste that turned out to be nothing but cuts into the folder
    /// their entries already occupy: the ghost stops marking entries that are
    /// no longer going anywhere, and the status line explains the no-op.
    ///
    /// The system clipboard is only wiped while it still holds exactly these
    /// entries — it is a shared desktop resource, and another application's
    /// content is not ours to discard. Same rule as `finish_paste`.
    fn settle_same_folder_cut(mut self: Pin<&mut Self>, cuts: &[PathBuf]) {
        let held = clipboard_paths(&qobject::system_clipboard_read_uris());
        if super::paste::holds_exactly(&held, cuts) {
            qobject::system_clipboard_clear();
        }
        self.as_mut().clear_clipboard();
        self.as_mut().push_notice(
            super::display::same_folder_cut_status(cuts.len()),
            "info",
            super::notices::NoticeTone::Info,
            false,
        );
    }

    /// Shows the collision now being asked about — its name and how many are
    /// still undecided, this one included.
    pub(crate) fn publish_conflict(mut self: Pin<&mut Self>) {
        let (name, remaining) = match self.rust().pending_paste.as_ref() {
            Some(pending) => {
                let remaining = pending.colliding.len().saturating_sub(pending.cursor);
                let name = pending
                    .colliding
                    .get(pending.cursor)
                    .and_then(|index| pending.sources.get(*index))
                    .map(|source| display_name(source))
                    .unwrap_or_default();
                (name, remaining)
            }
            None => (String::new(), 0),
        };
        self.as_mut()
            .set_conflict_count(remaining.min(i32::MAX as usize) as i32);
        self.as_mut()
            .set_conflict_name(QString::from(name.as_str()));
        self.as_mut().set_conflict_pending(remaining > 0);
    }

    /// Applies the user's choice ("skip" / "replace" / "keepboth") to the
    /// collision being asked about — or, with `apply_to_all`, to every one that
    /// is left — and starts the paste once nothing is undecided.
    pub fn resolve_conflict(mut self: Pin<&mut Self>, strategy: &QString, apply_to_all: bool) {
        let Some(strategy) = ConflictStrategy::from_key(&strategy.to_string()) else {
            return;
        };
        {
            let Some(pending) = self.as_mut().rust_mut().get_mut().pending_paste.as_mut() else {
                return;
            };
            if apply_to_all {
                for position in pending.cursor..pending.colliding.len() {
                    let index = pending.colliding[position];
                    pending.decisions[index] = Some(strategy);
                }
                pending.cursor = pending.colliding.len();
            } else if let Some(index) = pending.colliding.get(pending.cursor).copied() {
                pending.decisions[index] = Some(strategy);
                pending.cursor += 1;
            }
        }

        let decided = self
            .rust()
            .pending_paste
            .as_ref()
            .is_some_and(|pending| pending.cursor >= pending.colliding.len());
        if !decided {
            self.as_mut().publish_conflict();
            return;
        }

        let Some(pending) = self.as_mut().rust_mut().get_mut().pending_paste.take() else {
            return;
        };
        self.as_mut().set_conflict_pending(false);
        let strategies = pending
            .decisions
            .iter()
            .map(|choice| choice.unwrap_or(ConflictStrategy::Skip))
            .collect();
        self.as_mut().spawn_paste(
            pending.sources,
            pending.destination,
            pending.cut,
            strategies,
        );
    }

    /// Dismisses a pending conflict without pasting anything.
    pub fn cancel_conflicts(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().pending_paste = None;
        self.as_mut().set_conflict_pending(false);
        self.as_mut().push_notice(
            "Pegado cancelado",
            "circle-stop",
            super::notices::NoticeTone::Info,
            false,
        );
    }

    /// Starts the paste worker with a decided conflict `strategy`. Copies and
    /// moves can be long, so the whole batch runs off the Qt thread: it publishes
    /// progress back and honours the cancellation token behind `cancel_op`, then
    /// finalises on the Qt thread via `finish_paste`.
    pub(crate) fn spawn_paste(
        mut self: Pin<&mut Self>,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        cut: bool,
        // One strategy per source, decided before the worker starts — so a
        // batch can skip one collision and replace the next.
        strategies: Vec<ConflictStrategy>,
    ) {
        let (job, token) = self.as_mut().start_job(
            if cut { "Moviendo…" } else { "Copiando…" },
            if cut {
                super::jobs::JobKind::Move
            } else {
                super::jobs::JobKind::Copy
            },
            sources.len(),
        );

        let qt = self.qt_thread();
        self.as_mut().run_job(job, move || {
            let mut outcome = PasteOutcome {
                total: sources.len(),
                sources: sources.clone(),
                failures: Vec::new(),
                unmoved: Vec::new(),
                undo_moves: Vec::new(),
                skipped: 0,
                conflict_touched: false,
                cancelled: false,
            };

            for (index, source) in sources.iter().enumerate() {
                if token.is_cancelled() {
                    break;
                }

                let mut on_progress = entry_progress(&qt, job, index, source);

                super::paste::paste_one(
                    source,
                    &destination,
                    cut,
                    strategies
                        .get(index)
                        .copied()
                        .unwrap_or(ConflictStrategy::Skip),
                    &token,
                    &mut on_progress,
                    &mut outcome,
                );
            }

            outcome.cancelled = token.is_cancelled();
            let _ = qt.queue(move |controller| {
                controller.finish_paste(job, cut, outcome);
            });
            super::jobs::JobEnd::Done
        });
    }

    /// Trips the running operation's cancellation token. The worker stops at the
    /// next check and finalises through `finish_paste`, so a cancelled cross-
    /// device move still leaves every source intact.
    pub fn cancel_op(mut self: Pin<&mut Self>) {
        // No notice: the ring being cancelled is still on screen and its
        // callout is where a person asked for this.
        self.as_mut().cancel_all_jobs();
    }

    /// Finalises a pasted batch back on the Qt thread: restores the idle state,
    /// settles the clipboard and undo record, refreshes the view and reports any
    /// per-entry failures (noting skips and part-way cancellation).
    pub(crate) fn finish_paste(
        mut self: Pin<&mut Self>,
        job: u64,
        cut: bool,
        outcome: PasteOutcome,
    ) {
        self.as_mut().end_job(job);

        if cut {
            if outcome.unmoved.is_empty() {
                // A fully-consumed cut clears both clipboards, matching the
                // convention other managers follow after a move-paste — but the
                // system clipboard is shared, so it is only wiped while it still
                // holds the very entries this move consumed. Another application
                // may have copied something during a long move, and that content
                // is not ours to discard.
                let held = clipboard_paths(&qobject::system_clipboard_read_uris());
                if super::paste::holds_exactly(&held, &outcome.sources) {
                    qobject::system_clipboard_clear();
                }
                self.as_mut().clear_clipboard();
            } else {
                self.as_mut().set_clipboard(outcome.unmoved, true);
            }
            // A batch that replaced or kept-both is too tangled to reverse in one
            // step; only a clean set of plain moves offers undo.
            if !outcome.conflict_touched && !outcome.undo_moves.is_empty() {
                self.as_mut().set_undo(Some(UndoAction::Move {
                    entries: outcome.undo_moves,
                }));
            } else {
                self.as_mut().set_undo(None);
            }
        } else if outcome.failures.len() < outcome.total {
            self.as_mut().set_undo(None);
        }

        self.as_mut().finish_batch(outcome.total, &outcome.failures);
        if outcome.failures.is_empty() {
            if outcome.cancelled {
                self.as_mut().notice_cancelled();
            } else if outcome.skipped > 0 {
                let message = format!("{} omitidos", outcome.skipped);
                self.as_mut().push_notice(
                    message.as_str(),
                    "info",
                    super::notices::NoticeTone::Info,
                    false,
                );
            }
        }
    }

    /// Reverses the last undoable operation (rename / move / trash). Single
    /// level: the action is consumed, and like a batch write the view refreshes
    /// once and any per-entry failures are reported together.
    ///
    /// It runs as a job. Undoing a move to another disk copies everything back,
    /// and undoing a trash from another disk's Trash does too; on the Qt thread
    /// either froze the window for as long as it took, with no progress and no
    /// Cancel.
    pub fn undo(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let Some(action) = self.as_mut().rust_mut().get_mut().last_undo.take() else {
            return;
        };
        self.as_mut().set_undo(None);

        let total = match &action {
            UndoAction::Rename { .. } => 1,
            UndoAction::Move { entries } => entries.len(),
            UndoAction::Trash { infos } => infos.len(),
        };
        let (job, token) =
            self.as_mut()
                .start_job(action.label(), super::jobs::JobKind::Undo, total);
        let qt = self.qt_thread();
        self.as_mut().run_job(job, move || {
            let failures = reverse(action, &token, &qt, job);
            let cancelled = token.is_cancelled();
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().end_job(job);
                controller.as_mut().finish_batch(total, &failures);
                if failures.is_empty() && cancelled {
                    controller.as_mut().notice_cancelled();
                }
            });
            super::jobs::JobEnd::Done
        });
    }

    /// Records (or clears) how to reverse the last operation, keeping the
    /// `can_undo` / `undo_label` properties in step for the menu and shortcut.
    pub(crate) fn set_undo(mut self: Pin<&mut Self>, action: Option<UndoAction>) {
        let (can_undo, label) = match &action {
            Some(action) => (true, QString::from(action.label())),
            None => (false, QString::default()),
        };
        self.as_mut().rust_mut().get_mut().last_undo = action;
        self.as_mut().set_can_undo(can_undo);
        self.as_mut().set_undo_label(label);
    }

    /// After a write: refresh the view on success, or surface the error on
    /// failure without letting the async rescan wipe it.
    pub(crate) fn finish_op(mut self: Pin<&mut Self>, outcome: Result<(), OpError>) {
        match outcome {
            Ok(()) => self.as_mut().refresh(),
            Err(error) => self
                .as_mut()
                .set_op_error(QString::from(error.to_string().as_str())),
        }
    }

    /// After a batch write: always refresh (a partial success still changed the
    /// directory), then surface any per-entry failures together. `refresh`
    /// clears `op_error` for the new scan, so the error is set last and survives
    /// until the next operation or navigation.
    pub(crate) fn finish_batch(mut self: Pin<&mut Self>, total: usize, failures: &[String]) {
        self.as_mut().refresh();
        if failures.is_empty() {
            return;
        }
        let summary = if failures.len() == total {
            failures.join("\n")
        } else {
            format!(
                "{} de {} operaciones fallaron:\n{}",
                failures.len(),
                total,
                failures.join("\n")
            )
        };
        self.as_mut().set_op_error(QString::from(summary.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::clipboard_paths;
    use celestina_core::file_uri::from_path;
    use cxx_qt_lib::{QString, QStringList};
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    /// What the system clipboard would hand back for these paths, once the C++
    /// shim has published them and read them again: the URIs, unchanged, since
    /// `QUrl` carries percent-encoded bytes verbatim.
    fn round_trip(paths: &[PathBuf]) -> Vec<PathBuf> {
        let published: QStringList = paths
            .iter()
            .filter_map(|path| from_path(path))
            .map(|uri| QString::from(uri.as_str()))
            .collect();
        clipboard_paths(&published)
    }

    #[test]
    fn a_name_that_is_not_utf8_survives_the_system_clipboard_byte_for_byte() {
        // A byte fixture, not text, so the language contract does not apply.
        let path = PathBuf::from(OsString::from_vec(b"/tmp/na\xffme".to_vec()));
        assert_eq!(round_trip(std::slice::from_ref(&path)), vec![path]);
    }

    #[test]
    fn the_names_a_uri_used_to_truncate_survive_too() {
        let paths: Vec<PathBuf> = ["/tmp/informe#3.pdf", "/tmp/a b.txt", "/tmp/q?.txt"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(round_trip(&paths), paths);
    }

    #[test]
    fn a_clipboard_entry_that_is_not_a_local_file_uri_is_skipped() {
        let held: QStringList = ["", "http://example.com/x", "file:///tmp/nota.txt"]
            .iter()
            .map(|value| QString::from(*value))
            .collect();
        assert_eq!(clipboard_paths(&held), vec![PathBuf::from("/tmp/nota.txt")]);
    }

    /// SID-18: another host's file is not a local file. The shared clipboard
    /// used to turn `file://otherhost/etc/passwd` into the local `/etc/passwd`,
    /// and a query or fragment into part of the name.
    #[test]
    fn a_clipboard_uri_naming_another_host_is_skipped() {
        let held: QStringList = [
            "file://otherhost/etc/passwd",
            "file:///tmp/a?b",
            "file:///tmp/a#b",
            "file://localhost/tmp/nota.txt",
        ]
        .iter()
        .map(|value| QString::from(*value))
        .collect();
        assert_eq!(clipboard_paths(&held), vec![PathBuf::from("/tmp/nota.txt")]);
    }
}
