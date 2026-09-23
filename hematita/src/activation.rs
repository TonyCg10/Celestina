//! One Hematita.
//!
//! The first instance takes a bus name and serves `Activate`; every later
//! launch finds the name owned, asks the running window to raise itself and
//! exits without building one. Failing to reach the bus is never fatal: the
//! launch carries on and opens its own window.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};

const SERVICE: &str = "org.celestina.Hematita";
const OBJECT: &str = "/org/celestina/Hematita";
const INTERFACE: &str = "org.celestina.Hematita";

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type HematitaActivation = super::HematitaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut HematitaActivation>);

        /// Starts serving the activation name, once. Best-effort.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaActivation>);
    }

    impl cxx_qt::Threading for HematitaActivation {}
}

#[derive(Default)]
pub struct HematitaActivationRust {
    started: bool,
}

impl qobject::HematitaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            if let Err(error) = serve(qt) {
                eprintln!("hematita: D-Bus activation unavailable: {error}");
            }
        });
    }
}

struct Activation {
    qt: cxx_qt::CxxQtThread<qobject::HematitaActivation>,
}

#[zbus::interface(name = "org.celestina.Hematita")]
impl Activation {
    fn activate(&self) {
        let _ = self
            .qt
            .queue(|activation: Pin<&mut qobject::HematitaActivation>| {
                activation.raise_requested();
            });
    }
}

fn serve(qt: cxx_qt::CxxQtThread<qobject::HematitaActivation>) -> zbus::Result<()> {
    // `DoNotQueue`: without it a second instance sits in the name's queue and
    // inherits the name the moment the first exits, stranding a process.
    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at(OBJECT, Activation { qt })?
        .build()?;
    connection.request_name_with_flags(SERVICE, zbus::fdo::RequestNameFlags::DoNotQueue.into())?;
    let _connection = connection;
    loop {
        std::thread::park();
    }
}

/// Asks a running Hematita to raise itself. `true` means it did and this
/// launch should exit; any failure answers `false` and the launch opens its
/// own window.
///
/// The hand-off needs the session bus: without one, a second launch cannot
/// find the first and opens a second window. That is never fatal, but it is
/// said once on stderr, so two running Hematitas have a reason on record. No
/// instance owning the name is the ordinary first launch and says nothing.
#[must_use]
pub fn hand_off() -> bool {
    let connection = match zbus::blocking::Connection::session() {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("hematita: no session bus, cannot hand off to a running window: {error}");
            return false;
        }
    };
    let proxy = match zbus::blocking::Proxy::<'_>::new(&connection, SERVICE, OBJECT, INTERFACE) {
        Ok(proxy) => proxy,
        Err(error) => {
            eprintln!("hematita: cannot hand off to a running window: {error}");
            return false;
        }
    };
    match proxy.call::<_, _, ()>("Activate", &()) {
        Ok(()) => true,
        Err(error) => {
            if !nobody_owns_the_name(&error) {
                eprintln!("hematita: cannot hand off to a running window: {error}");
            }
            false
        }
    }
}

/// Whether a failed `Activate` only means no Hematita is running.
fn nobody_owns_the_name(error: &zbus::Error) -> bool {
    match error {
        zbus::Error::MethodError(name, _, _) => {
            name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown"
                || name.as_str() == "org.freedesktop.DBus.Error.NameHasNoOwner"
        }
        zbus::Error::FDO(fdo) => matches!(
            **fdo,
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
        ),
        _ => false,
    }
}
