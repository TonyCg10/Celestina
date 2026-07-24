//! `org.freedesktop.impl.portal.FileChooser` — the backend behind every "open a
//! file" and "save as" dialog an application asks the desktop for.
//!
//! An application does not call this: it calls `xdg-desktop-portal`, which picks
//! a *backend* from the ones installed and routes the request here when
//! `portals.conf` says so. That indirection is the whole reason this is worth
//! implementing — the file chooser other apps show can become Siderita's without
//! any of them knowing, over a standard, and without adopting a foreign toolkit's
//! dialog.
//!
//! Shape, and why it is this shape:
//!
//! - The backend interface is **synchronous-looking but long-lived**: the method
//!   reply *is* the answer, so the call is held open — for as long as the user
//!   takes — and only then answered. Each request therefore runs on its own
//!   async task, parks on a channel, and is woken by the picker.
//! - Qt is not touched from this thread. A request is marshalled onto the Qt
//!   thread (like `dbus.rs` does), which opens a picker window; the window
//!   answers through the channel this task is waiting on.
//! - Every request also exports an `org.freedesktop.impl.portal.Request` object
//!   at the handle the caller chose, so the front-end can withdraw the dialog
//!   (`Close()`) if the requesting app disappears. Without it a stranded picker
//!   would outlive the app that asked for it.

use core::pin::Pin;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

/// Portal response codes (xdg-desktop-portal's `XdpResponse`).
const RESPONSE_SUCCESS: u32 = 0;
const RESPONSE_CANCELLED: u32 = 1;

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
        type FileChooserPortal = super::FileChooserPortalRust;

        /// A desktop application is asking for files. `token` identifies this
        /// request when answering; `mode` is `open` | `save` | `saves`.
        #[qsignal]
        fn pick_requested(
            self: Pin<&mut FileChooserPortal>,
            token: QString,
            mode: QString,
            app_id: QString,
            title: QString,
            accept_label: QString,
            multiple: bool,
            directory: bool,
            current_folder: QString,
            current_name: QString,
            filters: QStringList,
        );

        /// The front-end withdrew a request (the asking app went away): the QML
        /// closes that picker without answering.
        #[qsignal]
        fn pick_withdrawn(self: Pin<&mut FileChooserPortal>, token: QString);

        /// The answer has been handed back to the caller. The picker closes on
        /// *this*, not on the click: a dialog that vanishes before its reply is
        /// delivered is a click treated as proof, and on a portal-activated
        /// process the exit can outrun the reply.
        #[qsignal]
        fn pick_answered(self: Pin<&mut FileChooserPortal>, token: QString);

        #[qinvokable]
        fn start(self: Pin<&mut FileChooserPortal>);

        /// Whether this process was started to serve the portal (`--portal`,
        /// how the D-Bus service file activates it) rather than to browse. In
        /// that case there is no main window — only the pickers it is asked for.
        #[qinvokable]
        fn portal_mode(self: &FileChooserPortal) -> bool;

        /// Answers a request with the chosen paths — an empty list is the user
        /// cancelling. Called from the picker window.
        #[qinvokable]
        fn answer(self: Pin<&mut FileChooserPortal>, token: &QString, paths: &QStringList);
    }

    impl cxx_qt::Threading for FileChooserPortal {}
}

/// The picker's answer travels down this. A capacity of one is the whole
/// protocol: exactly one answer per request, and dropping the sender (the
/// front-end withdrawing) closes it, which the waiting task reads as a cancel.
type Slot = async_channel::Sender<Vec<String>>;

#[derive(Default)]
pub struct FileChooserPortalRust {
    started: bool,
    pending: Arc<Mutex<HashMap<String, Slot>>>,
}

impl qobject::FileChooserPortal {
    /// Starts serving the backend, once. Best-effort, exactly like
    /// `FileManager1`: without a session bus, or with the name already taken by
    /// another backend, it logs and gives up rather than failing the app.
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let pending = Arc::clone(&self.rust().pending);
        std::thread::spawn(move || {
            if let Err(error) = serve(qt, pending) {
                eprintln!("Siderita: portal FileChooser no disponible: {error}");
            }
        });
    }

    /// Whether `--portal` was passed: activated as the desktop's file chooser.
    pub fn portal_mode(&self) -> bool {
        portal_mode()
    }

    /// Hands the picker's result to the waiting D-Bus task. An empty list is
    /// the user cancelling — the task tells the difference, not this side.
    pub fn answer(self: Pin<&mut Self>, token: &QString, paths: &QStringList) {
        let token = token.to_string();
        let paths: Vec<String> = paths.iter().map(ToString::to_string).collect();
        let slot = self
            .rust()
            .pending
            .lock()
            .ok()
            .and_then(|map| map.get(&token).cloned());
        if let Some(slot) = slot {
            let _ = slot.try_send(paths);
        }
    }
}

/// The served object. It owns no Qt state: it marshals onto the Qt thread and
/// waits for the picker to answer.
struct FileChooser {
    qt: cxx_qt::CxxQtThread<qobject::FileChooserPortal>,
    pending: Arc<Mutex<HashMap<String, Slot>>>,
    next_token: AtomicU64,
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    #[allow(clippy::too_many_arguments)]
    async fn open_file(
        &self,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        handle: OwnedObjectPath,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(server, handle, "open", app_id, title, options).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_file(
        &self,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        handle: OwnedObjectPath,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(server, handle, "save", app_id, title, options).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_files(
        &self,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        handle: OwnedObjectPath,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(server, handle, "saves", app_id, title, options)
            .await
    }
}

impl FileChooser {
    async fn run(
        &self,
        server: &zbus::ObjectServer,
        handle: OwnedObjectPath,
        mode: &str,
        app_id: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let token = format!("p{}", self.next_token.fetch_add(1, Ordering::Relaxed));
        let (sender, receiver) = async_channel::bounded::<Vec<String>>(1);
        if let Ok(mut map) = self.pending.lock() {
            map.insert(token.clone(), sender);
        }

        // The front-end cancels through this object; it lives exactly as long as
        // the request does.
        let request = Request {
            token: token.clone(),
            pending: Arc::clone(&self.pending),
            qt: self.qt.clone(),
        };
        let _ = server.at(&handle, request).await;

        let multiple = bool_option(&options, "multiple");
        let directory = bool_option(&options, "directory");
        let accept_label = string_option(&options, "accept_label").unwrap_or_default();
        let current_folder = current_folder(&options).unwrap_or_default();
        let current_name = string_option(&options, "current_name").unwrap_or_default();
        let filters = filters(&options);

        {
            let token = token.clone();
            let mode = mode.to_owned();
            let _ = self.qt.queue(move |portal| {
                portal.pick_requested(
                    QString::from(token.as_str()),
                    QString::from(mode.as_str()),
                    QString::from(app_id.as_str()),
                    QString::from(title.as_str()),
                    QString::from(accept_label.as_str()),
                    multiple,
                    directory,
                    QString::from(current_folder.as_str()),
                    QString::from(current_name.as_str()),
                    filters.iter().map(|f| QString::from(f.as_str())).collect(),
                );
            });
        }

        // Await the picker. This is a real await point, not a blocked thread,
        // so the connection keeps serving other calls while a dialog is open —
        // and holding the reply is correct: for a backend, the method reply *is*
        // the answer, and it is expected to take as long as the user does.
        let chosen = receiver.recv().await.ok();

        if let Ok(mut map) = self.pending.lock() {
            map.remove(&token);
        }
        let _ = server.remove::<Request, _>(&handle).await;
        {
            let token = token.clone();
            let _ = self.qt.queue(move |portal| {
                portal.pick_answered(QString::from(token.as_str()));
            });
        }

        match chosen {
            Some(paths) if !paths.is_empty() => {
                let uris: Vec<String> = paths.iter().map(|path| path_to_uri(path)).collect();
                let mut results: HashMap<String, OwnedValue> = HashMap::new();
                if let Ok(value) = OwnedValue::try_from(Value::from(uris)) {
                    results.insert("uris".to_owned(), value);
                }
                if let Ok(value) = OwnedValue::try_from(Value::from(true)) {
                    results.insert("writable".to_owned(), value);
                }
                (RESPONSE_SUCCESS, results)
            }
            _ => (RESPONSE_CANCELLED, HashMap::new()),
        }
    }
}

/// The per-request `Request` object the front-end closes to withdraw a dialog.
struct Request {
    token: String,
    pending: Arc<Mutex<HashMap<String, Slot>>>,
    qt: cxx_qt::CxxQtThread<qobject::FileChooserPortal>,
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    fn close(&self) {
        // Dropping the sender closes the channel, which the waiting task reads
        // as "withdrawn" and answers as cancelled.
        if let Ok(mut map) = self.pending.lock() {
            map.remove(&self.token);
        }
        let token = self.token.clone();
        let _ = self.qt.queue(move |portal| {
            portal.pick_withdrawn(QString::from(token.as_str()));
        });
    }
}

fn serve(
    qt: cxx_qt::CxxQtThread<qobject::FileChooserPortal>,
    pending: Arc<Mutex<HashMap<String, Slot>>>,
) -> zbus::Result<()> {
    let chooser = FileChooser {
        qt,
        pending,
        next_token: AtomicU64::new(1),
    };
    // The blocking builder, like FileManager1: it owns an async connection and
    // its executor internally, so the async interface methods below still get to
    // await without a runtime of our own.
    let _connection = zbus::blocking::connection::Builder::session()?
        .name("org.freedesktop.impl.portal.desktop.celestina")?
        .serve_at("/org/freedesktop/portal/desktop", chooser)?
        .build()?;
    // Keep the connection — and thus the backend — alive for the process.
    loop {
        std::thread::park();
    }
}

/// `--portal` anywhere in the arguments: the D-Bus service file passes it when
/// xdg-desktop-portal activates this process to answer a request.
pub fn portal_mode() -> bool {
    std::env::args_os().any(|arg| arg == "--portal")
}

fn bool_option(options: &HashMap<String, OwnedValue>, key: &str) -> bool {
    options
        .get(key)
        .and_then(|value| bool::try_from(value).ok())
        .unwrap_or(false)
}

fn string_option(options: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    options
        .get(key)
        .and_then(|value| String::try_from(value.try_clone().ok()?).ok())
}

/// `current_folder` arrives as a NUL-terminated byte array, not a string — it is
/// a path, and a path is bytes.
fn current_folder(options: &HashMap<String, OwnedValue>) -> Option<String> {
    let value = options.get("current_folder")?;
    let bytes = Vec::<u8>::try_from(value.try_clone().ok()?).ok()?;
    let trimmed = bytes
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .collect::<Vec<u8>>();
    if trimmed.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&trimmed).into_owned())
}

/// The caller's filters, flattened to `name\tpattern|pattern|…` lines the QML
/// turns into a chooser. The portal's own shape is
/// `a(sa(us))` — a list of (name, list of (kind, pattern)) where kind 0 is a
/// glob and 1 a MIME type.
fn filters(options: &HashMap<String, OwnedValue>) -> Vec<String> {
    let Some(value) = options.get("filters") else {
        return Vec::new();
    };
    let Ok(list) = Vec::<(String, Vec<(u32, String)>)>::try_from(value.try_clone().unwrap_or_else(
        |_| OwnedValue::try_from(Value::from(0u32)).expect("scalar value"),
    )) else {
        return Vec::new();
    };
    list.into_iter()
        .map(|(name, patterns)| {
            let joined = patterns
                .into_iter()
                .map(|(kind, pattern)| {
                    if kind == 1 {
                        format!("mime:{pattern}")
                    } else {
                        pattern
                    }
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("{name}\t{joined}")
        })
        .collect()
}

/// A local path as a `file://` URI, percent-encoding everything a URI cannot
/// carry raw. Byte-wise, so a non-UTF-8 path survives the trip.
fn path_to_uri(path: &str) -> String {
    let mut uri = String::from("file://");
    for byte in path.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                uri.push(*byte as char);
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_become_percent_encoded_file_uris() {
        assert_eq!(path_to_uri("/home/u/a.txt"), "file:///home/u/a.txt");
        assert_eq!(path_to_uri("/home/u/a b.txt"), "file:///home/u/a%20b.txt");
        // Non-ASCII is encoded byte by byte, so it round-trips through the
        // front-end's own decoder.
        assert_eq!(path_to_uri("/home/u/á"), "file:///home/u/%C3%A1");
    }

    #[test]
    fn a_uri_round_trips_through_the_shared_decoder() {
        let path = "/home/u/some dir/ñ.txt";
        let uri = path_to_uri(path);
        assert_eq!(
            crate::dbus::uri_to_path(&uri),
            Some(std::path::PathBuf::from(path))
        );
    }
}
