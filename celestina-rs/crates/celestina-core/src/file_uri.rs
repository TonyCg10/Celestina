//! `file://` URIs and the local paths they name: one strict parser.
//!
//! A `file://` URI arrives from D-Bus (MPRIS art, portal answers, KDE Connect
//! shares), from a `.desktop` `%u`, from a drag or from a clipboard, and every
//! one of those senders is outside this process. The suite used to answer the
//! same URI several different ways: one copy skipped percent-decoding, one took
//! any host as local, one refused `localhost`, one refused non-UTF-8 names.
//! [`to_path`] is the single reading, lifted from magnetitad's strict parser:
//!
//! - the scheme is `file`, compared without regard to ASCII case (RFC 3986);
//! - the authority is empty or `localhost` (again case-insensitive); any other
//!   host names another machine and is refused as [`FileUriError::NotLocal`];
//! - the path is decoded with [`crate::percent::decode_strict`], by bytes, so a
//!   non-UTF-8 name round-trips exactly (ADR 0008) and a malformed escape is an
//!   error rather than a guess;
//! - a raw `?` or `#` starts a query or a fragment, which no local file name
//!   has, so the URI is refused instead of silently truncated or misread;
//! - a decoded NUL is refused, because every syscall would truncate there.
//!
//! The path is not normalized: `..` stays `..`, exactly as the sender wrote it.
//! A consumer that needs a canonical path canonicalizes on its own worker.
//!
//! [`from_path`] is the inverse for an absolute path: the suite's canonical
//! encoding, which every encoder in the suite already spells this way and which
//! Qt and GLib both decode.
//!
//! # Adoption
//!
//! No consumer calls this yet (ruling R-A3 keeps the unit that introduced it
//! purely additive). Each copy moves here in its owner's bug unit: the shell's
//! media cover (`celestina/src/provider_adapter/media.rs`), Siderita's
//! `dbus::uri_to_path`, Grafita's `url::local_path`, Fluorita's
//! `folders::local_path` and `activation::local_path`, and magnetitad's
//! `path_for_file_uri`. A caller that also accepts a plain path treats
//! [`FileUriError::NotFileScheme`] as "not a URI" and takes the argument as a
//! path; every other error means "a URI, but not one naming a local file".

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::percent;

/// Why a string is not a `file://` URI naming a local file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileUriError {
    /// The scheme is not `file` (or there is no scheme at all).
    NotFileScheme,
    /// The authority names a host other than this one.
    NotLocal,
    /// A `file:` URI without the `//` authority marker, or without a path.
    Malformed,
    /// A `%` that is not followed by two hexadecimal digits.
    InvalidEscape,
    /// A raw `?` or `#`: the URI carries a query or a fragment.
    QueryOrFragment,
    /// The decoded path contains a NUL byte.
    ContainsNul,
}

impl fmt::Display for FileUriError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotFileScheme => "not a file:// URI",
            Self::NotLocal => "the file:// URI names another host",
            Self::Malformed => "the file:// URI has no authority marker or no path",
            Self::InvalidEscape => "the file:// URI has a malformed percent escape",
            Self::QueryOrFragment => "the file:// URI carries a query or a fragment",
            Self::ContainsNul => "the file:// URI decodes to a path containing NUL",
        })
    }
}

impl Error for FileUriError {}

const SCHEME: &[u8] = b"file:";

/// The local path a `file://` URI names, decoded byte for byte.
///
/// # Errors
///
/// Returns the [`FileUriError`] that says which rule of the module
/// documentation the input broke.
pub fn to_path(uri: &str) -> Result<PathBuf, FileUriError> {
    let scheme = uri
        .as_bytes()
        .get(..SCHEME.len())
        .ok_or(FileUriError::NotFileScheme)?;
    if !scheme.eq_ignore_ascii_case(SCHEME) {
        return Err(FileUriError::NotFileScheme);
    }
    // The scheme is ASCII, so this index is a character boundary.
    let rest = uri[SCHEME.len()..]
        .strip_prefix("//")
        .ok_or(FileUriError::Malformed)?;
    let encoded = match rest.find('/') {
        // No authority: the path starts immediately.
        Some(0) => rest,
        // `file://localhost/...` is this host spelled the long way.
        Some(index) if rest[..index].eq_ignore_ascii_case("localhost") => &rest[index..],
        Some(_) => return Err(FileUriError::NotLocal),
        None if rest.is_empty() || rest.eq_ignore_ascii_case("localhost") => {
            return Err(FileUriError::Malformed)
        }
        None => return Err(FileUriError::NotLocal),
    };
    if encoded.contains(['?', '#']) {
        return Err(FileUriError::QueryOrFragment);
    }
    let bytes = percent::decode_strict(encoded).ok_or(FileUriError::InvalidEscape)?;
    if bytes.contains(&0) {
        return Err(FileUriError::ContainsNul);
    }
    // `encoded` starts with a literal `/`, so the decoded bytes do too and the
    // path is absolute by construction.
    Ok(percent::path_from_bytes(&bytes))
}

/// The canonical `file://` URI of an absolute path, or `None` for a relative
/// one, which names no file until a working directory is chosen.
///
/// The encoding is [`crate::percent::encode`]'s: `?`, `#`, `%`, spaces and
/// every non-ASCII byte are escaped, so [`to_path`] reads back the same bytes.
#[must_use]
pub fn from_path(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    Some(format!(
        "file://{}",
        percent::encode(&percent::path_bytes(path))
    ))
}

#[cfg(test)]
mod tests {
    use super::{from_path, to_path, FileUriError};
    use std::path::{Path, PathBuf};

    #[test]
    fn a_local_uri_decodes_to_its_path() {
        assert_eq!(
            to_path("file:///home/u/My%20Music/cover.jpg"),
            Ok(PathBuf::from("/home/u/My Music/cover.jpg"))
        );
        assert_eq!(to_path("file:///"), Ok(PathBuf::from("/")));
    }

    #[test]
    fn localhost_is_this_host_and_any_other_host_is_not() {
        assert_eq!(
            to_path("file://localhost/etc/x"),
            Ok(PathBuf::from("/etc/x"))
        );
        assert_eq!(
            to_path("file://LocalHost/etc/x"),
            Ok(PathBuf::from("/etc/x"))
        );
        assert_eq!(
            to_path("file://otherhost/etc/x"),
            Err(FileUriError::NotLocal)
        );
        assert_eq!(
            to_path("file://localhost:8080/x"),
            Err(FileUriError::NotLocal)
        );
        assert_eq!(
            to_path("file://user@localhost/x"),
            Err(FileUriError::NotLocal)
        );
        assert_eq!(to_path("file://otherhost"), Err(FileUriError::NotLocal));
    }

    #[test]
    fn the_scheme_is_case_insensitive_and_must_be_file() {
        assert_eq!(to_path("FILE:///tmp/a"), Ok(PathBuf::from("/tmp/a")));
        assert_eq!(to_path("https:///tmp/a"), Err(FileUriError::NotFileScheme));
        assert_eq!(to_path("/tmp/a"), Err(FileUriError::NotFileScheme));
        assert_eq!(to_path(""), Err(FileUriError::NotFileScheme));
        assert_eq!(to_path("fil"), Err(FileUriError::NotFileScheme));
        // A multi-byte character where the scheme would end is not a panic.
        assert_eq!(to_path("file\u{e9}"), Err(FileUriError::NotFileScheme));
    }

    #[test]
    fn a_uri_without_the_authority_marker_or_a_path_is_malformed() {
        assert_eq!(to_path("file:/tmp/a"), Err(FileUriError::Malformed));
        assert_eq!(to_path("file:tmp/a"), Err(FileUriError::Malformed));
        assert_eq!(to_path("file://"), Err(FileUriError::Malformed));
        assert_eq!(to_path("file://localhost"), Err(FileUriError::Malformed));
    }

    #[test]
    fn escapes_are_strict() {
        assert_eq!(to_path("file:///a%2"), Err(FileUriError::InvalidEscape));
        assert_eq!(to_path("file:///a%zz"), Err(FileUriError::InvalidEscape));
        assert_eq!(to_path("file:///a%41%2f"), Ok(PathBuf::from("/aA/")));
    }

    #[test]
    fn a_nul_byte_is_refused() {
        assert_eq!(to_path("file:///a%00b"), Err(FileUriError::ContainsNul));
    }

    #[test]
    fn a_query_or_a_fragment_is_refused_rather_than_read_as_a_name() {
        assert_eq!(to_path("file:///a?b"), Err(FileUriError::QueryOrFragment));
        assert_eq!(to_path("file:///a#b"), Err(FileUriError::QueryOrFragment));
        // Escaped, the same characters are ordinary name bytes.
        assert_eq!(to_path("file:///a%3Fb%23c"), Ok(PathBuf::from("/a?b#c")));
    }

    #[test]
    fn non_utf8_names_decode_byte_exactly() {
        use std::os::unix::ffi::OsStrExt;
        let path = to_path("file:///home/u/%FF%FEname").expect("a local path");
        assert_eq!(path.as_os_str().as_bytes(), b"/home/u/\xff\xfename");
    }

    #[test]
    fn encoding_and_decoding_round_trip_every_absolute_path() {
        use std::os::unix::ffi::OsStrExt;
        let names: [&[u8]; 5] = [
            b"/home/u/a b.txt",
            b"/home/u/100%/q?#",
            b"/home/u/\xc3\xb1and\xff",
            b"/",
            b"/a;b=c&d+e",
        ];
        for raw in names {
            let path = Path::new(std::ffi::OsStr::from_bytes(raw));
            let uri = from_path(path).expect("an absolute path encodes");
            assert!(uri.starts_with("file:///"), "{uri}");
            assert_eq!(to_path(&uri).as_deref(), Ok(path), "{uri}");
        }
    }

    #[test]
    fn a_relative_path_has_no_file_uri() {
        assert_eq!(from_path(Path::new("relative/name")), None);
        assert_eq!(from_path(Path::new("")), None);
    }
}
