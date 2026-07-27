use std::ffi::OsString;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use cxx_qt_lib::{QString, QStringList};
use notify_debouncer_full::notify::RecommendedWatcher;
use notify_debouncer_full::{Debouncer, RecommendedCache};
use siderita_core::{
    DirectorySnapshot, NavigationHistory, ScanCoordinator, ScanExecutor, SortDirection, SortField,
    ViewOptions, WatchState,
};

/// The filesystem debouncer type kept alive for the controller's lifetime.
type FsDebouncer = Debouncer<RecommendedWatcher, RecommendedCache>;
use siderita_ops::{Progress, TrashEntry};
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

        // The freedesktop-thumbnail image provider (see cpp/thumbnailprovider.cpp),
        // added onto the engine before the QML loads.
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("siderita/thumbnailprovider.h");

        #[rust_name = "register_thumbnail_provider"]
        fn register_siderita_thumbnail_provider(engine: Pin<&mut QQmlApplicationEngine>);

        // Pins the freedesktop icon theme named icons resolve against
        // (see cpp/icontheme.cpp), set once before the QML loads.
        include!("siderita/icontheme.h");

        #[rust_name = "apply_icon_theme"]
        fn siderita_apply_icon_theme(theme: &QString);
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
        // Trash shown as a content-view location (like search): its entries ride
        // the same entry model, so list/grid/details/thumbnails just work.
        #[qproperty(bool, trash_active)]
        // Recientes: the desktop's own recently-used list, read (never written)
        // and shown as another content-view location.
        #[qproperty(bool, recent_active)]
        #[qproperty(i32, recent_count)]
        // Per-path custom icon overrides, exposed as ONE list of `path\ticon`
        // entries the QML folds into a path→icon map. Deliberately not two
        // parallel lists: those are set in sequence, so a QML handler woken by
        // the first sees the second still stale — the override then only
        // appeared on the next start.
        #[qproperty(QStringList, custom_icon_entries)]
        // Starred entries — one `path\tkind` line each, where kind is
        // `directory`, `file` or `missing` (a favourite outlives what it points
        // at, and the sidebar says so rather than pretending). One property, so
        // a reader never sees half an update.
        #[qproperty(QStringList, favorite_entries)]
        #[qproperty(bool, open_with_pending)]
        #[qproperty(QString, open_with_target)]
        #[qproperty(QStringList, open_with_apps)]
        #[qproperty(i32, open_with_default_index)]
        #[qproperty(QStringList, volume_names)]
        #[qproperty(QStringList, volume_devices)]
        #[qproperty(QStringList, volume_mounts)]
        #[qproperty(bool, volume_busy)]
        #[qproperty(i32, hidden_device_count)]
        // Connected phones from Magnetita (org.celestina.Devices1): parallel
        // name / type / mount-path lists. An empty mount means connected but not
        // yet mounted (so not openable).
        #[qproperty(QStringList, phone_names)]
        #[qproperty(QStringList, phone_types)]
        #[qproperty(QStringList, phone_mounts)]
        // The sidebar's places: the keys that exist on this machine, in the
        // user's order, minus the ones they hid.
        #[qproperty(QStringList, place_keys)]
        #[qproperty(i32, hidden_place_count)]
        // How this folder was left, if it was ever arranged: the view mode to
        // show ("" = follow the global default) and whether a record exists at
        // all (so the folder menu can offer to forget it).
        #[qproperty(QString, folder_view_mode)]
        #[qproperty(bool, folder_view_pinned)]
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

        /// Restricts the listing to these name patterns (`*.png`), the way a
        /// file chooser's type filter does. An empty list shows everything;
        /// folders are never filtered.
        #[qinvokable]
        fn apply_name_filters(self: Pin<&mut SideritaController>, patterns: &QStringList);

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

        /// The kind ("directory" | "file" | "symlink") of the entry at `index`
        /// — the quick-look overlay uses it to pick a folder/file glyph.
        #[qinvokable]
        fn entry_kind(self: &SideritaController, index: i32) -> QString;

        /// Sets (or, with an empty `icon`, clears) the custom icon for `path`,
        /// persisting it. Refreshes `custom_icon_entries`.
        #[qinvokable]
        fn set_custom_icon(self: Pin<&mut SideritaController>, path: &QString, icon: &QString);

        /// Re-reads the saved overrides — how a tab picks up an icon another
        /// tab just changed.
        #[qinvokable]
        fn reload_custom_icons(self: Pin<&mut SideritaController>);

        /// Stars `path` if it is not starred, un-stars it if it is, and
        /// persists either way. Refreshes `favorite_paths`.
        #[qinvokable]
        fn toggle_favorite(self: Pin<&mut SideritaController>, path: &QString);

        /// Re-reads the starred paths — how a tab picks up a star another tab
        /// just set.
        #[qinvokable]
        fn reload_favorites(self: Pin<&mut SideritaController>);

        /// Navigates to the folder holding `path` and selects that entry.
        #[qinvokable]
        fn reveal_path(self: Pin<&mut SideritaController>, path: &QString);

        /// A bounded, read-only text preview of the file at `path` for the
        /// quick-look overlay: up to a fixed byte budget, decoded lossily.
        /// Returns an empty string for a binary file (or one it cannot read),
        /// which the overlay reads as "no text preview".
        #[qinvokable]
        fn preview_text(self: &SideritaController, path: &QString) -> QString;

        #[qinvokable]
        fn add_bookmark(self: Pin<&mut SideritaController>, path: &QString);

        #[qinvokable]
        fn remove_bookmark(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn rename_bookmark(self: Pin<&mut SideritaController>, index: i32, name: &QString);

        /// Reorders the sidebar: moves the bookmark at `from` so it sits at
        /// `to`, and persists the new order.
        #[qinvokable]
        fn move_bookmark(self: Pin<&mut SideritaController>, from: i32, to: i32);

        #[qinvokable]
        fn place_path(self: &SideritaController, key: &QString) -> QString;

        /// Reorders the sidebar's places (indices into `place_keys`) and
        /// persists the order.
        #[qinvokable]
        fn move_place(self: Pin<&mut SideritaController>, from: i32, to: i32);

        /// Drops a place from the sidebar until the user asks for it back.
        #[qinvokable]
        fn hide_place(self: Pin<&mut SideritaController>, key: &QString);

        /// Brings every hidden place back.
        #[qinvokable]
        fn unhide_all_places(self: Pin<&mut SideritaController>);

        /// Re-reads the persisted place order and hidden set.
        #[qinvokable]
        fn reload_places(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn new_folder(self: Pin<&mut SideritaController>, name: &QString);

        #[qinvokable]
        fn new_file(self: Pin<&mut SideritaController>, name: &QString);

        #[qinvokable]
        fn rename_path(self: Pin<&mut SideritaController>, path: &QString, new_name: &QString);

        /// Renames a whole selection: `paths[i]` becomes `names[i]`. Each is
        /// attempted independently; collisions fail alone and are reported.
        #[qinvokable]
        fn rename_paths(
            self: Pin<&mut SideritaController>,
            paths: &QStringList,
            names: &QStringList,
        );

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

        /// Answers the collision currently being asked about with "skip" /
        /// "replace" / "keepboth". With `apply_to_all`, the same answer settles
        /// every collision left in the batch.
        #[qinvokable]
        fn resolve_conflict(
            self: Pin<&mut SideritaController>,
            strategy: &QString,
            apply_to_all: bool,
        );

        #[qinvokable]
        fn cancel_conflicts(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn undo(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn load_trash(self: Pin<&mut SideritaController>);

        /// Opens Recientes — the desktop's recently-used list — as a
        /// content-view location.
        #[qinvokable]
        fn open_recent(self: Pin<&mut SideritaController>);

        /// Leaves Recientes and repaints the folder underneath.
        #[qinvokable]
        fn close_recent(self: Pin<&mut SideritaController>);

        /// Opens Trash as a content-view location (fills the entry model with
        /// the trashed items and flips `trash_active`).
        #[qinvokable]
        fn open_trash(self: Pin<&mut SideritaController>);

        /// Leaves Trash and returns the content box to the current folder.
        #[qinvokable]
        fn close_trash(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn restore_trash(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn restore_all_trash(self: Pin<&mut SideritaController>);

        /// Permanently deletes one trashed entry (by its index in the list).
        #[qinvokable]
        fn purge_trash(self: Pin<&mut SideritaController>, index: i32);

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
        fn load_phones(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn open_phone(self: Pin<&mut SideritaController>, index: i32);

        #[qinvokable]
        fn send_to_phone(self: Pin<&mut SideritaController>, path: &QString);

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

        /// Remembers `mode` for the folder being shown, so returning to it
        /// brings that view back.
        #[qinvokable]
        fn remember_view_mode(self: Pin<&mut SideritaController>, mode: &QString);

        /// Drops this folder's remembered view and sort.
        #[qinvokable]
        fn forget_folder_view(self: Pin<&mut SideritaController>);

        /// The window size to reopen at, and how to record a resize.
        #[qinvokable]
        fn saved_window_width(self: &SideritaController) -> i32;

        #[qinvokable]
        fn saved_window_height(self: &SideritaController) -> i32;

        #[qinvokable]
        fn save_window_size(self: Pin<&mut SideritaController>, width: i32, height: i32);

        /// The folders that were open in tabs last time, and which was active.
        #[qinvokable]
        fn saved_tabs(self: &SideritaController) -> QStringList;

        #[qinvokable]
        fn saved_active_tab(self: &SideritaController) -> i32;

        #[qinvokable]
        fn save_tabs(self: Pin<&mut SideritaController>, paths: &QStringList, active: i32);

        /// Whether the process was handed a location to open (argv or a
        /// `file://` URI). A launch that names a folder is about that folder,
        /// so the saved session must not talk over it.
        #[qinvokable]
        fn launch_path_given(self: &SideritaController) -> bool;

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
            sizes: QStringList,
            dates: QStringList,
        );
    }

    impl cxx_qt::Threading for SideritaController {}
}

/// How to reverse the last loss-free operation. Only the three verbs the
/// roadmap names as undoable are recorded — create and copy are not, since
/// undoing them would mean deleting data the user did not ask to lose.
pub(crate) enum UndoAction {
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

mod fileops;
mod find;
mod marks;
mod mounts;
mod navigation;
mod scan;
mod selection;
mod session;
mod shell;
mod trash;
mod view_options;

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
pub(crate) enum ConflictStrategy {
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
/// for the user's conflict choices before the worker starts.
///
/// The choice is per collision: `decisions[i]` is what to do with `sources[i]`,
/// and `cursor` is the collision being asked about. Entries that do not collide
/// carry `Skip` and never reach that code path. One "apply to all" fills the
/// rest in at once — the old behaviour, now something the user opts into rather
/// than the only option.
struct PendingPaste {
    sources: Vec<PathBuf>,
    destination: PathBuf,
    cut: bool,
    decisions: Vec<Option<ConflictStrategy>>,
    colliding: Vec<usize>,
    cursor: usize,
}

/// What a pasted batch did, carried from the worker thread to `finish_paste`.
pub(crate) struct PasteOutcome {
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
pub(crate) enum PendingNav {
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
    trash_active: bool,
    recent_active: bool,
    recent_count: i32,
    custom_icon_entries: QStringList,
    custom_icons: std::collections::HashMap<String, String>,
    favorite_entries: QStringList,
    favorites: std::collections::BTreeSet<String>,
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
    trash_entries: Vec<TrashEntry>,
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
    // Set once the UDisks2 hotplug watch thread is running for this controller.
    volume_watch_started: bool,
    hidden_device_count: i32,
    phone_names: QStringList,
    phone_types: QStringList,
    phone_mounts: QStringList,
    // Set once the Magnetita Changed-signal watch thread is running.
    phone_watch_started: bool,
    phones: Vec<crate::devices::Device>,
    place_keys: QStringList,
    hidden_place_count: i32,
    folder_view_mode: QString,
    folder_view_pinned: bool,
    folder_views: Vec<crate::folder_views::FolderView>,
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
        let custom_icons = crate::icons::load();
        let custom_icon_entries = icon_override_entries(&custom_icons);
        let favorites = crate::favorites::load();
        let favorite_entries = favorite_entry_list(&favorites);
        Self {
            current_path: QString::default(),
            status_text: QString::from("Preparando Siderita…"),
            error_text: QString::default(),
            entry_names: QStringList::default(),
            custom_icons,
            custom_icon_entries,
            favorites,
            favorite_entries,
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
            trash_active: false,
            recent_active: false,
            recent_count: 0,
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
            trash_entries: Vec::new(),
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
            volume_watch_started: false,
            hidden_device_count: 0,
            phone_names: QStringList::default(),
            phone_types: QStringList::default(),
            phone_mounts: QStringList::default(),
            phone_watch_started: false,
            phones: Vec::new(),
            place_keys: QStringList::default(),
            hidden_place_count: 0,
            folder_view_mode: QString::default(),
            folder_view_pinned: false,
            folder_views: crate::folder_views::load(),
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

    /// Whether the rows on screen are a *location's* rows rather than a
    /// folder's: search hits, the Trash, or Recientes. All three ride the same
    /// `search_hits` list, so every row lookup takes the same path.
    fn virtual_rows(&self) -> bool {
        self.search_active || self.trash_active || self.recent_active
    }

    /// A search hit by its token (the hit's index in the results).
    fn search_hit(&self, token: &QString) -> Option<&crate::search::SearchHit> {
        let index = token.to_string().parse::<usize>().ok()?;
        self.search_hits.get(index)
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
            let freed = siderita_ops::next_available(destination_dir, name, "copia");
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

/// The final path component, for a compact per-entry line in a batch error.
/// Falls back to the full lossy path when there is no file name (e.g. `/`).
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// The first non-flag argument: the location to open. Flags (`--portal`) are
/// how the process is told *why* it started, not *where*.
fn launch_argument() -> Option<std::ffi::OsString> {
    std::env::args_os()
        .skip(1)
        .find(|arg| !arg.to_string_lossy().starts_with('-'))
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

/// Builds the parallel (paths, icons) QStringLists the QML folds into its
/// custom-icon map, in a stable sorted order.
/// The overrides as one `path\ticon` line per entry — a single property, so the
/// QML sees a whole map or none of it, never half of one.
fn icon_override_entries(map: &std::collections::HashMap<String, String>) -> QStringList {
    let mut entries: Vec<(&String, &String)> = map.iter().collect();
    entries.sort();
    entries
        .iter()
        .map(|(path, icon)| QString::from(format!("{path}\t{icon}").as_str()))
        .collect()
}

const RECENT_LIMIT: usize = 100;

/// The starred paths as `path\tkind` lines. The kind is resolved here, once per
/// refresh, so the sidebar can show a folder as a folder and say plainly when a
/// favourite's target is gone rather than offering a row that leads nowhere.
fn favorite_entry_list(paths: &std::collections::BTreeSet<String>) -> QStringList {
    paths
        .iter()
        .map(|path| {
            let kind = match std::fs::metadata(path) {
                Ok(meta) if meta.is_dir() => "directory",
                Ok(_) => "file",
                Err(_) => "missing",
            };
            QString::from(format!("{path}\t{kind}").as_str())
        })
        .collect()
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

    format!(
        "{} · {}",
        kind_label(row.kind()),
        crate::format::size(row.size())
    )
}

/// The containing folder of a search hit, shown as its subtitle so a result
/// carries where it lives (the one thing a flat folder row doesn't need).
fn search_hit_parent(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::sort_field_from_index;
    use siderita_core::SortField;
    use std::path::Path;

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
    fn sort_field_indices_are_stable_for_qml() {
        assert_eq!(sort_field_from_index(0), Some(SortField::Name));
        assert_eq!(sort_field_from_index(1), Some(SortField::Size));
        assert_eq!(sort_field_from_index(2), Some(SortField::Modified));
        assert_eq!(sort_field_from_index(3), Some(SortField::Kind));
        assert_eq!(sort_field_from_index(4), None);
    }
}
