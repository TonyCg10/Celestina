//! The byte-exact identity of a file, as it crosses a Qt seam.
//!
//! A Linux filename is a byte string that is not required to be UTF-8, and the
//! suite's cores keep it that way. Publishing such a path to QML with
//! `to_string_lossy` throws that away: each invalid byte becomes U+FFFD, and the
//! string that comes back names a file that does not exist.
//!
//! [ADR 0008] settles the rule this module implements. Two representations
//! cross the seam and they are not interchangeable: a **path key** is opaque
//! ASCII produced by [`encode`] and accepted by every invokable that acts on a
//! file, while display text is the lossy conversion, shown to a person and
//! never handed back.
//!
//! This adds no codec. It names one composition over [`crate::percent`] — the
//! codec the whole suite already speaks — and, more importantly, it names the
//! *refusal*: a value that did not come from [`encode`] is rejected with a
//! typed error instead of being salvaged into a path that addresses the wrong
//! file. `percent::decode_strict` alone will not do that job, because it passes
//! a raw non-ASCII byte through verbatim, so a caller that handed over a
//! display string would get a plausible path for an ASCII name and the wrong
//! one for exactly the names this exists to protect.
//!
//! Not to be confused with [`crate::percent::encode_qt_path`], which preserves a
//! different set because it addresses a freedesktop thumbnail cache entry and
//! must match Qt's own spelling byte for byte.
//!
//! A Qt-flavoured wrapper — taking a `QString`, publishing a `QStringList` —
//! belongs to each application's adapter. This crate is pure and stays that way.
//!
//! [ADR 0008]: ../../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md

use std::fmt;
use std::path::{Path, PathBuf};

use crate::percent;

/// Why a value handed across the seam is not a usable path key.
///
/// Every variant means the same thing about provenance — this did not come from
/// [`encode`] — and they are distinguished so a diagnostic can say which shape
/// was wrong rather than only that something was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathKeyError {
    /// No key at all, where one was required.
    Empty,
    /// A byte outside ASCII. [`encode`] emits only unreserved ASCII and `%XX`
    /// escapes, so a raw high byte means the caller passed display text.
    NotAscii,
    /// A `%` escape not followed by two hexadecimal digits.
    Malformed,
    /// Well formed, but not the absolute path every published key names.
    NotAbsolute,
}

impl fmt::Display for PathKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("no path was given"),
            Self::NotAscii => formatter.write_str("the path key is not a key but display text"),
            Self::Malformed => formatter.write_str("the path key is malformed"),
            Self::NotAbsolute => formatter.write_str("the path key is not absolute"),
        }
    }
}

impl std::error::Error for PathKeyError {}

/// `path` as the key that identifies it across the seam.
#[must_use]
pub fn encode(path: &Path) -> String {
    percent::encode(&percent::path_bytes(path))
}

/// The path `key` names, or the typed reason it names none.
pub fn decode(key: &str) -> Result<PathBuf, PathKeyError> {
    if key.is_empty() {
        return Err(PathKeyError::Empty);
    }
    if !key.is_ascii() {
        return Err(PathKeyError::NotAscii);
    }
    let bytes = percent::decode_strict(key).ok_or(PathKeyError::Malformed)?;
    if bytes.is_empty() {
        return Err(PathKeyError::Empty);
    }
    let path = percent::path_from_bytes(&bytes);
    if !path.is_absolute() {
        return Err(PathKeyError::NotAbsolute);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, PathKeyError};
    use std::path::{Path, PathBuf};

    /// A name whose bytes are not valid UTF-8 — the case the seam exists for.
    #[cfg(unix)]
    fn non_utf8_path() -> PathBuf {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        Path::new("/home/toni").join(OsStr::from_bytes(b"na\xffme.txt"))
    }

    #[test]
    #[cfg(unix)]
    fn a_name_that_is_not_utf8_survives_the_round_trip_byte_for_byte() {
        let path = non_utf8_path();
        let key = encode(&path);
        assert!(key.is_ascii(), "a key is transportable ASCII: {key}");
        assert_eq!(key, "/home/toni/na%FFme.txt");
        assert_eq!(decode(&key), Ok(path));
    }

    #[test]
    #[cfg(unix)]
    fn the_key_differs_from_the_lossy_spelling_that_used_to_be_published() {
        let path = non_utf8_path();
        // The old seam published this, and it names no file on disk.
        let lossy = path.to_string_lossy().into_owned();
        assert_ne!(encode(&path), lossy);
        assert_eq!(decode(&lossy), Err(PathKeyError::NotAscii));
    }

    #[test]
    fn an_ordinary_name_round_trips_and_stays_readable() {
        let path = Path::new("/home/toni/nota.txt");
        assert_eq!(encode(path), "/home/toni/nota.txt");
        assert_eq!(decode("/home/toni/nota.txt"), Ok(path.to_path_buf()));
    }

    #[test]
    fn a_space_and_a_hash_are_escaped_rather_than_passed_through() {
        let path = Path::new("/home/toni/informe #3.pdf");
        let key = encode(path);
        assert_eq!(key, "/home/toni/informe%20%233.pdf");
        assert_eq!(decode(&key), Ok(path.to_path_buf()));
    }

    #[test]
    fn a_value_this_process_did_not_emit_is_refused_rather_than_salvaged() {
        assert_eq!(decode(""), Err(PathKeyError::Empty));
        assert_eq!(decode("%%"), Err(PathKeyError::Malformed));
        assert_eq!(decode("/home/%f"), Err(PathKeyError::Malformed));
        assert_eq!(decode("nota.txt"), Err(PathKeyError::NotAbsolute));
        assert_eq!(decode("%2E"), Err(PathKeyError::NotAbsolute));
    }

    #[test]
    fn every_refusal_says_something_different() {
        let reasons = [
            PathKeyError::Empty,
            PathKeyError::NotAscii,
            PathKeyError::Malformed,
            PathKeyError::NotAbsolute,
        ]
        .map(|error| error.to_string());
        for (index, reason) in reasons.iter().enumerate() {
            assert!(!reason.is_empty());
            assert!(
                !reasons[index + 1..].contains(reason),
                "duplicated: {reason}"
            );
        }
    }
}
