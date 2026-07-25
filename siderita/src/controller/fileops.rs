use core::pin::Pin;
use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use celestina_core::CancellationToken;
use siderita_ops::{OpError, Progress};

use super::qobject;
use super::{
    display_name, format_size, paste_one, qstringlist_to_paths, ConflictStrategy,
    PasteOutcome, PendingPaste, UndoAction,
};

impl qobject::SideritaController {
    pub fn copy_to_clipboard(mut self: Pin<&mut Self>, path: &QString, cut: bool) {
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        self.as_mut().set_clipboard(vec![PathBuf::from(path)], cut);
    }

    /// Loads a multi-selection into the internal clipboard for a later paste,
    /// as either a copy (`cut = false`) or a move (`cut = true`).
    pub fn copy_paths_to_clipboard(mut self: Pin<&mut Self>, paths: &QStringList, cut: bool) {
        let paths = qstringlist_to_paths(paths);
        if paths.is_empty() {
            return;
        }
        self.as_mut().set_clipboard(paths, cut);
    }

    pub(crate) fn set_clipboard(mut self: Pin<&mut Self>, paths: Vec<PathBuf>, cut: bool) {
        // Publish to the system clipboard too, so other file managers can paste
        // what Siderita copied or cut (text/uri-list + gnome-copied-files).
        let uris: QStringList = paths
            .iter()
            .map(|path| QString::from(path.to_string_lossy().as_ref()))
            .collect();
        qobject::system_clipboard_set_uris(&uris, cut);
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
            .set_cut_paths(if cut { uris } else { QStringList::default() });
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
        if *self.op_running() || *self.conflict_pending() {
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
                qstringlist_to_paths(&qobject::system_clipboard_read_uris()),
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
        paths: &QStringList,
        destination: &QString,
        move_entries: bool,
    ) {
        if *self.op_running() || *self.conflict_pending() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let sources = qstringlist_to_paths(paths);

        let destination = destination.to_string();
        let destination = if destination.is_empty() {
            self.rust().history.current().map(Path::to_path_buf)
        } else {
            Some(PathBuf::from(destination))
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

    /// Shared tail of paste / drop: refuse an empty set, detect destination
    /// collisions up front (on the Qt thread), and either start the worker
    /// straight away or hold the batch back for a conflict choice.
    pub(crate) fn begin_paste(
        mut self: Pin<&mut Self>,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        cut: bool,
    ) {
        if sources.is_empty() {
            return;
        }

        let colliding: Vec<usize> = sources
            .iter()
            .enumerate()
            .filter(|(_, source)| {
                source
                    .file_name()
                    .map(|name| destination.join(name))
                    .is_some_and(|target| std::fs::symlink_metadata(target).is_ok())
            })
            .map(|(index, _)| index)
            .collect();

        if colliding.is_empty() {
            let strategies = vec![ConflictStrategy::Skip; sources.len()];
            self.as_mut()
                .spawn_paste(sources, destination, cut, strategies);
            return;
        }

        let decisions = vec![None; sources.len()];
        self.as_mut().rust_mut().get_mut().pending_paste = Some(PendingPaste {
            sources,
            destination,
            cut,
            decisions,
            colliding,
            cursor: 0,
        });
        self.as_mut().publish_conflict();
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
        self.as_mut()
            .spawn_paste(pending.sources, pending.destination, pending.cut, strategies);
    }

    /// Dismisses a pending conflict without pasting anything.
    pub fn cancel_conflicts(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().pending_paste = None;
        self.as_mut().set_conflict_pending(false);
        self.as_mut()
            .set_status_text(QString::from("Pegado cancelado"));
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
        let token = CancellationToken::new();
        self.as_mut().rust_mut().get_mut().op_cancel = Some(token.clone());
        self.as_mut().set_op_running(true);
        self.as_mut()
            .set_op_total(sources.len().min(i32::MAX as usize) as i32);
        self.as_mut().set_op_done(0);
        self.as_mut().set_op_current(QString::default());
        self.as_mut().set_op_detail(QString::default());
        self.as_mut().set_status_text(QString::from(if cut {
            "Moviendo…"
        } else {
            "Copiando…"
        }));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let mut outcome = PasteOutcome {
                total: sources.len(),
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

                let name = display_name(source);
                let done = index as i32;
                let announced = name.clone();
                let _ = qt.queue(move |mut controller| {
                    controller.as_mut().set_op_done(done);
                    controller
                        .as_mut()
                        .set_op_current(QString::from(announced.as_str()));
                    controller.as_mut().set_op_detail(QString::default());
                });

                // Throttled byte progress: at most ~one update per 60 ms, so a
                // large file animates without flooding the Qt event loop.
                let qt_progress = qt.clone();
                let mut last = std::time::Instant::now();
                let mut on_progress = move |progress: Progress| {
                    if last.elapsed().as_millis() < 60 {
                        return;
                    }
                    last = std::time::Instant::now();
                    let detail = format!("{} copiados", format_size(progress.bytes));
                    let _ = qt_progress.queue(move |mut controller| {
                        controller
                            .as_mut()
                            .set_op_detail(QString::from(detail.as_str()));
                    });
                };

                paste_one(
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
                controller.finish_paste(cut, outcome);
            });
        });
    }

    /// Trips the running operation's cancellation token. The worker stops at the
    /// next check and finalises through `finish_paste`, so a cancelled cross-
    /// device move still leaves every source intact.
    pub fn cancel_op(mut self: Pin<&mut Self>) {
        if let Some(token) = self.as_mut().rust_mut().get_mut().op_cancel.as_ref() {
            token.cancel();
        }
        self.as_mut().set_status_text(QString::from("Cancelando…"));
    }

    /// Finalises a pasted batch back on the Qt thread: restores the idle state,
    /// settles the clipboard and undo record, refreshes the view and reports any
    /// per-entry failures (noting skips and part-way cancellation).
    pub(crate) fn finish_paste(mut self: Pin<&mut Self>, cut: bool, outcome: PasteOutcome) {
        self.as_mut().set_op_running(false);
        self.as_mut().rust_mut().get_mut().op_cancel = None;
        self.as_mut().set_op_current(QString::default());
        self.as_mut().set_op_detail(QString::default());
        self.as_mut().set_op_done(0);
        self.as_mut().set_op_total(0);

        if cut {
            if outcome.unmoved.is_empty() {
                // A fully-consumed cut clears both clipboards, matching the
                // convention other managers follow after a move-paste.
                qobject::system_clipboard_clear();
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
                self.as_mut()
                    .set_status_text(QString::from("Operación cancelada"));
            } else if outcome.skipped > 0 {
                let message = format!("{} omitidos", outcome.skipped);
                self.as_mut()
                    .set_status_text(QString::from(message.as_str()));
            }
        }
    }

    /// Reverses the last undoable operation (rename / move / trash). Single
    /// level: the action is consumed, and like a batch write the view refreshes
    /// once and any per-entry failures are reported together.
    pub fn undo(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let Some(action) = self.as_mut().rust_mut().get_mut().last_undo.take() else {
            return;
        };
        self.as_mut().set_undo(None);

        let cancellation = CancellationToken::new();
        let mut failures = Vec::new();
        let total = match &action {
            UndoAction::Rename { .. } => 1,
            UndoAction::Move { entries } => entries.len(),
            UndoAction::Trash { infos } => infos.len(),
        };

        match action {
            UndoAction::Rename { renamed, old_name } => {
                if let Err(error) =
                    siderita_ops::rename(&renamed, old_name.as_os_str(), &cancellation)
                {
                    failures.push(format!("{}: {error}", display_name(&renamed)));
                }
            }
            UndoAction::Move { entries } => {
                for (moved_to, original_parent) in &entries {
                    if let Err(error) = siderita_ops::move_entry(
                        moved_to,
                        original_parent,
                        &cancellation,
                        &mut |_| {},
                    ) {
                        failures.push(format!("{}: {error}", display_name(moved_to)));
                    }
                }
            }
            UndoAction::Trash { infos } => {
                for info in &infos {
                    if let Err(error) = siderita_ops::restore_from_trash(info, &cancellation) {
                        failures.push(format!("{}: {error}", display_name(info)));
                    }
                }
            }
        }

        self.as_mut().finish_batch(total, &failures);
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
