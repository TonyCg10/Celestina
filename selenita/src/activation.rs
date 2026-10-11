//! Selenita's side of the suite's activation interface.
//!
//! `celestina_core::activation` owns the claim, the hand-off to a running
//! instance and the served `org.celestina.Application1` object; this adapter
//! only turns its requests into Qt signals. Selenita opens no files (design
//! §5.2): an `Open` is ignored, whatever paths it carries, and only an
//! `Activate` raises the window.
//!
//! Beside the shared interface, the same object path serves Selenita's own
//! [`CAPTURE_INTERFACE`] on the connection that owns the name: `Capture(s)`
//! takes `screen`, `window` or `region` (the niri key bindings' `selenita
//! --screenshot`), `ToggleRecording()` starts a recording or stops the one
//! under way (`selenita --record`), `StopRecording()` only stops
//! (`selenita --stop`) and `Adopt(s key)` hands back a file another
//! application wrote (Fluorita's edited copy, ADR 0012 PRV-1): its
//! `pathkey` key goes on to the window, which lets the capture worker
//! decide; a key that does not decode is ignored without error. A request
//! that arrives before the window started waits and is replayed by `start`.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock, PoisonError};

use celestina_core::activation::{self, Activatable, ActivationError, Owner};
use celestina_core::pathkey;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use selenita_core::TargetKind;

/// Selenita's own interface, served at the activation object path.
pub const CAPTURE_INTERFACE: &str = "org.celestina.Selenita1";

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
        type SelenitaActivation = super::SelenitaActivationRust;

        /// Another launch asked this window to come forward.
        #[qsignal]
        fn raise_requested(self: Pin<&mut SelenitaActivation>);

        /// A key binding asked for a capture of `target` (`screen`,
        /// `window` or `region`).
        #[qsignal]
        fn capture_requested(self: Pin<&mut SelenitaActivation>, target: QString);

        /// A key binding asked about the recording: `toggle` or `stop`.
        #[qsignal]
        fn recording_requested(self: Pin<&mut SelenitaActivation>, action: QString);

        /// Another application handed back the file `key` (`Adopt`).
        #[qsignal]
        fn adopt_requested(self: Pin<&mut SelenitaActivation>, key: QString);

        /// Connects this object to the claim `main` made, once; requests that
        /// waited in the inbox are replayed then.
        #[qinvokable]
        fn start(self: Pin<&mut SelenitaActivation>);
    }

    impl cxx_qt::Threading for SelenitaActivation {}
}

#[derive(Default)]
pub struct SelenitaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::SelenitaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();

impl qobject::SelenitaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let _ = QT.set(self.qt_thread());
        if let Some(owner) = OWNER.get() {
            owner.attach();
        }
        let waiting = std::mem::take(&mut inbox().waiting);
        for request in waiting {
            QtTarget.handle(request);
        }
    }
}

/// What a key binding asks about the recording.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordingAction {
    /// Start, or stop the one under way.
    Toggle,
    /// Stop the one under way, if any.
    Stop,
}

impl RecordingAction {
    /// The word the Qt signal carries.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Stop => "stop",
        }
    }

    /// The method on [`CAPTURE_INTERFACE`].
    fn method(self) -> &'static str {
        match self {
            Self::Toggle => "ToggleRecording",
            Self::Stop => "StopRecording",
        }
    }
}

/// A request from the bus or a key binding, as the window receives it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    Capture(TargetKind),
    Recording(RecordingAction),
    /// A file handed back, by its `pathkey` key.
    Adopt(String),
}

/// The requests that arrived before `start`.
#[derive(Default)]
struct Inbox {
    waiting: Vec<Request>,
}

static INBOX: Mutex<Inbox> = Mutex::new(Inbox {
    waiting: Vec::new(),
});

/// A poisoned lock still holds valid requests.
fn inbox() -> std::sync::MutexGuard<'static, Inbox> {
    INBOX.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether a key binding's request waits for the window: this launch came
/// from `--screenshot` or `--record` and the window has not started yet.
/// Such a launch never shows the main window; the corner preview shows the
/// result.
pub fn launch_waiting() -> bool {
    inbox()
        .waiting
        .iter()
        .any(|request| matches!(request, Request::Capture(_) | Request::Recording(_)))
}

/// How many requests may wait for the window: a key held down must not grow
/// memory without bound.
const WAITING_LIMIT: usize = 4;

/// Hands a request to the window, or keeps it until `start` when the window
/// is not up yet: a fresh instance started by `--screenshot` or `--record`
/// does this before Qt exists.
pub fn request(request: Request) {
    if QT.get().is_some() {
        QtTarget.handle(request);
        return;
    }
    let mut inbox = inbox();
    // Checked again under the lock: `start` may have run meanwhile.
    if QT.get().is_some() {
        drop(inbox);
        QtTarget.handle(request);
    } else if inbox.waiting.len() < WAITING_LIMIT {
        inbox.waiting.push(request);
    }
}

/// Where a request ends: the Qt thread in the binary, a tally under test.
trait Target: Send + 'static {
    fn raise(&self);
    fn handle(&self, request: Request);
}

struct QtTarget;

impl Target for QtTarget {
    fn handle(&self, request: Request) {
        if let Some(qt) = QT.get() {
            let _ = qt.queue(
                move |activation: Pin<&mut qobject::SelenitaActivation>| match request {
                    Request::Capture(kind) => {
                        activation.capture_requested(QString::from(kind.as_str()));
                    }
                    Request::Recording(action) => {
                        activation.recording_requested(QString::from(action.as_str()));
                    }
                    Request::Adopt(key) => activation.adopt_requested(QString::from(&key)),
                },
            );
        }
    }

    fn raise(&self) {
        // The queue fails only while the window is going away; nothing is
        // left to bring forward then.
        if let Some(qt) = QT.get() {
            let _ = qt.queue(|activation: Pin<&mut qobject::SelenitaActivation>| {
                activation.raise_requested();
            });
        }
    }
}

struct Adapter<T>(T);

impl<T: Target> Activatable for Adapter<T> {
    fn activate(&self) {
        self.0.raise();
    }

    fn open(&self, _paths: Vec<PathBuf>) {
        // Ignored by design: a file handed to Selenita never triggers
        // anything.
    }
}

/// The served `org.celestina.Selenita1`: it only hands requests on.
struct Capturer(Box<dyn Target + Sync>);

impl Capturer {
    fn take(&self, word: &str) -> zbus::fdo::Result<()> {
        self.0.handle(Request::Capture(capture_word(word)?));
        Ok(())
    }

    fn recording(&self, action: RecordingAction) {
        self.0.handle(Request::Recording(action));
    }

    /// Hands on a key that names a path; anything else is ignored, as the
    /// contract says, so a stray caller learns nothing and changes nothing.
    fn hand_back(&self, key: &str) {
        if pathkey::decode(key).is_ok() {
            self.0.handle(Request::Adopt(key.to_owned()));
        }
    }
}

/// A `Capture` word that is not a target, as the bus answers it.
fn capture_word(word: &str) -> zbus::fdo::Result<TargetKind> {
    word.parse::<TargetKind>()
        .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))
}

#[zbus::interface(name = "org.celestina.Selenita1")]
impl Capturer {
    fn capture(&self, target: &str) -> zbus::fdo::Result<()> {
        self.take(target)
    }

    fn toggle_recording(&self) {
        self.recording(RecordingAction::Toggle);
    }

    fn stop_recording(&self) {
        self.recording(RecordingAction::Stop);
    }

    fn adopt(&self, key: &str) {
        self.hand_back(key);
    }
}

/// Where the bus's requests go in the binary: [`request`].
struct Pending;

impl Target for Pending {
    fn raise(&self) {}

    fn handle(&self, asked: Request) {
        request(asked);
    }
}

/// Claims `org.celestina.Selenita` before any window exists, handing over no
/// paths. When a running Selenita takes this launch the process ends here
/// (status 0); otherwise the owner is kept for `start` to attach, with
/// [`CAPTURE_INTERFACE`] served beside the shared one.
pub fn claim() {
    if let Some(owner) =
        activation::claim_or_exit(activation::SELENITA, Box::new(Adapter(QtTarget)), &[])
    {
        let path = activation::object_path(&activation::SELENITA);
        match owner
            .connection()
            .object_server()
            .at(path.as_str(), Capturer(Box::new(Pending)))
        {
            Ok(_) => {}
            Err(error) => eprintln!("selenita: key-binding requests are unavailable: {error}"),
        }
        let _ = OWNER.set(owner);
    }
}

/// `selenita --screenshot <target>`'s first step: asks a running Selenita to
/// take the capture. `Ok(true)` when it did; `Ok(false)` when nobody runs
/// (this launch then becomes the instance and takes it itself).
///
/// # Errors
///
/// The bus failed, or the running instance refused.
pub fn send_capture(kind: TargetKind) -> Result<bool, ActivationError> {
    send("Capture", &(kind.as_str(),))
}

/// `selenita --record`'s and `--stop`'s first step: asks a running Selenita.
/// `Ok(true)` when it answered; `Ok(false)` when nobody runs.
///
/// # Errors
///
/// The bus failed, or the running instance refused.
pub fn send_recording(action: RecordingAction) -> Result<bool, ActivationError> {
    send(action.method(), &())
}

fn send<B>(method: &str, body: &B) -> Result<bool, ActivationError>
where
    B: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
{
    let connection = zbus::blocking::connection::Builder::session()
        .map(|builder| builder.method_timeout(activation::HAND_OFF_TIMEOUT))
        .and_then(zbus::blocking::connection::Builder::build)?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&connection)?;
    let name = zbus::names::BusName::try_from(activation::SELENITA.0)
        .map_err(|error| ActivationError::Bus(error.to_string()))?;
    if !bus.name_has_owner(name).map_err(zbus::Error::from)? {
        return Ok(false);
    }
    let path = activation::object_path(&activation::SELENITA);
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        activation::SELENITA.0,
        path.as_str(),
        CAPTURE_INTERFACE,
    )?;
    proxy.call::<_, _, ()>(method, body)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{capture_word, Adapter, Capturer, RecordingAction, Request, Target};
    use celestina_core::activation::Activatable;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Tally(Arc<Mutex<Vec<String>>>);

    impl Tally {
        fn seen(&self) -> Vec<String> {
            self.0.lock().expect("tally").clone()
        }
    }

    impl Target for Tally {
        fn raise(&self) {
            self.0.lock().expect("tally").push("raise".to_owned());
        }

        fn handle(&self, request: Request) {
            let word = match request {
                Request::Capture(kind) => format!("capture:{kind}"),
                Request::Recording(action) => format!("recording:{}", action.as_str()),
                Request::Adopt(key) => format!("adopt:{key}"),
            };
            self.0.lock().expect("tally").push(word);
        }
    }

    #[test]
    fn activate_brings_the_window_forward() {
        let tally = Tally::default();
        Adapter(tally.clone()).activate();
        assert_eq!(tally.seen(), ["raise"]);
    }

    #[test]
    fn open_is_ignored_whatever_it_carries() {
        let tally = Tally::default();
        Adapter(tally.clone()).open(vec![PathBuf::from("/tmp/captura.png")]);
        assert!(tally.seen().is_empty());
    }

    #[test]
    fn capture_takes_the_three_words_only() {
        let tally = Tally::default();
        let capturer = Capturer(Box::new(tally.clone()));
        for word in ["screen", "window", "region"] {
            assert!(capturer.take(word).is_ok(), "{word}");
        }
        assert_eq!(
            tally.seen(),
            ["capture:screen", "capture:window", "capture:region"]
        );
        assert!(capture_word("desktop").is_err());
        assert!(capturer.take("").is_err());
        assert_eq!(tally.seen().len(), 3);
    }

    #[test]
    fn the_recording_methods_hand_their_action_on() {
        let tally = Tally::default();
        let capturer = Capturer(Box::new(tally.clone()));
        capturer.recording(RecordingAction::Toggle);
        capturer.recording(RecordingAction::Stop);
        assert_eq!(tally.seen(), ["recording:toggle", "recording:stop"]);
        assert_eq!(RecordingAction::Toggle.method(), "ToggleRecording");
        assert_eq!(RecordingAction::Stop.method(), "StopRecording");
    }

    /// `Adopt` hands on a key that decodes, byte for byte, and ignores the
    /// rest without an error.
    #[test]
    fn adopt_hands_on_a_path_key_and_ignores_anything_else() {
        let tally = Tally::default();
        let capturer = Capturer(Box::new(tally.clone()));
        let key = celestina_core::pathkey::encode(std::path::Path::new(
            "/pictures/Capturas/Captura 1 (2).png",
        ));
        capturer.hand_back(&key);
        capturer.hand_back("");
        capturer.hand_back("not a key");
        assert_eq!(tally.seen(), [format!("adopt:{key}")]);
    }
}
