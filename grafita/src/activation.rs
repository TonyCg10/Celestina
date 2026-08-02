//! One Grafita, many documents.
//!
//! Opening a second file used to map a second *window*, which is not what a
//! text editor should do to a desktop. So the first Grafita takes a bus name and
//! serves `OpenDocument`; every later launch finds that name already owned,
//! hands its path over and exits without ever building a window.
//!
//! Failing to reach the bus is never fatal: without a session bus, or with the
//! call refused, the launch simply carries on and opens its own window. A
//! missing nicety must not stop the editor from editing.

use std::path::Path;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

const SERVICE: &str = "org.celestina.Grafita";
const OBJECT: &str = "/org/celestina/Grafita";
const INTERFACE: &str = "org.celestina.Grafita";

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type GrafitaActivation = super::GrafitaActivationRust;

        /// Another launch handed this window a document to open in a tab.
        #[qsignal]
        fn open_requested(self: Pin<&mut GrafitaActivation>, path: QString);

        /// Starts serving the activation name, once. Best-effort: a window that
        /// cannot own the name still edits perfectly well, it just does not
        /// collect other launches' documents.
        #[qinvokable]
        fn start(self: Pin<&mut GrafitaActivation>);
    }

    impl cxx_qt::Threading for GrafitaActivation {}
}

#[derive(Default)]
pub struct GrafitaActivationRust {
    started: bool,
}

impl qobject::GrafitaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            if let Err(error) = serve(qt) {
                eprintln!("Grafita: activación por D-Bus no disponible: {error}");
            }
        });
    }
}

/// The served object. It owns no Qt state: it marshals onto the Qt thread.
struct Activation {
    qt: cxx_qt::CxxQtThread<qobject::GrafitaActivation>,
}

#[zbus::interface(name = "org.celestina.Grafita")]
impl Activation {
    /// Opens `path` in a tab of the running window.
    ///
    /// Answers immediately: the caller is a launcher waiting to exit, not
    /// something that needs to know how the open went. Whether the file is
    /// editable is still decided by its bytes, on the window's own worker.
    fn open_document(&self, path: String) {
        let _ = self
            .qt
            .queue(move |activation: Pin<&mut qobject::GrafitaActivation>| {
                activation.open_requested(QString::from(path.as_str()));
            });
    }
}

fn serve(qt: cxx_qt::CxxQtThread<qobject::GrafitaActivation>) -> zbus::Result<()> {
    // `DoNotQueue` explicitly: without it the bus answers `InQueue` rather than
    // `Exists`, zbus does not treat that as an error, and a second Grafita would
    // sit in the name's queue for the rest of the session — inheriting the name
    // the moment the real one exits. Siderita's portal backend was doing exactly
    // that, and it cost 3.5 GiB of stranded processes.
    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at(OBJECT, Activation { qt })?
        .build()?;
    connection.request_name_with_flags(SERVICE, zbus::fdo::RequestNameFlags::DoNotQueue.into())?;
    let _connection = connection;
    loop {
        std::thread::park();
    }
}

/// Hands `path` to a Grafita that is already running.
///
/// Returns whether it was accepted, which is the caller's cue to exit without
/// building a window. Any failure — no bus, no running instance, a refused call
/// — answers `false`, so the launch falls back to opening its own window rather
/// than failing.
#[must_use]
pub fn hand_off(path: &Path) -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::Proxy::<'_>::new(&connection, SERVICE, OBJECT, INTERFACE)
    else {
        return false;
    };
    // An absolute path: the running instance has its own working directory, and
    // a relative one would resolve against the wrong place.
    let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    proxy
        .call::<_, _, ()>("OpenDocument", &(absolute.to_string_lossy().as_ref(),))
        .is_ok()
}
