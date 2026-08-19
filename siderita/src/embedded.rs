//! The bridge that hands the C++ thumbnail provider a file's own picture.
//!
//! A separate bridge from `crate::thumbnails` because it points the other way:
//! that one imports three C++ helpers, this one exports one Rust function. The
//! parsing itself is `siderita-embedded`'s — a program's icon out of its
//! resource section, an album cover out of a music tag, an app's launcher art
//! out of its package — and none of it belongs to Qt.

#[cxx::bridge]
mod ffi {
    extern "Rust" {
        /// The image the file at these raw path bytes carries inside itself, or
        /// an empty vector when it carries none.
        fn siderita_embedded_image(path_bytes: &[u8]) -> Vec<u8>;
    }
}

/// Raw path bytes in, image bytes out.
///
/// Bytes rather than a string because a file name is not text: a name that is
/// not valid UTF-8 still names a file whose icon a person expects to see.
fn siderita_embedded_image(path_bytes: &[u8]) -> Vec<u8> {
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStrExt;
        std::path::PathBuf::from(std::ffi::OsStr::from_bytes(path_bytes))
    };
    #[cfg(not(unix))]
    let path = std::path::PathBuf::from(String::from_utf8_lossy(path_bytes).into_owned());
    siderita_embedded::embedded_image(&path).unwrap_or_default()
}
