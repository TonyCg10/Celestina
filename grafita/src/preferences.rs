//! The window's adapter over [`grafita_core::preferences`].
//!
//! One object per window, not one per tab: the text size is a property of how
//! the user reads, not of which document is in front. `Main.qml` owns it and
//! hands it down to every tab's surface.
//!
//! Every change is written through immediately rather than on exit — Grafita
//! can be closed by the compositor, and a preference only saved at quit is a
//! preference that does not survive the way people actually stop programs.

use std::pin::Pin;

use cxx_qt::CxxQtType;
use grafita_core::preferences::Preferences;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // fontSize — the editing surface's text size, in pixels
        // wrap      — whether long lines fold to the width of the surface
        //
        // Moved with the invokables below rather than assigned: they are what
        // bounds these values and what writes them back.
        #[qobject]
        #[qml_element]
        #[qproperty(i32, font_size)]
        #[qproperty(bool, wrap)]
        type GrafitaPreferences = super::GrafitaPreferencesRust;

        /// Makes the text one step larger, up to what still fits a line.
        #[qinvokable]
        fn enlarge_text(self: Pin<&mut GrafitaPreferences>);

        /// Makes the text one step smaller, down to what is still legible.
        #[qinvokable]
        fn shrink_text(self: Pin<&mut GrafitaPreferences>);

        /// Turns line wrapping off and on.
        #[qinvokable]
        fn toggle_wrap(self: Pin<&mut GrafitaPreferences>);
    }
}

/// Qt-side mirror of the stored preferences.
pub struct GrafitaPreferencesRust {
    font_size: i32,
    wrap: bool,
    stored: Preferences,
}

impl Default for GrafitaPreferencesRust {
    fn default() -> Self {
        // Read at construction: the first window paints the way the user left
        // it, with no flash of the shipped defaults.
        let stored = Preferences::load();
        Self {
            font_size: i32::try_from(stored.font_size())
                .unwrap_or(grafita_core::preferences::DEFAULT_FONT_SIZE as i32),
            wrap: stored.wrap(),
            stored,
        }
    }
}

impl qobject::GrafitaPreferences {
    pub fn enlarge_text(self: Pin<&mut Self>) {
        self.nudge(1);
    }

    pub fn shrink_text(self: Pin<&mut Self>) {
        self.nudge(-1);
    }

    pub fn toggle_wrap(mut self: Pin<&mut Self>) {
        let mut stored = self.rust().stored;
        stored.toggle_wrap();
        self.as_mut().adopt(stored);
    }

    /// Moves the size and, if it actually moved, publishes and stores it.
    fn nudge(mut self: Pin<&mut Self>, steps: i32) {
        let mut stored = self.rust().stored;
        stored.nudge_font_size(steps);
        self.as_mut().adopt(stored);
    }

    /// Publishes and writes through a changed set.
    ///
    /// An action that changes nothing — a size keypress at a limit — neither
    /// notifies QML nor rewrites the file: holding Ctrl − at the smallest size
    /// should not keep touching the disk.
    fn adopt(mut self: Pin<&mut Self>, stored: Preferences) {
        if stored == self.rust().stored {
            return;
        }
        let size = i32::try_from(stored.font_size()).unwrap_or(self.rust().font_size);
        let wrap = stored.wrap();
        self.as_mut().rust_mut().get_mut().stored = stored;
        self.as_mut().set_font_size(size);
        self.as_mut().set_wrap(wrap);
        stored.store();
    }
}
