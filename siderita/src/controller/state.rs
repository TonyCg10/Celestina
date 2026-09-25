//! The controller's own state, and the reads that only need it.
//!
//! This is the data half of `controller.rs`: the struct cxx-qt maps the QObject
//! onto, its starting values, and the four lookups that ask nothing of Qt. It
//! lives here because the coordinator had grown to its inventoried ceiling and
//! a new published contract no longer fit — and because a file of ninety field
//! declarations and their defaults is not where anyone goes looking for
//! behaviour.
//!
//! Everything the parent has in scope is imported wholesale: this module is the
//! parent's own declarations, moved, not a new boundary. The fields are
//! `pub(super)` for the same reason — every sibling module in `controller/`
//! already read them when they were its neighbours.

use super::*;

pub struct SideritaControllerRust {
    pub(super) current_path: QString,
    pub(super) current_path_key: QString,
    pub(super) marked_key: QString,
    pub(super) path_crumbs: QStringList,
    pub(super) collapsed_sections: QStringList,
    pub(super) error_text: QString,
    pub(super) entry_names: QStringList,
    pub(super) selected_token: QString,
    pub(super) query: QString,
    pub(super) loading: bool,
    pub(super) can_go_back: bool,
    pub(super) can_go_forward: bool,
    pub(super) can_go_up: bool,
    pub(super) show_hidden: bool,
    pub(super) sort_field: i32,
    pub(super) sort_ascending: bool,
    pub(super) notice_rows: QStringList,
    pub(super) notices: notices::Notices,
    pub(super) coordinator: ScanCoordinator,
    pub(super) executor: Option<ScanExecutor>,
    pub(super) history: NavigationHistory,
    pub(super) adapter: SnapshotAdapter,
    pub(super) options: ViewOptions,
    pub(super) snapshot: Option<DirectorySnapshot>,
    // Where the window says it is: what was published, and for what folder.
    pub(super) published_digest: Option<u64>,
    pub(super) published_location: Option<PathBuf>,
    pub(super) view: Option<ViewSnapshot>,
    pub(super) pending_nav: Option<PendingNav>,
    /// Whether the scan generation now in flight is a background watcher
    /// refresh. A quiet scan owns no banner: it must never write `error_text`
    /// or the status line, because the folder it is re-reading is being changed
    /// underneath it and the user did not ask for anything.
    pub(super) quiet_scan: bool,
    pub(super) watch: Option<WatchState>,
    pub(super) watched: Option<PathBuf>,
    pub(super) watch_degraded: bool,
    pub(super) folder_visible_count: i32,
    pub(super) folder_total_count: i32,
    pub(super) folder_directory_count: i32,
    pub(super) folder_file_count: i32,
    pub(super) folder_hidden_count: i32,
    pub(super) folder_size: QString,
    pub(super) folder_modified: QString,
    pub(super) folder_accessed: QString,
    pub(super) folder_created: QString,
    pub(super) selection_count: i32,
    pub(super) properties_pending: bool,
    pub(super) prop_name: QString,
    pub(super) prop_path: QString,
    pub(super) prop_key: QString,
    pub(super) prop_kind: QString,
    pub(super) prop_mime: QString,
    pub(super) prop_size: QString,
    pub(super) prop_permissions: QString,
    pub(super) prop_owner: QString,
    pub(super) prop_modified: QString,
    pub(super) prop_accessed: QString,
    pub(super) prop_symlink: QString,
    pub(super) prop_is_dir: bool,
    pub(super) search_active: bool,
    pub(super) trash_active: bool,
    pub(super) recent_active: bool,
    pub(super) recent_count: i32,
    pub(super) custom_icon_entries: QStringList,
    pub(super) custom_icons: std::collections::HashMap<String, crate::icons::IconAppearance>,
    pub(super) favorite_entries: QStringList,
    pub(super) favorites: std::collections::BTreeSet<String>,
    pub(super) search_running: bool,
    pub(super) search_query: QString,
    pub(super) search_summary: QString,
    pub(super) search_names: QStringList,
    pub(super) search_paths: QStringList,
    pub(super) search_kinds: QStringList,
    pub(super) search_hits: Vec<crate::search::SearchHit>,
    pub(super) search_cancel: Option<CancellationToken>,
    pub(super) pending_select_path: Option<PathBuf>,
    pub(super) bookmark_names: QStringList,
    pub(super) bookmark_paths: QStringList,
    pub(super) op_error: QString,
    pub(super) can_paste: bool,
    pub(super) cut_paths: QStringList,
    pub(super) can_undo: bool,
    pub(super) undo_label: QString,
    pub(super) op_running: bool,
    pub(super) op_ids: QStringList,
    pub(super) op_labels: QStringList,
    pub(super) op_currents: QStringList,
    pub(super) op_details: QStringList,
    pub(super) op_percents: QStringList,
    pub(super) op_icons: QStringList,
    pub(super) op_steps: QStringList,
    pub(super) op_paused: QStringList,
    pub(super) pending_password: Option<crate::controller::archive::Pending>,
    pub(super) conflict_pending: bool,
    pub(super) conflict_count: i32,
    pub(super) conflict_name: QString,
    pub(super) password_pending: bool,
    pub(super) password_archive: QString,
    pub(super) password_retry: bool,
    pub(super) pending_paste: Option<PendingPaste>,
    pub(super) trash_names: QStringList,
    pub(super) trash_origins: QStringList,
    pub(super) trash_dates: QStringList,
    pub(super) trash_entries: Vec<TrashEntry>,
    pub(super) open_with_pending: bool,
    pub(super) open_with_target: QString,
    pub(super) open_with_apps: QStringList,
    pub(super) open_with_default_index: i32,
    pub(super) open_with_path: PathBuf,
    pub(super) open_with_mime: String,
    pub(super) open_with_ids: Vec<String>,
    pub(super) volume_names: QStringList,
    pub(super) volume_devices: QStringList,
    pub(super) volume_mounts: QStringList,
    pub(super) volume_busy: bool,
    // Set once the UDisks2 hotplug watch thread is running for this controller.
    pub(super) volume_watch_started: bool,
    pub(super) hidden_device_count: i32,
    pub(super) phone_names: QStringList,
    pub(super) phone_types: QStringList,
    pub(super) phone_mounts: QStringList,
    pub(super) phone_revision: i32,
    pub(super) phone_watch_started: bool,
    pub(super) phones: Vec<crate::devices::Device>,
    pub(super) place_keys: QStringList,
    pub(super) hidden_place_count: i32,
    pub(super) folder_view_mode: QString,
    pub(super) folder_view_pinned: bool,
    pub(super) folder_views: Vec<crate::folder_views::FolderView>,
    pub(super) volumes: Vec<crate::volumes::Volume>,
    pub(super) settings: crate::settings::Settings,
    pub(super) clipboard: Vec<PathBuf>,
    pub(super) clipboard_cut: bool,
    pub(super) last_undo: Option<UndoAction>,
    pub(super) bookmarks: Vec<crate::bookmarks::Bookmark>,
    pub(super) places: std::collections::HashMap<String, String>,
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
            current_path_key: QString::default(),
            marked_key: QString::default(),
            path_crumbs: QStringList::default(),
            // Read at construction so a folded section is already folded when
            // the sidebar first draws.
            collapsed_sections: marks::folded_list(&settings.collapsed_sections),
            error_text: QString::default(),
            entry_names: QStringList::default(),
            custom_icons,
            custom_icon_entries,
            favorites,
            favorite_entries,
            notice_rows: QStringList::default(),
            notices: notices::Notices::default(),
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
            published_digest: None,
            published_location: None,
            view: None,
            pending_nav: None,
            quiet_scan: false,
            watch: None,
            watched: None,
            watch_degraded: false,
            folder_visible_count: 0,
            folder_total_count: 0,
            folder_directory_count: 0,
            folder_file_count: 0,
            folder_hidden_count: 0,
            folder_size: QString::default(),
            folder_modified: QString::default(),
            folder_accessed: QString::default(),
            folder_created: QString::default(),
            selection_count: 0,
            properties_pending: false,
            prop_name: QString::default(),
            prop_path: QString::default(),
            prop_key: QString::default(),
            prop_kind: QString::default(),
            prop_mime: QString::default(),
            prop_size: QString::default(),
            prop_permissions: QString::default(),
            prop_owner: QString::default(),
            prop_modified: QString::default(),
            prop_accessed: QString::default(),
            prop_symlink: QString::default(),
            prop_is_dir: false,
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
            op_ids: QStringList::default(),
            op_labels: QStringList::default(),
            op_currents: QStringList::default(),
            op_details: QStringList::default(),
            op_percents: QStringList::default(),
            op_icons: QStringList::default(),
            op_steps: QStringList::default(),
            op_paused: QStringList::default(),
            pending_password: None,
            conflict_pending: false,
            conflict_count: 0,
            conflict_name: QString::default(),
            password_pending: false,
            password_archive: QString::default(),
            password_retry: false,
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
            phone_revision: 0,
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
            // Published as path keys, like every other path this bridge hands
            // out, so the sidebar can navigate to one without ever spelling it.
            places: crate::places::resolve()
                .into_iter()
                .map(|(name, path)| (name, crate::pathkey::encode(&path)))
                .collect(),
        }
    }
}

impl SideritaControllerRust {
    pub(super) fn row(&self, index: i32) -> Option<&EntryRow> {
        let index = usize::try_from(index).ok()?;
        self.view.as_ref()?.row(index)
    }

    pub(super) fn row_by_token(&self, token: &QString) -> Option<&EntryRow> {
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
    pub(super) fn virtual_rows(&self) -> bool {
        self.search_active || self.trash_active || self.recent_active
    }

    /// A search hit by its token (the hit's index in the results).
    pub(super) fn search_hit(&self, token: &QString) -> Option<&crate::search::SearchHit> {
        let index = token.to_string().parse::<usize>().ok()?;
        self.search_hits.get(index)
    }
}
