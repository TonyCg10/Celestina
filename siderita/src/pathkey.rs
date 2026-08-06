//! Siderita's Qt-flavoured face of the path key.
//!
//! The rule itself — what a key is, and which values are refused — has one
//! owner: [`celestina_core::pathkey`], settled by
//! [ADR 0008](../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md).
//! This module adds no codec and no policy. It is the marshalling that core
//! crate deliberately leaves to each application: taking a `QString`, taking a
//! `QStringList`, publishing a `QString`, and migrating a record persisted
//! before the decision.
//!
//! The half that is *not* here matters just as much. Display text — the lossy
//! conversion a person reads — travels under its own property names, is never
//! an argument to anything, and never returns to Rust as a path. Paths that
//! leave the process keep their own spellings: `crate::dbus` encodes `file://`
//! URIs, `crate::portal` answers the portal's spec, and the Trash codec lives
//! in `siderita-ops`.

use std::path::{Path, PathBuf};

use celestina_core::percent;
use cxx_qt_lib::{QString, QStringList};

pub use celestina_core::pathkey::{encode, PathKeyError as KeyError};

/// `path` as a key, ready to publish to QML.
pub fn publish(path: &Path) -> QString {
    QString::from(encode(path).as_str())
}

/// The path `key` names, or the typed reason it names none.
pub fn decode(key: &QString) -> Result<PathBuf, KeyError> {
    celestina_core::pathkey::decode(&key.to_string())
}

/// [`decode`] for a key already held as a Rust string (a persisted record, a
/// tab-separated field).
pub fn decode_str(key: &str) -> Result<PathBuf, KeyError> {
    celestina_core::pathkey::decode(key)
}

/// Every key in `list`, skipping empty entries, or the first refusal.
///
/// A batch verb acts on all of its entries or on none: a malformed key means
/// the caller is confused about what it is holding, and guessing which half of
/// the batch to honour is how a batch write reaches the wrong file.
pub fn decode_list(list: &QStringList) -> Result<Vec<PathBuf>, KeyError> {
    list.iter()
        .map(QString::to_string)
        .filter(|key| !key.is_empty())
        .map(|key| decode_str(&key))
        .collect()
}

/// A persisted identity string as a key.
///
/// Records written before ADR 0008 hold the raw path, so this decodes first and
/// re-encodes: a raw path carrying no `%` escape decodes to itself, and a key
/// round-trips, which makes the conversion idempotent over both spellings.
/// Known limit: a legacy raw path that literally contains something shaped like
/// `%XX` normalizes to a different key, and its star or icon is forgotten.
pub fn normalize(stored: &str) -> String {
    percent::encode(&percent::decode(stored))
}

#[cfg(test)]
mod tests {
    use super::{decode_str, encode, normalize, KeyError};
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    /// A name whose bytes are not valid UTF-8 — the case the whole seam exists
    /// for. A byte fixture, not text, so the language contract does not apply.
    fn non_utf8_path() -> PathBuf {
        PathBuf::from(OsStr::from_bytes(b"/tmp/na\xffme"))
    }

    #[test]
    fn the_app_side_agrees_with_the_codec_it_delegates_to() {
        let path = non_utf8_path();
        let key = encode(&path);
        assert_eq!(key, "/tmp/na%FFme");
        assert_eq!(decode_str(&key), Ok(path));
    }

    #[test]
    fn a_malformed_key_is_refused_rather_than_salvaged() {
        assert_eq!(decode_str("/tmp/bad%2"), Err(KeyError::Malformed));
        assert_eq!(decode_str("/tmp/bad%zz"), Err(KeyError::Malformed));
        assert_eq!(decode_str(""), Err(KeyError::Empty));
        assert_eq!(decode_str("relative/name"), Err(KeyError::NotAbsolute));
        // Display text handed back where a key was expected.
        assert_eq!(
            decode_str("/tmp/na\u{fffd}me"),
            Err(KeyError::NotAscii),
            "the lossy spelling is not a key"
        );
    }

    #[test]
    fn normalizing_accepts_a_legacy_raw_path_and_is_idempotent() {
        let key = normalize("/home/u/mis fotos");
        assert_eq!(key, "/home/u/mis%20fotos");
        assert_eq!(normalize(&key), key);
        // A record already written as a key for a non-UTF-8 name survives too.
        let encoded = encode(&non_utf8_path());
        assert_eq!(normalize(&encoded), encoded);
        assert_eq!(decode_str(&normalize(&encoded)), Ok(non_utf8_path()));
    }
}
