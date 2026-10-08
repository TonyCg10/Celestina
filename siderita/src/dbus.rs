//! `org.freedesktop.FileManager1` — the D-Bus interface other applications call
//! for "Show in file manager". A background thread owns a session-bus connection
//! serving the interface; each call is marshalled onto the Qt thread as a signal
//! the QML turns into a tab.
//!
//! The service is best-effort: if another manager already owns the name, or
//! there is no session bus, it simply does not register — the app is unaffected.

use core::pin::Pin;
use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    // Match the controller bridge: expose snake_case Rust names to QML in
    // camelCase, so the signal is `openFolderRequested` (handler
    // `onOpenFolderRequested`) — without this the QML sees the raw
    // `open_folder_requested` and the handler assignment fails to resolve.
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type FileManager1Service = super::FileManager1ServiceRust;

        /// Emitted on the Qt thread when another application asks (over D-Bus)
        /// to show a folder; the QML routes it to a new tab. The folder is
        /// named by its path key (ADR 0008), which is what a tab opens on — the
        /// `file://` URIs of the D-Bus interface itself are unchanged.
        #[qsignal]
        fn open_folder_requested(self: Pin<&mut FileManager1Service>, path: QString);

        /// Emitted on the Qt thread when the name could not be served (no
        /// session bus, or another file manager owns it). A process D-Bus
        /// activated for the name has nothing left to do and leaves.
        #[qsignal]
        fn service_unavailable(self: Pin<&mut FileManager1Service>, reason: QString);

        /// Emitted on the Qt thread when a request names no local folder
        /// (another host, a query, the root's parent). A process activated for
        /// that request alone would otherwise hold the name hidden forever.
        #[qsignal]
        fn request_without_folder(self: Pin<&mut FileManager1Service>);

        #[qinvokable]
        fn start(self: Pin<&mut FileManager1Service>);

        /// Whether this process was started by D-Bus activation to serve the
        /// name (`--file-manager`, passed by the service file): the window
        /// stays hidden until the first request names a folder.
        #[qinvokable]
        fn file_manager_mode(self: &FileManager1Service) -> bool;
    }

    impl cxx_qt::Threading for FileManager1Service {}
}

#[derive(Default)]
pub struct FileManager1ServiceRust {
    started: bool,
}

impl qobject::FileManager1Service {
    /// Starts serving `org.freedesktop.FileManager1`, once. Best-effort: a taken
    /// name or an absent session bus logs and gives up rather than failing.
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            let notifier = qt.clone();
            if let Err(error) = serve(qt) {
                eprintln!("Siderita: FileManager1 D-Bus no disponible: {error}");
                let reason = error.to_string();
                let _ = notifier.queue(move |service| {
                    service.service_unavailable(QString::from(reason.as_str()));
                });
            }
        });
    }

    /// Whether `--file-manager` was passed: activated to serve the name.
    pub fn file_manager_mode(&self) -> bool {
        file_manager_mode()
    }
}

/// `--file-manager` anywhere in the arguments: the D-Bus service file passes it
/// when the bus activates this process for a `FileManager1` call. A flag, so
/// `launch_argument` never takes it for a folder.
pub fn file_manager_mode() -> bool {
    crate::portal::has_flag("--file-manager")
}

/// The served object: it forwards each request onto the Qt thread and never
/// touches Qt state directly.
struct FileManager1 {
    qt: cxx_qt::CxxQtThread<qobject::FileManager1Service>,
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager1 {
    fn show_folders(&self, uris: Vec<String>, _startup_id: String) {
        self.request_folders(folders_of(&uris, Target::Folders));
    }

    fn show_items(&self, uris: Vec<String>, _startup_id: String) {
        self.request_folders(folders_of(&uris, Target::Items));
    }

    fn show_item_properties(&self, uris: Vec<String>, _startup_id: String) {
        // A properties panel is CP3; land the user in the containing folder.
        self.request_folders(folders_of(&uris, Target::Items));
    }
}

/// What the URIs of a request name: the folders themselves, or items whose
/// containing folder is shown (selecting the items is a refinement).
#[derive(Clone, Copy)]
enum Target {
    Folders,
    Items,
}

/// The local folders a request resolves to. Any session peer may call; a URI
/// naming another host, or carrying a query or a fragment, names no local
/// folder, and neither does an item without a parent.
fn folders_of(uris: &[String], target: Target) -> Vec<PathBuf> {
    uris.iter()
        .filter_map(|uri| match target {
            Target::Folders => celestina_core::file_uri::to_path(uri).ok(),
            Target::Items => parent_folder(uri),
        })
        .collect()
}

impl FileManager1 {
    fn request_folders(&self, folders: Vec<PathBuf>) {
        if folders.is_empty() {
            let _ = self.qt.queue(|service| service.request_without_folder());
            return;
        }
        for folder in folders {
            let key = crate::pathkey::encode(&folder);
            let _ = self.qt.queue(move |service| {
                service.open_folder_requested(QString::from(key.as_str()));
            });
        }
    }
}

fn serve(qt: cxx_qt::CxxQtThread<qobject::FileManager1Service>) -> zbus::Result<()> {
    // As in the portal backend: serve the object first, then request the name
    // with `DoNotQueue`, so a second Siderita answers `Exists` (an error here)
    // instead of waiting in the name's queue for the rest of the session.
    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at("/org/freedesktop/FileManager1", FileManager1 { qt })?
        .build()?;
    connection.request_name_with_flags(
        "org.freedesktop.FileManager1",
        zbus::fdo::RequestNameFlags::DoNotQueue.into(),
    )?;
    let _connection = connection;
    // Keep the connection — and thus the service — alive for the process.
    loop {
        std::thread::park();
    }
}

/// The containing folder of a `file://` item URI naming a local file.
fn parent_folder(uri: &str) -> Option<PathBuf> {
    celestina_core::file_uri::to_path(uri)
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use super::{folders_of, parent_folder, Target};
    use std::path::PathBuf;

    /// SID-H1-R: a request naming no local folder must be told apart, so a
    /// hidden `--file-manager` process can leave instead of holding the name.
    #[test]
    fn a_request_naming_no_local_folder_resolves_to_none() {
        let uris = |list: &[&str]| list.iter().map(|u| u.to_string()).collect::<Vec<_>>();
        assert!(folders_of(&uris(&["smb://host/x"]), Target::Folders).is_empty());
        assert!(folders_of(&uris(&["file:///home/toni?x=1"]), Target::Folders).is_empty());
        assert!(folders_of(&uris(&["file:///"]), Target::Items).is_empty());
        assert_eq!(
            folders_of(&uris(&["smb://host/x", "file:///tmp"]), Target::Folders),
            vec![PathBuf::from("/tmp")]
        );
    }

    #[test]
    fn parent_folder_of_an_item_uri() {
        assert_eq!(
            parent_folder("file:///home/toni/nota.txt"),
            Some(PathBuf::from("/home/toni"))
        );
    }

    /// SID-18: `FileManager1` answers any session peer, and another host's
    /// file used to open the local path of the same name.
    #[test]
    fn an_item_on_another_host_has_no_local_folder() {
        assert_eq!(parent_folder("file://otherhost/etc/passwd"), None);
        assert_eq!(parent_folder("file:///home/toni/nota.txt#x"), None);
        assert_eq!(
            parent_folder("file://localhost/home/toni/nota.txt"),
            Some(PathBuf::from("/home/toni"))
        );
    }
}
