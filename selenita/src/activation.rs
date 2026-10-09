//! Selenita's side of the suite's activation interface.
//!
//! `celestina_core::activation` owns the claim, the hand-off to a running
//! instance and the served `org.celestina.Application1` object; this adapter
//! only turns its requests into Qt signals. Selenita opens no files (design
//! §5.2): an `Open` is ignored, whatever paths it carries, and only an
//! `Activate` raises the window.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::OnceLock;

use celestina_core::activation::{self, Activatable, Owner};
use cxx_qt::{CxxQtType, Threading};

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type SelenitaActivation = super::SelenitaActivationRust;

        /// Another launch asked this window to come forward.
        #[qsignal]
        fn raise_requested(self: Pin<&mut SelenitaActivation>);

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
    }
}

/// Where a request ends: the Qt thread in the binary, a tally under test.
trait Target: Send + 'static {
    fn raise(&self);
}

struct QtTarget;

impl Target for QtTarget {
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

/// Claims `org.celestina.Selenita` before any window exists, handing over no
/// paths. When a running Selenita takes this launch the process ends here
/// (status 0); otherwise the owner is kept for `start` to attach.
pub fn claim() {
    if let Some(owner) =
        activation::claim_or_exit(activation::SELENITA, Box::new(Adapter(QtTarget)), &[])
    {
        let _ = OWNER.set(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::{Adapter, Target};
    use celestina_core::activation::Activatable;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct Tally(Arc<AtomicUsize>);

    impl Target for Tally {
        fn raise(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
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
}
