//! The window-wide controller, a QML singleton.
//!
//! In the skeleton it carries the appearance to the window, which hands
//! `appearanceReducedMotion` and `appearanceTextScale` to
//! `CelestinaAppearance`, and the smoke switch. The document state (path,
//! pages, zoom, recents) joins it in CAL-1-A. The singleton lives as long as
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
        type CalcitaController = super::CalcitaControllerRust;
    }

    impl cxx_qt::Threading for CalcitaController {}
    impl cxx_qt::Initialize for CalcitaController {}
}

/// `smokeReport` is `CALCITA_SMOKE_REPORT` set: `scripts/smoke.sh` asks the
/// window to print what it shows once it is up.
pub struct CalcitaControllerRust {
    appearance_reduced_motion: bool,
    appearance_text_scale: f64,
    smoke_report: bool,
    /// Held for the singleton's life; dropping it stops the follower.
    follower: Option<Follower>,
}

impl Default for CalcitaControllerRust {
    fn default() -> Self {
        let initial = appearance::initial();
        Self {
            appearance_reduced_motion: initial.reduced_motion,
            appearance_text_scale: initial.text_scale,
            smoke_report: std::env::var_os("CALCITA_SMOKE_REPORT").is_some(),
            follower: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::CalcitaController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let follower = appearance::follow(move |values| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::CalcitaController>| {
                controller.apply_appearance(values);
            });
        });
        self.as_mut().rust_mut().follower = Some(follower);
    }
}

impl qobject::CalcitaController {
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
