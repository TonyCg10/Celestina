//! What Fluorita was asked to open, before anything is opened.
//!
//! The desktop hands over either a path or a `file://` URI, and either may be
//! bytes that are not valid UTF-8. The raw `PathBuf` is what would eventually
//! reach the engine; the lossy string beside it exists only to put something on
//! screen and must never be turned back into a path.
//!
//! Classification here is deliberately free: `fluorita-core` decides an item's
//! kind from its name alone, so the scaffold can say *what* it was handed
//! without starting a decoder. That is the same contract the library relies on
//! while browsing.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use celestina_core::file_uri::{self, FileUriError};
use fluorita_core::MediaKind;

/// The item named on the command line, if any.
#[derive(Clone, Debug, Default)]
pub struct RequestedMedia {
    /// The real path, byte-exact. `None` when Fluorita was launched with no
    /// argument, which is the ordinary "open the library" case.
    pub path: Option<PathBuf>,
    /// A label for the window. Lossy on purpose, and never reopened.
    pub label: String,
    pub kind: Option<MediaKind>,
}

impl RequestedMedia {
    /// The Spanish label the window shows for the classified kind. An
    /// unclassified file says so plainly rather than being guessed at.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            Some(MediaKind::Image) => "imagen",
            Some(MediaKind::Video) => "vídeo",
            Some(MediaKind::Audio) => "audio",
            None if self.path.is_some() => "tipo no reconocido",
            None => "",
        }
    }
}

/// Reads the first argument that names something to open.
///
/// Options are skipped rather than treated as filenames, and only the first
/// item is taken: this window opens one item, so silently swallowing the rest
/// would be worse than showing the one it took.
#[must_use]
pub fn requested_media() -> RequestedMedia {
    std::env::args_os()
        .skip(1)
        .find(|argument| !is_option(argument))
        .map_or_else(RequestedMedia::default, |argument| describe(&argument))
}

fn is_option(argument: &OsString) -> bool {
    argument
        .to_str()
        .is_some_and(|text| text.starts_with('-') && text != "-")
}

fn describe(argument: &OsString) -> RequestedMedia {
    // Resolved once, here: everything downstream — the engine's source handle,
    // the `file://` URL a still is shown through — refuses a relative path
    // rather than guess a working directory.
    let path = local_path(argument).map(|path| std::fs::canonicalize(&path).unwrap_or(path));
    let label = path
        .as_deref()
        .map(display_label)
        .unwrap_or_else(|| argument.to_string_lossy().into_owned());
    let kind = path.as_deref().and_then(MediaKind::classify_path);
    RequestedMedia { path, label, kind }
}

/// Turns one argument into a local path: a `file://` URI is read by the
/// suite's one strict parser, anything else is taken as a path as-is.
///
/// A URI with a host other than the local machine is refused, and so is one
/// that is malformed — an escape that is not one, a query, a NUL: Fluorita's
/// library is local, and quietly reading either as a local path would open
/// the wrong file.
#[must_use]
pub fn local_path(argument: &OsString) -> Option<PathBuf> {
    // Not text, so not a URI: the argument is the path, bytes and all.
    let Some(text) = argument.to_str() else {
        return Some(PathBuf::from(argument));
    };
    match file_uri::to_path(text) {
        Ok(path) => Some(path),
        // Not a URI: the argument is the path.
        Err(FileUriError::NotFileScheme) => Some(PathBuf::from(argument)),
        Err(_) => None,
    }
}

/// A human-readable name for the window title. Lossy by design, and bounded
/// and stripped like every name a file claims.
fn display_label(path: &Path) -> String {
    fluorita_core::displayed_name(path)
}

#[cfg(test)]
mod tests {
    use super::{describe, local_path, RequestedMedia};
    use fluorita_core::MediaKind;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn a_plain_path_is_taken_as_it_is() {
        assert_eq!(
            local_path(&OsString::from("/home/toni/Vídeos/clip.mp4")),
            Some(PathBuf::from("/home/toni/Vídeos/clip.mp4"))
        );
    }

    #[test]
    fn a_file_uri_is_decoded_with_the_suite_codec() {
        assert_eq!(
            local_path(&OsString::from("file:///home/toni/a%20b.mp4")),
            Some(PathBuf::from("/home/toni/a b.mp4"))
        );
        assert_eq!(
            local_path(&OsString::from("file://localhost/home/toni/x.mp3")),
            Some(PathBuf::from("/home/toni/x.mp3"))
        );
    }

    #[test]
    fn a_remote_authority_is_refused_rather_than_read_as_local() {
        assert_eq!(
            local_path(&OsString::from("file://otherhost/etc/x.mp4")),
            None
        );
    }

    #[test]
    fn a_malformed_uri_is_refused_rather_than_guessed_at() {
        // The folder chooser already refused these; the command line now
        // reads a URI the same way.
        assert_eq!(local_path(&OsString::from("file:///m/half%2")), None);
        assert_eq!(local_path(&OsString::from("file:///m/x.mp4?t=3")), None);
        assert_eq!(local_path(&OsString::from("file:///m/a%00b.mp4")), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_argument_keeps_its_bytes() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let argument = OsString::from_vec(b"/home/toni/mal-\xFF.mp4".to_vec());
        let path = local_path(&argument).expect("a path");

        assert_eq!(path.as_os_str().as_bytes(), b"/home/toni/mal-\xFF.mp4");
        // The label may be lossy; the path must not be.
        assert!(describe(&argument).label.contains('\u{FFFD}'));
    }

    #[test]
    fn the_kind_label_never_guesses() {
        let video = describe(&OsString::from("/home/toni/clip.mkv"));
        assert_eq!(video.kind, Some(MediaKind::Video));
        assert_eq!(video.kind_label(), "vídeo");
        assert_eq!(video.label, "clip.mkv");

        let unknown = describe(&OsString::from("/home/toni/notas.txt"));
        assert_eq!(unknown.kind, None);
        assert_eq!(unknown.kind_label(), "tipo no reconocido");

        assert_eq!(RequestedMedia::default().kind_label(), "");
    }
}
