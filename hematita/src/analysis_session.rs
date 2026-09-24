//! The analysed session behind the storage hub, as a plain struct: the
//! scanned tree, its findings and verdicts, the selection, the analysed
//! folder, the id-keyed marks the page paints, the running workers' handles
//! and the epochs that retire their late results.
//!
//! Nothing here touches Qt or the disk. The hub (`analysis.rs`) spawns the
//! workers, keeps the scan generation and writes the properties; every rule
//! about which ids are still meant, what a pruning or a graft changes and
//! what the page lists is decided here, so each is tested without a QObject.
//!
//! Ids never change meaning. A pruning empties and unlinks the removed
//! subtrees; a graft unlinks the stale subtree a stopped deletion left and
//! appends a fresh scan of the same folder under new ids. Either moves the
//! confirm epoch first, so no verdict about the older tree lands, and
//! [`Session::node_id`] answers only for ids still reachable from the root.

use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::Arc;

use hematita_core::usage::duplicates::Verified;
use hematita_core::usage::layout::{squarify, Rect};
use hematita_core::usage::tree::{Kind, NodeId, Tree};

use crate::actions::Item;
use crate::analysis_view::{self, Findings, GroupRow, Kept};
use crate::usage_worker::WorkerHandle;

pub fn lossy(name: &OsString) -> String {
    name.to_string_lossy().into_owned()
}

pub fn flag(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

/// Bytes as the doubles QML reads; exact up to 2^53, which no disk reaches.
pub fn bytes(value: u64) -> f64 {
    value as f64
}

fn kind_token(kind: Kind) -> &'static str {
    match kind {
        Kind::Dir => "dir",
        Kind::File => "file",
        Kind::Other => "other",
    }
}

fn mark(marks: &[bool], id: NodeId) -> f64 {
    flag(marks.get(id.0 as usize).copied().unwrap_or(false))
}

/// The analysed lists, computed from the owned tree before any property is
/// written.
#[derive(Default)]
pub struct Analysed {
    pub ids: Vec<f64>,
    pub names: Vec<String>,
    pub kinds: Vec<String>,
    pub allocated: Vec<f64>,
    pub apparent: Vec<f64>,
    pub shares: Vec<f64>,
    pub files_below: Vec<f64>,
    pub empty: Vec<f64>,
    pub duplicate: Vec<f64>,
    pub unreadable: Vec<f64>,
    pub rects: Vec<f64>,
    pub current_allocated: f64,
    pub current_unreadable: i32,
    pub group_sizes: Vec<f64>,
    pub group_counts: Vec<f64>,
    pub group_verified: Vec<f64>,
    pub group_unreadable: Vec<f64>,
    pub copies: Vec<f64>,
    pub member_groups: Vec<f64>,
    pub member_ids: Vec<f64>,
    pub member_names: Vec<String>,
    pub member_paths: Vec<String>,
}

pub struct Session {
    /// The scanned tree; its root is the last path of the hub's stack.
    pub tree: Option<Arc<Tree>>,
    pub findings: Findings,
    /// Per candidate group: its content verdict, once checked.
    pub verdicts: Vec<Option<Vec<Verified>>>,
    /// Per candidate group: a copy could not be read by the content check.
    pub unreadable_groups: Vec<bool>,
    /// Per node: the member count of the unverified candidate row it is in.
    pub candidate_copies: Vec<u32>,
    /// Moved by every start, cancel, pruning and graft of the content check.
    pub confirm_epoch: u64,
    pub confirm: Option<WorkerHandle>,
    /// Moved by every action started; a report under another is dropped.
    pub action_epoch: u64,
    pub action: Option<WorkerHandle>,
    /// The graft scans running, one per folder a deletion stopped inside.
    pub grafts: Vec<(NodeId, WorkerHandle)>,
    /// Per node: a duplicate or empty folder exactly, and one at or below.
    pub duplicate_exact: Vec<bool>,
    pub duplicate_below: Vec<bool>,
    pub empty_exact: Vec<bool>,
    pub empty_below: Vec<bool>,
    /// The analysed folder.
    pub current: NodeId,
    /// Changed only through the methods below, each of which moves
    /// `selection_revision`.
    selection: Vec<NodeId>,
    /// Moved by every change of the selected set; a confirmation carries
    /// the revision it was asked under, so a swap of equal count is caught.
    selection_revision: i32,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            tree: None,
            findings: Findings::default(),
            verdicts: Vec::new(),
            unreadable_groups: Vec::new(),
            candidate_copies: Vec::new(),
            confirm_epoch: 0,
            confirm: None,
            action_epoch: 0,
            action: None,
            grafts: Vec::new(),
            duplicate_exact: Vec::new(),
            duplicate_below: Vec::new(),
            empty_exact: Vec::new(),
            empty_below: Vec::new(),
            current: NodeId(0),
            selection: Vec::new(),
            selection_revision: 0,
        }
    }
}

impl Session {
    /// Forgets the analysis; dropping the handles cancels the workers.
    pub fn reset(&mut self) {
        self.stop_confirm();
        self.action = None;
        self.action_epoch = self.action_epoch.wrapping_add(1);
        self.grafts.clear();
        self.tree = None;
        self.findings = Findings::default();
        self.verdicts.clear();
        self.unreadable_groups.clear();
        self.candidate_copies.clear();
        self.duplicate_exact.clear();
        self.duplicate_below.clear();
        self.empty_exact.clear();
        self.empty_below.clear();
        self.clear_selection();
    }

    /// Takes a finished scan and its findings as the analysis.
    pub fn load(&mut self, tree: Tree, findings: Findings) {
        self.verdicts = vec![None; findings.candidates.len()];
        self.unreadable_groups = vec![false; findings.candidates.len()];
        self.current = tree.root;
        self.findings = findings;
        self.tree = Some(Arc::new(tree));
        self.clear_selection();
        self.mark_empty();
        self.mark_duplicates();
    }

    /// A content check, an action or a graft is running.
    pub fn busy(&self) -> bool {
        self.confirm.is_some() || self.action.is_some() || !self.grafts.is_empty()
    }

    /// Whether an action may start: nothing else is changing the tree.
    pub fn idle_for_action(&self) -> bool {
        self.action.is_none() && self.grafts.is_empty()
    }

    /// The duplicate rows as the page lists them.
    pub fn group_rows(&self) -> Vec<GroupRow> {
        analysis_view::group_rows(
            &self.findings.candidates,
            &self.verdicts,
            &self.unreadable_groups,
        )
    }

    fn mark_empty(&mut self) {
        let Some(tree) = self.tree.as_ref() else {
            return;
        };
        let empty = &self.findings.empty;
        self.empty_exact = analysis_view::marks_exact(tree.nodes.len(), empty.iter().copied());
        self.empty_below = analysis_view::marks_below(tree, empty.iter().copied());
    }

    /// Marks the members of the listed duplicate rows, after a verdict: the
    /// filter keeps every listed member; only a verified copy is painted as
    /// a duplicate, and an unverified candidate carries its group's count.
    pub fn mark_duplicates(&mut self) {
        let Some(tree) = self.tree.as_ref() else {
            return;
        };
        let rows = self.group_rows();
        let verified: Vec<NodeId> = rows
            .iter()
            .filter(|row| row.verified)
            .flat_map(|row| row.nodes.iter().copied())
            .collect();
        let mut copies = vec![0_u32; tree.nodes.len()];
        for row in rows.iter().filter(|row| !row.verified) {
            let count = u32::try_from(row.nodes.len()).unwrap_or(u32::MAX);
            for id in &row.nodes {
                if let Some(slot) = copies.get_mut(id.0 as usize) {
                    *slot = count;
                }
            }
        }
        self.duplicate_exact = analysis_view::marks_exact(tree.nodes.len(), verified);
        self.duplicate_below =
            analysis_view::marks_below(tree, rows.into_iter().flat_map(|row| row.nodes));
        self.candidate_copies = copies;
    }

    /// Keeps the running content check's handle.
    pub fn set_confirm(&mut self, handle: WorkerHandle) {
        self.confirm = Some(handle);
    }

    /// The content check finished; its handle is dropped.
    pub fn confirm_done(&mut self) {
        self.confirm = None;
    }

    /// Moves the action epoch for an action about to start and answers it.
    pub fn next_action_epoch(&mut self) -> u64 {
        self.action_epoch = self.action_epoch.wrapping_add(1);
        self.action_epoch
    }

    /// Keeps the running trash or deletion's handle.
    pub fn set_action(&mut self, handle: WorkerHandle) {
        self.action = Some(handle);
    }

    /// The action reported; its handle is dropped.
    pub fn action_done(&mut self) {
        self.action = None;
    }

    /// Keeps the handle of the graft scan of `id`.
    pub fn start_graft(&mut self, id: NodeId, handle: WorkerHandle) {
        self.grafts.push((id, handle));
    }

    /// Takes the graft scan of `id` off the running list; none when no
    /// graft of `id` is awaited.
    pub fn take_graft(&mut self, id: NodeId) -> Option<WorkerHandle> {
        let at = self.grafts.iter().position(|(pending, _)| *pending == id)?;
        Some(self.grafts.remove(at).1)
    }

    /// The selected ids, in the order they were chosen.
    pub fn selection(&self) -> &[NodeId] {
        &self.selection
    }

    /// The revision of the selected set, as the page carries it.
    pub fn selection_revision(&self) -> i32 {
        self.selection_revision
    }

    fn selection_changed(&mut self) {
        self.selection_revision = self.selection_revision.wrapping_add(1);
    }

    /// Empties the selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selection_changed();
    }

    /// Cancels a running content check and moves the epoch, so nothing it
    /// queued lands.
    pub fn stop_confirm(&mut self) {
        self.confirm = None;
        self.confirm_epoch = self.confirm_epoch.wrapping_add(1);
    }

    /// Whether a content-check result asked under `epoch` still describes
    /// the tree being shown (the hub checks the generation).
    pub fn confirm_current(&self, epoch: u64) -> bool {
        epoch == self.confirm_epoch
    }

    /// The candidate groups not yet checked, each with its index.
    pub fn pending_groups(&self) -> Vec<(usize, hematita_core::usage::duplicates::Group)> {
        self.findings
            .candidates
            .iter()
            .enumerate()
            .filter(|(index, _)| self.verdicts.get(*index).is_some_and(Option::is_none))
            .map(|(index, group)| (index, group.clone()))
            .collect()
    }

    /// Records the verdict of candidate `index`; false when there is none.
    pub fn set_verdict(&mut self, index: usize, verified: Vec<Verified>) -> bool {
        let Some(slot) = self.verdicts.get_mut(index) else {
            return false;
        };
        *slot = Some(verified);
        self.mark_duplicates();
        true
    }

    /// A copy of candidate `index` could not be read; false when there is
    /// no such candidate.
    pub fn set_unreadable(&mut self, index: usize) -> bool {
        let Some(slot) = self.unreadable_groups.get_mut(index) else {
            return false;
        };
        *slot = true;
        true
    }

    /// The outermost selected entries as the action workers take them,
    /// each with the device and inode the scan recorded.
    pub fn action_items(&self) -> Vec<Item> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        analysis_view::outermost(tree, &self.selection)
            .into_iter()
            .filter_map(|id| {
                tree.node(id).map(|node| Item {
                    id,
                    path: tree.path_of(id),
                    allocated: node.allocated,
                    dev: node.dev,
                    ino: node.ino,
                })
            })
            .collect()
    }

    /// The outermost selection's count and allocated bytes.
    pub fn selected_totals(&self) -> (i32, f64) {
        let items = self.action_items();
        let total = items
            .iter()
            .map(|item| item.allocated)
            .fold(0_u64, u64::saturating_add);
        (i32::try_from(items.len()).unwrap_or(i32::MAX), bytes(total))
    }

    /// Adds `id` to the selection or takes it out; the root is never chosen.
    pub fn toggle(&mut self, id: NodeId) {
        if self.tree.as_ref().is_some_and(|tree| tree.root == id) {
            return;
        }
        match self.selection.iter().position(|chosen| *chosen == id) {
            Some(at) => {
                self.selection.remove(at);
            }
            None => self.selection.push(id),
        }
        self.selection_changed();
    }

    /// Selects every copy of the verified row `group` but one; false when
    /// the row is not verified or could not be read.
    pub fn select_all_but_one(&mut self, group: usize) -> bool {
        let Some(row) = self.group_rows().into_iter().nth(group) else {
            return false;
        };
        if !row.verified || row.unreadable {
            return false;
        }
        for id in analysis_view::all_but_one(&row.nodes) {
            if !self.selection.contains(&id) {
                self.selection.push(id);
            }
        }
        self.selection_changed();
        true
    }

    /// Prunes the entries an action removed and brings every id-keyed cache
    /// in line; a running content check is cancelled first.
    pub fn prune(&mut self, removed: &[NodeId]) {
        self.stop_confirm();
        let Some(tree) = self.tree.as_mut() else {
            return;
        };
        let tree = Arc::make_mut(tree);
        for id in removed {
            tree.prune(*id);
        }
        self.forget(removed);
    }

    /// Replaces the stale subtree at `id` with `fresh`, a scan of the same
    /// folder, and its `found` findings (in `fresh`'s own ids). The old
    /// subtree's ids leave every list; the fresh empty folders and unreadable
    /// counts join them under their new ids. Candidates inside `fresh` are
    /// not offered until the next full scan. False, changing nothing but the
    /// confirm epoch, when the tree refuses the graft.
    pub fn graft(&mut self, id: NodeId, fresh: Tree, found: &Findings) -> bool {
        self.stop_confirm();
        let Some(tree) = self.tree.as_mut() else {
            return false;
        };
        let tree = Arc::make_mut(tree);
        let Ok(offset) = u32::try_from(tree.nodes.len()) else {
            return false;
        };
        let fresh_root = fresh.root;
        let follow = analysis_view::gone(tree, self.current, &HashSet::from([id]));
        if !tree.graft(id, fresh) {
            return false;
        }
        let new_id = NodeId(offset + fresh_root.0);
        self.forget(&[id]);
        let unreadable = &mut self.findings.unreadable;
        unreadable.resize(offset as usize, 0);
        unreadable.extend(found.unreadable.iter().copied());
        if let Some(tree) = self.tree.as_ref() {
            unreadable.resize(tree.nodes.len(), 0);
            let below = unreadable.get(new_id.0 as usize).copied().unwrap_or(0);
            let mut cursor = tree.node(new_id).and_then(|n| n.parent);
            while let Some(ancestor) = cursor {
                if let Some(count) = unreadable.get_mut(ancestor.0 as usize) {
                    *count = count.saturating_add(below);
                }
                cursor = tree.node(ancestor).and_then(|n| n.parent);
            }
        }
        self.findings
            .empty
            .extend(found.empty.iter().map(|local| NodeId(local.0 + offset)));
        if follow {
            self.current = new_id;
        }
        self.mark_empty();
        self.mark_duplicates();
        true
    }

    /// After the tree dropped `removed`, drops their ids from the findings,
    /// the verdicts and the selection, and leaves a gone analysed folder.
    fn forget(&mut self, removed: &[NodeId]) {
        let Self {
            tree,
            findings,
            verdicts,
            unreadable_groups,
            selection,
            current,
            ..
        } = self;
        let Some(tree) = tree.as_ref() else {
            return;
        };
        analysis_view::forget_removed(
            tree,
            removed,
            Kept {
                findings,
                verdicts,
                unreadable_groups,
                selection,
            },
        );
        let removed_set: HashSet<NodeId> = removed.iter().copied().collect();
        if analysis_view::gone(tree, *current, &removed_set) {
            *current = tree.root;
        }
        self.selection_changed();
        self.mark_empty();
        self.mark_duplicates();
    }

    /// The analysed folder's ancestors below the scanned root, root excluded,
    /// outermost first.
    pub fn chain(&self) -> Vec<NodeId> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        let mut chain = Vec::new();
        let mut cursor = Some(self.current);
        while let Some(id) = cursor {
            if id == tree.root {
                break;
            }
            chain.push(id);
            cursor = tree.node(id).and_then(|n| n.parent);
        }
        chain.reverse();
        chain
    }

    /// A node id QML sent, if it names a node still in the tree: an id of a
    /// pruned or replaced subtree is refused.
    pub fn node_id(&self, id: i32) -> Option<NodeId> {
        let tree = self.tree.as_ref()?;
        let id = NodeId(u32::try_from(id).ok()?);
        tree.is_live(id).then_some(id)
    }

    /// Whether `id` is a folder with something to show inside.
    pub fn enterable(&self, id: NodeId) -> bool {
        self.tree
            .as_ref()
            .and_then(|t| t.node(id))
            .is_some_and(|n| n.kind == Kind::Dir && !n.children.is_empty())
    }

    /// The analysed folder's parent; none at the scanned root.
    pub fn parent(&self) -> Option<NodeId> {
        let tree = self.tree.as_ref()?;
        if self.current == tree.root {
            return None;
        }
        tree.node(self.current).and_then(|n| n.parent)
    }

    /// What the analysed mode lists, under the two filters.
    pub fn view(&self, show_duplicates: bool, show_empty: bool) -> Analysed {
        let mut view = Analysed::default();
        let Some(tree) = self.tree.as_ref() else {
            return view;
        };
        let Some(current) = tree.node(self.current) else {
            return view;
        };
        view.current_allocated = bytes(current.allocated);
        view.current_unreadable = self
            .findings
            .unreadable
            .get(self.current.0 as usize)
            .map_or(0, |count| i32::try_from(*count).unwrap_or(i32::MAX));

        let children = tree.children_by_size(self.current);
        let sizes: Vec<u64> = children
            .iter()
            .map(|id| tree.node(*id).map_or(0, |n| n.allocated))
            .collect();
        let unit = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };
        view.rects = analysis_view::flat_rects(&children, &squarify(&sizes, unit));

        let mut filters: Vec<&[bool]> = Vec::new();
        if show_duplicates {
            filters.push(&self.duplicate_below);
        }
        if show_empty {
            filters.push(&self.empty_below);
        }
        for id in analysis_view::project(&children, &filters) {
            let Some(node) = tree.node(id) else {
                continue;
            };
            view.ids.push(f64::from(id.0));
            view.names.push(lossy(&node.name));
            view.kinds.push(kind_token(node.kind).to_owned());
            view.allocated.push(bytes(node.allocated));
            view.apparent.push(bytes(node.apparent));
            view.shares.push(if current.allocated > 0 {
                bytes(node.allocated) / bytes(current.allocated)
            } else {
                0.0
            });
            view.files_below.push(bytes(node.files_below));
            view.empty.push(mark(&self.empty_exact, id));
            view.duplicate.push(mark(&self.duplicate_exact, id));
            view.unreadable.push(flag(node.unreadable));
            view.copies.push(f64::from(
                self.candidate_copies
                    .get(id.0 as usize)
                    .copied()
                    .unwrap_or(0),
            ));
        }

        if show_duplicates {
            for (row, group) in self.group_rows().into_iter().enumerate() {
                view.group_sizes.push(bytes(group.size));
                view.group_counts.push(group.nodes.len() as f64);
                view.group_verified.push(flag(group.verified));
                view.group_unreadable.push(flag(group.unreadable));
                for id in group.nodes {
                    view.member_groups.push(row as f64);
                    view.member_ids.push(f64::from(id.0));
                    view.member_names
                        .push(tree.node(id).map(|n| lossy(&n.name)).unwrap_or_default());
                    view.member_paths
                        .push(tree.path_of(id).to_string_lossy().into_owned());
                }
            }
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use hematita_core::usage::tree::Node;

    use super::*;

    fn node(name: &str, kind: Kind, parent: Option<u32>, size: u64, children: &[u32]) -> Node {
        Node {
            name: OsString::from(name),
            kind,
            parent: parent.map(NodeId),
            allocated: size,
            apparent: size,
            files_below: u64::from(kind == Kind::File),
            others_below: 0,
            unreadable: false,
            other_device: false,
            dev: 1,
            ino: name.len() as u64,
            children: children.iter().copied().map(NodeId).collect(),
        }
    }

    /// `/r` holding folder `a` (files `f`, `g`) and folder `e` (empty).
    fn tree() -> Tree {
        let mut nodes = vec![
            node("/r", Kind::Dir, None, 300, &[1, 4]),
            node("a", Kind::Dir, Some(0), 300, &[2, 3]),
            node("f", Kind::File, Some(1), 100, &[]),
            node("g", Kind::File, Some(1), 200, &[]),
            node("e", Kind::Dir, Some(0), 0, &[]),
        ];
        nodes[0].files_below = 2;
        nodes[1].files_below = 2;
        Tree {
            root: NodeId(0),
            path: PathBuf::from("/r"),
            device: 1,
            nodes,
            unreadable_dirs: 0,
            hard_link_names: 0,
        }
    }

    /// What a scan of `/r/a` finds after `f` went: `a` holding `g`.
    fn rest_of_a() -> Tree {
        let mut nodes = vec![
            node("/r/a", Kind::Dir, None, 200, &[1]),
            node("g", Kind::File, Some(0), 200, &[]),
        ];
        nodes[0].files_below = 1;
        Tree {
            root: NodeId(0),
            path: PathBuf::from("/r/a"),
            device: 1,
            nodes,
            unreadable_dirs: 0,
            hard_link_names: 0,
        }
    }

    fn loaded() -> Session {
        let tree = tree();
        let found = analysis_view::findings(&tree);
        let mut session = Session::default();
        session.load(tree, found);
        session
    }

    #[test]
    fn a_verdict_from_an_older_confirm_epoch_is_dropped() {
        let mut session = Session::default();
        session.stop_confirm();
        let first = session.confirm_epoch;
        assert!(session.confirm_current(first));
        // A cancel, a restart, a pruning or a graft moves the epoch.
        session.stop_confirm();
        assert!(!session.confirm_current(first));
        let before = session.confirm_epoch;
        let mut session = loaded();
        session.confirm_epoch = before;
        session.prune(&[NodeId(4)]);
        assert!(!session.confirm_current(before));
    }

    #[test]
    fn a_pruned_id_is_no_longer_accepted() {
        let mut session = loaded();
        assert_eq!(session.node_id(2), Some(NodeId(2)));
        session.selection = vec![NodeId(1)];
        session.prune(&[NodeId(1)]);
        assert_eq!(session.node_id(1), None);
        assert_eq!(session.node_id(2), None, "below a pruned folder");
        assert_eq!(session.node_id(4), Some(NodeId(4)));
        assert!(session.selection.is_empty());
        assert_eq!(session.tree.as_ref().map(|t| t.nodes[0].allocated), Some(0));
    }

    #[test]
    fn a_graft_replaces_the_stale_folder_with_what_is_left() {
        let mut session = loaded();
        session.current = NodeId(1);
        session.selection = vec![NodeId(2)];
        let fresh = rest_of_a();
        let found = analysis_view::findings(&fresh);
        assert!(session.graft(NodeId(1), fresh, &found));
        let Some(tree) = session.tree.clone() else {
            panic!("the analysis is loaded");
        };
        assert_eq!(tree.nodes[0].allocated, 200, "the true remaining size");
        assert_eq!(tree.nodes[0].files_below, 1);
        assert_eq!(session.node_id(1), None, "the stale folder's id is retired");
        assert_eq!(session.node_id(2), None);
        assert_eq!(session.node_id(5), Some(NodeId(5)), "the fresh folder");
        assert_eq!(session.current, NodeId(5), "the analysed folder follows it");
        assert!(session.selection.is_empty());
        assert_eq!(session.findings.unreadable.len(), tree.nodes.len());
        assert_eq!(session.findings.empty, vec![NodeId(4)]);
        let view = session.view(false, false);
        assert_eq!(view.ids, vec![6.0]);
    }

    #[test]
    fn a_graft_on_a_retired_id_changes_nothing() {
        let mut session = loaded();
        session.prune(&[NodeId(1)]);
        let fresh = rest_of_a();
        let found = analysis_view::findings(&fresh);
        let before = session.tree.as_ref().map(|t| t.nodes.len());
        assert!(!session.graft(NodeId(1), fresh, &found));
        assert_eq!(session.tree.as_ref().map(|t| t.nodes.len()), before);
    }

    #[test]
    fn the_totals_count_the_outermost_selection() {
        let mut session = loaded();
        session.toggle(NodeId(1));
        session.toggle(NodeId(2));
        session.toggle(NodeId(0));
        assert_eq!(session.selected_totals(), (1, 300.0));
        let items = session.action_items();
        assert_eq!((items[0].dev, items[0].ino), (1, 1));
        session.toggle(NodeId(1));
        assert_eq!(session.selected_totals(), (1, 100.0));
    }

    #[test]
    fn a_running_graft_holds_actions_back_and_counts_as_busy() {
        let mut session = loaded();
        assert!(session.idle_for_action() && !session.busy());
        let (handle, token) = WorkerHandle::pair();
        session.start_graft(NodeId(1), handle);
        assert!(!session.idle_for_action() && session.busy());
        session.reset();
        assert!(token.is_cancelled(), "a reset cancels the graft scan");
    }

    #[test]
    fn a_swap_of_equal_count_moves_the_selection_revision() {
        let mut session = loaded();
        session.toggle(NodeId(2));
        let asked = session.selection_revision();
        assert_eq!(session.selected_totals().0, 1);
        session.toggle(NodeId(2));
        session.toggle(NodeId(3));
        assert_eq!(session.selected_totals().0, 1, "the same count");
        assert_ne!(session.selection_revision(), asked);
        let before = session.selection_revision();
        session.prune(&[NodeId(4)]);
        assert_ne!(session.selection_revision(), before, "a pruning moves it");
    }

    #[test]
    fn a_taken_graft_is_no_longer_awaited() {
        let mut session = loaded();
        let (handle, _token) = WorkerHandle::pair();
        session.start_graft(NodeId(1), handle);
        assert!(session.take_graft(NodeId(4)).is_none());
        assert!(session.take_graft(NodeId(1)).is_some());
        assert!(session.take_graft(NodeId(1)).is_none());
        assert!(session.idle_for_action());
    }
}
