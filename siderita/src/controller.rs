use std::path::PathBuf;

use celestina_core::CancellationToken;
use cxx_qt_lib::{QString, QStringList};
use notify_debouncer_full::notify::RecommendedWatcher;
use notify_debouncer_full::{Debouncer, RecommendedCache};
use siderita_core::{
    DirectorySnapshot, NavigationHistory, ScanCoordinator, ScanExecutor, SortDirection, SortField,
    ViewOptions, WatchState,
};

/// The debouncer type the shared watch register owns.
type FsDebouncer = Debouncer<RecommendedWatcher, RecommendedCache>;
use siderita_ops::TrashEntry;
use siderita_qt::{EntryRow, SnapshotAdapter, ViewSnapshot};

/// Every path in this bridge — published or accepted — is a **path key**, the
/// byte-exact identity defined by ADR 0008 and encoded by [`crate::pathkey`].
/// It is opaque ASCII; QML never takes it apart, joins it or decodes it. The
/// text a person reads travels under its own properties (`entry_names`,
/// `current_path`, the subtitles) and is never an argument to anything here.
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
        fn siderita_set_clipboard_uris(uris: &QStringList, cut: bool);

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

    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        // The folder being shown: `current_path` is the lossy text the path bar
        // and the headings read, `current_path_key` its byte-exact identity.
        #[qproperty(QString, current_path)]
        #[qproperty(QString, current_path_key)]
        #[qproperty(QString, marked_key)]
        /// The breadcrumbs for the folder being shown, `key\tname` per entry.
        /// Composed here because QML does not build paths: a Magnetita mount
        /// collapses into one device crumb, and each crumb carries the key its
        /// click navigates to.
        #[qproperty(QStringList, path_crumbs)]
        #[qproperty(QStringList, collapsed_sections)]
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
        /// The running write operations, a job per entry (`controller/jobs.rs`).
        #[qproperty(bool, op_running)]
        #[qproperty(QStringList, op_ids)]
        #[qproperty(QStringList, op_labels)]
        #[qproperty(QStringList, op_currents)]
        #[qproperty(QStringList, op_details)]
        #[qproperty(QStringList, op_percents)]
        #[qproperty(QStringList, op_icons)]
        #[qproperty(QStringList, op_steps)]
        #[qproperty(QStringList, op_paused)]
        /// The transient announcements, `id\ticon\ttone\trunning\ttext` per
        /// entry, cut at the first four tabs because a name may contain one
        /// (`controller/notices.rs`).
        #[qproperty(QStringList, notice_rows)]
        /// An extraction parked on a password: which, and whether it was wrong.
        #[qproperty(bool, password_pending)]
        #[qproperty(QString, password_archive)]
        #[qproperty(bool, password_retry)]
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
        // Per-path custom icon appearances, exposed as ONE list of
        // `path\ticon\taccent` entries the QML folds into a path→appearance map.
        // Deliberately not parallel properties: those are set in sequence, so a
        // QML handler woken by the first can observe the second while still stale.
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
        // Phones from Magnetita; revision publishes one stable device snapshot.
        #[qproperty(QStringList, phone_names)]
        #[qproperty(QStringList, phone_types)]
        #[qproperty(QStringList, phone_mounts)]
        #[qproperty(i32, phone_revision)]
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
        #[qproperty(i32, folder_visible_count)]
        #[qproperty(i32, folder_total_count)]
        #[qproperty(i32, folder_directory_count)]
        #[qproperty(i32, folder_file_count)]
        #[qproperty(i32, folder_hidden_count)]
        #[qproperty(QString, folder_size)]
        #[qproperty(QString, folder_modified)]
        #[qproperty(QString, folder_accessed)]
        #[qproperty(QString, folder_created)]
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

        /// Navigates to whatever a person typed into the path bar: a path,
        /// `~`, or a `file://` URI. The one entry that takes prose rather than
        /// a key, because prose is what a keyboard produces.
        #[qinvokable]
        fn open_location(self: Pin<&mut SideritaController>, location: &QString);

        /// Navigates to the folder a path key names.
        #[qinvokable]
        fn open_key(self: Pin<&mut SideritaController>, key: &QString);

        /// The key for `name` inside the folder being shown — how a surface
        /// that lets someone type a file name (the save picker) names the file
        /// without concatenating anything. Empty for a name that would leave
        /// the folder (`/`, `.`, `..`) or for no folder at all.
        #[qinvokable]
        fn child_key(self: &SideritaController, name: &QString) -> QString;

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

        /// Legacy one-line detail used by contextual/virtual locations.
        #[qinvokable]
        fn entry_detail(self: &SideritaController, index: i32) -> QString;

        /// Compact, presentation-ready lines for the selected entry info card:
        /// kind, optional size and abbreviated local modification date.
        #[qinvokable]
        fn entry_info(self: &SideritaController, index: i32) -> QStringList;

        #[qinvokable]
        fn entry_path(self: &SideritaController, index: i32) -> QString;

        /// The kind ("directory" | "file" | "symlink") of the entry at `index`
        /// — the quick-look overlay uses it to pick a folder/file glyph.
        #[qinvokable]
        fn entry_kind(self: &SideritaController, index: i32) -> QString;

        /// Whether activating the entry at `index` enters a directory: a
        /// folder, or a symlink whose target is one.
        #[qinvokable]
        fn entry_targets_directory(self: &SideritaController, index: i32) -> bool;

        /// Sets (or, with an empty `icon`, clears) the custom icon for `path`,
        /// persisting it. Refreshes `custom_icon_entries`.
        #[qinvokable]
        fn set_custom_icon(self: Pin<&mut SideritaController>, path: &QString, icon: &QString);

        /// Sets (or, with an empty key, restores automatic) the custom Lucide
        /// accent for `path`, independently of its custom icon shape.
        #[qinvokable]
        fn set_custom_icon_accent(
            self: Pin<&mut SideritaController>,
            path: &QString,
            accent: &QString,
        );

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

        /// A bounded, read-only text preview for the quick-look overlay,
        /// decoded lossily. Empty for a binary file — or one it cannot read —
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

        /// Whether the entry a key names is an archive Siderita can extract,
        /// decided by its bytes.
        #[qinvokable]
        fn is_archive(self: &SideritaController, key: &QString) -> bool;

        /// The same question for a whole selection, so a menu offers the
        /// extract verb only when every selected entry really is one.
        #[qinvokable]
        fn are_archives(self: &SideritaController, keys: &QStringList) -> bool;

        /// The file name the compress dialog opens with for this selection and
        /// container format, already stepped past any name that is taken.
        #[qinvokable]
        fn archive_suggested_name(
            self: &SideritaController,
            keys: &QStringList,
            format: &QString,
        ) -> QString;

        /// Extracts every archive in `keys` into the folder being shown.
        #[qinvokable]
        fn extract_keys(self: Pin<&mut SideritaController>, keys: &QStringList);

        /// Compresses every entry in `keys` into `name` (a plain file name in
        /// the folder being shown) using the container `format` names.
        #[qinvokable]
        fn compress_keys(
            self: Pin<&mut SideritaController>,
            keys: &QStringList,
            name: &QString,
            format: &QString,
        );

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

        /// The same drop from a raw `text/uri-list`: the `file://` URIs are
        /// decoded here, by bytes, so a name another manager percent-encoded
        /// outside UTF-8 does not abort the whole batch.
        #[qinvokable]
        fn drop_uri_list(
            self: Pin<&mut SideritaController>,
            uris: &QStringList,
            destination: &QString,
            move_entries: bool,
        );

        #[qinvokable]
        fn cancel_op(self: Pin<&mut SideritaController>);

        /// Cancels one operation by id (a number: QML has no 64-bit integer).
        #[qinvokable]
        fn cancel_job(self: Pin<&mut SideritaController>, id: f64);
        #[qinvokable]
        fn cancel_all_jobs(self: Pin<&mut SideritaController>);

        /// Drops one announcement; `"error"` and `"op-error"` instead clear the
        /// two error properties, which other surfaces also read.
        #[qinvokable]
        fn dismiss_notice(self: Pin<&mut SideritaController>, id: &QString);

        /// Holds one operation where it is, or lets it carry on.
        #[qinvokable]
        fn toggle_job_paused(self: Pin<&mut SideritaController>, id: f64);
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

        /// Resumes the parked extraction with this password. Never stored: it
        /// reaches the domain for that one archive and is dropped with the call.
        #[qinvokable]
        fn answer_password(self: Pin<&mut SideritaController>, password: &QString);

        /// Skips the archive that asked for a password and carries on with the
        /// rest of the batch.
        #[qinvokable]
        fn cancel_password(self: Pin<&mut SideritaController>);

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

        /// Restores one trashed entry, named by the path of its body in the
        /// Trash (never by row index: the list reloads under the menu).
        #[qinvokable]
        fn restore_trash(self: Pin<&mut SideritaController>, trashed: &QString);

        #[qinvokable]
        fn restore_all_trash(self: Pin<&mut SideritaController>);

        /// Permanently deletes one trashed entry, named by the path of its
        /// body in the Trash. Irreversible, so it resolves an identity rather
        /// than a position.
        #[qinvokable]
        fn purge_trash(self: Pin<&mut SideritaController>, trashed: &QString);

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
        fn phone_info(self: &SideritaController, index: i32) -> QStringList;
        #[qinvokable]
        fn ring_phone(self: &SideritaController, index: i32);
        #[qinvokable]
        fn control_phone_media(self: &SideritaController, index: i32, action: &QString);
        #[qinvokable]
        fn display_location_name(self: &SideritaController, path: &QString) -> QString;

        /// `path` as a `file://` URI for a `text/uri-list` payload, encoded by
        /// the same codec the portal answers with.
        #[qinvokable]
        fn path_uri(self: &SideritaController, path: &QString) -> QString;

        /// Whether anything already occupies `path` — the question the save
        /// picker asks before it agrees to overwrite. A symlink counts, and a
        /// link to nothing counts too: the name is taken either way.
        #[qinvokable]
        fn path_exists(self: &SideritaController, path: &QString) -> bool;
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

        /// A name's glyph, the tint that separates what shares it, its own face.
        #[qinvokable]
        fn glyph_for_name(self: &SideritaController, name: &QString) -> QString;
        #[qinvokable]
        fn glyph_accent_for_name(self: &SideritaController, name: &QString) -> QString;
        #[qinvokable]
        fn own_icon_url(self: &SideritaController, key: &QString) -> QString;

        #[qinvokable]
        fn set_section_collapsed(
            self: Pin<&mut SideritaController>,
            section: &QString,
            collapsed: bool,
        );

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

mod actions;
mod archive;
mod display;
mod fileops;
mod find;
mod glyphs;
mod jobs;
mod keys;
mod marks;
mod mounts;
mod navigation;
mod notices;
mod paste;
mod pendingnav;
mod scan;
mod selection;
mod session;
pub(crate) mod shell;
mod sorting;
mod state;
mod trash;
mod view_options;
mod watchreg;

pub(crate) use actions::{ConflictStrategy, UndoAction};
pub(crate) use display::{display_name, kind_key, kind_label, row_subtitle, search_hit_parent};
pub(crate) use marks::{favorite_entry_list, icon_override_entries};
pub(crate) use paste::{PasteOutcome, PendingPaste};
pub(crate) use pendingnav::PendingNav;
pub(crate) use sorting::{sort_field_from_index, RECENT_LIMIT};
pub use state::SideritaControllerRust;

/// The first non-flag argument: the location to open. Flags (`--portal`) are
/// how the process is told *why* it started, not *where*.
fn launch_argument() -> Option<std::ffi::OsString> {
    std::env::args_os()
        .skip(1)
        .find(|arg| !arg.to_string_lossy().starts_with('-'))
}
