//! The window-wide controller, a QML singleton.
//!
//! In the skeleton it carries the appearance to the window, which hands
//! `appearanceReducedMotion` and `appearanceTextScale` to
//! `CelestinaAppearance`, the smoke switch and the fake switch. The capture
//! settings, the recording state and the history join it in SEL-1-A/B. The singleton lives as long as
//! the engine, so its follower lives until the process exits.

use std::pin::Pin;

use celestina_settings::Follower;
use cxx_qt::{CxxQtType, Threading};

use crate::appearance::{self, Values};

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(
            bool,
            appearance_reduced_motion,
            cxx_name = "appearanceReducedMotion",
            READ,
            NOTIFY
        )]
        #[qproperty(
            f64,
            appearance_text_scale,
            cxx_name = "appearanceTextScale",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, smoke_report, cxx_name = "smokeReport", READ, CONSTANT)]
        #[qproperty(bool, fake, READ, CONSTANT)]
        type SelenitaController = super::SelenitaControllerRust;
    }

    impl cxx_qt::Threading for SelenitaController {}
    impl cxx_qt::Initialize for SelenitaController {}
}

/// `smokeReport` is `SELENITA_SMOKE_REPORT` set: `scripts/smoke.sh` asks the
/// window to print what it shows once it is up. `fake` is `SELENITA_FAKE=1`:
/// the tests and the smoke run without `grim`, `slurp`, niri or the portal;
/// SEL-1-A/B route their workers to fakes through it. Nothing is faked yet.
pub struct SelenitaControllerRust {
    appearance_reduced_motion: bool,
    appearance_text_scale: f64,
    smoke_report: bool,
    fake: bool,
    /// Held for the singleton's life; dropping it stops the follower.
    follower: Option<Follower>,
}

impl Default for SelenitaControllerRust {
    fn default() -> Self {
        let initial = appearance::initial();
        Self {
            appearance_reduced_motion: initial.reduced_motion,
            appearance_text_scale: initial.text_scale,
            smoke_report: std::env::var_os("SELENITA_SMOKE_REPORT").is_some(),
            fake: fake_requested(std::env::var_os("SELENITA_FAKE").as_deref()),
            follower: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::SelenitaController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let follower = appearance::follow(move |values| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::SelenitaController>| {
                controller.apply_appearance(values);
            });
        });
        self.as_mut().rust_mut().follower = Some(follower);
    }
}

impl qobject::SelenitaController {
    fn apply_appearance(mut self: Pin<&mut Self>, values: Values) {
        if self.rust().appearance_reduced_motion != values.reduced_motion {
            self.as_mut().rust_mut().appearance_reduced_motion = values.reduced_motion;
            self.as_mut().appearance_reduced_motion_changed();
        }
        if self.rust().appearance_text_scale != values.text_scale {
            self.as_mut().rust_mut().appearance_text_scale = values.text_scale;
            self.as_mut().appearance_text_scale_changed();
        }
    }
}

/// Only the exact value `1` asks for the fakes, so a stray empty or `0` never
/// does.
fn fake_requested(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use super::fake_requested;
    use std::ffi::OsStr;

    #[test]
    fn only_one_asks_for_the_fakes() {
        assert!(fake_requested(Some(OsStr::new("1"))));
        assert!(!fake_requested(Some(OsStr::new("0"))));
        assert!(!fake_requested(Some(OsStr::new(""))));
        assert!(!fake_requested(None));
    }
}
