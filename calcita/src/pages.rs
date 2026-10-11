//! The page provider's seam.
//!
//! The provider is hand-written C++ (`cpp/page_provider.cpp`): cxx-qt cannot
//! subclass a `QQuickAsyncImageProvider`, and QML's `PdfPageImage` cannot
//! render a page with its annotations. This module binds what the
//! application calls: registering it before the QML loads and draining its
//! render thread on quit. The page view names `image://calcitapage/…`.

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("calcita/page_provider.h");

        /// Adds the provider under "calcitapage", before the QML that names
        /// `image://calcitapage/…` is loaded.
        #[rust_name = "register_page_provider"]
        fn calcita_register_page_provider(engine: Pin<&mut QQmlApplicationEngine>);

        /// Drops the renders not started and waits at most `milliseconds`
        /// for the running one. Once, after the event loop has returned.
        #[rust_name = "page_provider_shutdown"]
        fn calcita_page_provider_shutdown(milliseconds: i32);
    }
}
