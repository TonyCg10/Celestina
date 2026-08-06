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
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use zbus::fdo::RequestNameFlags;
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

        // The transient-parent shim (see cpp/windowparent.cpp): the picker asks
        // it to make itself a child of the window named by `parent_window`.
        include!("siderita/windowparent.h");

        #[rust_name = "register_window_parent"]
        fn register_siderita_window_parent();
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type FileChooserPortal = super::FileChooserPortalRust;

        /// A desktop application is asking for files. `token` identifies this
        /// request when answering; `mode` is `open` | `save` | `saves`. The
        /// caller's own window is deliberately *not* here: see `parent_window`.
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

        /// This process cannot serve the file-chooser backend — almost always
        /// because another one already holds the bus name.
        ///
        /// It matters most when the process was *activated* to be that backend:
        /// it then has no window and nothing to answer, and staying alive means
        /// a few hundred megabytes parked for the rest of the session.
        #[qsignal]
        fn backend_unavailable(self: Pin<&mut FileChooserPortal>, reason: QString);

        #[qinvokable]
        fn start(self: Pin<&mut FileChooserPortal>);

        /// Whether this process was started to serve the portal (`--portal`,
        /// how the D-Bus service file activates it) rather than to browse. In
        /// that case there is no main window — only the pickers it is asked for.
        #[qinvokable]
        fn portal_mode(self: &FileChooserPortal) -> bool;

        /// The window the request came from, as the portal describes it
        /// (`wayland:<xdg-foreign handle>`; empty when the caller sent none or
        /// the request is already over).
        ///
        /// Asked for by token rather than delivered with the request: a signal's
        /// arity is the QML contract, and this is read once, late — when the
        /// picker has a surface to make a child of — by the one window that
        /// cares. Carrying it through the request model would make every other
        /// reader of that model carry it too.
        #[qinvokable]
        fn parent_window(self: &FileChooserPortal, token: &QString) -> QString;

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
type Slot = async_channel::Sender<Vec<PathBuf>>;

#[derive(Default)]
pub struct FileChooserPortalRust {
    started: bool,
    pending: Arc<Mutex<HashMap<String, Slot>>>,
    /// The caller's window per open request, for the picker to ask about.
    parents: Arc<Mutex<HashMap<String, String>>>,
}

impl qobject::FileChooserPortal {
    /// Starts serving the backend, once. Best-effort, exactly like
    /// `FileManager1`: without a session bus, or with the name already taken by
    /// another backend, it logs and gives up rather than failing the app.
    ///
    /// Giving up is reported rather than only logged. A browsing Siderita can
    /// happily carry on without owning the backend, but a process the bus
    /// *activated* to be the backend has no other reason to exist, and one that
    /// lingers is a few hundred megabytes that never come back.
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let notifier = qt.clone();
        let pending = Arc::clone(&self.rust().pending);
        let parents = Arc::clone(&self.rust().parents);
        std::thread::spawn(move || {
            if let Err(error) = serve(qt, pending, parents) {
                eprintln!("Siderita: portal FileChooser no disponible: {error}");
                let reason = error.to_string();
                let _ = notifier.queue(move |portal: Pin<&mut qobject::FileChooserPortal>| {
                    portal.backend_unavailable(QString::from(reason.as_str()));
                });
            }
        });
    }

    /// Whether `--portal` was passed: activated as the desktop's file chooser.
    pub fn portal_mode(&self) -> bool {
        portal_mode()
    }

    /// The window that asked, for a request still open.
    pub fn parent_window(&self, token: &QString) -> QString {
        let token = token.to_string();
        let handle = self
            .rust()
            .parents
            .lock()
            .ok()
            .and_then(|map| map.get(&token).cloned())
            .unwrap_or_default();
        QString::from(handle.as_str())
    }

    /// Hands the picker's result to the waiting D-Bus task. An empty list is
    /// the user cancelling — the task tells the difference, not this side.
    ///
    /// `keys` are the path keys of ADR 0008, decoded here; the URIs the caller
    /// finally receives keep the portal's own spelling, produced downstream by
    /// `crate::dbus::path_to_uri`. A key that will not decode is dropped rather
    /// than handed to another application as a name it cannot resolve.
    pub fn answer(self: Pin<&mut Self>, token: &QString, keys: &QStringList) {
        let token = token.to_string();
        let paths: Vec<PathBuf> = keys
            .iter()
            .filter_map(|key| crate::pathkey::decode(key).ok())
            .collect();
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
    parents: Arc<Mutex<HashMap<String, String>>>,
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
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(
            server,
            handle,
            "open",
            app_id,
            parent_window,
            title,
            options,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_file(
        &self,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(
            server,
            handle,
            "save",
            app_id,
            parent_window,
            title,
            options,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_files(
        &self,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(
            server,
            handle,
            "saves",
            app_id,
            parent_window,
            title,
            options,
        )
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
        parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let token = format!("p{}", self.next_token.fetch_add(1, Ordering::Relaxed));
        let (sender, receiver) = async_channel::bounded::<Vec<PathBuf>>(1);
        if let Ok(mut map) = self.pending.lock() {
            map.insert(token.clone(), sender);
        }
        // Kept for as long as the request is open: the picker reads it once it
        // has a surface, which is well after this point.
        if let Ok(mut map) = self.parents.lock() {
            map.insert(token.clone(), parent_window);
        }

        // The front-end cancels through this object; it lives exactly as long as
        // the request does.
        let request = Request {
            token: token.clone(),
            pending: Arc::clone(&self.pending),
            qt: self.qt.clone(),
        };
        let _ = server.at(&handle, request).await;

        // `SaveFiles` asks for a *folder* and supplies the names itself, so the
        // dialog is a directory chooser however the caller filled the rest in.
        let saving_many = mode == "saves";
        let requested_names = if saving_many {
            file_names(&options)
        } else {
            Vec::new()
        };

        let multiple = bool_option(&options, "multiple") && !saving_many;
        let directory = bool_option(&options, "directory") || saving_many;
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
        if let Ok(mut map) = self.parents.lock() {
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
                // A `SaveFiles` answer is one folder; the files are the caller's
                // own list, composed against it here so no two of them land on
                // the same name.
                let chosen: Vec<PathBuf> = if saving_many {
                    compose_save_files(&paths[0], &requested_names)
                } else {
                    paths
                };
                if chosen.is_empty() {
                    return (RESPONSE_CANCELLED, HashMap::new());
                }
                let uris: Vec<String> = chosen
                    .iter()
                    .map(|path| crate::dbus::path_to_uri(path))
                    .collect();
                let mut results: HashMap<String, OwnedValue> = HashMap::new();
                if let Ok(value) = OwnedValue::try_from(Value::from(uris)) {
                    results.insert("uris".to_owned(), value);
                }
                if let Ok(value) = OwnedValue::try_from(Value::from(writable(mode, &options))) {
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
    parents: Arc<Mutex<HashMap<String, String>>>,
) -> zbus::Result<()> {
    let chooser = FileChooser {
        qt,
        pending,
        parents,
        next_token: AtomicU64::new(1),
    };
    // The blocking builder, like FileManager1: it owns an async connection and
    // its executor internally, so the async interface methods below still get to
    // await without a runtime of our own.
    //
    // The name is requested *after* building rather than through
    // `Builder::name`, so `DoNotQueue` can be set. Its documentation claims the
    // flag is always enabled; the code passes the flags through untouched, and
    // they default to empty. Without it the bus answers `InQueue` instead of
    // `Exists`, zbus does not treat that as an error, and a second backend sits
    // in the name's queue for the rest of the session — a few hundred megabytes
    // that serve nothing and, worse, silently inherit the name the moment the
    // real backend dies, leaving in-flight picker requests unanswered. Serving
    // the object before asking still comes first, so no call arrives before the
    // interface exists.
    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at("/org/freedesktop/portal/desktop", chooser)?
        .build()?;
    connection.request_name_with_flags(
        "org.freedesktop.impl.portal.desktop.celestina",
        RequestNameFlags::DoNotQueue.into(),
    )?;
    let _connection = connection;
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

/// Whether the answer grants write access to what was chosen.
///
/// `writable` is a *result* of `OpenFile`, documented as defaulting to `false`,
/// and the backend interface defines no request option that asks for it: an
/// application that wanted to read a file has not asked to be able to change it,
/// and the front-end uses this flag to decide how the document portal exports
/// the file to a sandbox. Answering `true` unconditionally handed every reader
/// write access to whatever the user pointed at. A save is the opposite case —
/// its whole purpose is to write — and an `OpenFile` that does carry the key is
/// taken at its word.
fn writable(mode: &str, options: &HashMap<String, OwnedValue>) -> bool {
    match mode {
        "save" | "saves" => true,
        _ => bool_option(options, "writable"),
    }
}

/// The `files` option of `SaveFiles`: an array of NUL-terminated byte arrays,
/// the names the caller wants written into the folder the user picks.
///
/// Every name is a bare file name or it is discarded. This list comes from the
/// requesting application, so `../../.bashrc` is a name it may well send, and a
/// path component here would write outside the folder the user agreed to.
fn file_names(options: &HashMap<String, OwnedValue>) -> Vec<OsString> {
    let Some(value) = options.get("files") else {
        return Vec::new();
    };
    let Ok(raw) = value
        .try_clone()
        .ok()
        .ok_or(())
        .and_then(|value| Vec::<Vec<u8>>::try_from(value).map_err(|_| ()))
    else {
        return Vec::new();
    };
    raw.into_iter()
        .filter_map(|bytes| {
            let trimmed: Vec<u8> = bytes.into_iter().take_while(|byte| *byte != 0).collect();
            let name = celestina_core::percent::path_from_bytes(&trimmed);
            let name = name.file_name()?.to_os_string();
            (name != OsStr::new(".") && name != OsStr::new("..")).then_some(name)
        })
        .collect()
}

/// Composes one destination per requested name inside `folder`, giving each a
/// name nothing else holds.
///
/// The spec allows a backend to construct a unique name when the folder already
/// contains one of them, and requires the answer to keep the caller's order.
/// De-duplication also covers the batch against itself: a caller that asks to
/// save two files called `informe.pdf` must get two files.
fn compose_save_files(folder: &Path, names: &[OsString]) -> Vec<PathBuf> {
    let mut taken: Vec<PathBuf> = Vec::with_capacity(names.len());
    for name in names {
        let mut candidate = folder.join(name);
        let mut attempt = 2u32;
        while taken.contains(&candidate) || std::fs::symlink_metadata(&candidate).is_ok() {
            candidate = folder.join(numbered(name, attempt));
            let Some(next) = attempt.checked_add(1) else {
                break;
            };
            attempt = next;
        }
        taken.push(candidate);
    }
    taken
}

/// `informe.pdf` at 2 becomes `informe (2).pdf`; a name without an extension
/// keeps the suffix at the end.
fn numbered(name: &OsStr, attempt: u32) -> OsString {
    let as_path = Path::new(name);
    let stem = as_path.file_stem().unwrap_or(name);
    let mut out = OsString::from(stem);
    out.push(format!(" ({attempt})"));
    if let Some(extension) = as_path.extension() {
        out.push(".");
        out.push(extension);
    }
    out
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
/// The caller's starting folder, as the path key the picker's `start_at` takes.
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
    // The caller sends raw bytes, and the picker is handed a path key
    // (ADR 0008), so a starting folder whose name is not valid UTF-8 opens
    // where it was asked to rather than at a lossy near-miss.
    Some(celestina_core::percent::encode(&trimmed))
}

/// The caller's filters, flattened to `name\tpattern|pattern|…` lines the QML
/// turns into a chooser. The portal's own shape is
/// `a(sa(us))` — a list of (name, list of (kind, pattern)) where kind 0 is a
/// glob and 1 a MIME type.
fn filters(options: &HashMap<String, OwnedValue>) -> Vec<String> {
    let Some(value) = options.get("filters") else {
        return Vec::new();
    };
    let Ok(list) = Vec::<(String, Vec<(u32, String)>)>::try_from(
        value
            .try_clone()
            .unwrap_or_else(|_| OwnedValue::try_from(Value::from(0u32)).expect("scalar value")),
    ) else {
        return Vec::new();
    };
    list.into_iter()
        .map(|(name, patterns)| {
            let joined = patterns
                .into_iter()
                .flat_map(|(kind, pattern)| {
                    if kind == 1 {
                        globs_for_mime(&pattern)
                    } else {
                        vec![pattern]
                    }
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("{name}\t{joined}")
        })
        .collect()
}

/// The name patterns a MIME filter stands for.
///
/// The portal speaks MIME; the listing knows names. Rather than carry a MIME
/// database into the core, the handful of types a file chooser actually asks
/// for are mapped to their extensions here. **An unrecognised type widens to
/// `*`**: a filter that cannot be understood must never hide a file the asking
/// application would have accepted.
fn globs_for_mime(mime: &str) -> Vec<String> {
    let globs: &[&str] = match mime {
        "image/*" => &[
            "*.png", "*.jpg", "*.jpeg", "*.gif", "*.webp", "*.bmp", "*.svg", "*.avif", "*.tif",
            "*.tiff", "*.ico", "*.heic",
        ],
        "video/*" => &[
            "*.mp4", "*.mkv", "*.webm", "*.mov", "*.avi", "*.m4v", "*.mpg", "*.mpeg", "*.wmv",
        ],
        "audio/*" => &[
            "*.mp3", "*.flac", "*.ogg", "*.opus", "*.wav", "*.m4a", "*.aac", "*.wma",
        ],
        "text/*" => &[
            "*.txt", "*.md", "*.csv", "*.log", "*.json", "*.xml", "*.yml", "*.yaml",
        ],
        "application/pdf" => &["*.pdf"],
        "image/png" => &["*.png"],
        "image/jpeg" => &["*.jpg", "*.jpeg"],
        "image/gif" => &["*.gif"],
        "image/webp" => &["*.webp"],
        "image/svg+xml" => &["*.svg"],
        "text/plain" => &["*.txt", "*.md", "*.log"],
        "text/csv" => &["*.csv"],
        "application/json" => &["*.json"],
        "application/zip" => &["*.zip"],
        _ => &["*"],
    };
    globs.iter().map(|glob| (*glob).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mime_filter_becomes_the_extensions_it_stands_for() {
        assert!(globs_for_mime("image/png").contains(&"*.png".to_owned()));
        assert!(globs_for_mime("image/*").contains(&"*.jpeg".to_owned()));
        // The safety property: an unknown type widens rather than hides.
        assert_eq!(
            globs_for_mime("application/x-invented"),
            vec!["*".to_owned()]
        );
    }

    fn options(pairs: &[(&str, Value<'static>)]) -> HashMap<String, OwnedValue> {
        pairs
            .iter()
            .filter_map(|(key, value)| {
                Some((
                    (*key).to_owned(),
                    OwnedValue::try_from(value.try_clone().ok()?).ok()?,
                ))
            })
            .collect()
    }

    #[test]
    fn opening_a_file_grants_write_access_only_when_it_was_asked_for() {
        // The safety property: a request to read is answered as read-only.
        assert!(!writable("open", &options(&[])));
        assert!(!writable(
            "open",
            &options(&[("writable", Value::from(false))])
        ));
        assert!(writable(
            "open",
            &options(&[("writable", Value::from(true))])
        ));
        // Saving is writing, whatever the options say.
        assert!(writable("save", &options(&[])));
        assert!(writable("saves", &options(&[])));
    }

    #[test]
    fn save_files_names_are_read_and_stripped_of_any_path() {
        let requested: Vec<Vec<u8>> = vec![
            b"informe.pdf\0".to_vec(),
            b"../../.bashrc\0".to_vec(),
            b"sub/dir/nota.txt".to_vec(),
            b"..\0".to_vec(),
            b"\0".to_vec(),
        ];
        let names = file_names(&options(&[("files", Value::from(requested))]));

        // A traversal is reduced to its last component, and `..` and the empty
        // name are dropped outright.
        assert_eq!(
            names,
            vec![
                OsString::from("informe.pdf"),
                OsString::from(".bashrc"),
                OsString::from("nota.txt"),
            ]
        );
        assert!(file_names(&options(&[])).is_empty());
    }

    #[test]
    fn composed_save_files_keep_order_and_never_share_a_name() {
        let folder = std::env::temp_dir().join(format!(
            "celestina-portal-saves-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&folder).expect("create fixture folder");
        std::fs::write(folder.join("informe.pdf"), b"existing").expect("write fixture");

        let composed = compose_save_files(
            &folder,
            &[
                OsString::from("informe.pdf"),
                OsString::from("informe.pdf"),
                OsString::from("notas"),
            ],
        );

        assert_eq!(
            composed,
            vec![
                folder.join("informe (2).pdf"),
                folder.join("informe (3).pdf"),
                folder.join("notas"),
            ]
        );
        let _ = std::fs::remove_dir_all(&folder);
    }
}
