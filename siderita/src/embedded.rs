//! The bridge that hands the C++ thumbnail provider a file's own picture.
//!
//! A separate bridge from `crate::thumbnails` because it points the other way:
//! that one imports three C++ helpers for tests, this one exports two Rust
//! functions — and imports the one call quitting makes to drain the pool. The
//! parsing itself is `siderita-embedded`'s — a program's icon out of its
//! resource section, an album cover out of a music tag, an app's launcher art
//! out of its package — and none of it belongs to Qt.

#[cxx::bridge]
pub(crate) mod ffi {
    extern "Rust" {
        /// The image the file at these raw path bytes carries inside itself, or
        /// an empty vector when it carries none.
        fn siderita_embedded_image(path_bytes: &[u8]) -> Vec<u8>;

        /// The raw path bytes of the icon file a launcher (`.desktop`) at
        /// these raw path bytes names, or an empty vector when it names none
        /// that is installed.
        fn siderita_own_icon_path(path_bytes: &[u8]) -> Vec<u8>;
    }

    unsafe extern "C++" {
        include!("siderita/thumbnailprovider.h");

        /// Drains the thumbnail pool on quit, waiting at most `milliseconds`
        /// for the decodes still running.
        fn siderita_thumbnail_shutdown(milliseconds: i32);
    }
}

/// Raw path bytes as the path they name.
///
/// Bytes rather than a string because a file name is not text: a name that is
/// not valid UTF-8 still names a file whose icon a person expects to see.
fn path_of(path_bytes: &[u8]) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(path_bytes))
}

/// Raw path bytes in, image bytes out.
fn siderita_embedded_image(path_bytes: &[u8]) -> Vec<u8> {
    siderita_embedded::embedded_image(&path_of(path_bytes)).unwrap_or_default()
}

/// Raw path bytes in, icon path bytes out. Runs on the thumbnail pool: it
/// reads the launcher and searches the icon themes.
fn siderita_own_icon_path(path_bytes: &[u8]) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    crate::ownicon::own_icon(&path_of(path_bytes))
        .map(|icon| icon.as_os_str().as_bytes().to_vec())
        .unwrap_or_default()
}
