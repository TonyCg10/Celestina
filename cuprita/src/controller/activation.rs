//! Cuprita's side of the suite's activation interface.
//!
//! The claim, the hand-off and the served `org.celestina.Application1` object
//! belong to `celestina_core::activation`; this adapter only turns its
//! requests into Qt work. Cuprita opens no files, so `Open` raises the window
//! exactly as `Activate` does.

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
        type CupritaActivation = super::CupritaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut CupritaActivation>);

        /// Attaches this object to the claim `main` made, once.
        #[qinvokable]
        fn start(self: Pin<&mut CupritaActivation>);
    }

    impl cxx_qt::Threading for CupritaActivation {}
}

#[derive(Default)]
pub struct CupritaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::CupritaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();

impl qobject::CupritaActivation {
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

/// Where a raise goes: the Qt thread in the application, a counter in tests.
trait Sink: Send + 'static {
    fn raise(&self);
}

struct QtSink;

impl Sink for QtSink {
    fn raise(&self) {
        // A failed queue means the window is closing: nobody is left to raise.
        if let Some(qt) = QT.get() {
            let _ = qt.queue(|activation: Pin<&mut qobject::CupritaActivation>| {
                activation.raise_requested();
            });
        }
    }
}

struct Forward<S>(S);

impl<S: Sink> Activatable for Forward<S> {
    fn activate(&self) {
        self.0.raise();
    }

    fn open(&self, _paths: Vec<PathBuf>) {
        self.0.raise();
    }
}

/// Claims Cuprita's name before any window exists. A running window that
/// takes this launch ends the process here (status 0); otherwise the owner is
/// kept for `start` to attach.
pub fn claim() {
    if let Some(owner) =
        activation::claim_or_exit(activation::CUPRITA, Box::new(Forward(QtSink)), &[])
    {
        let _ = OWNER.set(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::{Forward, Sink};
    use celestina_core::activation::Activatable;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct Counter(Arc<AtomicUsize>);

    impl Sink for Counter {
        fn raise(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn an_open_raises_the_window_like_an_activate() {
        let counter = Counter::default();
        let forward = Forward(counter.clone());
        forward.open(vec![PathBuf::from("/tmp/x")]);
        forward.activate();
        assert_eq!(counter.0.load(Ordering::Relaxed), 2);
    }
}
