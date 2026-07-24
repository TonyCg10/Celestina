use core::pin::Pin;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use siderita_core::{
    DirectorySnapshot, NavigationHistory, PublishOutcome, ScanCoordinator, ScanExecutor,
    ScanResult, SortDirection, SortField, ViewOptions, WatchState,
};

/// The filesystem debouncer type kept alive for the controller's lifetime.
type FsDebouncer = Debouncer<RecommendedWatcher, RecommendedCache>;
use siderita_ops::{OpError, Progress};
use siderita_qt::{EntryRow, RowKind, SnapshotAdapter, ViewSnapshot};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;

        // The hand-written system-clipboard shim (see cpp/clipboard.cpp).
        include!("siderita/clipboard.h");

        #[rust_name = "system_clipboard_set_uris"]
        fn siderita_set_clipboard_uris(paths: &QStringList, cut: bool);

        #[rust_name = "system_clipboard_read_uris"]
        fn siderita_read_clipboard_uris() -> QStringList;

        #[rust_name = "system_clipboard_is_cut"]
        fn siderita_clipboard_is_cut() -> bool;

        #[rust_name = "system_clipboard_has_uris"]
        fn siderita_clipboard_has_uris() -> bool;

        #[rust_name = "system_clipboard_clear"]
        fn siderita_clear_clipboard();

        // The hand-written native list model (see cpp/entrymodel.cpp).
        include!("siderita/entrymodel.h");

        #[rust_name = "register_entry_model"]
        fn register_siderita_entry_model();
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, current_path)]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, error_text)]
        #[qproperty(QStringList, entry_names)]
        #[qproperty(QString, selected_token)]
        #[qproperty(QString, query)]
        #[qproperty(bool, loading)]
        #[qproperty(bool, can_go_back)]
        #[qproperty(bool, can_go_forward)]
        #[qproperty(bool, can_go_up)]
        #[qproperty(bool, show_hidden)]
        #[qproperty(i32, sort_field)]
        #[qproperty(bool, sort_ascending)]
        #[qproperty(QStringList, bookmark_names)]
        #[qproperty(QStringList, bookmark_paths)]
        #[qproperty(QString, op_error)]
        #[qproperty(bool, can_paste)]
        // Absolute paths currently held as a *cut* (not a copy); the views ghost
        // any visible entry whose path is in this list. Empty for a copy.
        #[qproperty(QStringList, cut_paths)]
        #[qproperty(bool, can_undo)]
        #[qproperty(QString, undo_label)]
        #[qproperty(bool, op_running)]
        #[qproperty(QString, op_current)]
        #[qproperty(QString, op_detail)]
        #[qproperty(i32, op_done)]
        #[qproperty(i32, op_total)]
        #[qproperty(bool, conflict_pending)]
        #[qproperty(i32, conflict_count)]
        #[qproperty(QString, conflict_name)]
        #[qproperty(QStringList, trash_names)]
        #[qproperty(QStringList, trash_origins)]
        #[qproperty(QStringList, trash_dates)]
        #[qproperty(bool, open_with_pending)]
        #[qproperty(QString, open_with_target)]
        #[qproperty(QStringList, open_with_apps)]
        #[qproperty(i32, open_with_default_index)]
        #[qproperty(QStringList, volume_names)]
        #[qproperty(QStringList, volume_devices)]
        #[qproperty(QStringList, volume_mounts)]
        #[qproperty(bool, volume_busy)]
        #[qproperty(i32, hidden_device_count)]
        #[qproperty(bool, watch_degraded)]
        #[qproperty(QString, folder_size)]
        // Mirrors the QML multi-selection count so the window-scope info box can
        // read it from the active tab's controller.
        #[qproperty(i32, selection_count)]
        #[qproperty(bool, properties_pending)]
        #[qproperty(QString, prop_name)]
        #[qproperty(QString, prop_path)]
        #[qproperty(QString, prop_kind)]
        #[qproperty(QString, prop_mime)]
        #[qproperty(QString, prop_size)]
        #[qproperty(QString, prop_permissions)]
        #[qproperty(QString, prop_owner)]
        #[qproperty(QString, prop_modified)]
        #[qproperty(QString, prop_accessed)]
        #[qproperty(QString, prop_symlink)]
        #[qproperty(bool, prop_is_dir)]
        #[qproperty(bool, search_active)]
        #[qproperty(bool, search_running)]
        #[qproperty(QString, search_query)]
        #[qproperty(QString, search_summary)]
        #[qproperty(QStringList, search_names)]
        #[qproperty(QStringList, search_paths)]
        #[qproperty(QStringList, search_kinds)]
        type SideritaController = super::SideritaControllerRust;

        #[qinvokable]
        fn start(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn start_at(self: Pin<&mut SideritaController>, location: &QString);

        #[qinvokable]
        fn reload_bookmarks(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn refresh(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn go_home(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn go_back(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn go_forward(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn go_up(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn open_location(self: Pin<&mut SideritaController>, location: &QString);

        #[qinvokable]
        fn toggle_hidden(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn change_sort_field(self: Pin<&mut SideritaController>, field: i32);

        #[qinvokable]
        fn toggle_sort_direction(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn apply_query(self: Pin<&mut SideritaController>, query: &QString);

        #[qinvokable]
        fn select_token(self: Pin<&mut SideritaController>, token: &QString);

        #[qinvokable]
        fn activate_token(self: Pin<&mut SideritaController>, token: &QString);

        #[qinvokable]
        fn entry_token(self: &SideritaController, index: i32) -> QString;

        #[qinvokable]
        fn index_for_token(self: &SideritaController, token: &QString) -> i32;

        /// "Kind · size · date" for the entry at `index` — the info panel's line
        /// for a single selected item.
        #[qinvokable]
        fn entry_detail(self: &SideritaController, index: i32) -> QString;

        #[qinvokable]
        fn entry_path(self: &SideritaController, index: i32) -> QString;

        #[qinvokable]
        fn add_bookmark(self: Pin<&mut SideritaController>, path: &QString);

        #[qinvokable]
        fn remove_bookmark(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn rename_bookmark(self: Pin<&mut SideritaController>, index: i32, name: &QString);

        #[qinvokable]
        fn place_path(self: &SideritaController, key: &QString) -> QString;

        #[qinvokable]
        fn new_folder(self: Pin<&mut SideritaController>, name: &QString);

        #[qinvokable]
        fn new_file(self: Pin<&mut SideritaController>, name: &QString);

        #[qinvokable]
        fn rename_path(self: Pin<&mut SideritaController>, path: &QString, new_name: &QString);

        #[qinvokable]
        fn trash_path(self: Pin<&mut SideritaController>, path: &QString);

        #[qinvokable]
        fn trash_paths(self: Pin<&mut SideritaController>, paths: &QStringList);

        #[qinvokable]
        fn copy_to_clipboard(self: Pin<&mut SideritaController>, path: &QString, cut: bool);

        #[qinvokable]
        fn copy_paths_to_clipboard(
            self: Pin<&mut SideritaController>,
            paths: &QStringList,
            cut: bool,
        );

        #[qinvokable]
        fn clear_clipboard(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn refresh_paste_state(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn paste(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn drop_uris(
            self: Pin<&mut SideritaController>,
            paths: &QStringList,
            destination: &QString,
            move_entries: bool,
        );

        #[qinvokable]
        fn cancel_op(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn resolve_conflicts(self: Pin<&mut SideritaController>, strategy: &QString);

        #[qinvokable]
        fn cancel_conflicts(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn undo(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn load_trash(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn restore_trash(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn restore_all_trash(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn empty_trash(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn open_with(self: Pin<&mut SideritaController>, path: &QString);

        #[qinvokable]
        fn open_with_app(self: Pin<&mut SideritaController>, index: i32, set_default: bool);

        #[qinvokable]
        fn cancel_open_with(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn load_volumes(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn mount_volume(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn unmount_volume(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn open_volume(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn open_properties(self: Pin<&mut SideritaController>, path: &QString);

        #[qinvokable]
        fn close_properties(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn search_recursive(self: Pin<&mut SideritaController>, query: &QString);

        #[qinvokable]
        fn cancel_search(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn close_search(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn open_terminal(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn saved_view_mode(self: &SideritaController) -> QString;

        #[qinvokable]
        fn saved_content_icon_scale(self: &SideritaController) -> f64;

        #[qinvokable]
        fn saved_content_text_scale(self: &SideritaController) -> f64;

        #[qinvokable]
        fn saved_interface_icon_scale(self: &SideritaController) -> f64;

        #[qinvokable]
        fn saved_interface_text_scale(self: &SideritaController) -> f64;

        #[qinvokable]
        fn saved_sidebar_icon_scale(self: &SideritaController) -> f64;

        #[qinvokable]
        fn saved_sidebar_text_scale(self: &SideritaController) -> f64;

        #[qinvokable]
        fn save_view_mode(self: Pin<&mut SideritaController>, mode: &QString);

        /// Persists the four independent size scales (content icons/text, sidebar
        /// icons/text).
        #[qinvokable]
        fn save_sizing(
            self: Pin<&mut SideritaController>,
            content_icon: f64,
            content_text: f64,
            interface_icon: f64,
            interface_text: f64,
            sidebar_icon: f64,
            sidebar_text: f64,
        );

        #[qinvokable]
        fn hide_device(self: Pin<&mut SideritaController>, name: &QString);

        #[qinvokable]
        fn unhide_all_devices(self: Pin<&mut SideritaController>);

        /// Emitted whenever the projected view changes; the QML feeds it straight
        /// into the native SideritaEntryModel (parallel role columns).
        #[qsignal]
        fn rows_ready(
            self: Pin<&mut SideritaController>,
            names: QStringList,
            tokens: QStringList,
            kinds: QStringList,
            subtitles: QStringList,
            paths: QStringList,
            sections: QStringList,
        );
    }

    impl cxx_qt::Threading for SideritaController {}
}

/// How to reverse the last loss-free operation. Only the three verbs the
/// roadmap names as undoable are recorded — create and copy are not, since
/// undoing them would mean deleting data the user did not ask to lose.
enum UndoAction {
    /// A rename: the entry now sits at `renamed`; put its `old_name` back.
    Rename {
        renamed: PathBuf,
        old_name: OsString,
    },
    /// One or more moves (a cut-paste): move each entry from where it landed
    /// back into the directory it came from.
    Move { entries: Vec<(PathBuf, PathBuf)> },
    /// One or more sends-to-Trash: restore each from its recorded `.trashinfo`.
    Trash { infos: Vec<PathBuf> },
}

impl UndoAction {
    /// A short Spanish label for what undo will reverse, for the menu/tooltip.
    fn label(&self) -> &'static str {
        match self {
            Self::Rename { .. } => "Deshacer renombrar",
            Self::Move { .. } => "Deshacer mover",
            Self::Trash { .. } => "Deshacer enviar a la papelera",
        }
    }
}

/// How to resolve entries whose paste destination already exists.
#[derive(Clone, Copy)]
enum ConflictStrategy {
    /// Leave the existing entry; the source is not pasted.
    Skip,
    /// Send the existing entry to Trash (recoverable), then paste over it.
    Replace,
    /// Paste beside the existing entry under a freed "(copia)" name.
    KeepBoth,
}

impl ConflictStrategy {
    fn from_key(key: &str) -> Option<Self> {
        match key {
            "skip" => Some(Self::Skip),
            "replace" => Some(Self::Replace),
            "keepboth" => Some(Self::KeepBoth),
            _ => None,
        }
    }
}

/// A paste held back because at least one destination already exists, waiting
/// for the user's conflict choice before the worker starts.
struct PendingPaste {
    sources: Vec<PathBuf>,
    destination: PathBuf,
    cut: bool,
}

/// What a pasted batch did, carried from the worker thread to `finish_paste`.
struct PasteOutcome {
    total: usize,
    failures: Vec<String>,
    /// Cut sources that could not be moved (kept on the clipboard for a retry).
    unmoved: Vec<PathBuf>,
    /// Plain (non-colliding) moves, for the undo record.
    undo_moves: Vec<(PathBuf, PathBuf)>,
    skipped: usize,
    /// Whether any entry went through replace/keep-both, which makes the batch
    /// too tangled to offer a single-step undo for.
    conflict_touched: bool,
    cancelled: bool,
}

/// A navigation whose history change is held back until its scan succeeds, so a
/// failed back / forward / up / home / activate never leaves the path pointing
/// at an unreadable directory while the list still shows the previous one.
enum PendingNav {
    Back(PathBuf),
    Forward(PathBuf),
    To(PathBuf),
}

impl PendingNav {
    fn destination(&self) -> &Path {
        match self {
            PendingNav::Back(path) | PendingNav::Forward(path) | PendingNav::To(path) => path,
        }
    }

    /// Applies the navigation to `history` once its scan has succeeded.
    fn commit(self, history: &mut NavigationHistory) {
        match self {
            PendingNav::Back(_) => {
                history.go_back();
            }
            PendingNav::Forward(_) => {
                history.go_forward();
            }
            PendingNav::To(path) => {
                history.navigate_to(path);
            }
        }
    }
}

pub struct SideritaControllerRust {
    current_path: QString,
    status_text: QString,
    error_text: QString,
    entry_names: QStringList,
    selected_token: QString,
    query: QString,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    can_go_up: bool,
    show_hidden: bool,
    sort_field: i32,
    sort_ascending: bool,
    coordinator: ScanCoordinator,
    executor: Option<ScanExecutor>,
    history: NavigationHistory,
    adapter: SnapshotAdapter,
    options: ViewOptions,
    snapshot: Option<DirectorySnapshot>,
    view: Option<ViewSnapshot>,
    pending_nav: Option<PendingNav>,
    watch: Option<WatchState>,
    watched: Option<PathBuf>,
    debouncer: Option<FsDebouncer>,
    watch_degraded: bool,
    folder_size: QString,
    selection_count: i32,
    properties_pending: bool,
    prop_name: QString,
    prop_path: QString,
    prop_kind: QString,
    prop_mime: QString,
    prop_size: QString,
    prop_permissions: QString,
    prop_owner: QString,
    prop_modified: QString,
    prop_accessed: QString,
    prop_symlink: QString,
    prop_is_dir: bool,
    prop_size_cancel: Option<CancellationToken>,
    search_active: bool,
    search_running: bool,
    search_query: QString,
    search_summary: QString,
    search_names: QStringList,
    search_paths: QStringList,
    search_kinds: QStringList,
    search_hits: Vec<crate::search::SearchHit>,
    search_cancel: Option<CancellationToken>,
    pending_select_path: Option<PathBuf>,
    bookmark_names: QStringList,
    bookmark_paths: QStringList,
    op_error: QString,
    can_paste: bool,
    cut_paths: QStringList,
    can_undo: bool,
    undo_label: QString,
    op_running: bool,
    op_current: QString,
    op_detail: QString,
    op_done: i32,
    op_total: i32,
    op_cancel: Option<CancellationToken>,
    conflict_pending: bool,
    conflict_count: i32,
    conflict_name: QString,
    pending_paste: Option<PendingPaste>,
    trash_names: QStringList,
    trash_origins: QStringList,
    trash_dates: QStringList,
    trash_infos: Vec<PathBuf>,
    open_with_pending: bool,
    open_with_target: QString,
    open_with_apps: QStringList,
    open_with_default_index: i32,
    open_with_path: PathBuf,
    open_with_mime: String,
    open_with_ids: Vec<String>,
    volume_names: QStringList,
    volume_devices: QStringList,
    volume_mounts: QStringList,
    volume_busy: bool,
    hidden_device_count: i32,
    volumes: Vec<crate::volumes::Volume>,
    settings: crate::settings::Settings,
    clipboard: Vec<PathBuf>,
    clipboard_cut: bool,
    last_undo: Option<UndoAction>,
    bookmarks: Vec<crate::bookmarks::Bookmark>,
    places: std::collections::HashMap<String, String>,
}

impl Default for SideritaControllerRust {
    fn default() -> Self {
        // Restore the persisted sort / hidden config so a new tab opens the way
        // the user left it.
        let settings = crate::settings::load();
        let options = ViewOptions {
            sort_field: sort_field_from_index(settings.sort_field).unwrap_or(SortField::Name),
            sort_direction: if settings.sort_ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            },
            show_hidden: settings.show_hidden,
            ..ViewOptions::default()
        };
        Self {
            current_path: QString::default(),
            status_text: QString::from("Preparando Siderita…"),
            error_text: QString::default(),
            entry_names: QStringList::default(),
            selected_token: QString::default(),
            query: QString::default(),
            loading: false,
            can_go_back: false,
            can_go_forward: false,
            can_go_up: false,
            show_hidden: settings.show_hidden,
            sort_field: settings.sort_field,
            sort_ascending: settings.sort_ascending,
            coordinator: ScanCoordinator::new(),
            executor: None,
            history: NavigationHistory::default(),
            adapter: SnapshotAdapter::new(),
            options,
            snapshot: None,
            view: None,
            pending_nav: None,
            watch: None,
            watched: None,
            debouncer: None,
            watch_degraded: false,
            folder_size: QString::default(),
            selection_count: 0,
            properties_pending: false,
            prop_name: QString::default(),
            prop_path: QString::default(),
            prop_kind: QString::default(),
            prop_mime: QString::default(),
            prop_size: QString::default(),
            prop_permissions: QString::default(),
            prop_owner: QString::default(),
            prop_modified: QString::default(),
            prop_accessed: QString::default(),
            prop_symlink: QString::default(),
            prop_is_dir: false,
            prop_size_cancel: None,
            search_active: false,
            search_running: false,
            search_query: QString::default(),
            search_summary: QString::default(),
            search_names: QStringList::default(),
            search_paths: QStringList::default(),
            search_kinds: QStringList::default(),
            search_hits: Vec::new(),
            search_cancel: None,
            pending_select_path: None,
            bookmark_names: QStringList::default(),
            bookmark_paths: QStringList::default(),
            op_error: QString::default(),
            can_paste: false,
            cut_paths: QStringList::default(),
            can_undo: false,
            undo_label: QString::default(),
            op_running: false,
            op_current: QString::default(),
            op_detail: QString::default(),
            op_done: 0,
            op_total: 0,
            op_cancel: None,
            conflict_pending: false,
            conflict_count: 0,
            conflict_name: QString::default(),
            pending_paste: None,
            trash_names: QStringList::default(),
            trash_origins: QStringList::default(),
            trash_dates: QStringList::default(),
            trash_infos: Vec::new(),
            open_with_pending: false,
            open_with_target: QString::default(),
            open_with_apps: QStringList::default(),
            open_with_default_index: -1,
            open_with_path: PathBuf::new(),
            open_with_mime: String::new(),
            open_with_ids: Vec::new(),
            volume_names: QStringList::default(),
            volume_devices: QStringList::default(),
            volume_mounts: QStringList::default(),
            volume_busy: false,
            hidden_device_count: 0,
            volumes: Vec::new(),
            settings,
            clipboard: Vec::new(),
            clipboard_cut: false,
            last_undo: None,
            bookmarks: Vec::new(),
            places: crate::places::resolve()
                .into_iter()
                .map(|(key, path)| (key, path.to_string_lossy().into_owned()))
                .collect(),
        }
    }
}

impl SideritaControllerRust {
    fn row(&self, index: i32) -> Option<&EntryRow> {
        let index = usize::try_from(index).ok()?;
        self.view.as_ref()?.row(index)
    }

    fn row_by_token(&self, token: &QString) -> Option<&EntryRow> {
        let token = token.to_string().parse::<u64>().ok()?;
        self.view
            .as_ref()?
            .rows()
            .iter()
            .find(|row| row.token().value() == token)
    }

    /// A search hit by its token (the hit's index in the results).
    fn search_hit(&self, token: &QString) -> Option<&crate::search::SearchHit> {
        let index = token.to_string().parse::<usize>().ok()?;
        self.search_hits.get(index)
    }
}

impl qobject::SideritaController {
    pub fn start(self: Pin<&mut Self>) {
        let initial = initial_location();
        self.start_common(initial);
    }

    /// Starts a tab directly at `location`, without the argv/HOME detour `start`
    /// uses. New tabs open on the folder that spawned them, not the first tab's
    /// initial location.
    pub fn start_at(self: Pin<&mut Self>, location: &QString) {
        let initial = resolve_location(&location.to_string(), None);
        self.start_common(initial);
    }

    fn start_common(mut self: Pin<&mut Self>, initial: PathBuf) {
        if self.rust().executor.is_none() {
            let qt_thread = self.qt_thread();
            let executor = ScanExecutor::new(move |result| {
                let _ = qt_thread.queue(move |controller| {
                    controller.handle_scan_result(result);
                });
            });
            self.as_mut().rust_mut().get_mut().executor = Some(executor);
        }

        self.as_mut().reload_bookmarks();

        if self.rust().history.current().is_none() {
            self.as_mut().rust_mut().get_mut().history = NavigationHistory::new(initial.clone());
        }

        let destination = self
            .rust()
            .history
            .current()
            .map(Path::to_path_buf)
            .unwrap_or(initial);
        self.as_mut().request_scan(destination);
    }

    /// Re-reads the bookmark file into this controller and republishes the
    /// name/path properties. Called on tab activation so a bookmark added in one
    /// tab becomes visible in the others, and once as part of `start_common`.
    pub fn reload_bookmarks(mut self: Pin<&mut Self>) {
        let loaded = crate::bookmarks::load();
        self.as_mut().rust_mut().get_mut().bookmarks = loaded;
        self.as_mut().refresh_bookmark_properties();
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        if let Some(location) = self.rust().history.current().map(Path::to_path_buf) {
            self.as_mut().request_scan(location);
        }
    }

    pub fn go_home(mut self: Pin<&mut Self>) {
        let destination = home_location();
        self.as_mut().request_nav_scan(PendingNav::To(destination));
    }

    pub fn go_back(mut self: Pin<&mut Self>) {
        let Some(destination) = self.rust().history.peek_back().map(Path::to_path_buf) else {
            return;
        };
        self.as_mut()
            .request_nav_scan(PendingNav::Back(destination));
    }

    pub fn go_forward(mut self: Pin<&mut Self>) {
        let Some(destination) = self.rust().history.peek_forward().map(Path::to_path_buf) else {
            return;
        };
        self.as_mut()
            .request_nav_scan(PendingNav::Forward(destination));
    }

    pub fn go_up(mut self: Pin<&mut Self>) {
        let Some(destination) = self
            .rust()
            .history
            .current()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
        else {
            return;
        };
        self.as_mut().request_nav_scan(PendingNav::To(destination));
    }

    pub fn open_location(mut self: Pin<&mut Self>, location: &QString) {
        let input = location.to_string();
        if input.is_empty() {
            self.as_mut()
                .set_error_text(QString::from("Escribe una ruta local"));
            self.as_mut()
                .set_status_text(QString::from("La ubicación está vacía"));
            return;
        }

        let destination = resolve_location(&input, self.rust().history.current());
        self.as_mut().request_nav_scan(PendingNav::To(destination));
    }

    pub fn toggle_hidden(mut self: Pin<&mut Self>) {
        let show_hidden = !*self.show_hidden();
        self.as_mut().set_show_hidden(show_hidden);
        self.as_mut().rust_mut().get_mut().options.show_hidden = show_hidden;
        self.as_mut().reproject();
        self.as_mut().persist_view_settings();
    }

    pub fn change_sort_field(mut self: Pin<&mut Self>, field: i32) {
        let Some(sort_field) = sort_field_from_index(field) else {
            return;
        };
        if self.rust().options.sort_field == sort_field {
            return;
        }

        self.as_mut().rust_mut().get_mut().options.sort_field = sort_field;
        self.as_mut().set_sort_field(field);
        self.as_mut().reproject();
        self.as_mut().persist_view_settings();
    }

    pub fn toggle_sort_direction(mut self: Pin<&mut Self>) {
        let ascending = !*self.sort_ascending();
        self.as_mut().rust_mut().get_mut().options.sort_direction = if ascending {
            SortDirection::Ascending
        } else {
            SortDirection::Descending
        };
        self.as_mut().set_sort_ascending(ascending);
        self.as_mut().reproject();
        self.as_mut().persist_view_settings();
    }

    /// Saves the current sort field / direction / hidden toggle so they persist
    /// (read fresh, change only these fields, write back — no cross-tab clobber).
    fn persist_view_settings(mut self: Pin<&mut Self>) {
        let mut settings = crate::settings::load();
        settings.sort_field = *self.sort_field();
        settings.sort_ascending = *self.sort_ascending();
        settings.show_hidden = *self.show_hidden();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
    }

    pub fn apply_query(mut self: Pin<&mut Self>, query: &QString) {
        if self.query() == query {
            return;
        }

        self.as_mut().set_query(query.clone());
        self.as_mut().rust_mut().get_mut().options.query = query.to_string();
        self.as_mut().reproject();
    }

    pub fn select_token(mut self: Pin<&mut Self>, token: &QString) {
        // The selected item's name and detail are shown in the sidebar info box
        // (driven by selected_token), so selecting no longer writes the status
        // line. A search hit's token is its index — accepted as-is if in range.
        if self.rust().search_active {
            if self.rust().search_hit(token).is_some() {
                self.as_mut().set_selected_token(token.clone());
            }
            return;
        }
        let selected = self
            .rust()
            .row_by_token(token)
            .map(|row| row.token().to_string());
        if let Some(token) = selected {
            self.as_mut()
                .set_selected_token(QString::from(token.as_str()));
        }
    }

    pub fn activate_token(mut self: Pin<&mut Self>, token: &QString) {
        // A search hit acts exactly like a folder entry: a folder navigates in
        // (leaving search), a file opens in its default app (search stays up so
        // more hits can be opened).
        if self.rust().search_active {
            let Some((path, is_dir, name)) = self
                .rust()
                .search_hit(token)
                .map(|hit| (PathBuf::from(&hit.path), hit.is_dir, hit.name.clone()))
            else {
                return;
            };
            if is_dir {
                self.as_mut().exit_search();
                self.as_mut().request_nav_scan(PendingNav::To(path));
            } else {
                self.as_mut().set_selected_token(token.clone());
                self.as_mut().open_in_default_app(&path, &name);
            }
            return;
        }

        let selected = self.rust().row_by_token(token).map(|row| {
            (
                row.path().to_path_buf(),
                row.kind(),
                row.display_name().to_owned(),
            )
        });

        let Some((path, kind, name)) = selected else {
            return;
        };

        if kind == RowKind::Directory {
            self.as_mut().request_nav_scan(PendingNav::To(path));
        } else {
            self.as_mut().select_token(token);
            self.as_mut().open_in_default_app(&path, &name);
        }
    }

    /// Hands a non-directory entry to the desktop's default handler via
    /// `xdg-open`, the freedesktop way to reach a viewer/editor/player without
    /// Siderita knowing anything about the file type. The launch is fire-and-
    /// forget — `xdg-open` resolves the handler and exits — but a failure to even
    /// start it (missing binary, no handler) is surfaced truthfully as `op_error`.
    fn open_in_default_app(mut self: Pin<&mut Self>, path: &Path, name: &str) {
        self.as_mut().set_op_error(QString::default());
        match open_with_default(path) {
            Ok(()) => {
                let message = format!("Abriendo {name}…");
                self.as_mut()
                    .set_status_text(QString::from(message.as_str()));
            }
            Err(error) => self.as_mut().set_op_error(QString::from(error.as_str())),
        }
    }

    pub fn entry_token(&self, index: i32) -> QString {
        if self.rust().search_active {
            let count = self.rust().search_hits.len() as i32;
            return if index >= 0 && index < count {
                QString::from(index.to_string().as_str())
            } else {
                QString::default()
            };
        }
        self.rust()
            .row(index)
            .map(|row| QString::from(row.token().to_string().as_str()))
            .unwrap_or_default()
    }

    pub fn entry_detail(&self, index: i32) -> QString {
        // A search hit's detail is where it lives — its containing folder.
        if self.rust().search_active {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().search_hits.get(i))
                .map(|hit| QString::from(search_hit_parent(&hit.path).as_str()))
                .unwrap_or_default();
        }
        let Some(row) = self.rust().row(index) else {
            return QString::default();
        };
        let kind = kind_label(row.kind());
        let date = row.modified().map(format_system_time).unwrap_or_default();
        // Folders show kind + date (their entry size is not meaningful); files
        // show kind · size · date.
        let detail = if row.kind() == RowKind::Directory {
            if date.is_empty() {
                kind.to_owned()
            } else {
                format!("{kind} · {date}")
            }
        } else {
            let size = format_size(row.size());
            if date.is_empty() {
                format!("{kind} · {size}")
            } else {
                format!("{kind} · {size} · {date}")
            }
        };
        QString::from(detail.as_str())
    }

    pub fn index_for_token(&self, token: &QString) -> i32 {
        if self.rust().search_active {
            return token
                .to_string()
                .parse::<usize>()
                .ok()
                .filter(|&i| i < self.rust().search_hits.len())
                .and_then(|i| i32::try_from(i).ok())
                .unwrap_or(-1);
        }
        let Ok(token) = token.to_string().parse::<u64>() else {
            return -1;
        };
        self.rust()
            .view
            .as_ref()
            .and_then(|view| {
                view.rows()
                    .iter()
                    .position(|row| row.token().value() == token)
            })
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    pub fn entry_path(&self, index: i32) -> QString {
        if self.rust().search_active {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().search_hits.get(i))
                .map(|hit| QString::from(hit.path.as_str()))
                .unwrap_or_default();
        }
        self.rust()
            .row(index)
            .map(|row| QString::from(row.path().to_string_lossy().as_ref()))
            .unwrap_or_default()
    }

    pub fn add_bookmark(mut self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        if path.is_empty() || self.rust().bookmarks.iter().any(|entry| entry.path == path) {
            return;
        }
        let name = crate::bookmarks::name_for(&path);
        self.as_mut()
            .rust_mut()
            .get_mut()
            .bookmarks
            .push(crate::bookmarks::Bookmark { name, path });
        self.as_mut().refresh_bookmark_properties();
        let _ = crate::bookmarks::save(&self.rust().bookmarks);
    }

    pub fn remove_bookmark(mut self: Pin<&mut Self>, index: i32) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if index >= self.rust().bookmarks.len() {
            return;
        }
        self.as_mut().rust_mut().get_mut().bookmarks.remove(index);
        self.as_mut().refresh_bookmark_properties();
        let _ = crate::bookmarks::save(&self.rust().bookmarks);
    }

    pub fn rename_bookmark(mut self: Pin<&mut Self>, index: i32, name: &QString) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let name = name.to_string();
        if name.is_empty() || index >= self.rust().bookmarks.len() {
            return;
        }
        self.as_mut().rust_mut().get_mut().bookmarks[index].name = name;
        self.as_mut().refresh_bookmark_properties();
        let _ = crate::bookmarks::save(&self.rust().bookmarks);
    }

    pub fn place_path(&self, key: &QString) -> QString {
        self.rust()
            .places
            .get(&key.to_string())
            .map(|path| QString::from(path.as_str()))
            .unwrap_or_default()
    }

    pub fn new_folder(mut self: Pin<&mut Self>, name: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(parent) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        let name = name.to_string();
        let outcome =
            siderita_ops::create_directory(&parent, OsStr::new(&name), &CancellationToken::new());
        // Creating is not undoable; a success supersedes the last undoable op.
        if outcome.is_ok() {
            self.as_mut().set_undo(None);
        }
        self.finish_op(outcome.map(|_| ()));
    }

    pub fn new_file(mut self: Pin<&mut Self>, name: &QString) {
        self.as_mut().set_op_error(QString::default());
        let Some(parent) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        let name = name.to_string();
        let outcome =
            siderita_ops::create_file(&parent, OsStr::new(&name), &CancellationToken::new());
        if outcome.is_ok() {
            self.as_mut().set_undo(None);
        }
        self.finish_op(outcome.map(|_| ()));
    }

    pub fn rename_path(mut self: Pin<&mut Self>, path: &QString, new_name: &QString) {
        self.as_mut().set_op_error(QString::default());
        let path = PathBuf::from(path.to_string());
        let new_name = new_name.to_string();
        let outcome = siderita_ops::rename(&path, OsStr::new(&new_name), &CancellationToken::new());
        if let Ok(renamed) = &outcome {
            let undo = path.file_name().map(|old_name| UndoAction::Rename {
                renamed: renamed.to.clone(),
                old_name: old_name.to_os_string(),
            });
            self.as_mut().set_undo(undo);
        }
        self.finish_op(outcome.map(|_| ()));
    }

    pub fn trash_path(mut self: Pin<&mut Self>, path: &QString) {
        self.as_mut().set_op_error(QString::default());
        let path = PathBuf::from(path.to_string());
        let outcome = siderita_ops::trash(&path, &CancellationToken::new());
        if let Ok(trashed) = &outcome {
            self.as_mut().set_undo(Some(UndoAction::Trash {
                infos: vec![trashed.info.clone()],
            }));
        }
        self.finish_op(outcome.map(|_| ()));
    }

    /// Sends every path in a multi-selection to Trash. Each entry is attempted
    /// independently; the view is refreshed once so successes appear, and any
    /// failures are reported together without hiding the ones that did land.
    pub fn trash_paths(mut self: Pin<&mut Self>, paths: &QStringList) {
        self.as_mut().set_op_error(QString::default());
        let paths = qstringlist_to_paths(paths);
        if paths.is_empty() {
            return;
        }
        let cancellation = CancellationToken::new();
        let mut failures = Vec::new();
        let mut infos = Vec::new();
        for path in &paths {
            match siderita_ops::trash(path, &cancellation) {
                Ok(trashed) => infos.push(trashed.info),
                Err(error) => failures.push(format!("{}: {error}", display_name(path))),
            }
        }
        if !infos.is_empty() {
            self.as_mut().set_undo(Some(UndoAction::Trash { infos }));
        }
        self.as_mut().finish_batch(paths.len(), &failures);
    }

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

    fn set_clipboard(mut self: Pin<&mut Self>, paths: Vec<PathBuf>, cut: bool) {
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
    fn begin_paste(
        mut self: Pin<&mut Self>,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        cut: bool,
    ) {
        if sources.is_empty() {
            return;
        }

        let collisions: Vec<&PathBuf> = sources
            .iter()
            .filter(|source| {
                source
                    .file_name()
                    .map(|name| destination.join(name))
                    .is_some_and(|target| std::fs::symlink_metadata(target).is_ok())
            })
            .collect();

        if collisions.is_empty() {
            self.as_mut()
                .spawn_paste(sources, destination, cut, ConflictStrategy::Skip);
            return;
        }

        let count = collisions.len();
        let first = collisions
            .first()
            .map(|source| display_name(source))
            .unwrap_or_default();
        self.as_mut().rust_mut().get_mut().pending_paste = Some(PendingPaste {
            sources,
            destination,
            cut,
        });
        self.as_mut()
            .set_conflict_count(count.min(i32::MAX as usize) as i32);
        self.as_mut()
            .set_conflict_name(QString::from(first.as_str()));
        self.as_mut().set_conflict_pending(true);
    }

    /// Applies the user's conflict choice ("skip" / "replace" / "keepboth") and
    /// starts the held-back paste on the worker thread.
    pub fn resolve_conflicts(mut self: Pin<&mut Self>, strategy: &QString) {
        let Some(strategy) = ConflictStrategy::from_key(&strategy.to_string()) else {
            return;
        };
        let Some(pending) = self.as_mut().rust_mut().get_mut().pending_paste.take() else {
            return;
        };
        self.as_mut().set_conflict_pending(false);
        self.as_mut()
            .spawn_paste(pending.sources, pending.destination, pending.cut, strategy);
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
    fn spawn_paste(
        mut self: Pin<&mut Self>,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        cut: bool,
        strategy: ConflictStrategy,
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
                    strategy,
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
    fn finish_paste(mut self: Pin<&mut Self>, cut: bool, outcome: PasteOutcome) {
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

    /// Reads the freedesktop Trash and publishes it to the Trash view (parallel
    /// name / origin / date lists), keeping the info paths for restore-by-index.
    pub fn load_trash(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let entries = match siderita_ops::list_home_trash() {
            Ok(entries) => entries,
            Err(error) => {
                self.as_mut()
                    .set_op_error(QString::from(error.to_string().as_str()));
                return;
            }
        };

        let names: QStringList = entries
            .iter()
            .map(|entry| QString::from(entry.name.as_str()))
            .collect();
        let origins: QStringList = entries
            .iter()
            .map(|entry| QString::from(entry.original.to_string_lossy().as_ref()))
            .collect();
        let dates: QStringList = entries
            .iter()
            .map(|entry| QString::from(format_trash_date(&entry.deletion_date).as_str()))
            .collect();
        let infos: Vec<PathBuf> = entries.into_iter().map(|entry| entry.info).collect();

        self.as_mut().rust_mut().get_mut().trash_infos = infos;
        self.as_mut().set_trash_names(names);
        self.as_mut().set_trash_origins(origins);
        self.as_mut().set_trash_dates(dates);
    }

    /// Restores the trashed entry at `index` in the loaded Trash list, then
    /// refreshes both the Trash view and the current folder (the entry may
    /// reappear there). A refusal (its origin is taken) surfaces as `op_error`.
    pub fn restore_trash(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().set_op_error(QString::default());
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(info) = self.rust().trash_infos.get(index).cloned() else {
            return;
        };
        match siderita_ops::restore_from_trash(&info, &CancellationToken::new()) {
            Ok(_) => {
                self.as_mut().load_trash();
                self.as_mut().refresh();
            }
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
        let infos = self.rust().trash_infos.clone();
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
        self.as_mut().load_trash();
        self.as_mut().refresh();
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
        let infos = self.rust().trash_infos.clone();
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

    /// Opens the "Abrir con…" chooser for `path`: classifies its MIME type,
    /// gathers the applications that declare it (plus the current default) and
    /// publishes them for the dialog. A type that cannot be classified is
    /// reported through `op_error`.
    pub fn open_with(mut self: Pin<&mut Self>, path: &QString) {
        self.as_mut().set_op_error(QString::default());
        let path = PathBuf::from(path.to_string());
        if path.as_os_str().is_empty() {
            return;
        }

        let Some(mime) = crate::apps::detect_mime(&path) else {
            self.as_mut()
                .set_op_error(QString::from("No se pudo determinar el tipo del archivo"));
            return;
        };

        let apps = crate::apps::apps_for_mime(&mime);
        let default_id = crate::apps::default_app_id(&mime);
        let default_index = default_id
            .as_ref()
            .and_then(|id| apps.iter().position(|app| &app.id == id))
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1);

        let names: QStringList = apps
            .iter()
            .map(|app| QString::from(app.name.as_str()))
            .collect();
        let target = display_name(&path);

        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.open_with_ids = apps.into_iter().map(|app| app.id).collect();
            state.open_with_path = path;
            state.open_with_mime = mime;
        }
        self.as_mut().set_open_with_apps(names);
        self.as_mut().set_open_with_default_index(default_index);
        self.as_mut()
            .set_open_with_target(QString::from(target.as_str()));
        self.as_mut().set_open_with_pending(true);
    }

    /// Launches the chosen application on the stored file, optionally making it
    /// the default for the file's MIME type first. Closes the chooser.
    pub fn open_with_app(mut self: Pin<&mut Self>, index: i32, set_default: bool) {
        self.as_mut().set_open_with_pending(false);
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let (id, path, mime) = {
            let state = self.rust();
            let Some(id) = state.open_with_ids.get(index).cloned() else {
                return;
            };
            (
                id,
                state.open_with_path.clone(),
                state.open_with_mime.clone(),
            )
        };

        if set_default {
            if let Err(error) = crate::apps::set_default_app(&mime, &id) {
                self.as_mut().set_op_error(QString::from(error.as_str()));
            }
        }
        match crate::apps::launch_with(&id, &path) {
            Ok(()) => {
                let message = format!("Abriendo {}…", display_name(&path));
                self.as_mut()
                    .set_status_text(QString::from(message.as_str()));
            }
            Err(error) => self.as_mut().set_op_error(QString::from(error.as_str())),
        }
    }

    pub fn cancel_open_with(mut self: Pin<&mut Self>) {
        self.as_mut().set_open_with_pending(false);
    }

    /// Reads the removable volumes UDisks2 reports and publishes them to the
    /// sidebar (parallel name / device / mount-point lists), keeping the full
    /// records for mount / unmount by index. Read-only and quick — runs inline.
    pub fn load_volumes(mut self: Pin<&mut Self>) {
        let mut volumes = match crate::volumes::list_volumes() {
            Ok(volumes) => volumes,
            Err(error) => {
                self.as_mut().set_op_error(QString::from(error.as_str()));
                return;
            }
        };

        // Drop the devices the user hid (read fresh so a hide in another tab is
        // honoured here too).
        let hidden = crate::settings::load().hidden_devices;
        volumes.retain(|volume| !hidden.iter().any(|name| name == &volume.name));
        self.as_mut()
            .set_hidden_device_count(hidden.len().min(i32::MAX as usize) as i32);

        let names: QStringList = volumes
            .iter()
            .map(|volume| QString::from(volume.name.as_str()))
            .collect();
        let devices: QStringList = volumes
            .iter()
            .map(|volume| QString::from(volume.device.as_str()))
            .collect();
        let mounts: QStringList = volumes
            .iter()
            .map(|volume| QString::from(volume.mount_point.as_str()))
            .collect();

        self.as_mut().rust_mut().get_mut().volumes = volumes;
        self.as_mut().set_volume_names(names);
        self.as_mut().set_volume_devices(devices);
        self.as_mut().set_volume_mounts(mounts);
    }

    /// Mounts the volume at `index` on a worker thread — mounting can block on a
    /// polkit authorization prompt, so it must never run on the Qt thread — then
    /// refreshes the list (or reports the failure) back on the Qt thread.
    pub fn mount_volume(mut self: Pin<&mut Self>, index: i32) {
        if *self.volume_busy() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.volume_path(index) else {
            return;
        };
        self.as_mut().set_volume_busy(true);
        self.as_mut().set_status_text(QString::from("Montando…"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::mount(&path);
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().set_volume_busy(false);
                match result {
                    Ok(_) => controller.as_mut().load_volumes(),
                    Err(error) => controller
                        .as_mut()
                        .set_op_error(QString::from(error.as_str())),
                }
            });
        });
    }

    /// Unmounts the volume at `index` on a worker thread, then refreshes.
    pub fn unmount_volume(mut self: Pin<&mut Self>, index: i32) {
        if *self.volume_busy() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.volume_path(index) else {
            return;
        };
        self.as_mut().set_volume_busy(true);
        self.as_mut().set_status_text(QString::from("Desmontando…"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::unmount(&path);
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().set_volume_busy(false);
                match result {
                    Ok(()) => controller.as_mut().load_volumes(),
                    Err(error) => controller
                        .as_mut()
                        .set_op_error(QString::from(error.as_str())),
                }
            });
        });
    }

    /// Opens the volume at `index`: navigates to its mount point, mounting it
    /// first (on a worker thread) if it is not yet mounted.
    pub fn open_volume(mut self: Pin<&mut Self>, index: i32) {
        if *self.volume_busy() {
            return;
        }
        self.as_mut().set_op_error(QString::default());
        let Some(path) = self.volume_path(index) else {
            return;
        };
        let mounted_at = usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().volumes.get(index))
            .map(|volume| volume.mount_point.clone())
            .unwrap_or_default();

        if !mounted_at.is_empty() {
            self.as_mut()
                .open_location(&QString::from(mounted_at.as_str()));
            return;
        }

        self.as_mut().set_volume_busy(true);
        self.as_mut().set_status_text(QString::from("Montando…"));

        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::mount(&path);
            let _ = qt.queue(move |mut controller| {
                controller.as_mut().set_volume_busy(false);
                match result {
                    Ok(mount_point) => {
                        controller.as_mut().load_volumes();
                        if !mount_point.is_empty() {
                            controller
                                .as_mut()
                                .open_location(&QString::from(mount_point.as_str()));
                        }
                    }
                    Err(error) => controller
                        .as_mut()
                        .set_op_error(QString::from(error.as_str())),
                }
            });
        });
    }

    fn volume_path(&self, index: i32) -> Option<String> {
        let index = usize::try_from(index).ok()?;
        self.rust()
            .volumes
            .get(index)
            .map(|volume| volume.object_path.clone())
    }

    /// Opens the properties panel for `path`: the metadata is gathered inline
    /// (fast), and a folder's recursive size is computed on a worker thread so a
    /// deep tree never blocks the UI.
    pub fn open_properties(mut self: Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        if path.as_os_str().is_empty() {
            return;
        }

        // Cancel any directory-size walk still running from a previous open.
        if let Some(token) = self.as_mut().rust_mut().get_mut().prop_size_cancel.take() {
            token.cancel();
        }

        let props = crate::properties::gather(&path);
        self.as_mut()
            .set_prop_name(QString::from(props.name.as_str()));
        self.as_mut()
            .set_prop_path(QString::from(props.path.as_str()));
        self.as_mut()
            .set_prop_kind(QString::from(props.kind.as_str()));
        self.as_mut()
            .set_prop_mime(QString::from(props.mime.as_str()));
        self.as_mut()
            .set_prop_permissions(QString::from(props.permissions.as_str()));
        self.as_mut()
            .set_prop_owner(QString::from(props.owner.as_str()));
        self.as_mut()
            .set_prop_modified(QString::from(props.modified.as_str()));
        self.as_mut()
            .set_prop_accessed(QString::from(props.accessed.as_str()));
        self.as_mut().set_prop_symlink(QString::from(
            props.symlink_target.unwrap_or_default().as_str(),
        ));
        self.as_mut().set_prop_is_dir(props.is_dir);

        match props.size {
            Some(size) => self
                .as_mut()
                .set_prop_size(QString::from(format_size_full(size).as_str())),
            None => {
                self.as_mut().set_prop_size(QString::from("Calculando…"));
                let token = CancellationToken::new();
                self.as_mut().rust_mut().get_mut().prop_size_cancel = Some(token.clone());
                let qt = self.qt_thread();
                let dir = path.clone();
                let dir_key = props.path.clone();
                std::thread::spawn(move || {
                    let size = crate::properties::directory_size(&dir, &token);
                    if token.is_cancelled() {
                        return;
                    }
                    let text = format_size_full(size);
                    let _ = qt.queue(
                        move |mut controller: Pin<&mut qobject::SideritaController>| {
                            // Ignore if the panel has since moved to another entry.
                            if controller.rust().prop_path.to_string() == dir_key {
                                controller
                                    .as_mut()
                                    .set_prop_size(QString::from(text.as_str()));
                            }
                        },
                    );
                });
            }
        }

        self.as_mut().set_properties_pending(true);
    }

    pub fn close_properties(mut self: Pin<&mut Self>) {
        if let Some(token) = self.as_mut().rust_mut().get_mut().prop_size_cancel.take() {
            token.cancel();
        }
        self.as_mut().set_properties_pending(false);
    }

    /// Runs a bounded recursive filename search of the current folder on a worker
    /// thread and shows the results overlay. Truthful about scope: the summary
    /// reports the match cap and whether the walk was cut short.
    pub fn search_recursive(mut self: Pin<&mut Self>, query: &QString) {
        let query = query.to_string();
        if query.trim().is_empty() {
            return;
        }
        let Some(root) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };

        if let Some(token) = self.as_mut().rust_mut().get_mut().search_cancel.take() {
            token.cancel();
        }
        let token = CancellationToken::new();
        self.as_mut().rust_mut().get_mut().search_cancel = Some(token.clone());
        self.as_mut()
            .set_search_query(QString::from(query.as_str()));
        // `search_active` only flips once results land and replace the folder
        // rows — during the walk the folder view stays live and interactive.
        self.as_mut().set_search_running(true);
        self.as_mut().set_search_summary(QString::from("Buscando…"));

        const LIMIT: usize = 500;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let outcome = crate::search::search(&root, &query, LIMIT, &token);
            if token.is_cancelled() && outcome.hits.is_empty() {
                // A search superseded before it found anything: drop it.
                return;
            }
            let _ = qt.queue(move |controller: Pin<&mut qobject::SideritaController>| {
                controller.publish_search(outcome);
            });
        });
    }

    /// Publishes a finished (or cancelled) search onto the Qt thread.
    fn publish_search(mut self: Pin<&mut Self>, outcome: crate::search::SearchOutcome) {
        let current = self.rust().history.current().map(Path::to_path_buf);
        let in_current =
            |hit: &crate::search::SearchHit| current.as_deref() == Path::new(&hit.path).parent();

        // Group the hits: those in the searched folder first, then everything
        // deeper — each group A→Z — so the two sections read contiguously.
        let mut hits = outcome.hits;
        hits.sort_by(|a, b| {
            in_current(b)
                .cmp(&in_current(a))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let summary = if outcome.cancelled {
            format!(
                "{} coincidencias · búsqueda detenida ({} carpetas)",
                hits.len(),
                outcome.dirs_scanned
            )
        } else if outcome.truncated {
            format!(
                "{}+ coincidencias · detenida en el límite ({} carpetas)",
                hits.len(),
                outcome.dirs_scanned
            )
        } else {
            format!(
                "{} coincidencias · {} carpetas exploradas",
                hits.len(),
                outcome.dirs_scanned
            )
        };

        // Parallel role columns so the hits ride the *same* model + roles the
        // folder view uses — the list/grid then render and behave identically
        // (single-click selects, double-click opens, keyboard, selection). The
        // token is the hit index, the subtitle its containing folder, and the
        // section the header the list groups it under.
        let names: QStringList = hits
            .iter()
            .map(|h| QString::from(h.name.as_str()))
            .collect();
        let paths: QStringList = hits
            .iter()
            .map(|h| QString::from(h.path.as_str()))
            .collect();
        let kinds: QStringList = hits
            .iter()
            .map(|h| QString::from(if h.is_dir { "directory" } else { "file" }))
            .collect();
        let tokens: QStringList = (0..hits.len())
            .map(|i| QString::from(i.to_string().as_str()))
            .collect();
        let subtitles: QStringList = hits
            .iter()
            .map(|h| QString::from(search_hit_parent(&h.path).as_str()))
            .collect();
        let sections: QStringList = hits
            .iter()
            .map(|h| {
                QString::from(if in_current(h) {
                    "En esta carpeta"
                } else {
                    "En subcarpetas"
                })
            })
            .collect();

        self.as_mut().rust_mut().get_mut().search_hits = hits;
        self.as_mut()
            .set_search_summary(QString::from(summary.as_str()));
        self.as_mut().set_search_running(false);
        self.as_mut().set_search_active(true);
        // A fresh result set drops any selection carried over from the folder.
        self.as_mut().set_selected_token(QString::default());
        self.as_mut().set_entry_names(names.clone());
        self.as_mut()
            .rows_ready(names, tokens, kinds, subtitles, paths, sections);
    }

    pub fn cancel_search(mut self: Pin<&mut Self>) {
        if let Some(token) = self.as_mut().rust_mut().get_mut().search_cancel.take() {
            token.cancel();
        }
    }

    /// Leaves search without touching the view — the caller repaints (a folder
    /// reproject, or a navigation scan) once it has decided what to show next.
    fn exit_search(mut self: Pin<&mut Self>) {
        self.as_mut().cancel_search();
        self.as_mut().rust_mut().get_mut().search_hits.clear();
        self.as_mut().set_search_running(false);
        self.as_mut().set_search_active(false);
    }

    /// Cancels search and returns the content box to the current folder's rows.
    pub fn close_search(mut self: Pin<&mut Self>) {
        self.as_mut().exit_search();
        self.as_mut().reproject();
    }

    /// Launches the desktop's terminal in the current folder (an external
    /// terminal — Siderita never embeds one). A failure is surfaced truthfully.
    pub fn open_terminal(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let Some(dir) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        if let Err(error) = open_terminal_in(&dir) {
            self.as_mut().set_op_error(QString::from(error.as_str()));
        }
    }

    /// The persisted list/grid mode and size scales, so a new tab / the sidebar
    /// opens the way the user last left it.
    pub fn saved_view_mode(&self) -> QString {
        QString::from(self.rust().settings.view_mode.as_str())
    }

    pub fn saved_content_icon_scale(&self) -> f64 {
        self.rust().settings.content_icon_scale
    }

    pub fn saved_content_text_scale(&self) -> f64 {
        self.rust().settings.content_text_scale
    }

    pub fn saved_interface_icon_scale(&self) -> f64 {
        self.rust().settings.interface_icon_scale
    }

    pub fn saved_interface_text_scale(&self) -> f64 {
        self.rust().settings.interface_text_scale
    }

    pub fn saved_sidebar_icon_scale(&self) -> f64 {
        self.rust().settings.sidebar_icon_scale
    }

    pub fn saved_sidebar_text_scale(&self) -> f64 {
        self.rust().settings.sidebar_text_scale
    }

    /// Persists the current view mode (list / grid).
    pub fn save_view_mode(mut self: Pin<&mut Self>, mode: &QString) {
        let mode = mode.to_string();
        // Read fresh, change only this field, write back — so a sort/hidden,
        // sizing or device change in another tab is not clobbered.
        let mut settings = crate::settings::load();
        settings.view_mode = if mode == "grid" {
            "grid".to_owned()
        } else {
            "list".to_owned()
        };
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
    }

    /// Persists the four independent size scales.
    pub fn save_sizing(
        mut self: Pin<&mut Self>,
        content_icon: f64,
        content_text: f64,
        interface_icon: f64,
        interface_text: f64,
        sidebar_icon: f64,
        sidebar_text: f64,
    ) {
        let mut settings = crate::settings::load();
        settings.content_icon_scale = content_icon;
        settings.content_text_scale = content_text;
        settings.interface_icon_scale = interface_icon;
        settings.interface_text_scale = interface_text;
        settings.sidebar_icon_scale = sidebar_icon;
        settings.sidebar_text_scale = sidebar_text;
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
    }

    /// Hides a removable device (by its display name) from the sidebar and
    /// remembers the choice; the list is re-read so it disappears at once.
    pub fn hide_device(mut self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        if name.is_empty() {
            return;
        }
        let mut settings = crate::settings::load();
        if !settings.hidden_devices.contains(&name) {
            settings.hidden_devices.push(name);
            let _ = crate::settings::save(&settings);
        }
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().load_volumes();
    }

    /// Un-hides every previously-hidden device.
    pub fn unhide_all_devices(mut self: Pin<&mut Self>) {
        let mut settings = crate::settings::load();
        settings.hidden_devices.clear();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().load_volumes();
    }

    /// Records (or clears) how to reverse the last operation, keeping the
    /// `can_undo` / `undo_label` properties in step for the menu and shortcut.
    fn set_undo(mut self: Pin<&mut Self>, action: Option<UndoAction>) {
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
    fn finish_op(mut self: Pin<&mut Self>, outcome: Result<(), OpError>) {
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
    fn finish_batch(mut self: Pin<&mut Self>, total: usize, failures: &[String]) {
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

    fn refresh_bookmark_properties(mut self: Pin<&mut Self>) {
        let (names, paths): (QStringList, QStringList) = {
            let bookmarks = &self.rust().bookmarks;
            (
                bookmarks
                    .iter()
                    .map(|entry| QString::from(entry.name.as_str()))
                    .collect(),
                bookmarks
                    .iter()
                    .map(|entry| QString::from(entry.path.as_str()))
                    .collect(),
            )
        };
        self.as_mut().set_bookmark_names(names);
        self.as_mut().set_bookmark_paths(paths);
    }

    /// Rescans the current location without a history change (refresh, initial).
    fn request_scan(mut self: Pin<&mut Self>, destination: PathBuf) {
        self.as_mut().rust_mut().get_mut().pending_nav = None;
        self.as_mut().request_scan_inner(destination, false);
    }

    /// A background rescan (the filesystem watcher) that must not disturb the UI:
    /// it keeps the current list and selection on screen and never flashes the
    /// "Leyendo carpeta…" loading state — the new snapshot simply replaces the old
    /// when it lands. This is what keeps an actively-changing folder from
    /// flickering.
    fn refresh_quiet(mut self: Pin<&mut Self>) {
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
    fn request_nav_scan(mut self: Pin<&mut Self>, nav: PendingNav) {
        let destination = nav.destination().to_path_buf();
        self.as_mut().rust_mut().get_mut().pending_nav = Some(nav);
        self.as_mut().request_scan_inner(destination, false);
    }

    /// `quiet` = a background refresh (watcher): leave the list, selection and
    /// status untouched and let the fresh snapshot swap in on success.
    fn request_scan_inner(mut self: Pin<&mut Self>, destination: PathBuf, quiet: bool) {
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

    fn handle_scan_result(mut self: Pin<&mut Self>, result: ScanResult) {
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

    fn reproject(mut self: Pin<&mut Self>) {
        // While search results occupy the content box, folder reprojections
        // (a watcher tick, a sort toggle) must not overwrite them; `close_search`
        // drops the flag first, then reprojects to restore the folder.
        if self.rust().search_active {
            return;
        }
        let projected = {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            let Some(snapshot) = state.snapshot.as_ref() else {
                return;
            };
            let total = snapshot.entries().len();
            state
                .adapter
                .adapt_projected(snapshot, &state.options)
                .map(|view| (view, total))
        };

        let (view, total) = match projected {
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
        let status = if visible == total {
            String::new()
        } else {
            format!("{visible} de {total}")
        };
        self.as_mut()
            .set_status_text(QString::from(status.as_str()));

        // Total size of the folder's files, for the info box's default line.
        let total_size: u64 = self
            .rust()
            .view
            .as_ref()
            .map(|view| {
                view.rows()
                    .iter()
                    .filter(|row| row.kind() != RowKind::Directory)
                    .map(|row| row.size())
                    .sum()
            })
            .unwrap_or(0);
        let folder_size = if total_size > 0 {
            format_size(total_size)
        } else {
            String::new()
        };
        self.as_mut()
            .set_folder_size(QString::from(folder_size.as_str()));

        // Hand the projected rows to the native model.
        self.as_mut()
            .rows_ready(names, tokens, kinds, subtitles, paths, sections);
    }

    fn update_navigation_state(mut self: Pin<&mut Self>) {
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
    fn rollback_pending_nav(mut self: Pin<&mut Self>) {
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
    fn ensure_debouncer(mut self: Pin<&mut Self>) {
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
    fn update_watch(mut self: Pin<&mut Self>, location: &Path) {
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
    fn on_fs_change(mut self: Pin<&mut Self>, degraded: bool) {
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

/// Collects a QML `list<string>` of paths into owned `PathBuf`s, skipping empty
/// strings so a stray blank never becomes a filesystem operation on `""`.
fn qstringlist_to_paths(list: &QStringList) -> Vec<PathBuf> {
    list.iter()
        .map(QString::to_string)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Pastes one source into `destination_dir` on the worker thread, applying the
/// decided `strategy` when the destination is already taken. Records the outcome
/// (failure, skip, undoable move, kept-back cut) into `outcome`.
fn paste_one(
    source: &Path,
    destination_dir: &Path,
    cut: bool,
    strategy: ConflictStrategy,
    token: &CancellationToken,
    on_progress: &mut dyn FnMut(Progress),
    outcome: &mut PasteOutcome,
) {
    let Some(name) = source.file_name() else {
        outcome
            .failures
            .push(format!("{}: sin nombre de archivo", display_name(source)));
        return;
    };
    let target = destination_dir.join(name);
    let colliding = std::fs::symlink_metadata(&target).is_ok();

    if !colliding {
        place_into(source, destination_dir, cut, token, on_progress, outcome);
        return;
    }

    outcome.conflict_touched = true;
    match strategy {
        ConflictStrategy::Skip => outcome.skipped += 1,
        ConflictStrategy::Replace => {
            // Trash the existing entry (recoverable) before placing the source,
            // so nothing is hard-deleted to make room.
            if let Err(error) = siderita_ops::trash(&target, token) {
                outcome
                    .failures
                    .push(format!("{}: {error}", display_name(source)));
                if cut {
                    outcome.unmoved.push(source.to_path_buf());
                }
                return;
            }
            place_into(source, destination_dir, cut, token, on_progress, outcome);
        }
        ConflictStrategy::KeepBoth => {
            let freed = next_free_name(destination_dir, name);
            let result = if cut {
                siderita_ops::move_as(source, &freed, token, on_progress).map(|_| ())
            } else {
                siderita_ops::copy_as(source, &freed, token, on_progress)
            };
            if let Err(error) = result {
                outcome
                    .failures
                    .push(format!("{}: {error}", display_name(source)));
                if cut {
                    outcome.unmoved.push(source.to_path_buf());
                }
            }
        }
    }
}

/// The plain placement (copy or move into a directory, keeping the source name),
/// shared by the no-collision path and by "replace" after the old entry is gone.
fn place_into(
    source: &Path,
    destination_dir: &Path,
    cut: bool,
    token: &CancellationToken,
    on_progress: &mut dyn FnMut(Progress),
    outcome: &mut PasteOutcome,
) {
    if cut {
        match siderita_ops::move_entry(source, destination_dir, token, on_progress) {
            Ok(moved) => {
                if let Some(parent) = moved.from.parent() {
                    outcome.undo_moves.push((moved.to, parent.to_path_buf()));
                }
            }
            Err(error) => {
                outcome
                    .failures
                    .push(format!("{}: {error}", display_name(source)));
                outcome.unmoved.push(source.to_path_buf());
            }
        }
    } else if let Err(error) = siderita_ops::copy(source, destination_dir, token, on_progress) {
        outcome
            .failures
            .push(format!("{}: {error}", display_name(source)));
    }
}

/// The first free `<stem> (copia)[.ext]`, `<stem> (copia 2)[.ext]`, … under
/// `dir` — the "keep both" name. Operates on `OsStr` so non-UTF-8 names survive.
fn next_free_name(dir: &Path, name: &OsStr) -> PathBuf {
    let as_path = Path::new(name);
    let stem = as_path.file_stem().unwrap_or(name);
    let extension = as_path.extension();

    for attempt in 1u64.. {
        let mut candidate = stem.to_os_string();
        if attempt == 1 {
            candidate.push(" (copia)");
        } else {
            candidate.push(format!(" (copia {attempt})"));
        }
        if let Some(extension) = extension {
            candidate.push(".");
            candidate.push(extension);
        }
        let path = dir.join(&candidate);
        if std::fs::symlink_metadata(&path).is_err() {
            return path;
        }
    }
    unreachable!("the free-name search always terminates before u64 wraps")
}

/// Presents a spec `YYYY-MM-DDThh:mm:ss` Trash deletion date as `YYYY-MM-DD
/// hh:mm` for the Trash view. Anything not in that shape is passed through, so a
/// malformed record still shows what it has rather than nothing.
fn format_trash_date(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let Some((date, time)) = raw.split_once('T') else {
        return raw.to_owned();
    };
    // Keep hh:mm; drop the seconds only when the time actually carries them
    // (two colons), so an already-short hh:mm is left intact.
    let hm = match (time.find(':'), time.rfind(':')) {
        (Some(first), Some(last)) if first != last => &time[..last],
        _ => time,
    };
    format!("{date} {hm}")
}

/// The final path component, for a compact per-entry line in a batch error.
/// Falls back to the full lossy path when there is no file name (e.g. `/`).
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Launches `xdg-open PATH`, detached from Siderita's stdio, and reaps the
/// short-lived launcher on a throwaway thread so it never lingers as a zombie.
/// The opened application is reparented and outlives Siderita. Returns a
/// user-facing Spanish message if the launcher could not even be spawned.
fn open_with_default(path: &Path) -> Result<(), String> {
    spawn_opener("xdg-open", path)
}

/// Spawns `program PATH` detached from Siderita's stdio and reaps the launcher on
/// a throwaway thread. Split out from [`open_with_default`] so the spawn/error
/// contract is testable without depending on `xdg-open` being installed.
fn spawn_opener(program: &str, path: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let child = Command::new(program)
        .arg(path.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match child {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("No se encontró «{program}» para abrir el archivo"))
        }
        Err(error) => Err(format!("No se pudo abrir el archivo: {error}")),
    }
}

/// Launches an external terminal with its working directory set to `dir`.
/// Honours `$TERMINAL`, then tries a list of common emulators, spawning the
/// first that exists (they open in the inherited cwd); the launcher is detached
/// and reaped like [`spawn_opener`].
fn open_terminal_in(dir: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let mut candidates: Vec<String> = Vec::new();
    if let Some(terminal) = std::env::var_os("TERMINAL") {
        candidates.push(terminal.to_string_lossy().into_owned());
    }
    candidates.extend(
        [
            "foot",
            "alacritty",
            "kitty",
            "wezterm",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "xterm",
        ]
        .iter()
        .map(|name| (*name).to_owned()),
    );

    for program in &candidates {
        let child = Command::new(program)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
            // Not installed — try the next candidate.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("No se pudo abrir la terminal: {error}")),
        }
    }

    Err("No se encontró ninguna terminal (define $TERMINAL)".to_owned())
}

fn initial_location() -> PathBuf {
    match std::env::args_os().nth(1) {
        // Accept a `file://` URI argument (e.g. from a desktop "open with").
        Some(arg) => {
            let text = arg.to_string_lossy();
            if text.starts_with("file:") {
                if let Some(path) = crate::dbus::uri_to_path(&text) {
                    return path;
                }
            }
            PathBuf::from(arg)
        }
        None => home_location(),
    }
}

fn home_location() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn resolve_location(input: &str, current: Option<&Path>) -> PathBuf {
    // A local file:// URI (typed, pasted, or from another app) → its path.
    if input.starts_with("file:") {
        if let Some(path) = crate::dbus::uri_to_path(input) {
            return path;
        }
    }

    let path = if input == "~" {
        home_location()
    } else if let Some(relative) = input.strip_prefix("~/") {
        home_location().join(relative)
    } else {
        PathBuf::from(input)
    };

    if path.is_absolute() {
        path
    } else {
        current
            .map(Path::to_path_buf)
            .unwrap_or_else(home_location)
            .join(path)
    }
}

const fn sort_field_from_index(index: i32) -> Option<SortField> {
    match index {
        0 => Some(SortField::Name),
        1 => Some(SortField::Size),
        2 => Some(SortField::Modified),
        3 => Some(SortField::Kind),
        _ => None,
    }
}

const fn kind_key(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Directory => "directory",
        RowKind::File => "file",
        RowKind::Symlink => "symlink",
        RowKind::Other => "other",
    }
}

const fn kind_label(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Directory => "Carpeta",
        RowKind::File => "Archivo",
        RowKind::Symlink => "Enlace simbólico",
        RowKind::Other => "Otro",
    }
}

fn row_subtitle(row: &EntryRow) -> String {
    if row.kind() == RowKind::Directory {
        return "Carpeta".to_owned();
    }

    format!("{} · {}", kind_label(row.kind()), format_size(row.size()))
}

/// The containing folder of a search hit, shown as its subtitle so a result
/// carries where it lives (the one thing a flat folder row doesn't need).
fn search_hit_parent(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Formats a `SystemTime` as local `YYYY-MM-DD HH:MM`, reusing the properties
/// panel's formatter. An empty string for a pre-epoch/absent time.
fn format_system_time(time: std::time::SystemTime) -> String {
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(elapsed) => crate::properties::format_time(elapsed.as_secs() as i64),
        Err(_) => String::new(),
    }
}

/// A human size plus the exact byte count, for the properties panel — the byte
/// count is dropped below 1 KiB where it would just repeat the human size.
fn format_size_full(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else {
        format!("{} · {bytes} bytes", format_size(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_location, sort_field_from_index};
    use siderita_core::SortField;
    use std::path::{Path, PathBuf};

    #[test]
    fn absolute_location_is_preserved() {
        assert_eq!(
            resolve_location("/tmp/una carpeta", Some(Path::new("/base"))),
            PathBuf::from("/tmp/una carpeta")
        );
    }

    #[test]
    fn relative_location_uses_current_directory() {
        assert_eq!(
            resolve_location("hija", Some(Path::new("/base"))),
            PathBuf::from("/base/hija")
        );
    }

    #[test]
    fn file_uri_resolves_to_its_local_path() {
        assert_eq!(
            resolve_location("file:///tmp/una%20carpeta", Some(Path::new("/base"))),
            PathBuf::from("/tmp/una carpeta")
        );
        // A bare relative name that merely starts with "file" is not a URI.
        assert_eq!(
            resolve_location("filename.txt", Some(Path::new("/base"))),
            PathBuf::from("/base/filename.txt")
        );
    }

    #[test]
    fn next_free_name_suffixes_copia_around_the_extension() {
        use std::ffi::OsStr;
        let dir = std::env::temp_dir().join(format!(
            "siderita-freename-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("mk test dir");

        // Nothing exists yet → the first "(copia)" name.
        let first = super::next_free_name(&dir, OsStr::new("nota.txt"));
        assert_eq!(first.file_name().unwrap(), OsStr::new("nota (copia).txt"));

        // Occupy it and the plain name; the next free is "(copia 2)".
        std::fs::write(&first, b"x").expect("seed copia");
        std::fs::write(dir.join("nota.txt"), b"x").expect("seed orig");
        let second = super::next_free_name(&dir, OsStr::new("nota.txt"));
        assert_eq!(
            second.file_name().unwrap(),
            OsStr::new("nota (copia 2).txt")
        );

        // A name without an extension keeps the suffix at the end.
        let no_ext = super::next_free_name(&dir, OsStr::new("carpeta"));
        assert_eq!(no_ext.file_name().unwrap(), OsStr::new("carpeta (copia)"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_trash_date_is_compact_and_lenient() {
        assert_eq!(
            super::format_trash_date("2026-07-21T18:04:09"),
            "2026-07-21 18:04"
        );
        // No seconds → left as-is (just the T replaced).
        assert_eq!(
            super::format_trash_date("2026-07-21T18:04"),
            "2026-07-21 18:04"
        );
        assert_eq!(super::format_trash_date(""), "");
        // A malformed value is passed through rather than dropped.
        assert_eq!(super::format_trash_date("desconocido"), "desconocido");
    }

    #[test]
    fn display_name_uses_the_final_component() {
        assert_eq!(
            super::display_name(Path::new("/home/toni/nota.txt")),
            "nota.txt"
        );
        assert_eq!(
            super::display_name(Path::new("/home/toni/carpeta")),
            "carpeta"
        );
        // No file name (root) falls back to the whole path.
        assert_eq!(super::display_name(Path::new("/")), "/");
    }

    #[test]
    fn spawn_opener_reports_a_missing_launcher() {
        let error =
            super::spawn_opener("siderita-no-such-launcher-xyz", Path::new("/tmp/whatever"))
                .unwrap_err();
        assert!(
            error.contains("siderita-no-such-launcher-xyz"),
            "message should name the missing launcher: {error}"
        );
    }

    #[test]
    fn spawn_opener_launches_an_existing_program() {
        // `true` ignores its argument and exits 0 — a side-effect-free stand-in
        // for xdg-open that proves the spawn path succeeds and reaps cleanly.
        super::spawn_opener("true", Path::new("/tmp/whatever"))
            .expect("spawning an existing launcher should succeed");
    }

    #[test]
    fn sort_field_indices_are_stable_for_qml() {
        assert_eq!(sort_field_from_index(0), Some(SortField::Name));
        assert_eq!(sort_field_from_index(1), Some(SortField::Size));
        assert_eq!(sort_field_from_index(2), Some(SortField::Modified));
        assert_eq!(sort_field_from_index(3), Some(SortField::Kind));
        assert_eq!(sort_field_from_index(4), None);
    }
}
