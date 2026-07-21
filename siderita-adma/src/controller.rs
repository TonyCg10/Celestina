use core::pin::Pin;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use siderita_core::{
    DirectorySnapshot, NavigationHistory, PublishOutcome, ScanCoordinator, ScanExecutor,
    ScanResult, SortDirection, SortField, ViewOptions,
};
use siderita_ops::OpError;
use siderita_qt::{EntryRow, RowKind, SnapshotAdapter, ViewSnapshot};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
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
        #[qproperty(i32, view_revision)]
        #[qproperty(QStringList, bookmark_names)]
        #[qproperty(QStringList, bookmark_paths)]
        #[qproperty(QString, op_error)]
        #[qproperty(bool, can_paste)]
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
        fn entry_kind(self: &SideritaController, index: i32) -> QString;

        #[qinvokable]
        fn entry_subtitle(self: &SideritaController, index: i32) -> QString;

        #[qinvokable]
        fn entry_is_directory(self: &SideritaController, index: i32) -> bool;

        #[qinvokable]
        fn index_for_token(self: &SideritaController, token: &QString) -> i32;

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
        fn copy_to_clipboard(self: Pin<&mut SideritaController>, path: &QString, cut: bool);

        #[qinvokable]
        fn clear_clipboard(self: Pin<&mut SideritaController>);

        #[qinvokable]
        fn paste(self: Pin<&mut SideritaController>);
    }

    impl cxx_qt::Threading for SideritaController {}
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
    view_revision: i32,
    coordinator: ScanCoordinator,
    executor: Option<ScanExecutor>,
    history: NavigationHistory,
    adapter: SnapshotAdapter,
    options: ViewOptions,
    snapshot: Option<DirectorySnapshot>,
    view: Option<ViewSnapshot>,
    pending_location: Option<PathBuf>,
    bookmark_names: QStringList,
    bookmark_paths: QStringList,
    op_error: QString,
    can_paste: bool,
    clipboard: Option<PathBuf>,
    clipboard_cut: bool,
    bookmarks: Vec<crate::bookmarks::Bookmark>,
    places: std::collections::HashMap<String, String>,
}

impl Default for SideritaControllerRust {
    fn default() -> Self {
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
            show_hidden: false,
            sort_field: 0,
            sort_ascending: true,
            view_revision: 0,
            coordinator: ScanCoordinator::new(),
            executor: None,
            history: NavigationHistory::default(),
            adapter: SnapshotAdapter::new(),
            options: ViewOptions::default(),
            snapshot: None,
            view: None,
            pending_location: None,
            bookmark_names: QStringList::default(),
            bookmark_paths: QStringList::default(),
            op_error: QString::default(),
            can_paste: false,
            clipboard: None,
            clipboard_cut: false,
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
        if self
            .as_mut()
            .rust_mut()
            .get_mut()
            .history
            .navigate_to(&destination)
        {
            self.as_mut().request_scan(destination);
        }
    }

    pub fn go_back(mut self: Pin<&mut Self>) {
        let destination = self.as_mut().rust_mut().get_mut().history.go_back();
        if let Some(destination) = destination {
            self.as_mut().request_scan(destination);
        }
    }

    pub fn go_forward(mut self: Pin<&mut Self>) {
        let destination = self.as_mut().rust_mut().get_mut().history.go_forward();
        if let Some(destination) = destination {
            self.as_mut().request_scan(destination);
        }
    }

    pub fn go_up(mut self: Pin<&mut Self>) {
        let destination = self.as_mut().rust_mut().get_mut().history.go_up();
        if let Some(destination) = destination {
            self.as_mut().request_scan(destination);
        }
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
        self.as_mut().request_location_scan(destination);
    }

    pub fn toggle_hidden(mut self: Pin<&mut Self>) {
        let show_hidden = !*self.show_hidden();
        self.as_mut().set_show_hidden(show_hidden);
        self.as_mut().rust_mut().get_mut().options.show_hidden = show_hidden;
        self.as_mut().reproject();
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
        let selected = self.rust().row_by_token(token).map(|row| {
            (
                row.token().to_string(),
                row.display_name().to_owned(),
                row.kind(),
            )
        });

        if let Some((token, name, kind)) = selected {
            self.as_mut()
                .set_selected_token(QString::from(token.as_str()));
            let message = format!("{} · {}", name, kind_label(kind));
            self.as_mut()
                .set_status_text(QString::from(message.as_str()));
        }
    }

    pub fn activate_token(mut self: Pin<&mut Self>, token: &QString) {
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
            if self
                .as_mut()
                .rust_mut()
                .get_mut()
                .history
                .navigate_to(&path)
            {
                self.as_mut().request_scan(path);
            }
        } else {
            self.as_mut().select_token(token);
            let message = format!("{name} · vista de solo lectura en Iteración 1");
            self.as_mut()
                .set_status_text(QString::from(message.as_str()));
        }
    }

    pub fn entry_token(&self, index: i32) -> QString {
        self.rust()
            .row(index)
            .map(|row| QString::from(row.token().to_string().as_str()))
            .unwrap_or_default()
    }

    pub fn entry_kind(&self, index: i32) -> QString {
        self.rust()
            .row(index)
            .map(|row| QString::from(kind_key(row.kind())))
            .unwrap_or_default()
    }

    pub fn entry_subtitle(&self, index: i32) -> QString {
        self.rust()
            .row(index)
            .map(row_subtitle)
            .map(|subtitle| QString::from(subtitle.as_str()))
            .unwrap_or_default()
    }

    pub fn entry_is_directory(&self, index: i32) -> bool {
        self.rust()
            .row(index)
            .is_some_and(|row| row.kind() == RowKind::Directory)
    }

    pub fn index_for_token(&self, token: &QString) -> i32 {
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
        self.finish_op(outcome.map(|_| ()));
    }

    pub fn rename_path(mut self: Pin<&mut Self>, path: &QString, new_name: &QString) {
        self.as_mut().set_op_error(QString::default());
        let path = PathBuf::from(path.to_string());
        let new_name = new_name.to_string();
        let outcome = siderita_ops::rename(&path, OsStr::new(&new_name), &CancellationToken::new());
        self.finish_op(outcome.map(|_| ()));
    }

    pub fn trash_path(mut self: Pin<&mut Self>, path: &QString) {
        self.as_mut().set_op_error(QString::default());
        let path = PathBuf::from(path.to_string());
        let outcome = siderita_ops::trash(&path, &CancellationToken::new());
        self.finish_op(outcome.map(|_| ()));
    }

    pub fn copy_to_clipboard(mut self: Pin<&mut Self>, path: &QString, cut: bool) {
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.clipboard = Some(PathBuf::from(path));
            state.clipboard_cut = cut;
        }
        self.as_mut().set_can_paste(true);
        self.as_mut().set_op_error(QString::default());
    }

    pub fn clear_clipboard(mut self: Pin<&mut Self>) {
        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.clipboard = None;
            state.clipboard_cut = false;
        }
        self.as_mut().set_can_paste(false);
    }

    pub fn paste(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let Some(destination) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        let Some(source) = self.rust().clipboard.clone() else {
            return;
        };
        let cut = self.rust().clipboard_cut;

        let cancellation = CancellationToken::new();
        let outcome = if cut {
            siderita_ops::move_entry(&source, &destination, &cancellation, &mut |_| {}).map(|_| ())
        } else {
            siderita_ops::copy(&source, &destination, &cancellation, &mut |_| {}).map(|_| ())
        };

        // A successful cut consumes the clipboard; a copy keeps it for reuse.
        if outcome.is_ok() && cut {
            self.as_mut().clear_clipboard();
        }
        self.finish_op(outcome);
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

    fn request_scan(mut self: Pin<&mut Self>, destination: PathBuf) {
        self.as_mut().request_scan_inner(destination, false);
    }

    fn request_location_scan(mut self: Pin<&mut Self>, destination: PathBuf) {
        self.as_mut().request_scan_inner(destination, true);
    }

    fn request_scan_inner(mut self: Pin<&mut Self>, destination: PathBuf, commit_on_success: bool) {
        self.as_mut().rust_mut().get_mut().pending_location =
            commit_on_success.then(|| destination.clone());

        let request = match self
            .as_mut()
            .rust_mut()
            .get_mut()
            .coordinator
            .begin(&destination)
        {
            Ok(request) => request,
            Err(error) => {
                self.as_mut().rust_mut().get_mut().pending_location = None;
                self.as_mut().set_loading(false);
                self.as_mut()
                    .set_error_text(QString::from(error.to_string().as_str()));
                return;
            }
        };

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
            self.as_mut().rollback_pending_location();
            self.as_mut().set_loading(false);
            self.as_mut().set_error_text(QString::from(message));
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
                let commits_location = self
                    .rust()
                    .pending_location
                    .as_deref()
                    .is_some_and(|pending| pending == location);

                if commits_location {
                    let state = self.as_mut().rust_mut();
                    let state = state.get_mut();
                    state.history.navigate_to(&location);
                    state.pending_location = None;
                }

                self.as_mut().rust_mut().get_mut().snapshot = Some(snapshot);
                self.as_mut()
                    .set_current_path(QString::from(display_path.as_str()));
                self.as_mut().set_loading(false);
                self.as_mut().set_error_text(QString::default());
                self.as_mut().update_navigation_state();
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
                self.as_mut().rollback_pending_location();
                self.as_mut().set_loading(false);
                self.as_mut()
                    .set_error_text(QString::from(message.as_str()));
                self.as_mut()
                    .set_status_text(QString::from("No se pudo leer la carpeta"));
            }
        }
    }

    fn reproject(mut self: Pin<&mut Self>) {
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
        let visible = view.rows().len();
        let selected_is_visible = {
            let selected = self.selected_token().to_string();
            !selected.is_empty()
                && view
                    .rows()
                    .iter()
                    .any(|row| row.token().to_string() == selected)
        };

        self.as_mut().rust_mut().get_mut().view = Some(view);
        self.as_mut().set_entry_names(names);
        let next_revision = self.view_revision().wrapping_add(1);
        self.as_mut().set_view_revision(next_revision);
        if !selected_is_visible {
            self.as_mut().set_selected_token(QString::default());
        }

        let status = if visible == total {
            format!("{visible} elementos")
        } else {
            format!("{visible} de {total} elementos")
        };
        self.as_mut()
            .set_status_text(QString::from(status.as_str()));
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

    fn rollback_pending_location(mut self: Pin<&mut Self>) {
        let previous_location = {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            let had_pending_location = state.pending_location.take().is_some();
            had_pending_location
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
}

fn initial_location() -> PathBuf {
    std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(home_location)
}

fn home_location() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn resolve_location(input: &str, current: Option<&Path>) -> PathBuf {
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
    fn sort_field_indices_are_stable_for_qml() {
        assert_eq!(sort_field_from_index(0), Some(SortField::Name));
        assert_eq!(sort_field_from_index(1), Some(SortField::Size));
        assert_eq!(sort_field_from_index(2), Some(SortField::Modified));
        assert_eq!(sort_field_from_index(3), Some(SortField::Kind));
        assert_eq!(sort_field_from_index(4), None);
    }
}
