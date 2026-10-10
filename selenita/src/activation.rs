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
//! --screenshot`), and `ToggleRecording()` is reserved for SEL-1-B and
//! answers an error until then. A capture that arrives before the window
//! started waits and is replayed by `start`.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock, PoisonError};

use celestina_core::activation::{self, Activatable, ActivationError, Owner};
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
        let waiting = std::mem::take(&mut captures().waiting);
        for kind in waiting {
            QtTarget.capture(kind);
        }
    }
}

/// The captures that arrived before `start`.
#[derive(Default)]
struct Captures {
    waiting: Vec<TargetKind>,
}

static CAPTURES: Mutex<Captures> = Mutex::new(Captures {
    waiting: Vec::new(),
});

/// A poisoned lock still holds valid requests.
fn captures() -> std::sync::MutexGuard<'static, Captures> {
    CAPTURES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether a capture waits for the window: this launch came from
/// `--screenshot` and the window has not started yet.
pub fn capture_waiting() -> bool {
    !captures().waiting.is_empty()
}

/// How many captures may wait for the window: a key held down must not grow
/// memory without bound.
const WAITING_LIMIT: usize = 4;

/// Hands a capture to the window, or keeps it until `start` when the window
/// is not up yet: a fresh instance started by `--screenshot` does this
/// before Qt exists.
pub fn request_capture(kind: TargetKind) {
    if QT.get().is_some() {
        QtTarget.capture(kind);
        return;
    }
    let mut captures = captures();
    // Checked again under the lock: `start` may have run meanwhile.
    if QT.get().is_some() {
        drop(captures);
        QtTarget.capture(kind);
    } else if captures.waiting.len() < WAITING_LIMIT {
        captures.waiting.push(kind);
    }
}

/// Where a request ends: the Qt thread in the binary, a tally under test.
trait Target: Send + 'static {
    fn raise(&self);
    fn capture(&self, kind: TargetKind);
}

struct QtTarget;

impl Target for QtTarget {
    fn capture(&self, kind: TargetKind) {
        if let Some(qt) = QT.get() {
            let _ = qt.queue(move |activation: Pin<&mut qobject::SelenitaActivation>| {
                activation.capture_requested(QString::from(kind.as_str()));
            });
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
        self.0.capture(capture_word(word)?);
        Ok(())
    }
}

/// A `Capture` word that is not a target, as the bus answers it.
fn capture_word(word: &str) -> zbus::fdo::Result<TargetKind> {
    word.parse::<TargetKind>()
        .map_err(|error| zbus::fdo::Error::InvalidArgs(error.to_string()))
}

/// `ToggleRecording`'s answer until SEL-1-B.
fn recording_reserved() -> zbus::fdo::Error {
    zbus::fdo::Error::NotSupported("recording arrives with SEL-1-B".to_owned())
}

#[zbus::interface(name = "org.celestina.Selenita1")]
impl Capturer {
    fn capture(&self, target: &str) -> zbus::fdo::Result<()> {
        self.take(target)
    }

    fn toggle_recording(&self) -> zbus::fdo::Result<()> {
        Err(recording_reserved())
    }
}

/// Where the bus's captures go in the binary: [`request_capture`].
struct Pending;

impl Target for Pending {
    fn raise(&self) {}

    fn capture(&self, kind: TargetKind) {
        request_capture(kind);
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
            Err(error) => eprintln!("selenita: key-binding captures are unavailable: {error}"),
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
    proxy.call::<_, _, ()>("Capture", &(kind.as_str(),))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{capture_word, recording_reserved, Adapter, Capturer, Target};
    use celestina_core::activation::Activatable;
    use selenita_core::TargetKind;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct Tally(Arc<AtomicUsize>);

    impl Target for Tally {
        fn raise(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }

        fn capture(&self, _kind: TargetKind) {
            self.0.fetch_add(100, Ordering::Relaxed);
        }
    }

    #[test]
    fn activate_brings_the_window_forward() {
        let tally = Tally::default();
        Adapter(tally.clone()).activate();
        assert_eq!(tally.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn open_is_ignored_whatever_it_carries() {
        let tally = Tally::default();
        Adapter(tally.clone()).open(vec![PathBuf::from("/tmp/captura.png")]);
        assert_eq!(tally.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn capture_takes_the_three_words_only() {
        let tally = Tally::default();
        let capturer = Capturer(Box::new(tally.clone()));
        for word in ["screen", "window", "region"] {
            assert!(capturer.take(word).is_ok(), "{word}");
        }
        assert_eq!(tally.0.load(Ordering::Relaxed), 300);
        assert!(capture_word("desktop").is_err());
        assert!(capturer.take("").is_err());
        assert_eq!(tally.0.load(Ordering::Relaxed), 300);
    }

    #[test]
    fn toggle_recording_is_reserved() {
        assert!(matches!(
            recording_reserved(),
            zbus::fdo::Error::NotSupported(_)
        ));
    }
}
