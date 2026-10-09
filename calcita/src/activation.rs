//! Calcita's side of the suite's activation interface.
//!
//! `celestina_core::activation` owns the claim, the hand-off to a running
//! instance and the served `org.celestina.Application1` object; this adapter
//! only turns its requests into Qt signals. In the skeleton an `Open` raises
//! the window like an `Activate`: opening the document arrives in CAL-1-A,
//! which gives the paths somewhere to go.

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
        type CalcitaActivation = super::CalcitaActivationRust;

        /// Another launch asked this window to come forward.
        #[qsignal]
        fn raise_requested(self: Pin<&mut CalcitaActivation>);

        /// Connects this object to the claim `main` made, once; requests that
        /// waited in the inbox are replayed then.
        #[qinvokable]
        fn start(self: Pin<&mut CalcitaActivation>);
    }

    impl cxx_qt::Threading for CalcitaActivation {}
}

#[derive(Default)]
pub struct CalcitaActivationRust {
    started: bool,
}

static QT: OnceLock<cxx_qt::CxxQtThread<qobject::CalcitaActivation>> = OnceLock::new();
static OWNER: OnceLock<Owner> = OnceLock::new();

impl qobject::CalcitaActivation {
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
            let _ = qt.queue(|activation: Pin<&mut qobject::CalcitaActivation>| {
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
        // CAL-1-A opens the first path here; until then the window comes
        // forward, which is what the person asked of it as well.
        self.0.raise();
    }
}

/// Claims `org.celestina.Calcita` before any window exists. When a running
/// Calcita takes this launch the process ends here (status 0); otherwise the
/// owner is kept for `start` to attach.
pub fn claim(argv_paths: &[PathBuf]) {
    if let Some(owner) =
        activation::claim_or_exit(activation::CALCITA, Box::new(Adapter(QtTarget)), argv_paths)
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
    fn activate_and_open_both_bring_the_window_forward() {
        let tally = Tally::default();
        let adapter = Adapter(tally.clone());
        adapter.activate();
        adapter.open(vec![PathBuf::from("/tmp/informe.pdf")]);
        assert_eq!(tally.0.load(Ordering::Relaxed), 2);
    }
}
