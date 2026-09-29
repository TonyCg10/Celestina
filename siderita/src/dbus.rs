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

        #[qinvokable]
        fn start(self: Pin<&mut FileManager1Service>);
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
            if let Err(error) = serve(qt) {
                eprintln!("Siderita: FileManager1 D-Bus no disponible: {error}");
            }
        });
    }
}

/// The served object: it forwards each request onto the Qt thread and never
/// touches Qt state directly.
struct FileManager1 {
    qt: cxx_qt::CxxQtThread<qobject::FileManager1Service>,
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager1 {
    fn show_folders(&self, uris: Vec<String>, _startup_id: String) {
        // Any session peer may call this; a URI naming another host, or one
        // carrying a query or a fragment, names no local folder.
        self.request_folders(
            uris.iter()
                .filter_map(|uri| celestina_core::file_uri::to_path(uri).ok()),
        );
    }

    fn show_items(&self, uris: Vec<String>, _startup_id: String) {
        // Selecting the items themselves is a refinement; for now land the user
        // in each item's containing folder.
        self.request_folders(uris.iter().filter_map(|uri| parent_folder(uri)));
    }

    fn show_item_properties(&self, uris: Vec<String>, _startup_id: String) {
        // A properties panel is CP3; land the user in the containing folder.
        self.request_folders(uris.iter().filter_map(|uri| parent_folder(uri)));
    }
}

impl FileManager1 {
    fn request_folders(&self, folders: impl Iterator<Item = PathBuf>) {
        for folder in folders {
            let key = crate::pathkey::encode(&folder);
            let _ = self.qt.queue(move |service| {
                service.open_folder_requested(QString::from(key.as_str()));
            });
        }
    }
}

fn serve(qt: cxx_qt::CxxQtThread<qobject::FileManager1Service>) -> zbus::Result<()> {
    let _connection = zbus::blocking::connection::Builder::session()?
        .name("org.freedesktop.FileManager1")?
        .serve_at("/org/freedesktop/FileManager1", FileManager1 { qt })?
        .build()?;
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
    use super::parent_folder;
    use std::path::PathBuf;

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
