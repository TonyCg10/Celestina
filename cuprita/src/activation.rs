//! Single instance: whoever owns `org.celestina.Cuprita` on the session bus is
//! the one Cuprita.
//!
//! The claim is the first thing `main` does, before any window exists. A launch
//! that wins keeps the connection for its lifetime and serves one method,
//! `Activate()`. A launch that loses calls `Activate()` on the winner, with a
//! short timeout, and exits. Two launches a few milliseconds apart therefore
//! cannot both build a window: the bus picks exactly one winner.
//!
//! The object is served before the Qt side exists, so the method only sets a
//! flag and calls whatever hook is installed. [`CupritaActivation`] (the QML
//! element) installs the hook that queues a raise onto the Qt thread, and
//! replays a request that arrived earlier.

use std::pin::Pin;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};

const SERVICE: &str = "org.celestina.Cuprita";
const OBJECT: &str = "/org/celestina/Cuprita";
const INTERFACE: &str = "org.celestina.Cuprita";

/// How long a losing launch waits for the owner to answer `Activate()`.
const HAND_OFF_TIMEOUT: Duration = Duration::from_secs(3);

/// What a launch should do after trying to claim the name.
pub enum Claim {
    /// This launch owns the name and builds the window.
    Owner,
    /// Another Cuprita owns it and has been asked to raise itself.
    HandedOff,
    /// The bus could not settle it (no bus, owner unresponsive): carry on and
    /// open a window, the failure having been logged.
    Unsettled,
}

/// What the served `Activate()` does: a raise request that may precede the
/// hook that can act on it.
#[derive(Default)]
struct Wakeup {
    pending: bool,
    hook: Option<Box<dyn Fn() + Send>>,
}

static WAKEUP: Mutex<Wakeup> = Mutex::new(Wakeup {
    pending: false,
    hook: None,
});
/// Keeps the bus connection, and with it the name, alive until exit.
static CONNECTION: OnceLock<zbus::blocking::Connection> = OnceLock::new();

struct Served;

#[zbus::interface(name = "org.celestina.Cuprita")]
impl Served {
    fn activate(&self) {
        if let Ok(mut wakeup) = WAKEUP.lock() {
            match &wakeup.hook {
                Some(hook) => hook(),
                None => wakeup.pending = true,
            }
        }
    }
}

/// Claims the name, or asks its owner to raise itself.
#[must_use]
pub fn claim() -> Claim {
    match try_claim() {
        Ok(claim) => claim,
        Err(error) => {
            eprintln!("cuprita: single-instance check unavailable: {error}");
            Claim::Unsettled
        }
    }
}

fn try_claim() -> zbus::Result<Claim> {
    let connection = zbus::blocking::Connection::session()?;
    connection.object_server().at(OBJECT, Served)?;
    // `DoNotQueue`: a queued loser would inherit the name when the owner
    // exits and sit on it without a window.
    let flags = zbus::fdo::RequestNameFlags::DoNotQueue;
    match connection.request_name_with_flags(SERVICE, flags.into()) {
        Ok(
            zbus::fdo::RequestNameReply::PrimaryOwner | zbus::fdo::RequestNameReply::AlreadyOwner,
        ) => {
            let _ = CONNECTION.set(connection);
            Ok(Claim::Owner)
        }
        Ok(_) | Err(zbus::Error::NameTaken) => Ok(ask_owner_to_raise(&connection)),
        Err(error) => Err(error),
    }
}

/// Calls `Activate()` on the owner, giving up after [`HAND_OFF_TIMEOUT`].
fn ask_owner_to_raise(connection: &zbus::blocking::Connection) -> Claim {
    let connection = connection.clone();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let call = zbus::blocking::Proxy::new(&connection, SERVICE, OBJECT, INTERFACE)
            .and_then(|proxy| proxy.call::<_, _, ()>("Activate", &()));
        let _ = sender.send(call);
    });
    match receiver.recv_timeout(HAND_OFF_TIMEOUT) {
        Ok(Ok(())) => Claim::HandedOff,
        Ok(Err(error)) => {
            eprintln!("cuprita: the running window did not answer Activate: {error}");
            Claim::Unsettled
        }
        Err(_) => {
            eprintln!("cuprita: the running window did not answer Activate within 3 s");
            Claim::Unsettled
        }
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type CupritaActivation = super::CupritaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut CupritaActivation>);

        /// Connects the served `Activate()` to this object, once.
        #[qinvokable]
        fn start(self: Pin<&mut CupritaActivation>);
    }

    impl cxx_qt::Threading for CupritaActivation {}
}

#[derive(Default)]
pub struct CupritaActivationRust {
    started: bool,
}

impl qobject::CupritaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let raise = move || {
            let _ = qt.queue(|activation: Pin<&mut qobject::CupritaActivation>| {
                activation.raise_requested();
            });
        };
        if let Ok(mut wakeup) = WAKEUP.lock() {
            if std::mem::take(&mut wakeup.pending) {
                raise();
            }
            wakeup.hook = Some(Box::new(raise));
        }
    }
}
