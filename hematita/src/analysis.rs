//! The storage section's state, as Qt properties: the mount locations, the
//! folder being browsed and — once the scan lands in S1-C — the analysed
//! tree, its duplicates and the selection.
//!
//! Every disk read happens on a named worker thread (`hematita-locations`,
//! `hematita-browse`); each request takes the next generation and a result
//! that comes back under an older one is dropped, so a person clicking
//! faster than the disk answers only ever sees the folder they are in.
//!
//! The path being browsed is a stack of byte-exact `PathBuf`s owned here.
//! QML never hands a path back: it names a row by index and the hub resolves
//! the index against the list it published. Names cross lossily, for display
//! only. Nothing here is prose a person reads: modes, kinds and outcomes are
//! tokens.

use std::ffi::OsString;
use std::path::PathBuf;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use crate::browse::{self, Entry, EntryKind};
use crate::lists::{doubles, strings};
use crate::locations::{self, Location};
use crate::publish;

/// The answer an invokable gives while the unit that fills it has not landed.
const REFUSED: &str = "refused";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Locations,
    Browsing,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Locations => "locations",
            Self::Browsing => "browsing",
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
        // progress*, entry*, treemapRects, current*, group*, member*,
        //   selectedIds, showDuplicates, showEmpty — the scan's contract,
        //   published empty until S1-C and S1-D fill it
        // revision — bumped once, after every list is in place
        // busy — a locations or browse read is in flight
        // action* — the last action's typed outcome
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

        /// Scans the browsed folder. Answers `refused` until S1-C.
        #[qinvokable]
        fn scan_here(self: Pin<&mut HematitaAnalysis>);

        /// Cancels the running scan. Answers `refused` until S1-C.
        #[qinvokable]
        fn cancel(self: Pin<&mut HematitaAnalysis>);

        /// Verifies duplicate candidates by content. Answers `refused` until
        /// S1-C.
        #[qinvokable]
        fn confirm_duplicates(self: Pin<&mut HematitaAnalysis>);

        /// Answers `refused` until S1-D.
        #[qinvokable]
        fn toggle_selected(self: Pin<&mut HematitaAnalysis>, id: i32);

        /// Answers `refused` until S1-D.
        #[qinvokable]
        fn clear_selection(self: Pin<&mut HematitaAnalysis>);

        /// Answers `refused` until S1-D.
        #[qinvokable]
        fn select_all_but_one(self: Pin<&mut HematitaAnalysis>, group: i32);

        /// Answers `refused` until S1-D.
        #[qinvokable]
        fn open_selected(self: Pin<&mut HematitaAnalysis>);

        /// Answers `refused` until S1-D.
        #[qinvokable]
        fn trash_selected(self: Pin<&mut HematitaAnalysis>);

        /// Answers `refused` until S1-D.
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
        }
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }
}

impl qobject::HematitaAnalysis {
    pub fn open(mut self: Pin<&mut Self>) {
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

    pub fn enter_crumb(mut self: Pin<&mut Self>, index: i32) {
        match usize::try_from(index) {
            Ok(index) if index < self.rust().stack.len() => {
                self.as_mut().rust_mut().stack.truncate(index + 1);
                self.browse();
            }
            Ok(_) => {}
            Err(_) => self.open(),
        }
    }

    pub fn up(mut self: Pin<&mut Self>) {
        if self.rust().state != Mode::Browsing {
            return;
        }
        if self.rust().stack.len() > 1 {
            self.as_mut().rust_mut().stack.pop();
            self.browse();
        } else {
            self.open();
        }
    }

    pub fn refresh(self: Pin<&mut Self>) {
        match self.rust().state {
            Mode::Locations => self.open(),
            Mode::Browsing => self.browse(),
        }
    }

    /// Lists the top of the stack on the `hematita-browse` thread. Another
    /// folder's rows are cleared at once, so an index the page sends before
    /// the new listing lands resolves against nothing rather than against the
    /// folder the person just left; the same folder's rows stay until the
    /// fresh ones replace them.
    fn browse(mut self: Pin<&mut Self>) {
        let Some(folder) = self.rust().stack.last().cloned() else {
            self.open();
            return;
        };
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

    /// Writes every list from the owned state, then the revision.
    fn publish(mut self: Pin<&mut Self>) {
        let rust = self.rust();
        let mode = QString::from(rust.state.as_str());
        let crumb_names = strings(rust.stack.iter().enumerate().map(|(depth, path)| {
            if depth == 0 {
                path.to_string_lossy().into_owned()
            } else {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            }
        }));
        let crumb_paths = strings(
            rust.stack
                .iter()
                .map(|path| path.to_string_lossy().into_owned()),
        );
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

        self.as_mut().set_mode(mode);
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
        // Last, so the page rebuilds once, with every list in place.
        let next = self.rust().revision.wrapping_add(1).max(1);
        self.as_mut().set_revision(next);
    }

    /// The answer of an invokable whose unit has not landed yet.
    fn refuse(mut self: Pin<&mut Self>, kind: &str) {
        self.as_mut().set_action_kind(QString::from(kind));
        self.as_mut().set_action_done(0);
        self.as_mut().set_action_total(0);
        self.as_mut().set_action_outcome(QString::from(REFUSED));
    }

    pub fn scan_here(self: Pin<&mut Self>) {
        self.refuse("");
    }

    pub fn cancel(self: Pin<&mut Self>) {
        self.refuse("");
    }

    pub fn confirm_duplicates(self: Pin<&mut Self>) {
        self.refuse("");
    }

    pub fn toggle_selected(self: Pin<&mut Self>, _id: i32) {
        self.refuse("");
    }

    pub fn clear_selection(self: Pin<&mut Self>) {
        self.refuse("");
    }

    pub fn select_all_but_one(self: Pin<&mut Self>, _group: i32) {
        self.refuse("");
    }

    pub fn open_selected(self: Pin<&mut Self>) {
        self.refuse("open");
    }

    pub fn trash_selected(self: Pin<&mut Self>) {
        self.refuse("trash");
    }

    pub fn delete_selected(self: Pin<&mut Self>) {
        self.refuse("delete");
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
}
