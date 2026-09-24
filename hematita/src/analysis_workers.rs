//! Starts the storage hub's workers on the analysed side and applies what
//! they answer: the scan, the content check, the selection, the three
//! actions and the graft scans of what a stopped deletion left behind.
//!
//! A child of the bridge module, so it reads the hub's fields directly; the
//! rules about ids live in [`crate::analysis_session`], and every result is
//! checked against the generation (and the confirm or action epoch) it was
//! asked under before it changes anything.

use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use hematita_core::usage::duplicates::Verified;
use hematita_core::usage::tree::{NodeId, Tree};
use hematita_core::usage::walk::{Progress, ScanError};

use super::{qobject, Mode, FAILED, REFUSED};
use crate::actions::{self, ActionKind, ActionReport, Item};
use crate::analysis_session::bytes;
use crate::analysis_view::Findings;
use crate::lists::doubles;
use crate::publish;
use crate::usage_worker::{self, WorkerHandle};

impl qobject::HematitaAnalysis {
    pub fn scan_here(mut self: Pin<&mut Self>) {
        if !matches!(self.rust().state, Mode::Browsing | Mode::Analysed) {
            return;
        }
        let Some(root) = self.rust().stack.last().cloned() else {
            return;
        };
        // Cancels the content check, the action and every graft scan.
        self.as_mut().rust_mut().session.reset();
        self.as_mut().set_busy(false);
        let generation = self.as_mut().rust_mut().next_generation();
        self.as_mut().rust_mut().state = Mode::Scanning;
        self.as_mut().set_progress_files(0.0);
        self.as_mut().set_progress_bytes(0.0);
        self.as_mut()
            .set_progress_path(QString::from(root.to_string_lossy().as_ref()));
        self.as_mut().set_action_outcome(QString::default());
        self.as_mut().publish();
        let qt = self.qt_thread();
        match usage_worker::spawn_scan(root, generation, qt) {
            Ok(handle) => self.as_mut().rust_mut().scan = Some(handle),
            Err(_) => {
                self.as_mut().set_start_failed(true);
                self.browse();
            }
        }
    }

    pub(crate) fn apply_progress(mut self: Pin<&mut Self>, generation: u64, progress: &Progress) {
        if !publish::still_current(generation, self.rust().generation)
            || self.rust().state != Mode::Scanning
        {
            return;
        }
        self.as_mut().set_progress_files(bytes(progress.files));
        self.as_mut().set_progress_bytes(bytes(progress.bytes));
        self.as_mut()
            .set_progress_path(QString::from(progress.current.to_string_lossy().as_ref()));
    }

    pub(crate) fn apply_tree(
        mut self: Pin<&mut Self>,
        generation: u64,
        scanned: Result<(Tree, Findings), ScanError>,
    ) {
        if !publish::still_current(generation, self.rust().generation)
            || self.rust().state != Mode::Scanning
        {
            return;
        }
        self.as_mut().rust_mut().scan = None;
        match scanned {
            Ok((tree, findings)) => {
                self.as_mut().rust_mut().session.load(tree, findings);
                self.as_mut().rust_mut().state = Mode::Analysed;
                self.publish();
            }
            Err(ScanError::Cancelled) => self.browse(),
            Err(_) => {
                // A scan that did not produce a tree: back to browsing.
                self.as_mut().set_action_kind(QString::default());
                self.as_mut().set_action_done(0);
                self.as_mut().set_action_total(0);
                self.as_mut().set_action_outcome(QString::from(FAILED));
                self.browse();
            }
        }
    }

    pub fn cancel(mut self: Pin<&mut Self>) {
        match self.rust().state {
            Mode::Scanning => {
                self.as_mut().rust_mut().next_generation();
                self.browse();
            }
            Mode::Analysed if self.rust().session.confirm.is_some() => {
                self.as_mut().rust_mut().session.stop_confirm();
                self.refresh_busy();
            }
            _ => {}
        }
    }

    pub fn set_filters(mut self: Pin<&mut Self>, duplicates: bool, empty: bool) {
        self.as_mut().set_show_duplicates(duplicates);
        self.as_mut().set_show_empty(empty);
        if self.rust().state == Mode::Analysed {
            self.publish();
        }
    }

    pub fn confirm_duplicates(mut self: Pin<&mut Self>) {
        let session = &self.rust().session;
        if self.rust().state != Mode::Analysed || session.confirm.is_some() {
            return;
        }
        let Some(tree) = session.tree.as_ref().map(Arc::downgrade) else {
            return;
        };
        let pending = session.pending_groups();
        if pending.is_empty() {
            return;
        }
        let generation = self.rust().generation;
        self.as_mut().rust_mut().session.stop_confirm();
        let epoch = self.rust().session.confirm_epoch;
        let qt = self.qt_thread();
        match usage_worker::spawn_confirm(tree, pending, generation, epoch, qt) {
            Ok(handle) => {
                self.as_mut().rust_mut().session.set_confirm(handle);
                self.as_mut().refresh_busy();
            }
            Err(_) => self.as_mut().set_start_failed(true),
        }
    }

    pub(crate) fn apply_verified(
        mut self: Pin<&mut Self>,
        generation: u64,
        epoch: u64,
        index: usize,
        verified: Vec<Verified>,
    ) {
        if !self.rust().confirm_current(generation, epoch) || self.rust().session.confirm.is_none()
        {
            return;
        }
        if self
            .as_mut()
            .rust_mut()
            .session
            .set_verdict(index, verified)
        {
            self.publish();
        }
    }

    /// A copy of candidate `index` could not be read; its row says so.
    pub(crate) fn apply_unreadable(
        mut self: Pin<&mut Self>,
        generation: u64,
        epoch: u64,
        index: usize,
    ) {
        if !self.rust().confirm_current(generation, epoch) || self.rust().session.confirm.is_none()
        {
            return;
        }
        if self.as_mut().rust_mut().session.set_unreadable(index) {
            self.publish();
        }
    }

    pub(crate) fn confirm_finished(mut self: Pin<&mut Self>, generation: u64, epoch: u64) {
        if !self.rust().confirm_current(generation, epoch) {
            return;
        }
        self.as_mut().rust_mut().session.confirm_done();
        self.refresh_busy();
    }

    pub fn toggle_selected(mut self: Pin<&mut Self>, id: i32) {
        let Some(id) = self.rust().session.node_id(id) else {
            return;
        };
        self.as_mut().rust_mut().session.toggle(id);
        self.publish_selection();
    }

    pub fn clear_selection(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().session.clear_selection();
        self.publish_selection();
    }

    pub fn select_all_but_one(mut self: Pin<&mut Self>, group: i32) {
        let Ok(group) = usize::try_from(group) else {
            return;
        };
        if self.as_mut().rust_mut().session.select_all_but_one(group) {
            self.publish_selection();
        }
    }

    /// The selection alone changes; rows read it without being rebuilt.
    pub(super) fn publish_selection(mut self: Pin<&mut Self>) {
        let session = &self.rust().session;
        let selected: Vec<f64> = session
            .selection()
            .iter()
            .map(|id| f64::from(id.0))
            .collect();
        let (selected_count, selected_bytes) = session.selected_totals();
        let selection_revision = session.selection_revision();
        self.as_mut().set_selected_ids(doubles(&selected));
        self.as_mut().set_selected_bytes(selected_bytes);
        self.as_mut().set_selected_count(selected_count);
        self.as_mut().set_selection_revision(selection_revision);
    }

    /// Whether an action may start: the analysis is shown and nothing else
    /// is changing the tree.
    fn may_act(&self) -> bool {
        self.rust().state == Mode::Analysed && self.rust().session.idle_for_action()
    }

    /// Clears the last outcome, sets the counts a trash or deletion reports
    /// progress against and takes the next action epoch.
    fn begin_action(mut self: Pin<&mut Self>, total: usize) -> u64 {
        self.as_mut().set_action_outcome(QString::default());
        self.as_mut().set_action_done(0);
        self.as_mut()
            .set_action_total(i32::try_from(total).unwrap_or(i32::MAX));
        self.as_mut().rust_mut().session.next_action_epoch()
    }

    pub fn open_selected(mut self: Pin<&mut Self>) {
        if !self.may_act() {
            return;
        }
        let Some(item) = self.rust().session.action_items().into_iter().next() else {
            return;
        };
        let epoch = self.as_mut().begin_action(1);
        let generation = self.rust().generation;
        let qt = self.qt_thread();
        if actions::spawn_open(item.path, generation, epoch, qt).is_err() {
            self.as_mut().set_start_failed(true);
        }
    }

    /// The items a confirmed trash or deletion acts on, or none: the page
    /// asked under selection revision `asked`, and a selection changed since
    /// (even to another set of the same count) is refused with a typed
    /// outcome that names the live count.
    fn confirmed_items(mut self: Pin<&mut Self>, kind: ActionKind, asked: i32) -> Vec<Item> {
        if !self.may_act() {
            return Vec::new();
        }
        let session = &self.rust().session;
        if session.selection_revision() != asked {
            let (live, _) = session.selected_totals();
            self.as_mut().set_action_kind(QString::from(kind.as_str()));
            self.as_mut().set_action_done(0);
            self.as_mut().set_action_total(live);
            self.as_mut().set_action_bytes(0.0);
            self.as_mut().set_action_outcome(QString::from(REFUSED));
            return Vec::new();
        }
        session.action_items()
    }

    pub fn trash_selected(mut self: Pin<&mut Self>, asked: i32) {
        let items = self.as_mut().confirmed_items(ActionKind::Trash, asked);
        if items.is_empty() {
            return;
        }
        let epoch = self.as_mut().begin_action(items.len());
        let generation = self.rust().generation;
        let qt = self.qt_thread();
        match actions::spawn_trash(items, generation, epoch, qt) {
            Ok(handle) => self.as_mut().started(handle),
            Err(_) => self.as_mut().set_start_failed(true),
        }
    }

    pub fn delete_selected(mut self: Pin<&mut Self>, asked: i32) {
        let items = self.as_mut().confirmed_items(ActionKind::Delete, asked);
        let within = self.rust().session.tree.as_ref().map(|t| t.path.clone());
        let Some(within) = within.filter(|_| !items.is_empty()) else {
            return;
        };
        let epoch = self.as_mut().begin_action(items.len());
        let generation = self.rust().generation;
        let qt = self.qt_thread();
        match actions::spawn_delete(items, within, generation, epoch, qt) {
            Ok(handle) => self.as_mut().started(handle),
            Err(_) => self.as_mut().set_start_failed(true),
        }
    }

    fn started(mut self: Pin<&mut Self>, handle: WorkerHandle) {
        self.as_mut().rust_mut().session.set_action(handle);
        self.refresh_busy();
    }

    /// Cancels the running trash or deletion without moving the action
    /// epoch: the handle stays until the worker reports, so the entries it
    /// removed before stopping are still pruned.
    pub fn cancel_action(self: Pin<&mut Self>) {
        if let Some(action) = self.rust().session.action.as_ref() {
            action.cancel();
        }
    }

    fn action_current(&self, generation: u64, epoch: u64) -> bool {
        publish::still_current(generation, self.rust().generation)
            && epoch == self.rust().session.action_epoch
    }

    /// One more item of the running action finished.
    pub(crate) fn apply_action_progress(
        mut self: Pin<&mut Self>,
        generation: u64,
        epoch: u64,
        done: usize,
    ) {
        if self.action_current(generation, epoch) {
            self.as_mut()
                .set_action_done(i32::try_from(done).unwrap_or(i32::MAX));
        }
    }

    /// An action's report: its outcome, then — for what it removed — the
    /// pruned tree, republished, and for each item it removed only in part
    /// a graft scan of what is left.
    pub(crate) fn apply_action(
        mut self: Pin<&mut Self>,
        generation: u64,
        epoch: u64,
        report: ActionReport,
    ) {
        if !self.action_current(generation, epoch) {
            return;
        }
        self.as_mut().rust_mut().session.action_done();
        let freed = report
            .removed
            .iter()
            .map(|(_, allocated)| *allocated)
            .fold(report.partial_bytes, u64::saturating_add);
        let count = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
        self.as_mut()
            .set_action_kind(QString::from(report.kind.as_str()));
        self.as_mut().set_action_done(count(report.done));
        self.as_mut().set_action_total(count(report.total));
        self.as_mut().set_action_bytes(bytes(freed));
        self.as_mut()
            .set_action_outcome(QString::from(report.outcome));
        if self.rust().state != Mode::Analysed {
            self.refresh_busy();
            return;
        }
        if !report.removed.is_empty() {
            let removed: Vec<NodeId> = report.removed.iter().map(|(id, _)| *id).collect();
            self.as_mut().rust_mut().session.prune(&removed);
        }
        for id in report.partial {
            self.as_mut().spawn_graft(id);
        }
        self.publish();
    }

    /// Scans the folder `id` again on a `hematita-graft` thread; nothing is
    /// read here. A graft that cannot start leaves the stale subtree shown.
    fn spawn_graft(mut self: Pin<&mut Self>, id: NodeId) {
        let session = &self.rust().session;
        let Some(path) = session
            .tree
            .as_ref()
            .filter(|tree| tree.is_live(id))
            .map(|tree| tree.path_of(id))
        else {
            return;
        };
        let generation = self.rust().generation;
        let qt = self.qt_thread();
        match usage_worker::spawn_graft(path, id, generation, qt) {
            Ok(handle) => self.as_mut().rust_mut().session.start_graft(id, handle),
            Err(_) => self.as_mut().set_start_failed(true),
        }
    }

    /// A graft scan's answer: the fresh subtree takes the stale one's place,
    /// or, when the folder is gone, the stale one is pruned; any other
    /// failure leaves it as it was.
    pub(crate) fn apply_graft(
        mut self: Pin<&mut Self>,
        generation: u64,
        id: NodeId,
        scanned: Result<(Tree, Findings), ScanError>,
    ) {
        if !publish::still_current(generation, self.rust().generation) {
            return;
        }
        let session = &mut self.as_mut().rust_mut().get_mut().session;
        if session.take_graft(id).is_none() {
            return;
        }
        let session = &mut self.as_mut().rust_mut().get_mut().session;
        match scanned {
            Ok((fresh, found)) => {
                session.graft(id, fresh, &found);
            }
            Err(ScanError::NotADirectory { .. }) => session.prune(&[id]),
            Err(ScanError::Root { source, .. }) if source.kind() == ErrorKind::NotFound => {
                session.prune(&[id]);
            }
            Err(_) => {}
        }
        self.publish();
    }
}
