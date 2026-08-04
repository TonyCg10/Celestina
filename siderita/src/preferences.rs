//! Siderita's adapter over [`grafita_core::preferences`].
//!
//! The same stored file Grafita's own window reads, so the text size a reader
//! chose there is the size the embedded editor and the quick look show here.
//! Only the Qt marshalling lives in this file: the bounds, the file format and
//! the write-through are `grafita-core`'s, exactly as the document rules behind
//! [`crate::editor`] are.
//!
//! Two things differ from the standalone application's adapter, and both come
//! from being a guest rather than the owner:
//!
//! - **It re-reads on demand.** Grafita may be running beside Siderita and may
//!   have changed the size since this object was built, and Siderita's own
//!   folder views each hold one. Reloading when a surface opens is what keeps
//!   every one of them showing what is actually stored.
//! - **It offers no wrap control.** Siderita's surfaces are a modal editor and
//!   a peek, and neither claims a key for wrapping; the stored value is
//!   published so a host that wants it can honour it without a second copy of
//!   the preference.

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
        // fontSize — the text size the reader chose, in pixels
        // wrap      — whether long lines fold to the width of the surface
        #[qobject]
        #[qml_element]
        #[qproperty(i32, font_size)]
        #[qproperty(bool, wrap)]
        type GrafitaPreferences = super::GrafitaPreferencesRust;

        /// Re-reads what is stored. Called when a surface opens, so a size
        /// changed in Grafita — or in another of this window's folder views —
        /// is the one that appears.
        #[qinvokable]
        fn reload(self: Pin<&mut GrafitaPreferences>);

        /// Makes the text one step larger, up to what still fits a line.
        #[qinvokable]
        fn enlarge_text(self: Pin<&mut GrafitaPreferences>);

        /// Makes the text one step smaller, down to what is still legible.
        #[qinvokable]
        fn shrink_text(self: Pin<&mut GrafitaPreferences>);
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
        let stored = Preferences::load();
        Self {
            font_size: size_of_stored(stored),
            wrap: stored.wrap(),
            stored,
        }
    }
}

/// The stored size as Qt's pixel type, falling back to the shipped default
/// rather than to nothing: a preference that cannot be represented is still a
/// surface that has to render.
fn size_of_stored(stored: Preferences) -> i32 {
    i32::try_from(stored.font_size()).unwrap_or_else(|_| {
        i32::try_from(grafita_core::preferences::DEFAULT_FONT_SIZE).unwrap_or(11)
    })
}

impl qobject::GrafitaPreferences {
    pub fn reload(mut self: Pin<&mut Self>) {
        let stored = Preferences::load();
        self.as_mut().adopt(stored, false);
    }

    pub fn enlarge_text(self: Pin<&mut Self>) {
        self.nudge(1);
    }

    pub fn shrink_text(self: Pin<&mut Self>) {
        self.nudge(-1);
    }

    /// Moves the size from what is *stored right now*, not from what this
    /// object last published. Another surface may have moved it since, and
    /// nudging a stale value would undo their change.
    fn nudge(mut self: Pin<&mut Self>, steps: i32) {
        let mut stored = Preferences::load();
        stored.nudge_font_size(steps);
        self.as_mut().adopt(stored, true);
    }

    /// Publishes a set, and writes it back only when this surface is the one
    /// that changed it. Reloading must never rewrite the file: it is reading
    /// someone else's decision, not making one.
    fn adopt(mut self: Pin<&mut Self>, stored: Preferences, write_through: bool) {
        if stored == self.rust().stored {
            return;
        }
        let size = size_of_stored(stored);
        let wrap = stored.wrap();
        self.as_mut().rust_mut().get_mut().stored = stored;
        self.as_mut().set_font_size(size);
        self.as_mut().set_wrap(wrap);
        if write_through {
            stored.store();
        }
    }
}
