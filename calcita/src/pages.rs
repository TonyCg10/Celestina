//! The page provider's seam.
//!
//! The provider is hand-written C++ (`cpp/page_provider.cpp`): cxx-qt cannot
//! subclass a `QQuickAsyncImageProvider`, and QML's `PdfPageImage` cannot
//! render a page with its annotations. This module binds what the
//! application calls: registering it before the QML loads, handing it the
//! in-memory copy a window edits over, and draining its render thread on
//! quit.
//!
//! A page's address is `image://calcitapage/<page>/<revision>/<key>`. The
//! revision names a version of what the window shows: a window takes a new
//! one ([`next_revision`]) when it opens a document, when the file changed
//! on disk, after a save, and for the copy it edits over, so a page the
//! image cache still holds from another version is never shown again. The
//! provider renders the copy registered under that key and revision
//! ([`ffi::show_copy`]) when there is one, else the file.

use std::sync::atomic::{AtomicI32, Ordering};

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("calcita/page_provider.h");

        /// Adds the provider under "calcitapage", before the QML that names
        /// `image://calcitapage/…` is loaded.
        #[rust_name = "register_page_provider"]
        fn calcita_register_page_provider(engine: Pin<&mut QQmlApplicationEngine>);

        /// Registers `bytes`, a whole PDF, as what pages of the document
        /// `key` (a path key) at `revision` render from. Any thread; the
        /// bytes are copied.
        #[rust_name = "show_copy"]
        fn calcita_page_provider_show_copy(key: &QString, revision: i32, bytes: &[u8]);

        /// Forgets the copy of `key` at `revision`. Any thread.
        #[rust_name = "drop_copy"]
        fn calcita_page_provider_drop_copy(key: &QString, revision: i32);

        /// Drops the renders not started and waits at most `milliseconds`
        /// for the running one. Once, after the event loop has returned.
        #[rust_name = "page_provider_shutdown"]
        fn calcita_page_provider_shutdown(milliseconds: i32);
    }
}

/// A revision no page address of this process has used yet.
pub fn next_revision() -> i32 {
    static NEXT: AtomicI32 = AtomicI32::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
