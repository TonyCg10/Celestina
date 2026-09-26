//! The storage section's state, as Qt properties: the mount locations, the
//! folder being browsed, the scan's progress and the analysed tree, its
//! duplicates, empty folders and the selection.
//!
//! Every disk read happens on a named worker thread (`hematita-locations`,
//! `hematita-browse`, `hematita-scan`, `hematita-confirm`,
//! `hematita-actions`, `hematita-graft`); each request takes
//! the next generation and a result that comes back under an older one is
//! dropped, so a person clicking faster than the disk answers only ever sees
//! the folder they are in. A new scan, a cancellation or leaving the analysis
//! drops the worker handles, which cancels them.
//!
//! The scanned tree lives here as an `Arc<Tree>`; navigating it answers from
//! memory. QML names a node by the id it was published with, never by path.
//! An action that removed entries prunes exactly the ids its worker reported
//! gone and re-aggregates the ancestors instead of scanning again; a
//! deletion that stopped inside an item has that folder scanned again on a
//! `hematita-graft` thread and the fresh subtree grafted in place of the
//! stale one. The session state and every rule about ids live in
//! [`crate::analysis_session`]; this file is the bridge, the locations and
//! browsing state, the workers' spawning and the publication.
//!
//! The path being browsed is a stack of byte-exact `PathBuf`s owned here.
//! QML never hands a path back: it names a row by index and the hub resolves
//! the index against the list it published. Names cross lossily, for display
//! only. Nothing here is prose a person reads: modes, kinds and outcomes are
//! tokens.

use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use crate::analysis_session::{bytes, flag, lossy, Analysed, Session};
use crate::browse::{self, Entry, EntryKind};
use crate::lists::{doubles, strings};
use crate::locations::{self, Location};
use crate::publish;
use crate::usage_worker::WorkerHandle;

#[path = "analysis_workers.rs"]
mod workers;

/// A scan that could not start or could not read its root.
const FAILED: &str = "failed";
/// A confirmed action whose selection no longer matches what was asked.
const REFUSED: &str = "refused";

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
        // busy — a locations, browse, content-check or graft read is in flight
        // hiddenGroupCount — candidate groups beyond the listed cap
        // groupUnreadable — a copy of the group could not be read
        // entryCopies — the unverified candidate group size of the entry
        // selectedBytes, selectedCount — what an action on the selection
        // would free and touch (entries inside a selected folder count once)
        // selectionRevision — moved by every change of the selected set; a
        // confirmation hands back the one it was asked under
        // actionRunning — a trash or deletion is in flight; cancelAction stops it
        // actionOutcome — "" | done | partial | failed | refused | cancelled
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
        #[qproperty(i32, selected_count)]
        #[qproperty(i32, selection_revision)]
        #[qproperty(bool, action_running)]
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

        /// Browses `path` as a new stack of one folder, as if the person had
        /// entered it; ignored with a diagnostic when it is not a folder.
        #[qinvokable]
        fn open_path(self: Pin<&mut HematitaAnalysis>, path: &QString);

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

        /// Moves the selection to the trash; the page has asked first
        /// under `selectionRevision` `asked`, and a selection changed since
        /// is refused.
        #[qinvokable]
        fn trash_selected(self: Pin<&mut HematitaAnalysis>, asked: i32);

        /// Deletes the selection permanently; the page has asked first
        /// under `selectionRevision` `asked`, and a selection changed since
        /// is refused.
        #[qinvokable]
        fn delete_selected(self: Pin<&mut HematitaAnalysis>, asked: i32);

        /// Stops a running trash or deletion; what it already removed is
        /// still reported and pruned.
        #[qinvokable]
        fn cancel_action(self: Pin<&mut HematitaAnalysis>);
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
    selected_count: i32,
    selection_revision: i32,
    action_running: bool,
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
    /// The running scan; dropping it cancels the walk.
    scan: Option<WorkerHandle>,
    /// The analysed tree and everything keyed by its ids.
    session: Session,
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
            selected_count: 0,
            selection_revision: 0,
            action_running: false,
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
            scan: None,
            session: Session::default(),
        }
    }
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

    /// Whether a content-check result asked under `generation` and `epoch`
    /// still describes the tree being shown.
    fn confirm_current(&self, generation: u64, epoch: u64) -> bool {
        publish::still_current(generation, self.generation) && self.session.confirm_current(epoch)
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
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
        if let (Mode::Analysed, Some(tree)) = (self.state, self.session.tree.as_ref()) {
            for id in self.session.chain() {
                let name = tree.node(id).map(|n| lossy(&n.name)).unwrap_or_default();
                crumbs.push((name, tree.path_of(id).to_string_lossy().into_owned()));
            }
        }
        crumbs
    }
}

impl qobject::HematitaAnalysis {
    pub fn open(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().session.reset();
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

    /// The folder handed on the command line or through D-Bus `Open`. The
    /// one `is_dir` check is a single `stat` on the Qt thread, once per
    /// request, the same cost `browse` pays to name its folder; the listing
    /// itself runs on the browse thread. The path is kept as handed: a scan
    /// resolves its links on the scan thread, and the tree and every
    /// deletion bounded by it use the resolved path.
    pub fn open_path(mut self: Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        if !path.is_dir() {
            eprintln!("hematita: not a folder, ignored: {}", path.display());
            return;
        }
        self.as_mut().rust_mut().stack = vec![path];
        self.browse();
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
            let session = &self.rust().session;
            let target = if below == 0 {
                session.tree.as_ref().map(|tree| tree.root)
            } else {
                session.chain().get(below - 1).copied()
            };
            if let Some(target) = target {
                self.as_mut().rust_mut().session.current = target;
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
        let Some(id) = self.rust().session.node_id(id) else {
            return;
        };
        if self.rust().session.enterable(id) {
            self.as_mut().rust_mut().session.current = id;
            self.publish();
        }
    }

    pub fn up(mut self: Pin<&mut Self>) {
        match self.rust().state {
            Mode::Locations => {}
            Mode::Scanning => self.cancel(),
            Mode::Analysed => match self.rust().session.parent() {
                Some(parent) => {
                    self.as_mut().rust_mut().session.current = parent;
                    self.publish();
                }
                // At the scanned root: back to browsing that folder.
                None => self.browse(),
            },
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
        self.as_mut().rust_mut().session.reset();
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
        let view = if rust.state == Mode::Analysed {
            rust.session.view(rust.show_duplicates, rust.show_empty)
        } else {
            Analysed::default()
        };
        let hidden = i32::try_from(rust.session.findings.hidden.len()).unwrap_or(i32::MAX);

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
        self.as_mut().publish_selection();
        self.as_mut().refresh_busy();
        self.as_mut().set_mode(mode);
        // Last, so the page rebuilds once, with every list in place.
        let next = self.rust().revision.wrapping_add(1).max(1);
        self.as_mut().set_revision(next);
    }

    /// Busy while a content check, an action or a graft runs; the locations
    /// and browse reads set it themselves.
    fn refresh_busy(mut self: Pin<&mut Self>) {
        let running = self.rust().session.action.is_some();
        let busy = self.rust().session.busy();
        self.as_mut().set_action_running(running);
        if self.rust().state == Mode::Analysed || busy {
            self.as_mut().set_busy(busy);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

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
}
