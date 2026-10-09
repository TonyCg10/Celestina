//! Turning what a desktop handler passes into a local path, and back.
//!
//! A `.desktop` entry's `%u` hands over a `file://` URL, a `%f` a plain path,
//! and a shell hands over whatever the user typed. All three arrive at the same
//! place, so the conversion lives here rather than being guessed at each call
//! site — and anything that is not a local file is refused rather than
//! half-understood.
//!
//! How a `file://` URI names a local path is not Grafita's rule: it is
//! [`celestina_core::file_uri`]'s, the suite's one strict reading, which
//! decodes by bytes so a name that is not UTF-8 arrives exactly as it is on
//! disk. What stays here is only Grafita's choice to accept a plain path in the
//! same place.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use celestina_core::file_uri::{self, FileUriError};

/// The local path behind `argument`, or `None` when it does not name one.
///
/// A plain path is taken as-is. A `file://` URI is read by
/// [`file_uri::to_path`]: only an empty or `localhost` authority counts as
/// local, because `file://otra-maquina/x` names someone else's filesystem,
/// which Grafita cannot write back to atomically and therefore will not
/// pretend to edit.
///
/// Only an argument with the `//` authority marker is a URI here. `file:` with
/// anything else after it, `file:notas.txt`, is a relative name that happens
/// to start that way, and it opens as one, as it always has.
#[must_use]
pub fn local_path(argument: &str) -> Option<PathBuf> {
    match file_uri::to_path(argument) {
        Ok(path) => Some(path),
        Err(FileUriError::NotFileScheme) => (!argument.is_empty()).then(|| PathBuf::from(argument)),
        Err(FileUriError::Malformed) if !has_authority_marker(argument) => {
            Some(PathBuf::from(argument))
        }
        Err(_) => None,
    }
}

/// Whether `argument`, already known to start with the `file:` scheme, goes
/// on with `//`.
fn has_authority_marker(argument: &str) -> bool {
    argument
        .get("file:".len()..)
        .is_some_and(|rest| rest.starts_with("//"))
}

/// The local path behind a command-line argument, which need not be UTF-8.
///
/// An argument that is not UTF-8 cannot be a URI, so it is a plain path and
/// its bytes are kept as they are.
#[must_use]
pub fn local_path_os(argument: &OsStr) -> Option<PathBuf> {
    match argument.to_str() {
        Some(text) => local_path(text),
        None => Some(PathBuf::from(argument)),
    }
}

/// How `path` travels through a QString to come back through [`local_path`]
/// unchanged, or `None` when it cannot.
///
/// A UTF-8 path travels as itself, which is also what an older Grafita on the
/// other end of the activation call understands. A path that is not UTF-8
/// cannot: a QString would replace its stray bytes, and the document opened
/// would be a different file or none. It travels as its canonical `file://`
/// URI instead, which is ASCII, and which exists only for an absolute path.
#[must_use]
pub fn qml_argument(path: &Path) -> Option<String> {
    match path.to_str() {
        Some(text) => Some(text.to_owned()),
        None => file_uri::from_path(path),
    }
}

/// What a dropped `text/uri-list` names: the local files, byte for byte, and
/// how many entries were not local files and are left alone.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Dropped {
    pub local: Vec<PathBuf>,
    pub ignored: usize,
}

/// Splits dropped URIs. Unlike a command-line argument, a drop always carries
/// URIs, so only [`file_uri::to_path`] decides: a remote URI (`smb://`,
/// `https://`, `file://otra-maquina/…`) or a malformed one is counted and
/// ignored, never read as a relative name.
#[must_use]
pub fn dropped<I, S>(uris: I) -> Dropped
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = Dropped::default();
    for uri in uris {
        match file_uri::to_path(uri.as_ref()) {
            Ok(path) => out.local.push(path),
            Err(_) => out.ignored += 1,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use super::{dropped, local_path, local_path_os, qml_argument};

    #[test]
    fn a_drop_keeps_local_files_by_bytes_and_ignores_remote_uris() {
        let drop = dropped([
            "file:///tmp/a%20b",
            "smb://x/y",
            "file:///tmp/%FFnota.txt",
            "file://otra-maquina/tmp/c",
            "notas.txt",
        ]);
        assert_eq!(
            drop.local,
            vec![
                PathBuf::from("/tmp/a b"),
                PathBuf::from(OsStr::from_bytes(b"/tmp/\xFFnota.txt")),
            ]
        );
        assert_eq!(drop.ignored, 3);
        // A name that is not UTF-8 reaches the tab in the form `openPath`
        // reads back unchanged.
        let argument = qml_argument(&drop.local[1]).expect("an argument");
        assert_eq!(local_path(&argument).as_ref(), Some(&drop.local[1]));
    }

    #[test]
    fn plain_paths_and_local_urls_both_arrive_as_paths() {
        let cases = [
            ("/home/toni/notas.txt", Some("/home/toni/notas.txt")),
            ("relativo/nota", Some("relativo/nota")),
            // No authority marker: a relative name, not a URI.
            ("file:notas.txt", Some("file:notas.txt")),
            ("FILE:dir/nota", Some("FILE:dir/nota")),
            ("file:///home/toni/notas.txt", Some("/home/toni/notas.txt")),
            (
                "file://localhost/home/toni/notas.txt",
                Some("/home/toni/notas.txt"),
            ),
            (
                "file:///home/toni/con%20espacio%20y%20%C3%B1.txt",
                Some("/home/toni/con espacio y ñ.txt"),
            ),
        ];

        for (argument, expected) in cases {
            assert_eq!(
                local_path(argument),
                expected.map(PathBuf::from),
                "{argument}"
            );
        }
    }

    #[test]
    fn anything_that_is_not_a_local_file_is_refused() {
        for argument in [
            "",
            "file://otra-maquina/home/toni/notas.txt",
            "file://",
            // A truncated or non-hexadecimal escape is a malformed name, not a
            // name with a literal percent in it.
            "file:///home/toni/roto%2",
            "file:///home/toni/roto%zz",
            // A decoded NUL would truncate the name at every syscall.
            "file:///home/toni/nul%00.txt",
        ] {
            assert_eq!(local_path(argument), None, "{argument}");
        }
    }

    #[test]
    fn a_name_that_is_not_utf8_arrives_byte_for_byte() {
        // %FF is not UTF-8, but it is a perfectly good Linux file name byte:
        // refusing it would make the file impossible to open from a chooser.
        assert_eq!(
            local_path("file:///home/toni/%FFnota.txt"),
            Some(PathBuf::from(OsStr::from_bytes(b"/home/toni/\xFFnota.txt")))
        );
    }

    #[test]
    fn a_command_line_argument_that_is_not_utf8_is_a_plain_path() {
        let raw = OsStr::from_bytes(b"/home/toni/\xE9t\xE9.txt");

        assert_eq!(local_path_os(raw), Some(PathBuf::from(raw)));
        assert_eq!(
            local_path_os(OsStr::new("file:///home/toni/a%20b")),
            Some(PathBuf::from("/home/toni/a b"))
        );
    }

    #[test]
    fn every_path_survives_the_trip_through_a_qstring() {
        let utf8 = Path::new("/home/toni/ørn.txt");
        let bytes = Path::new(OsStr::from_bytes(b"/home/toni/\xFF.txt"));

        assert_eq!(qml_argument(utf8).as_deref(), Some("/home/toni/ørn.txt"));
        for path in [utf8, bytes] {
            let argument = qml_argument(path).expect("an argument");
            assert_eq!(local_path(&argument).as_deref(), Some(path), "{argument}");
        }
        assert_eq!(
            qml_argument(Path::new(OsStr::from_bytes(b"relativo\xFF"))),
            None,
            "a relative name that is not UTF-8 has no URI"
        );
    }
}
