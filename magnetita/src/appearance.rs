//! Magnetita's side of the suite's appearance file.
//!
//! `celestina_settings` owns the file (`~/.config/celestina/appearance.toml`),
//! its watcher, its follower thread and the `CELESTINA_REDUCED_MOTION`
//! override; this adapter only carries the two values to the window, which
//! hands `appearanceReducedMotion` and `appearanceTextScale` to
//! `CelestinaAppearance`. The values are read once when the object is made,
//! before the window's first frame, so a larger text or reduced motion is
//! there from the start; the follower then queues each change to Qt.

use std::pin::Pin;

use celestina_settings::{Appearance, Follower};
use cxx_qt::{CxxQtType, Threading};

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
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
        type MagnetitaAppearance = super::MagnetitaAppearanceRust;
    }

    impl cxx_qt::Threading for MagnetitaAppearance {}
    impl cxx_qt::Initialize for MagnetitaAppearance {}
}

pub struct MagnetitaAppearanceRust {
    appearance_reduced_motion: bool,
    appearance_text_scale: f64,
    /// Held until the object goes; dropping it stops the follower.
    follower: Option<Follower>,
}

impl Default for MagnetitaAppearanceRust {
    fn default() -> Self {
        // A bounded read of a two-line file, so the theme is right before the
        // QML that reads it loads.
        let initial = celestina_settings::load();
        Self {
            appearance_reduced_motion: initial.reduced_motion,
            appearance_text_scale: initial.text_scale.factor(),
            follower: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::MagnetitaAppearance {
    fn initialize(mut self: Pin<&mut Self>) {
        let qt = self.qt_thread();
        let follower = celestina_settings::follow(move |value: Appearance| {
            let _ =
                qt.queue(move |object: Pin<&mut qobject::MagnetitaAppearance>| object.apply(value));
        });
        self.as_mut().rust_mut().follower = Some(follower);
    }
}

impl qobject::MagnetitaAppearance {
    fn apply(mut self: Pin<&mut Self>, value: Appearance) {
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
