//! The render seam, owned in one place.
//!
//! Two applications now put media on screen — Fluorita's window and Siderita's
//! embedded modal — and both need the same thing: a `QQuickFramebufferObject`
//! that drives libmpv's render API on Qt's render thread. CXX-Qt 0.9 cannot
//! express it (no virtual overriding, no render-thread hook), so it is
//! hand-written C++; and hand-written C++ has always lived in the app that
//! needed it.
//!
//! It cannot live in either app now. Copying it into the second one is the
//! duplication the contract forbids, and pointing Siderita at Fluorita's `cpp/`
//! would make one application depend on another's tree, which the dependency
//! direction forbids just as plainly. So the seam becomes a crate, beside
//! `siderita-qt` — the same idea: the stable contract toward Qt, owned where
//! both hosts can consume it without knowing about each other.
//!
//! This crate carries **no behaviour**. It has no dependencies, links nothing
//! and starts nothing; it names source files and an include directory so a
//! consuming `build.rs` can compile them. Everything about *playback* stays in
//! `fluorita-core` and `fluorita-engine`, and the handle the item renders from
//! is still the opaque address the engine hands out.
//!
//! # Using it
//!
//! Add it as a **build dependency** and feed the paths to `CxxQtBuilder`:
//!
//! ```ignore
//! let builder = CxxQtBuilder::new_qml_module(module)
//!     .cpp_file(fluorita_qt::VIDEO_ITEM_SOURCE)
//!     // The header carries Q_OBJECT, so it is moc'd as well as compiled.
//!     .cpp_file(fluorita_qt::VIDEO_ITEM_HEADER);
//! // SAFETY: only adds this crate's own include directory.
//! let builder = unsafe { builder.cc_builder(|cc| { cc.include(fluorita_qt::include_dir()); }) };
//! ```
//!
//! The host then calls `register_fluorita_video_item(engine)` — declared in the
//! header — once, before any window exists, and binds the item's `handle`
//! property to what the engine reports.

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// The C++ translation unit implementing the video item.
pub const VIDEO_ITEM_SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/cpp/mpvvideoitem.cpp");

/// Its header. It declares a `Q_OBJECT`, so a consumer compiles *and* moc's it.
pub const VIDEO_ITEM_HEADER: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/cpp/fluorita/mpvvideoitem.h");

/// The include directory that makes `#include "fluorita/mpvvideoitem.h"`
/// resolve, for both the item itself and the generated bridge code.
#[must_use]
pub fn include_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/cpp"))
}

/// Everything a consumer must hand to its C++ builder, in one call.
#[must_use]
pub fn cpp_sources() -> [&'static str; 2] {
    [VIDEO_ITEM_SOURCE, VIDEO_ITEM_HEADER]
}

/// The files a consumer's `build.rs` should watch, so an edit here rebuilds the
/// app that embeds it.
#[must_use]
pub fn rerun_paths() -> [&'static str; 2] {
    cpp_sources()
}

/// The QML module the item registers itself into.
///
/// Deliberately its own namespace rather than an application's: a CXX-Qt module
/// owns its URI, and Qt 6 refuses a `qmlRegisterType` into a namespace a module
/// already claimed.
pub const QML_MODULE: &str = "org.celestina.fluorita.render";

/// The QML type name the hosts instantiate.
pub const QML_TYPE: &str = "MpvVideo";

#[cfg(test)]
mod tests {
    use super::{
        cpp_sources, include_dir, rerun_paths, QML_MODULE, QML_TYPE, VIDEO_ITEM_HEADER,
        VIDEO_ITEM_SOURCE,
    };
    use std::path::Path;

    #[test]
    fn the_named_sources_exist() {
        // A crate whose whole job is to name files earns exactly one test:
        // that the names are true. A moved file would otherwise fail in a
        // consumer's C++ build, far from the cause.
        for source in cpp_sources() {
            assert!(Path::new(source).is_file(), "missing: {source}");
        }
        assert!(include_dir().is_dir());
        assert_eq!(rerun_paths(), cpp_sources());
    }

    #[test]
    fn the_header_is_reachable_through_the_include_directory() {
        // This is the include the C++ actually writes; if the layout moved, the
        // consumer's compile would fail with a missing header instead.
        assert!(include_dir().join("fluorita/mpvvideoitem.h").is_file());
        assert!(VIDEO_ITEM_HEADER.ends_with("fluorita/mpvvideoitem.h"));
        assert!(VIDEO_ITEM_SOURCE.ends_with(".cpp"));
    }

    #[test]
    fn the_qml_identity_is_not_an_application_namespace() {
        assert_eq!(QML_TYPE, "MpvVideo");
        assert!(QML_MODULE.starts_with("org.celestina.fluorita"));
        assert_ne!(
            QML_MODULE, "org.celestina.fluorita",
            "a CXX-Qt module owns its own URI; this type needs a separate one"
        );
    }
}
