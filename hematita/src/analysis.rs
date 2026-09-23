//! The storage section's state, as Qt properties: the mount locations, the
//! folder being browsed, the scan's progress and the analysed tree, its
//! duplicates, empty folders and the selection.
//!
//! Every disk read happens on a named worker thread (`hematita-locations`,
//! `hematita-browse`, `hematita-scan`, `hematita-confirm`,
//! `hematita-actions`); each request takes
//! the next generation and a result that comes back under an older one is
//! dropped, so a person clicking faster than the disk answers only ever sees
//! the folder they are in. A new scan, a cancellation or leaving the analysis
//! drops the worker handles, which cancels them.
//!
//! The scanned tree lives here as an `Arc<Tree>`; navigating it answers from
//! memory. QML names a node by the id it was published with, never by path.
//! An action that removed entries prunes exactly the ids its worker reported
//! gone and re-aggregates the ancestors instead of scanning again; ids never
//! change. The content check carries a confirm epoch that every start,
//! cancel and pruning moves, so no verdict about an older tree lands.
//!
//! The path being browsed is a stack of byte-exact `PathBuf`s owned here.
//! QML never hands a path back: it names a row by index and the hub resolves
//! the index against the list it published. Names cross lossily, for display
//! only. Nothing here is prose a person reads: modes, kinds and outcomes are
//! tokens.

use std::ffi::OsString;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use hematita_core::usage::duplicates::{Group, Verified};
use hematita_core::usage::layout::{squarify, Rect};
use hematita_core::usage::tree::{Kind, NodeId, Tree};
use hematita_core::usage::walk::{Progress, ScanError};

use crate::actions::{self, ActionReport, Item};
use crate::analysis_view::{self, Findings, Kept};
use crate::browse::{self, Entry, EntryKind};
use crate::lists::{doubles, strings};
use crate::locations::{self, Location};
use crate::publish;
use crate::usage_worker::{self, WorkerHandle};

/// A scan that could not start or could not read its root.
const FAILED: &str = "failed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Locations,
    Browsing,
    Scanning,
    Analysed,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Locations => "locations",
            Self::Browsing => "browsing",
            Self::Scanning => "scanning",
            Self::Analysed => "analysed",
        }
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // mode — locations | browsing | scanning | analysed
        // crumb* — the browsed path, root first
        // location* — index-aligned mount locations
        // browse* — index-aligned entries of the browsed folder
        // browseFailed — the browsed folder could not be listed
        // progress* — the running scan's totals and where it is
        // entry* — index-aligned children of the analysed folder, filtered
        // treemapRects — flat [id, x, y, w, h] per tile, remainder id -1
        // current* — the analysed folder's allocation and unreadable count
        // group*, member* — duplicate sets (while the filter is on)
        // selectedIds — node ids; showDuplicates, showEmpty — the filters
        // revision — bumped once, after every list is in place
        // busy — a locations, browse or content-check read is in flight
        // hiddenGroupCount — candidate groups beyond the listed cap
        // groupUnreadable — a copy of the group could not be read
        // entryCopies — the unverified candidate group size of the entry
        // selectedBytes — allocated bytes the selection would free
        // action* — the last action's typed outcome and bytes removed
        // startFailed — a worker thread could not be created
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QString, mode)]
        #[qproperty(QStringList, crumb_names)]
        #[qproperty(QStringList, crumb_paths)]
        #[qproperty(QStringList, location_names)]
        #[qproperty(QStringList, location_paths)]
        #[qproperty(QStringList, location_kinds)]
        #[qproperty(QVariant, location_used)]
        #[qproperty(QVariant, location_total)]
        #[qproperty(QVariant, location_readable)]
        #[qproperty(QStringList, browse_names)]
        #[qproperty(QStringList, browse_kinds)]
        #[qproperty(QVariant, browse_apparent)]
        #[qproperty(bool, browse_failed)]
        #[qproperty(f64, progress_files)]
        #[qproperty(f64, progress_bytes)]
        #[qproperty(QString, progress_path)]
        #[qproperty(QVariant, entry_ids)]
        #[qproperty(QStringList, entry_names)]
        #[qproperty(QStringList, entry_kinds)]
        #[qproperty(QVariant, entry_allocated)]
        #[qproperty(QVariant, entry_apparent)]
        #[qproperty(QVariant, entry_shares)]
        #[qproperty(QVariant, entry_files_below)]
        #[qproperty(QVariant, entry_empty)]
        #[qproperty(QVariant, entry_duplicate)]
        #[qproperty(QVariant, entry_unreadable)]
        #[qproperty(QVariant, treemap_rects)]
        #[qproperty(f64, current_allocated)]
        #[qproperty(i32, current_unreadable)]
        #[qproperty(QVariant, group_sizes)]
        #[qproperty(QVariant, group_counts)]
        #[qproperty(QVariant, group_verified)]
        #[qproperty(QVariant, group_unreadable)]
        #[qproperty(i32, hidden_group_count)]
        #[qproperty(QVariant, entry_copies)]
        #[qproperty(f64, selected_bytes)]
        #[qproperty(f64, action_bytes)]
        #[qproperty(QVariant, member_groups)]
        #[qproperty(QVariant, member_ids)]
        #[qproperty(QStringList, member_names)]
        #[qproperty(QStringList, member_paths)]
        #[qproperty(QVariant, selected_ids)]
        #[qproperty(bool, show_duplicates)]
        #[qproperty(bool, show_empty)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, start_failed)]
        #[qproperty(QString, action_outcome)]
        #[qproperty(QString, action_kind)]
        #[qproperty(i32, action_done)]
        #[qproperty(i32, action_total)]
        type HematitaAnalysis = super::HematitaAnalysisRust;

        /// Shows the locations, reading them again on a worker thread. The
        /// window calls it whenever the section is entered.
        #[qinvokable]
        fn open(self: Pin<&mut HematitaAnalysis>);

        /// Enters the location or the folder at `index` of the list the page
        /// is showing. A file, a link or an unreadable location is ignored.
        #[qinvokable]
        fn enter(self: Pin<&mut HematitaAnalysis>, index: i32);

        /// Goes back to the crumb at `index`; below zero, to the locations.
        #[qinvokable]
        fn enter_crumb(self: Pin<&mut HematitaAnalysis>, index: i32);

        /// One folder up; from a location's root, back to the locations.
        #[qinvokable]
        fn up(self: Pin<&mut HematitaAnalysis>);

        /// Reads what the page is showing again.
        #[qinvokable]
        fn refresh(self: Pin<&mut HematitaAnalysis>);

        /// Scans the browsed folder (or, analysed, the scanned one again).
        #[qinvokable]
        fn scan_here(self: Pin<&mut HematitaAnalysis>);

        /// Cancels the running scan, back to browsing, or the running
        /// content check, keeping what it verified.
        #[qinvokable]
        fn cancel(self: Pin<&mut HematitaAnalysis>);

        /// Enters the analysed folder with node `id`.
        #[qinvokable]
        fn enter_id(self: Pin<&mut HematitaAnalysis>, id: i32);

        /// Turns the two filters on or off.
        #[qinvokable]
        fn set_filters(self: Pin<&mut HematitaAnalysis>, duplicates: bool, empty: bool);

        /// Verifies the duplicate candidates by content, group by group.
        #[qinvokable]
        fn confirm_duplicates(self: Pin<&mut HematitaAnalysis>);

        /// Adds node `id` to the selection or takes it out.
        #[qinvokable]
        fn toggle_selected(self: Pin<&mut HematitaAnalysis>, id: i32);

        #[qinvokable]
        fn clear_selection(self: Pin<&mut HematitaAnalysis>);

        /// Selects every copy of the verified duplicate row `group` but one.
        #[qinvokable]
        fn select_all_but_one(self: Pin<&mut HematitaAnalysis>, group: i32);

        /// Opens the first selected entry in Siderita.
        #[qinvokable]
        fn open_selected(self: Pin<&mut HematitaAnalysis>);

        /// Moves the selection to the trash; the page has asked first.
        #[qinvokable]
        fn trash_selected(self: Pin<&mut HematitaAnalysis>);

        /// Deletes the selection permanently; the page has asked first.
        #[qinvokable]
        fn delete_selected(self: Pin<&mut HematitaAnalysis>);
    }

    impl cxx_qt::Threading for HematitaAnalysis {}
}

pub struct HematitaAnalysisRust {
    revision: i32,
    mode: QString,
    crumb_names: QStringList,
    crumb_paths: QStringList,
    location_names: QStringList,
    location_paths: QStringList,
    location_kinds: QStringList,
    location_used: QVariant,
    location_total: QVariant,
    location_readable: QVariant,
    browse_names: QStringList,
    browse_kinds: QStringList,
    browse_apparent: QVariant,
    browse_failed: bool,
    progress_files: f64,
    progress_bytes: f64,
    progress_path: QString,
    entry_ids: QVariant,
    entry_names: QStringList,
    entry_kinds: QStringList,
    entry_allocated: QVariant,
    entry_apparent: QVariant,
    entry_shares: QVariant,
    entry_files_below: QVariant,
    entry_empty: QVariant,
    entry_duplicate: QVariant,
    entry_unreadable: QVariant,
    treemap_rects: QVariant,
    current_allocated: f64,
    current_unreadable: i32,
    group_sizes: QVariant,
    group_counts: QVariant,
    group_verified: QVariant,
    group_unreadable: QVariant,
    hidden_group_count: i32,
    entry_copies: QVariant,
    selected_bytes: f64,
    action_bytes: f64,
    member_groups: QVariant,
    member_ids: QVariant,
    member_names: QStringList,
    member_paths: QStringList,
    selected_ids: QVariant,
    show_duplicates: bool,
    show_empty: bool,
    busy: bool,
    start_failed: bool,
    action_outcome: QString,
    action_kind: QString,
    action_done: i32,
    action_total: i32,
    /// Which list `enter` resolves an index against.
    state: Mode,
    /// The token of the read being waited for; a result under an older one
    /// is dropped.
    generation: u64,
    locations: Vec<Location>,
    /// The browsed path, root first, byte-exact.
    stack: Vec<PathBuf>,
    entries: Vec<Entry>,
    /// The folder `entries` lists, so a refresh of the same folder keeps its
    /// rows until the new listing lands.
    listed: Option<PathBuf>,
    /// The scanned tree; its root is the last path of `stack`.
    tree: Option<Arc<Tree>>,
    findings: Findings,
    /// Per candidate group: its content verdict, once checked.
    verdicts: Vec<Option<Vec<Verified>>>,
    /// Per candidate group: a copy could not be read by the content check.
    unreadable_groups: Vec<bool>,
    /// Per node: the member count of the unverified candidate row it is in.
    candidate_copies: Vec<u32>,
    /// Moved by every start, cancel and pruning of the content check.
    confirm_epoch: u64,
    /// Moved by every action started; a report under another is dropped.
    action_epoch: u64,
    action: Option<WorkerHandle>,
    /// Per node: a duplicate or empty folder exactly, and one at or below.
    duplicate_exact: Vec<bool>,
    duplicate_below: Vec<bool>,
    empty_exact: Vec<bool>,
    empty_below: Vec<bool>,
    /// The analysed folder.
    current: NodeId,
    selection: Vec<NodeId>,
    scan: Option<WorkerHandle>,
    confirm: Option<WorkerHandle>,
}

impl Default for HematitaAnalysisRust {
    fn default() -> Self {
        Self {
            revision: 0,
            mode: QString::from(Mode::Locations.as_str()),
            crumb_names: QStringList::default(),
            crumb_paths: QStringList::default(),
            location_names: QStringList::default(),
            location_paths: QStringList::default(),
            location_kinds: QStringList::default(),
            location_used: doubles(&[]),
            location_total: doubles(&[]),
            location_readable: doubles(&[]),
            browse_names: QStringList::default(),
            browse_kinds: QStringList::default(),
            browse_apparent: doubles(&[]),
            browse_failed: false,
            progress_files: 0.0,
            progress_bytes: 0.0,
            progress_path: QString::default(),
            entry_ids: doubles(&[]),
            entry_names: QStringList::default(),
            entry_kinds: QStringList::default(),
            entry_allocated: doubles(&[]),
            entry_apparent: doubles(&[]),
            entry_shares: doubles(&[]),
            entry_files_below: doubles(&[]),
            entry_empty: doubles(&[]),
            entry_duplicate: doubles(&[]),
            entry_unreadable: doubles(&[]),
            treemap_rects: doubles(&[]),
            current_allocated: 0.0,
            current_unreadable: 0,
            group_sizes: doubles(&[]),
            group_counts: doubles(&[]),
            group_verified: doubles(&[]),
            group_unreadable: doubles(&[]),
            hidden_group_count: 0,
            entry_copies: doubles(&[]),
            selected_bytes: 0.0,
            action_bytes: 0.0,
            member_groups: doubles(&[]),
            member_ids: doubles(&[]),
            member_names: QStringList::default(),
            member_paths: QStringList::default(),
            selected_ids: doubles(&[]),
            show_duplicates: false,
            show_empty: false,
            busy: false,
            start_failed: false,
            action_outcome: QString::default(),
            action_kind: QString::default(),
            action_done: 0,
            action_total: 0,
            state: Mode::Locations,
            generation: 0,
            locations: Vec::new(),
            stack: Vec::new(),
            entries: Vec::new(),
            listed: None,
            tree: None,
            findings: Findings::default(),
            verdicts: Vec::new(),
            unreadable_groups: Vec::new(),
            candidate_copies: Vec::new(),
            confirm_epoch: 0,
            action_epoch: 0,
            action: None,
            duplicate_exact: Vec::new(),
            duplicate_below: Vec::new(),
            empty_exact: Vec::new(),
            empty_below: Vec::new(),
            current: NodeId(0),
            selection: Vec::new(),
            scan: None,
            confirm: None,
        }
    }
}

fn lossy(name: &OsString) -> String {
    name.to_string_lossy().into_owned()
}

fn flag(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

/// Bytes as the doubles QML reads; exact up to 2^53, which no disk reaches.
fn bytes(value: u64) -> f64 {
    value as f64
}

/// What `enter(index)` means against the list being shown.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// Browse this path as a new root.
    Root(PathBuf),
    /// Browse this child of the current folder.
    Child(PathBuf),
    Nothing,
}

impl HematitaAnalysisRust {
    fn step(&self, index: i32) -> Step {
        let Ok(index) = usize::try_from(index) else {
            return Step::Nothing;
        };
        match self.state {
            Mode::Locations => match self.locations.get(index) {
                Some(location) if location.readable => Step::Root(location.path.clone()),
                _ => Step::Nothing,
            },
            Mode::Browsing => match (self.entries.get(index), self.stack.last()) {
                (Some(entry), Some(folder)) if entry.kind == EntryKind::Dir => {
                    Step::Child(folder.join(&entry.name))
                }
                _ => Step::Nothing,
            },
            Mode::Scanning | Mode::Analysed => Step::Nothing,
        }
    }

    /// Forgets the analysis and cancels its workers.
    fn stop_analysis(&mut self) {
        self.scan = None;
        self.stop_confirm();
        self.action = None;
        self.action_epoch = self.action_epoch.wrapping_add(1);
        self.tree = None;
        self.findings = Findings::default();
        self.verdicts.clear();
        self.unreadable_groups.clear();
        self.candidate_copies.clear();
        self.duplicate_exact.clear();
        self.duplicate_below.clear();
        self.empty_exact.clear();
        self.empty_below.clear();
        self.selection.clear();
    }

    /// The duplicate rows as the page lists them.
    fn group_rows(&self) -> Vec<analysis_view::GroupRow> {
        analysis_view::group_rows(
            &self.findings.candidates,
            &self.verdicts,
            &self.unreadable_groups,
        )
    }

    /// Marks the members of the listed duplicate rows, after a verdict: the
    /// filter keeps every listed member; only a verified copy is painted as
    /// a duplicate, and an unverified candidate carries its group's count.
    fn mark_duplicates(&mut self) {
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

    /// Cancels a running content check and moves the epoch, so nothing it
    /// queued lands.
    fn stop_confirm(&mut self) {
        self.confirm = None;
        self.confirm_epoch = self.confirm_epoch.wrapping_add(1);
    }

    /// Whether a content-check result asked under `generation` and `epoch`
    /// still describes the tree being shown.
    fn confirm_current(&self, generation: u64, epoch: u64) -> bool {
        publish::still_current(generation, self.generation) && epoch == self.confirm_epoch
    }

    /// The outermost selected entries as the action workers take them.
    fn action_items(&self) -> Vec<Item> {
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
                })
            })
            .collect()
    }

    fn selected_bytes(&self) -> f64 {
        bytes(
            self.action_items()
                .iter()
                .map(|item| item.allocated)
                .fold(0_u64, u64::saturating_add),
        )
    }

    /// Prunes the entries an action removed and brings every id-keyed cache
    /// in line; a running content check is cancelled first.
    fn prune(&mut self, removed: &[NodeId]) {
        self.stop_confirm();
        let Self {
            tree,
            findings,
            verdicts,
            unreadable_groups,
            selection,
            current,
            ..
        } = self;
        let Some(tree) = tree.as_mut() else {
            return;
        };
        let tree = Arc::make_mut(tree);
        for id in removed {
            tree.prune(*id);
        }
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
        let removed_set = removed.iter().copied().collect();
        if analysis_view::gone(tree, *current, &removed_set) {
            *current = tree.root;
        }
        let len = tree.nodes.len();
        self.empty_exact = analysis_view::marks_exact(len, self.findings.empty.iter().copied());
        if let Some(tree) = self.tree.as_ref() {
            self.empty_below =
                analysis_view::marks_below(tree, self.findings.empty.iter().copied());
        }
        self.mark_duplicates();
    }

    /// The analysed folder's ancestors below the scanned root, root excluded,
    /// outermost first.
    fn chain(&self) -> Vec<NodeId> {
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

    /// A node id QML sent, if it names a node of the tree.
    fn node_id(&self, id: i32) -> Option<NodeId> {
        let tree = self.tree.as_ref()?;
        let id = NodeId(u32::try_from(id).ok()?);
        tree.node(id).map(|_| id)
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }
}

/// The analysed lists, computed from the owned tree before any property is
/// written.
#[derive(Default)]
struct Analysed {
    ids: Vec<f64>,
    names: Vec<String>,
    kinds: Vec<String>,
    allocated: Vec<f64>,
    apparent: Vec<f64>,
    shares: Vec<f64>,
    files_below: Vec<f64>,
    empty: Vec<f64>,
    duplicate: Vec<f64>,
    unreadable: Vec<f64>,
    rects: Vec<f64>,
    current_allocated: f64,
    current_unreadable: i32,
    group_sizes: Vec<f64>,
    group_counts: Vec<f64>,
    group_verified: Vec<f64>,
    group_unreadable: Vec<f64>,
    copies: Vec<f64>,
    member_groups: Vec<f64>,
    member_ids: Vec<f64>,
    member_names: Vec<String>,
    member_paths: Vec<String>,
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

impl HematitaAnalysisRust {
    fn analysed(&self) -> Analysed {
        let mut view = Analysed::default();
        let Some(tree) = self.tree.as_ref() else {
            return view;
        };
        if self.state != Mode::Analysed {
            return view;
        }
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
        if self.show_duplicates {
            filters.push(&self.duplicate_below);
        }
        if self.show_empty {
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

        if self.show_duplicates {
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

    /// The crumbs: the browsed path down to the scanned root, then the
    /// analysed folder's ancestors below it.
    fn crumbs(&self) -> Vec<(String, String)> {
        let mut crumbs: Vec<(String, String)> = self
            .stack
            .iter()
            .enumerate()
            .map(|(depth, path)| {
                let name = if depth == 0 {
                    path.to_string_lossy().into_owned()
                } else {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default()
                };
                (name, path.to_string_lossy().into_owned())
            })
            .collect();
        if let (Mode::Analysed, Some(tree)) = (self.state, self.tree.as_ref()) {
            for id in self.chain() {
                let name = tree.node(id).map(|n| lossy(&n.name)).unwrap_or_default();
                crumbs.push((name, tree.path_of(id).to_string_lossy().into_owned()));
            }
        }
        crumbs
    }
}

impl qobject::HematitaAnalysis {
    pub fn open(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().stop_analysis();
        self.as_mut().set_busy(false);
        self.as_mut().rust_mut().state = Mode::Locations;
        self.as_mut().rust_mut().stack.clear();
        self.as_mut().rust_mut().entries.clear();
        self.as_mut().rust_mut().listed = None;
        // Busy before the revision, so nobody reading this publication takes
        // the empty in-between for the answer.
        self.as_mut().set_busy(true);
        self.as_mut().publish();
        let generation = self.as_mut().rust_mut().next_generation();
        let qt = self.qt_thread();
        let spawned = std::thread::Builder::new()
            .name("hematita-locations".to_owned())
            .spawn(move || {
                let found = locations::read_locations();
                let _ = qt.queue(move |mut hub: Pin<&mut qobject::HematitaAnalysis>| {
                    if !publish::still_current(generation, hub.rust().generation) {
                        return;
                    }
                    hub.as_mut().rust_mut().locations = found;
                    hub.as_mut().set_busy(false);
                    hub.as_mut().publish();
                });
            });
        if spawned.is_err() {
            self.as_mut().set_busy(false);
            self.as_mut().set_start_failed(true);
        }
    }

    pub fn enter(mut self: Pin<&mut Self>, index: i32) {
        match self.rust().step(index) {
            Step::Root(path) => {
                self.as_mut().rust_mut().stack = vec![path];
                self.browse();
            }
            Step::Child(path) => {
                self.as_mut().rust_mut().stack.push(path);
                self.browse();
            }
            Step::Nothing => {}
        }
    }

    /// Within the analysis, a crumb at or below the scanned root moves inside
    /// the tree; one above it leaves the analysis for that folder.
    pub fn enter_crumb(mut self: Pin<&mut Self>, index: i32) {
        let Ok(index) = usize::try_from(index) else {
            self.open();
            return;
        };
        let depth = self.rust().stack.len();
        if self.rust().state == Mode::Analysed && depth > 0 && index + 1 >= depth {
            let below = index + 1 - depth;
            let target = if below == 0 {
                self.rust().tree.as_ref().map(|tree| tree.root)
            } else {
                self.rust().chain().get(below - 1).copied()
            };
            if let Some(target) = target {
                self.as_mut().rust_mut().current = target;
                self.publish();
            }
            return;
        }
        if index < depth {
            self.as_mut().rust_mut().stack.truncate(index + 1);
            self.browse();
        }
    }

    pub fn enter_id(mut self: Pin<&mut Self>, id: i32) {
        if self.rust().state != Mode::Analysed {
            return;
        }
        let Some(id) = self.rust().node_id(id) else {
            return;
        };
        let enterable = self
            .rust()
            .tree
            .as_ref()
            .and_then(|t| t.node(id))
            .is_some_and(|n| n.kind == Kind::Dir && !n.children.is_empty());
        if enterable {
            self.as_mut().rust_mut().current = id;
            self.publish();
        }
    }

    pub fn up(mut self: Pin<&mut Self>) {
        match self.rust().state {
            Mode::Locations => {}
            Mode::Scanning => self.cancel(),
            Mode::Analysed => {
                let parent = self.rust().tree.as_ref().and_then(|tree| {
                    if self.rust().current == tree.root {
                        None
                    } else {
                        tree.node(self.rust().current).and_then(|n| n.parent)
                    }
                });
                match parent {
                    Some(parent) => {
                        self.as_mut().rust_mut().current = parent;
                        self.publish();
                    }
                    // At the scanned root: back to browsing that folder.
                    None => self.browse(),
                }
            }
            Mode::Browsing => {
                if self.rust().stack.len() > 1 {
                    self.as_mut().rust_mut().stack.pop();
                    self.browse();
                } else {
                    self.open();
                }
            }
        }
    }

    pub fn refresh(self: Pin<&mut Self>) {
        match self.rust().state {
            Mode::Locations => self.open(),
            Mode::Browsing => self.browse(),
            Mode::Scanning => {}
            Mode::Analysed => self.scan_here(),
        }
    }

    /// Lists the top of the stack on the `hematita-browse` thread. Another
    /// folder's rows are cleared at once, so an index the page sends before
    /// the new listing lands resolves against nothing rather than against the
    /// folder the person just left; the same folder's rows stay until the
    /// fresh ones replace them. Leaves any analysis.
    fn browse(mut self: Pin<&mut Self>) {
        let Some(folder) = self.rust().stack.last().cloned() else {
            self.open();
            return;
        };
        self.as_mut().rust_mut().stop_analysis();
        self.as_mut().rust_mut().state = Mode::Browsing;
        if self.rust().listed.as_ref() != Some(&folder) {
            self.as_mut().rust_mut().entries.clear();
            self.as_mut().rust_mut().listed = None;
        }
        self.as_mut().set_browse_failed(false);
        self.as_mut().set_busy(true);
        self.as_mut().publish();
        let generation = self.as_mut().rust_mut().next_generation();
        let qt = self.qt_thread();
        let spawned = std::thread::Builder::new()
            .name("hematita-browse".to_owned())
            .spawn(move || {
                let listed = browse::list_folder(&folder);
                let _ = qt.queue(move |mut hub: Pin<&mut qobject::HematitaAnalysis>| {
                    if !publish::still_current(generation, hub.rust().generation) {
                        return;
                    }
                    let failed = listed.is_err();
                    hub.as_mut().rust_mut().entries = listed.unwrap_or_default();
                    hub.as_mut().rust_mut().listed = Some(folder);
                    hub.as_mut().set_browse_failed(failed);
                    hub.as_mut().set_busy(false);
                    hub.as_mut().publish();
                });
            });
        if spawned.is_err() {
            self.as_mut().set_busy(false);
            self.as_mut().set_browse_failed(true);
            self.as_mut().set_start_failed(true);
        }
    }

    /// Writes every list from the owned state, then the mode, then the
    /// revision: a scan's result replaces its progress in one step.
    fn publish(mut self: Pin<&mut Self>) {
        let rust = self.rust();
        let mode = QString::from(rust.state.as_str());
        let crumbs = rust.crumbs();
        let crumb_names = strings(crumbs.iter().map(|(name, _)| name.clone()));
        let crumb_paths = strings(crumbs.into_iter().map(|(_, path)| path));
        let location_names = strings(rust.locations.iter().map(|l| l.name.clone()));
        let location_paths = strings(
            rust.locations
                .iter()
                .map(|l| l.path.to_string_lossy().into_owned()),
        );
        let location_kinds = strings(rust.locations.iter().map(|l| l.kind.as_str().to_owned()));
        let used: Vec<f64> = rust.locations.iter().map(|l| bytes(l.used)).collect();
        let total: Vec<f64> = rust.locations.iter().map(|l| bytes(l.total)).collect();
        let readable: Vec<f64> = rust.locations.iter().map(|l| flag(l.readable)).collect();
        let browse_names = strings(rust.entries.iter().map(|e| lossy(&e.name)));
        let browse_kinds = strings(rust.entries.iter().map(|e| e.kind.as_str().to_owned()));
        let apparent: Vec<f64> = rust.entries.iter().map(|e| bytes(e.apparent)).collect();
        let view = rust.analysed();
        let selected: Vec<f64> = rust.selection.iter().map(|id| f64::from(id.0)).collect();
        let selected_bytes = rust.selected_bytes();
        let hidden = i32::try_from(rust.findings.hidden_groups).unwrap_or(i32::MAX);

        self.as_mut().set_crumb_names(crumb_names);
        self.as_mut().set_crumb_paths(crumb_paths);
        self.as_mut().set_location_names(location_names);
        self.as_mut().set_location_paths(location_paths);
        self.as_mut().set_location_kinds(location_kinds);
        self.as_mut().set_location_used(doubles(&used));
        self.as_mut().set_location_total(doubles(&total));
        self.as_mut().set_location_readable(doubles(&readable));
        self.as_mut().set_browse_names(browse_names);
        self.as_mut().set_browse_kinds(browse_kinds);
        self.as_mut().set_browse_apparent(doubles(&apparent));
        self.as_mut().set_entry_ids(doubles(&view.ids));
        self.as_mut().set_entry_names(strings(view.names));
        self.as_mut().set_entry_kinds(strings(view.kinds));
        self.as_mut().set_entry_allocated(doubles(&view.allocated));
        self.as_mut().set_entry_apparent(doubles(&view.apparent));
        self.as_mut().set_entry_shares(doubles(&view.shares));
        self.as_mut()
            .set_entry_files_below(doubles(&view.files_below));
        self.as_mut().set_entry_empty(doubles(&view.empty));
        self.as_mut().set_entry_duplicate(doubles(&view.duplicate));
        self.as_mut()
            .set_entry_unreadable(doubles(&view.unreadable));
        self.as_mut().set_treemap_rects(doubles(&view.rects));
        self.as_mut().set_current_allocated(view.current_allocated);
        self.as_mut()
            .set_current_unreadable(view.current_unreadable);
        self.as_mut().set_group_sizes(doubles(&view.group_sizes));
        self.as_mut().set_group_counts(doubles(&view.group_counts));
        self.as_mut()
            .set_group_verified(doubles(&view.group_verified));
        self.as_mut()
            .set_member_groups(doubles(&view.member_groups));
        self.as_mut().set_member_ids(doubles(&view.member_ids));
        self.as_mut().set_member_names(strings(view.member_names));
        self.as_mut().set_member_paths(strings(view.member_paths));
        self.as_mut()
            .set_group_unreadable(doubles(&view.group_unreadable));
        self.as_mut().set_hidden_group_count(hidden);
        self.as_mut().set_entry_copies(doubles(&view.copies));
        self.as_mut().set_selected_ids(doubles(&selected));
        self.as_mut().set_selected_bytes(selected_bytes);
        self.as_mut().set_mode(mode);
        // Last, so the page rebuilds once, with every list in place.
        let next = self.rust().revision.wrapping_add(1).max(1);
        self.as_mut().set_revision(next);
    }

    /// Busy while a content check or an action runs.
    fn refresh_busy(mut self: Pin<&mut Self>) {
        let busy = self.rust().confirm.is_some() || self.rust().action.is_some();
        self.as_mut().set_busy(busy);
    }

    /// A scan that did not produce a tree; the section goes back to browsing.
    fn scan_failed(mut self: Pin<&mut Self>) {
        self.as_mut().set_action_kind(QString::default());
        self.as_mut().set_action_done(0);
        self.as_mut().set_action_total(0);
        self.as_mut().set_action_outcome(QString::from(FAILED));
    }

    pub fn scan_here(mut self: Pin<&mut Self>) {
        if !matches!(self.rust().state, Mode::Browsing | Mode::Analysed) {
            return;
        }
        let Some(root) = self.rust().stack.last().cloned() else {
            return;
        };
        self.as_mut().rust_mut().stop_analysis();
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
                let rust = self.as_mut().rust_mut();
                let rust = rust.get_mut();
                rust.empty_exact =
                    analysis_view::marks_exact(tree.nodes.len(), findings.empty.iter().copied());
                rust.empty_below =
                    analysis_view::marks_below(&tree, findings.empty.iter().copied());
                rust.verdicts = vec![None; findings.candidates.len()];
                rust.unreadable_groups = vec![false; findings.candidates.len()];
                rust.current = tree.root;
                rust.findings = findings;
                rust.tree = Some(Arc::new(tree));
                rust.selection.clear();
                rust.mark_duplicates();
                rust.state = Mode::Analysed;
                self.publish();
            }
            Err(ScanError::Cancelled) => self.browse(),
            Err(_) => {
                self.as_mut().scan_failed();
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
            Mode::Analysed if self.rust().confirm.is_some() => {
                self.as_mut().rust_mut().stop_confirm();
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
        if self.rust().state != Mode::Analysed || self.rust().confirm.is_some() {
            return;
        }
        let Some(tree) = self.rust().tree.clone() else {
            return;
        };
        let pending: Vec<(usize, Group)> = self
            .rust()
            .findings
            .candidates
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                self.rust()
                    .verdicts
                    .get(*index)
                    .is_some_and(Option::is_none)
            })
            .map(|(index, group)| (index, group.clone()))
            .collect();
        if pending.is_empty() {
            return;
        }
        let generation = self.rust().generation;
        self.as_mut().rust_mut().stop_confirm();
        let epoch = self.rust().confirm_epoch;
        let qt = self.qt_thread();
        match usage_worker::spawn_confirm(tree, pending, generation, epoch, qt) {
            Ok(handle) => {
                self.as_mut().rust_mut().confirm = Some(handle);
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
        if !self.rust().confirm_current(generation, epoch) || self.rust().confirm.is_none() {
            return;
        }
        let rust = self.as_mut().rust_mut();
        let rust = rust.get_mut();
        let Some(slot) = rust.verdicts.get_mut(index) else {
            return;
        };
        *slot = Some(verified);
        rust.mark_duplicates();
        self.publish();
    }

    /// A copy of candidate `index` could not be read; its row says so.
    pub(crate) fn apply_unreadable(
        mut self: Pin<&mut Self>,
        generation: u64,
        epoch: u64,
        index: usize,
    ) {
        if !self.rust().confirm_current(generation, epoch) || self.rust().confirm.is_none() {
            return;
        }
        let Some(slot) = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .unreadable_groups
            .get_mut(index)
        else {
            return;
        };
        *slot = true;
        self.publish();
    }

    pub(crate) fn confirm_finished(mut self: Pin<&mut Self>, generation: u64, epoch: u64) {
        if !self.rust().confirm_current(generation, epoch) {
            return;
        }
        self.as_mut().rust_mut().confirm = None;
        self.refresh_busy();
    }

    pub fn toggle_selected(mut self: Pin<&mut Self>, id: i32) {
        let Some(id) = self.rust().node_id(id) else {
            return;
        };
        if self
            .rust()
            .tree
            .as_ref()
            .is_some_and(|tree| tree.root == id)
        {
            return;
        }
        let selection = &mut self.as_mut().rust_mut().get_mut().selection;
        match selection.iter().position(|chosen| *chosen == id) {
            Some(at) => {
                selection.remove(at);
            }
            None => selection.push(id),
        }
        self.publish_selection();
    }

    pub fn clear_selection(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().selection.clear();
        self.publish_selection();
    }

    pub fn select_all_but_one(mut self: Pin<&mut Self>, group: i32) {
        let Ok(group) = usize::try_from(group) else {
            return;
        };
        let Some(row) = self.rust().group_rows().into_iter().nth(group) else {
            return;
        };
        if !row.verified || row.unreadable {
            return;
        }
        let selection = &mut self.as_mut().rust_mut().get_mut().selection;
        for id in analysis_view::all_but_one(&row.nodes) {
            if !selection.contains(&id) {
                selection.push(id);
            }
        }
        self.publish_selection();
    }

    /// The selection alone changes; rows read it without being rebuilt.
    fn publish_selection(mut self: Pin<&mut Self>) {
        let selected: Vec<f64> = self
            .rust()
            .selection
            .iter()
            .map(|id| f64::from(id.0))
            .collect();
        self.as_mut().set_selected_ids(doubles(&selected));
        let selected_bytes = self.rust().selected_bytes();
        self.as_mut().set_selected_bytes(selected_bytes);
    }

    /// Whether an action may start: the analysis is shown and no other
    /// action is running.
    fn may_act(&self) -> bool {
        self.rust().state == Mode::Analysed && self.rust().action.is_none()
    }

    /// Clears the last outcome and takes the next action epoch.
    fn begin_action(mut self: Pin<&mut Self>) -> u64 {
        self.as_mut().set_action_outcome(QString::default());
        let rust = self.as_mut().rust_mut().get_mut();
        rust.action_epoch = rust.action_epoch.wrapping_add(1);
        rust.action_epoch
    }

    pub fn open_selected(mut self: Pin<&mut Self>) {
        if !self.may_act() {
            return;
        }
        let Some(item) = self.rust().action_items().into_iter().next() else {
            return;
        };
        let epoch = self.as_mut().begin_action();
        let generation = self.rust().generation;
        let qt = self.qt_thread();
        if actions::spawn_open(item.path, generation, epoch, qt).is_err() {
            self.as_mut().set_start_failed(true);
        }
    }

    pub fn trash_selected(mut self: Pin<&mut Self>) {
        if !self.may_act() {
            return;
        }
        let items = self.rust().action_items();
        if items.is_empty() {
            return;
        }
        let epoch = self.as_mut().begin_action();
        let generation = self.rust().generation;
        let qt = self.qt_thread();
        match actions::spawn_trash(items, generation, epoch, qt) {
            Ok(handle) => {
                self.as_mut().rust_mut().action = Some(handle);
                self.refresh_busy();
            }
            Err(_) => self.as_mut().set_start_failed(true),
        }
    }

    pub fn delete_selected(mut self: Pin<&mut Self>) {
        if !self.may_act() {
            return;
        }
        let items = self.rust().action_items();
        let Some(within) = self.rust().tree.as_ref().map(|tree| tree.path.clone()) else {
            return;
        };
        if items.is_empty() {
            return;
        }
        let epoch = self.as_mut().begin_action();
        let generation = self.rust().generation;
        let qt = self.qt_thread();
        match actions::spawn_delete(items, within, generation, epoch, qt) {
            Ok(handle) => {
                self.as_mut().rust_mut().action = Some(handle);
                self.refresh_busy();
            }
            Err(_) => self.as_mut().set_start_failed(true),
        }
    }

    /// An action's report: its outcome, then — for what it removed — the
    /// pruned tree, republished.
    pub(crate) fn apply_action(
        mut self: Pin<&mut Self>,
        generation: u64,
        epoch: u64,
        report: ActionReport,
    ) {
        if !publish::still_current(generation, self.rust().generation)
            || epoch != self.rust().action_epoch
        {
            return;
        }
        self.as_mut().rust_mut().action = None;
        let freed = report
            .removed
            .iter()
            .map(|(_, allocated)| *allocated)
            .fold(0_u64, u64::saturating_add);
        let count = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
        self.as_mut()
            .set_action_kind(QString::from(report.kind.as_str()));
        self.as_mut().set_action_done(count(report.done));
        self.as_mut().set_action_total(count(report.total));
        self.as_mut().set_action_bytes(bytes(freed));
        self.as_mut()
            .set_action_outcome(QString::from(report.outcome));
        if !report.removed.is_empty() && self.rust().state == Mode::Analysed {
            let removed: Vec<NodeId> = report.removed.iter().map(|(id, _)| *id).collect();
            self.as_mut().rust_mut().get_mut().prune(&removed);
            self.as_mut().refresh_busy();
            self.publish();
        } else {
            self.refresh_busy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locations::LocationKind;

    fn location(path: &str, readable: bool) -> Location {
        Location {
            name: String::new(),
            path: PathBuf::from(path),
            kind: LocationKind::Disk,
            used: 0,
            total: 0,
            readable,
        }
    }

    fn entry(name: &str, kind: EntryKind) -> Entry {
        Entry {
            name: OsString::from(name),
            kind,
            apparent: 0,
        }
    }

    #[test]
    fn an_index_enters_a_readable_location_as_a_root() {
        let state = HematitaAnalysisRust {
            locations: vec![location("/mnt/a", true), location("/mnt/b", false)],
            ..HematitaAnalysisRust::default()
        };
        assert_eq!(state.step(0), Step::Root(PathBuf::from("/mnt/a")));
        assert_eq!(state.step(1), Step::Nothing, "unreadable");
        assert_eq!(state.step(2), Step::Nothing, "out of range");
        assert_eq!(state.step(-1), Step::Nothing);
    }

    #[test]
    fn an_index_enters_only_a_folder_of_the_browsed_listing() {
        let state = HematitaAnalysisRust {
            state: Mode::Browsing,
            stack: vec![PathBuf::from("/home"), PathBuf::from("/home/toni")],
            entries: vec![
                entry("docs", EntryKind::Dir),
                entry("a.txt", EntryKind::File),
            ],
            ..HematitaAnalysisRust::default()
        };
        assert_eq!(state.step(0), Step::Child(PathBuf::from("/home/toni/docs")));
        assert_eq!(state.step(1), Step::Nothing);
    }

    #[test]
    fn a_name_that_is_not_utf8_is_entered_with_its_exact_bytes() {
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(vec![b'x', 0xff]);
        let state = HematitaAnalysisRust {
            state: Mode::Browsing,
            stack: vec![PathBuf::from("/tmp")],
            entries: vec![Entry {
                name: raw.clone(),
                kind: EntryKind::Dir,
                apparent: 0,
            }],
            ..HematitaAnalysisRust::default()
        };
        assert_eq!(state.step(0), Step::Child(PathBuf::from("/tmp").join(raw)));
    }

    #[test]
    fn each_request_takes_a_newer_generation() {
        let mut state = HematitaAnalysisRust::default();
        let first = state.next_generation();
        let second = state.next_generation();
        assert!(!publish::still_current(first, state.generation));
        assert!(publish::still_current(second, state.generation));
    }

    #[test]
    fn a_verdict_from_an_older_confirm_epoch_is_dropped() {
        let mut state = HematitaAnalysisRust::default();
        let generation = state.next_generation();
        state.stop_confirm();
        let first = state.confirm_epoch;
        assert!(state.confirm_current(generation, first));
        // A cancel, a restart or a pruning moves the epoch.
        state.stop_confirm();
        assert!(!state.confirm_current(generation, first));
        assert!(state.confirm_current(generation, state.confirm_epoch));
        state.next_generation();
        assert!(!state.confirm_current(generation, state.confirm_epoch));
    }
}
