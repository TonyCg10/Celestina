//! One Hematita.
//!
//! A launch claims the bus name before it builds anything. The bus serialises
//! the claims, so of two launches in the same instant exactly one is answered
//! primary owner: it serves `Activate` and `Open` and goes on to open the
//! window. The other finds the name taken, asks the owner to raise itself
//! (and to browse the folder it was handed, if any) and exits without a
//! window. Claiming after the window, as before, left a gap the width of the
//! QML load in which both launches found nobody and both opened.
//!
//! The served object exists before the Qt side does: a request that arrives
//! before the QML attaches waits in a bounded inbox and is replayed onto the
//! Qt thread when it does. Failing to reach the bus is never fatal: the launch
//! says so once and opens its own window.

use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

const SERVICE: &str = "org.celestina.Hematita";
const OBJECT: &str = "/org/celestina/Hematita";
const INTERFACE: &str = "org.celestina.Hematita";

/// How long a launch waits for the running window to answer `Activate` or
/// `Open` before it gives up and opens its own window: a hung owner must not
/// hang the launch.
const HAND_OFF_TIMEOUT: Duration = Duration::from_secs(3);

/// How many requests the inbox keeps for the Qt side before it attaches; the
/// window is seconds away, and a peer flooding the name gets its oldest
/// requests dropped rather than unbounded memory.
const INBOX_LIMIT: usize = 8;

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
        type HematitaActivation = super::HematitaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut HematitaActivation>);

        /// Another launch handed this window a folder to browse.
        #[qsignal]
        fn open_requested(self: Pin<&mut HematitaActivation>, path: QString);

        /// Attaches this object to the claim `main` made, once: every request
        /// that arrived since, and every later one, reaches the Qt thread as
        /// a signal. Without a claim there is nothing to attach to.
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
        if let Some(claim) = CLAIM.get() {
            lock(&claim.inbox).attach(self.qt_thread());
        }
    }
}

/// How a launch goes on after the claim.
#[derive(Debug, PartialEq, Eq)]
pub enum Launch {
    /// This process owns the name, or could not reach the bus: open a window.
    Window,
    /// A running window took this launch: exit without one.
    HandedOff,
}

/// What a peer asked of the window.
#[derive(Debug, PartialEq, Eq)]
enum Request {
    Raise,
    Open(String),
}

/// Where a request ends up once the Qt side is attached.
trait Sink {
    fn deliver(&self, request: Request);
}

impl Sink for cxx_qt::CxxQtThread<qobject::HematitaActivation> {
    fn deliver(&self, request: Request) {
        // A queue that fails means the Qt object is gone: the window is
        // closing and nobody is left to raise.
        let _ = self.queue(
            move |activation: Pin<&mut qobject::HematitaActivation>| match request {
                Request::Raise => activation.raise_requested(),
                Request::Open(path) => activation.open_requested(QString::from(path.as_str())),
            },
        );
    }
}

/// The requests between the bus and the Qt thread: delivered straight through
/// once a sink is attached, kept (bounded, oldest dropped) until then.
struct Inbox<S> {
    sink: Option<S>,
    pending: Vec<Request>,
}

impl<S: Sink> Inbox<S> {
    const fn new() -> Self {
        Self {
            sink: None,
            pending: Vec::new(),
        }
    }

    fn deliver(&mut self, request: Request) {
        match &self.sink {
            Some(sink) => sink.deliver(request),
            None => {
                if self.pending.len() >= INBOX_LIMIT {
                    self.pending.remove(0);
                }
                self.pending.push(request);
            }
        }
    }

    fn attach(&mut self, sink: S) {
        for request in self.pending.drain(..) {
            sink.deliver(request);
        }
        self.sink = Some(sink);
    }
}

type SharedInbox = Arc<Mutex<Inbox<cxx_qt::CxxQtThread<qobject::HematitaActivation>>>>;

/// A poisoned inbox still holds valid requests: a panic elsewhere must not
/// strand the hand-off.
fn lock<S>(inbox: &Mutex<Inbox<S>>) -> std::sync::MutexGuard<'_, Inbox<S>> {
    inbox
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The name this process owns, kept for its lifetime: dropping the connection
/// would release the name and let the next launch open a second window.
struct Claim {
    _connection: zbus::blocking::Connection,
    inbox: SharedInbox,
}

static CLAIM: OnceLock<Claim> = OnceLock::new();

/// The served object. It owns no Qt state: it hands every request to the
/// inbox, which reaches the Qt thread once the window has attached.
struct Activation {
    inbox: SharedInbox,
}

#[zbus::interface(name = "org.celestina.Hematita")]
impl Activation {
    fn activate(&self) {
        lock(&self.inbox).deliver(Request::Raise);
    }

    fn open(&self, path: String) {
        lock(&self.inbox).deliver(Request::Open(path));
    }
}

/// Claims the activation name for this launch, before any window exists.
///
/// Owning it answers `Window`. Finding it owned asks that owner to raise
/// itself, and with `path` to browse that folder (`Open`) instead of only
/// raising (`Activate`), and answers `HandedOff` when it did: this launch
/// should exit. Any failure to reach the bus, to claim, or to be answered
/// within [`HAND_OFF_TIMEOUT`] is said once on stderr and answers `Window`,
/// so two running Hematitas always have a reason on record.
#[must_use]
pub fn claim(path: Option<&str>) -> Launch {
    let inbox: SharedInbox = Arc::new(Mutex::new(Inbox::new()));
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|builder| {
            builder.serve_at(
                OBJECT,
                Activation {
                    inbox: Arc::clone(&inbox),
                },
            )
        })
        .map(|builder| builder.method_timeout(HAND_OFF_TIMEOUT))
        .and_then(zbus::blocking::connection::Builder::build);
    let connection = match connection {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("hematita: no session bus, cannot keep to one window: {error}");
            return Launch::Window;
        }
    };
    // `DoNotQueue`: without it a second instance sits in the name's queue and
    // inherits the name the moment the first exits, stranding a process.
    match connection
        .request_name_with_flags(SERVICE, zbus::fdo::RequestNameFlags::DoNotQueue.into())
    {
        Ok(_) => {
            // `claim` runs once, from `main`; a second call would be a bug,
            // and the connection it built is dropped with its name.
            let _ = CLAIM.set(Claim {
                _connection: connection,
                inbox,
            });
            Launch::Window
        }
        Err(zbus::Error::NameTaken) => hand_off(&connection, path),
        Err(error) => {
            eprintln!("hematita: cannot claim {SERVICE}, opening a window anyway: {error}");
            Launch::Window
        }
    }
}

/// Asks the window that owns the name to raise itself, or to browse `path`.
fn hand_off(connection: &zbus::blocking::Connection, path: Option<&str>) -> Launch {
    let proxy = match zbus::blocking::Proxy::<'_>::new(connection, SERVICE, OBJECT, INTERFACE) {
        Ok(proxy) => proxy,
        Err(error) => {
            eprintln!("hematita: cannot hand off to the running window: {error}");
            return Launch::Window;
        }
    };
    let answer = match path {
        // An older running instance has no `Open`: raising it still beats
        // opening a second window.
        Some(path) => match proxy.call::<_, _, ()>("Open", &(path,)) {
            Err(error) if method_unknown(&error) => proxy.call::<_, _, ()>("Activate", &()),
            other => other,
        },
        None => proxy.call::<_, _, ()>("Activate", &()),
    };
    match answer {
        Ok(()) => Launch::HandedOff,
        Err(error) => {
            eprintln!("hematita: cannot hand off to the running window: {error}");
            Launch::Window
        }
    }
}

/// Whether a failed `Open` means the running instance predates it.
fn method_unknown(error: &zbus::Error) -> bool {
    match error {
        zbus::Error::MethodError(name, _, _) => {
            name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod"
        }
        zbus::Error::FDO(fdo) => matches!(**fdo, zbus::fdo::Error::UnknownMethod(_)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Inbox, Request, Sink, INBOX_LIMIT};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct Recorder(Rc<RefCell<Vec<Request>>>);

    impl Sink for Recorder {
        fn deliver(&self, request: Request) {
            self.0.borrow_mut().push(request);
        }
    }

    #[test]
    fn requests_before_the_window_attaches_are_replayed_in_order() {
        let mut inbox = Inbox::new();
        inbox.deliver(Request::Raise);
        inbox.deliver(Request::Open("/tmp".to_owned()));
        let sink = Recorder::default();
        inbox.attach(sink.clone());
        assert_eq!(
            *sink.0.borrow(),
            vec![Request::Raise, Request::Open("/tmp".to_owned())]
        );
        inbox.deliver(Request::Raise);
        assert_eq!(sink.0.borrow().len(), 3);
    }

    #[test]
    fn the_inbox_keeps_only_the_newest_requests_until_then() {
        let mut inbox = Inbox::new();
        for n in 0..(INBOX_LIMIT + 3) {
            inbox.deliver(Request::Open(n.to_string()));
        }
        let sink = Recorder::default();
        inbox.attach(sink.clone());
        let kept = sink.0.borrow();
        assert_eq!(kept.len(), INBOX_LIMIT);
        assert_eq!(kept[0], Request::Open("3".to_owned()));
        assert_eq!(
            kept[INBOX_LIMIT - 1],
            Request::Open((INBOX_LIMIT + 2).to_string())
        );
    }
}
