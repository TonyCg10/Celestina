use core::pin::Pin;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
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
mod scan;
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
        self.as_mut().refresh_place_props();

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

    /// Repaints whatever is on screen — which is not always a folder. Trash and
    /// Recientes are locations with their own listing, and a folder rescan would
    /// land in a projection that (rightly) refuses to overwrite them, so an entry
    /// deleted from one of those views stayed on screen. Each location refreshes
    /// itself instead.
    pub fn refresh(mut self: Pin<&mut Self>) {
        if self.rust().recent_active {
            self.as_mut().open_recent();
            return;
        }
        if self.rust().trash_active {
            self.as_mut().load_trash();
            self.as_mut().publish_trash();
            return;
        }
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

    pub fn apply_query(mut self: Pin<&mut Self>, query: &QString) {
        if self.query() == query {
            return;
        }

        self.as_mut().set_query(query.clone());
        self.as_mut().rust_mut().get_mut().options.query = query.to_string();
        self.as_mut().reproject();
    }

    pub fn apply_name_filters(mut self: Pin<&mut Self>, patterns: &QStringList) {
        let patterns: Vec<String> = patterns
            .iter()
            .map(ToString::to_string)
            .filter(|pattern| !pattern.is_empty())
            .collect();
        if self.rust().options.name_filters == patterns {
            return;
        }
        self.as_mut().rust_mut().get_mut().options.name_filters = patterns;
        self.as_mut().reproject();
    }

    pub fn select_token(mut self: Pin<&mut Self>, token: &QString) {
        // The selected item's name and detail are shown in the sidebar info box
        // (driven by selected_token), so selecting no longer writes the status
        // line. A search hit's token is its index — accepted as-is if in range.
        if self.rust().virtual_rows() {
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
        // In Trash, activating an entry does nothing — restore / delete are the
        // actions, offered by the context menu; nothing is "opened" from Trash.
        if self.rust().trash_active {
            return;
        }
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

    pub fn entry_token(&self, index: i32) -> QString {
        if self.rust().virtual_rows() {
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
        // A trashed entry's detail is where it came from and when it was deleted.
        if self.rust().trash_active {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().trash_entries.get(i))
                .map(|e| {
                    let origin = e.original.to_string_lossy();
                    let date = crate::format::trash_date(&e.deletion_date);
                    QString::from(
                        if date.is_empty() {
                            origin.into_owned()
                        } else {
                            format!("{origin} · {date}")
                        }
                        .as_str(),
                    )
                })
                .unwrap_or_default();
        }
        let Some(row) = self.rust().row(index) else {
            return QString::default();
        };
        let kind = kind_label(row.kind());
        let date = row
            .modified()
            .map(crate::format::system_time)
            .unwrap_or_default();
        // Folders show kind + date (their entry size is not meaningful); files
        // show kind · size · date.
        let detail = if row.kind() == RowKind::Directory {
            if date.is_empty() {
                kind.to_owned()
            } else {
                format!("{kind} · {date}")
            }
        } else {
            let size = crate::format::size(row.size());
            if date.is_empty() {
                format!("{kind} · {size}")
            } else {
                format!("{kind} · {size} · {date}")
            }
        };
        QString::from(detail.as_str())
    }

    pub fn index_for_token(&self, token: &QString) -> i32 {
        if self.rust().virtual_rows() {
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
        if self.rust().virtual_rows() {
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

    pub fn entry_kind(&self, index: i32) -> QString {
        if self.rust().virtual_rows() {
            return usize::try_from(index)
                .ok()
                .and_then(|i| self.rust().search_hits.get(i))
                .map(|hit| QString::from(if hit.is_dir { "directory" } else { "file" }))
                .unwrap_or_default();
        }
        self.rust()
            .row(index)
            .map(|row| QString::from(kind_key(row.kind())))
            .unwrap_or_default()
    }

    pub fn set_custom_icon(mut self: Pin<&mut Self>, path: &QString, icon: &QString) {
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        let icon = icon.to_string();
        {
            let map = &mut self.as_mut().rust_mut().get_mut().custom_icons;
            if icon.is_empty() {
                map.remove(&path);
            } else {
                map.insert(path, icon);
            }
        }
        let _ = crate::icons::save(&self.rust().custom_icons);
        self.as_mut().refresh_custom_icon_props();
    }

    /// Re-reads the saved overrides from disk. Each tab owns its own controller
    /// and its own copy of the map, so after one tab writes an override the
    /// others are told to reload — otherwise they keep the old icon until the
    /// next start.
    pub fn reload_custom_icons(mut self: Pin<&mut Self>) {
        let loaded = crate::icons::load();
        self.as_mut().rust_mut().get_mut().custom_icons = loaded;
        self.as_mut().refresh_custom_icon_props();
    }

    fn refresh_custom_icon_props(mut self: Pin<&mut Self>) {
        let entries = icon_override_entries(&self.rust().custom_icons);
        self.as_mut().set_custom_icon_entries(entries);
    }

    pub fn toggle_favorite(mut self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        {
            let set = &mut self.as_mut().rust_mut().get_mut().favorites;
            if !set.remove(&path) {
                set.insert(path);
            }
        }
        let _ = crate::favorites::save(&self.rust().favorites);
        self.as_mut().refresh_favorite_props();
    }

    /// Like the icon overrides: each tab holds its own copy, so a tab re-reads
    /// the file when it is activated rather than trusting what it loaded at
    /// start.
    pub fn reload_favorites(mut self: Pin<&mut Self>) {
        let loaded = crate::favorites::load();
        self.as_mut().rust_mut().get_mut().favorites = loaded;
        self.as_mut().refresh_favorite_props();
    }

    fn refresh_favorite_props(mut self: Pin<&mut Self>) {
        let entries = favorite_entry_list(&self.rust().favorites);
        self.as_mut().set_favorite_entries(entries);
    }

    /// Opens the folder holding `path` and selects that entry once it lands —
    /// how a starred *file* reveals itself from the sidebar, instead of the
    /// sidebar quietly launching an application.
    pub fn reveal_path(mut self: Pin<&mut Self>, path: &QString) {
        let path = PathBuf::from(path.to_string());
        let Some(parent) = path.parent().map(Path::to_path_buf) else {
            return;
        };
        self.as_mut().rust_mut().get_mut().pending_select_path = Some(path);
        self.as_mut().request_nav_scan(PendingNav::To(parent));
    }

    pub fn preview_text(&self, path: &QString) -> QString {
        // Cap the read: a preview only needs the first screenful or two, and this
        // runs on the GUI thread (the user pressed space), so it must stay cheap.
        const MAX_BYTES: usize = 128 * 1024;
        let path = path.to_string();
        if path.is_empty() {
            return QString::default();
        }
        let Ok(file) = std::fs::File::open(&path) else {
            return QString::default();
        };
        use std::io::Read;
        let mut buf = Vec::new();
        if file.take(MAX_BYTES as u64).read_to_end(&mut buf).is_err() {
            return QString::default();
        }
        // A NUL byte in the sample is the cheap, reliable "this is binary" tell —
        // real text files don't carry them, most binaries do within 128 KiB.
        if buf.contains(&0) {
            return QString::default();
        }
        // Lossy so one stray non-UTF-8 byte shows a � rather than blanking the
        // whole preview; genuinely binary content was already rejected above.
        QString::from(String::from_utf8_lossy(&buf).as_ref())
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

    pub fn move_bookmark(mut self: Pin<&mut Self>, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let moved = {
            let list = &mut self.as_mut().rust_mut().get_mut().bookmarks;
            crate::bookmarks::move_item(list, from, to)
        };
        if !moved {
            return;
        }
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

    /// Renames a whole selection in one pass: `paths[i]` becomes `names[i]`.
    /// Each rename is attempted independently and refuses to overwrite (the
    /// domain guarantees that), so a name that collides fails alone and is
    /// reported — nothing else in the batch is rolled back or lost.
    pub fn rename_paths(mut self: Pin<&mut Self>, paths: &QStringList, names: &QStringList) {
        self.as_mut().set_op_error(QString::default());
        let paths = qstringlist_to_paths(paths);
        let names: Vec<String> = names.iter().map(ToString::to_string).collect();
        if paths.is_empty() || paths.len() != names.len() {
            return;
        }
        let cancellation = CancellationToken::new();
        let mut failures = Vec::new();
        for (path, name) in paths.iter().zip(names.iter()) {
            // La validación del nombre la hace `siderita_ops::rename` (rechaza
            // vacío, separador, `.`/`..` y NUL), igual que el renombrado de uno
            // en uno; un pre-chequeo a mano aquí sólo repetía la mitad, peor.
            if let Err(error) = siderita_ops::rename(path, OsStr::new(name), &cancellation) {
                failures.push(format!("{}: {error}", display_name(path)));
            }
        }
        // Deliberately no undo: a batch rename is many renames, and the single
        // undo slot can only honestly reverse one.
        self.as_mut().set_undo(None);
        self.as_mut().finish_batch(paths.len(), &failures);
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

        // First load also arms the hotplug watch, so later plug/unplug events
        // refresh the list on their own.
        self.as_mut().start_volume_watch();
    }

    /// Starts, once per controller, a background thread that watches UDisks2 for
    /// a device being added or removed and reloads the list on the Qt thread —
    /// so plugging or unplugging a drive updates "Dispositivos" without a manual
    /// refresh. Best-effort: an unavailable bus just logs and gives up.
    fn start_volume_watch(mut self: Pin<&mut Self>) {
        if self.rust().volume_watch_started {
            return;
        }
        self.as_mut().rust_mut().get_mut().volume_watch_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::volumes::watch_changes(move || {
                let _ = qt.queue(|controller: Pin<&mut qobject::SideritaController>| {
                    controller.load_volumes();
                });
            });
            if let Err(error) = result {
                eprintln!("Siderita: watch de dispositivos no disponible: {error}");
            }
        });
    }

    /// Reads the phones Magnetita reports and publishes them to the sidebar
    /// (parallel name / type / mount-path lists), keeping the records for
    /// open-by-index. Read-only and quick — runs inline. Also arms the watch so
    /// later connect / mount / leave events refresh on their own.
    pub fn load_phones(mut self: Pin<&mut Self>) {
        let phones = crate::devices::list_devices().unwrap_or_default();

        let names: QStringList = phones
            .iter()
            .map(|phone| QString::from(phone.name.as_str()))
            .collect();
        let types: QStringList = phones
            .iter()
            .map(|phone| QString::from(phone.device_type.as_str()))
            .collect();
        let mounts: QStringList = phones
            .iter()
            .map(|phone| QString::from(phone.mount_path.as_str()))
            .collect();

        self.as_mut().rust_mut().get_mut().phones = phones;
        self.as_mut().set_phone_names(names);
        self.as_mut().set_phone_types(types);
        self.as_mut().set_phone_mounts(mounts);

        self.as_mut().start_phone_watch();
    }

    /// Starts, once per controller, a thread that watches Magnetita's `Changed`
    /// signal and reloads the phone list on the Qt thread — so a phone
    /// connecting, mounting or leaving updates "Dispositivos" without a manual
    /// refresh. Best-effort: an unavailable bus just logs and gives up.
    fn start_phone_watch(mut self: Pin<&mut Self>) {
        if self.rust().phone_watch_started {
            return;
        }
        self.as_mut().rust_mut().get_mut().phone_watch_started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let result = crate::devices::watch_changes(move || {
                let _ = qt.queue(|controller: Pin<&mut qobject::SideritaController>| {
                    controller.load_phones();
                });
            });
            if let Err(error) = result {
                eprintln!("Siderita: watch de Magnetita no disponible: {error}");
            }
        });
    }

    /// Opens the phone at `index` by navigating to its mount path. A phone that
    /// is connected but not yet mounted has no path, so this is a no-op until it
    /// is — the sidebar reflects that by not offering it as openable.
    pub fn open_phone(mut self: Pin<&mut Self>, index: i32) {
        let mount = usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().phones.get(index))
            .map(|phone| phone.mount_path.clone())
            .unwrap_or_default();
        if !mount.is_empty() {
            self.as_mut().open_location(&QString::from(mount.as_str()));
        }
    }

    /// Send a local file to the connected phone (the "Enviar al móvil" menu
    /// item). Sends to the first connected phone; a no-op if none is connected.
    pub fn send_to_phone(self: Pin<&mut Self>, path: &QString) {
        if let Some(phone) = self.rust().phones.first() {
            crate::devices::send_file(&phone.id, &path.to_string());
        }
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
                .set_prop_size(QString::from(crate::format::size_full(size).as_str())),
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
                    let text = crate::format::size_full(size);
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
        // Search always renders as the sectioned list, never the details
        // columns, so the size/date columns are left blank for hits.
        let blank: QStringList = hits.iter().map(|_| QString::default()).collect();

        self.as_mut().rust_mut().get_mut().search_hits = hits;
        self.as_mut()
            .set_search_summary(QString::from(summary.as_str()));
        self.as_mut().set_search_running(false);
        self.as_mut().set_search_active(true);
        // A fresh result set drops any selection carried over from the folder.
        self.as_mut().set_selected_token(QString::default());
        self.as_mut().set_entry_names(names.clone());
        self.as_mut().rows_ready(
            names,
            tokens,
            kinds,
            subtitles,
            paths,
            sections,
            blank.clone(),
            blank,
        );
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

    /// Republishes the sidebar's places: the keys that exist here, in the
    /// user's order, minus the ones they hid — plus how many are hidden, so the
    /// sidebar can offer them back.
    fn refresh_place_props(mut self: Pin<&mut Self>) {
        let (visible, hidden_count) = {
            let rust = self.rust();
            let existing: Vec<&str> = PLACE_CATALOGUE
                .iter()
                .copied()
                // TRASH and RECENT are not XDG directories but locations the
                // app always offers; the rest exist only if the folder does.
                .filter(|key| matches!(*key, "TRASH" | "RECENT") || rust.places.contains_key(*key))
                .collect();

            // The saved order first (only keys that still exist), then anything
            // it never mentioned, in catalogue order.
            let mut ordered: Vec<&str> = Vec::with_capacity(existing.len());
            for key in &rust.settings.place_order {
                if let Some(found) = existing.iter().find(|candidate| *candidate == key) {
                    if !ordered.contains(found) {
                        ordered.push(found);
                    }
                }
            }
            for key in &existing {
                if !ordered.contains(key) {
                    ordered.push(key);
                }
            }

            let hidden = &rust.settings.hidden_places;
            let visible: QStringList = ordered
                .iter()
                .filter(|key| !hidden.iter().any(|h| h == *key))
                .map(|key| QString::from(*key))
                .collect();
            let hidden_count = ordered
                .iter()
                .filter(|key| hidden.iter().any(|h| h == *key))
                .count();
            (visible, hidden_count)
        };
        self.as_mut().set_place_keys(visible);
        self.as_mut()
            .set_hidden_place_count(hidden_count.min(i32::MAX as usize) as i32);
    }

    /// Moves the place at `from` so it sits at `to` among the *visible* places,
    /// and persists the whole order.
    pub fn move_place(mut self: Pin<&mut Self>, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let mut keys: Vec<String> = self
            .place_keys()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if from == to || from >= keys.len() || to >= keys.len() {
            return;
        }
        let moved = keys.remove(from);
        keys.insert(to, moved);

        let mut settings = crate::settings::load();
        // Hidden places keep their relative place at the end, so un-hiding one
        // does not scramble the order the user just set.
        let hidden: Vec<String> = settings.hidden_places.clone();
        settings.place_order = keys.into_iter().chain(hidden).collect();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }

    /// Re-reads the sidebar settings — how a tab picks up an order or a hide
    /// another tab just set (the bookmarks and icons do the same).
    pub fn reload_places(mut self: Pin<&mut Self>) {
        let settings = crate::settings::load();
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }

    pub fn hide_place(mut self: Pin<&mut Self>, key: &QString) {
        let key = key.to_string();
        if key.is_empty() {
            return;
        }
        let mut settings = crate::settings::load();
        if !settings.hidden_places.contains(&key) {
            settings.hidden_places.push(key);
            let _ = crate::settings::save(&settings);
        }
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }

    /// Un-hides every previously-hidden place.
    pub fn unhide_all_places(mut self: Pin<&mut Self>) {
        let mut settings = crate::settings::load();
        settings.hidden_places.clear();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().refresh_place_props();
    }

    /// Un-hides every previously-hidden device.
    pub fn unhide_all_devices(mut self: Pin<&mut Self>) {
        let mut settings = crate::settings::load();
        settings.hidden_devices.clear();
        let _ = crate::settings::save(&settings);
        self.as_mut().rust_mut().get_mut().settings = settings;
        self.as_mut().load_volumes();
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

fn initial_location() -> PathBuf {
    match launch_argument() {
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

/// The first non-flag argument: the location to open. Flags (`--portal`) are
/// how the process is told *why* it started, not *where*.
fn launch_argument() -> Option<std::ffi::OsString> {
    std::env::args_os()
        .skip(1)
        .find(|arg| !arg.to_string_lossy().starts_with('-'))
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

/// The starred paths as `path\tkind` lines. The kind is resolved here, once per
/// refresh, so the sidebar can show a folder as a folder and say plainly when a
/// favourite's target is gone rather than offering a row that leads nowhere.
/// Every sidebar place Siderita knows how to offer, in the order it offers them
/// before the user rearranges anything. The keys are the vocabulary the QML
/// maps to a label and an icon; `TRASH` is Siderita's own, the rest are XDG.
const PLACE_CATALOGUE: &[&str] = &[
    "HOME",
    "DESKTOP",
    "DOCUMENTS",
    "DOWNLOAD",
    "MUSIC",
    "PICTURES",
    "VIDEOS",
    "RECENT",
    "TRASH",
];

const RECENT_LIMIT: usize = 100;

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
