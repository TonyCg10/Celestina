//! The window-wide state: the appearance and the smoke switch. Each section
//! has its own controller beside this one.
//!
//! `celestina_settings` owns the suite's appearance file
//! (`~/.config/celestina/appearance.toml`), its watcher, its follower thread
//! and the `CELESTINA_REDUCED_MOTION` override; this controller only carries
//! the two values to the window, which hands `appearanceReducedMotion` and
//! `appearanceTextScale` to `CelestinaAppearance`. The values are read once
//! when the singleton is made, before the window's first frame; the follower
//! then queues each change to Qt. The singleton lives as long as the engine,
//! so its follower thread lives until the process exits.

use std::pin::Pin;

use celestina_settings::{Appearance, Follower};
use cxx_qt::{CxxQtType, Threading};

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
        #[qproperty(bool, smoke_report, cxx_name = "smokeReport")]
        type CupritaController = super::CupritaControllerRust;
    }

    impl cxx_qt::Threading for CupritaController {}
    impl cxx_qt::Initialize for CupritaController {}
}

/// `smokeReport` is `CUPRITA_SMOKE_REPORT` set: `scripts/smoke.sh` asks the
/// window to print its list models' row counts once the pages are up, so the
/// smoke checks the controller-to-model wiring over the fakes; the same switch
/// prints the theme's text scale and `fontBody` whenever they change.
pub struct CupritaControllerRust {
    appearance_reduced_motion: bool,
    appearance_text_scale: f64,
    smoke_report: bool,
    /// Held for the singleton's life; dropping it stops the follower.
    follower: Option<Follower>,
}

impl Default for CupritaControllerRust {
    fn default() -> Self {
        // A bounded read of a two-line file, so the theme is right before the
        // QML that reads it loads.
        let initial = celestina_settings::load();
        Self {
            appearance_reduced_motion: initial.reduced_motion,
            appearance_text_scale: initial.text_scale.factor(),
            smoke_report: std::env::var_os("CUPRITA_SMOKE_REPORT").is_some(),
            follower: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::CupritaController {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let follower = celestina_settings::follow(move |value: Appearance| {
            let _ = qt.queue(move |controller: Pin<&mut qobject::CupritaController>| {
                controller.apply_appearance(value);
            });
        });
        self.as_mut().rust_mut().follower = Some(follower);
    }
}

impl qobject::CupritaController {
    fn apply_appearance(mut self: Pin<&mut Self>, value: Appearance) {
        if self.rust().appearance_reduced_motion != value.reduced_motion {
            self.as_mut().rust_mut().appearance_reduced_motion = value.reduced_motion;
            self.as_mut().appearance_reduced_motion_changed();
        }
        let scale = value.text_scale.factor();
        if self.rust().appearance_text_scale != scale {
            self.as_mut().rust_mut().appearance_text_scale = scale;
            self.as_mut().appearance_text_scale_changed();
        }
    }
}
