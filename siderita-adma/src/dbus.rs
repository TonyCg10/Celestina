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

        /// Emitted on the Qt thread when another application asks (over D-Bus) to
        /// show a folder; the QML routes it to a new tab.
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
        self.request_folders(uris.iter().filter_map(|uri| uri_to_path(uri)));
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
            let path = folder.to_string_lossy().into_owned();
            let _ = self.qt.queue(move |service| {
                service.open_folder_requested(QString::from(path.as_str()));
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

/// The containing folder of a `file://` item URI.
fn parent_folder(uri: &str) -> Option<PathBuf> {
    uri_to_path(uri).and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Converts a `file://` URI to a local path, percent-decoded byte-for-byte so a
/// non-UTF-8 path round-trips. Returns `None` for a non-file URI. Shared with the
/// path bar's `file://` handling.
pub(crate) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // Drop an optional authority (host) before the path's leading '/'.
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(index) => &rest[index..],
        None => return None,
    };
    let bytes = percent_decode(path);
    if bytes.is_empty() {
        return None;
    }
    Some(path_from_bytes(&bytes))
}

/// Decodes `%XX` escapes to bytes; a malformed escape is kept verbatim so a
/// stray `%` never drops the rest of the path.
fn percent_decode(value: &str) -> Vec<u8> {
    let raw = value.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if let (Some(high), Some(low)) = (
                raw.get(index + 1).and_then(|b| hex_value(*b)),
                raw.get(index + 2).and_then(|b| hex_value(*b)),
            ) {
                out.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        out.push(raw[index]);
        index += 1;
    }
    out
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{parent_folder, uri_to_path};
    use std::path::PathBuf;

    #[test]
    fn decodes_a_plain_file_uri() {
        assert_eq!(
            uri_to_path("file:///home/toni/nota.txt"),
            Some(PathBuf::from("/home/toni/nota.txt"))
        );
    }

    #[test]
    fn decodes_percent_escapes_and_an_authority() {
        assert_eq!(
            uri_to_path("file:///home/toni/a%20b"),
            Some(PathBuf::from("/home/toni/a b"))
        );
        // A host authority before the path is dropped.
        assert_eq!(
            uri_to_path("file://localhost/etc/hosts"),
            Some(PathBuf::from("/etc/hosts"))
        );
    }

    #[test]
    fn rejects_a_non_file_uri() {
        assert!(uri_to_path("http://example.com/x").is_none());
        assert!(uri_to_path("trash:///").is_none());
    }

    #[test]
    fn parent_folder_of_an_item_uri() {
        assert_eq!(
            parent_folder("file:///home/toni/nota.txt"),
            Some(PathBuf::from("/home/toni"))
        );
    }
}
