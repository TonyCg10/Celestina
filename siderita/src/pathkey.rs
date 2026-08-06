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

/// The marker every record written from now on carries, so that reading one
/// back is a decision rather than a guess.
///
/// It is deliberately not a legal start for either spelling it has to be told
/// apart from: a key and a raw path both begin with `/`.
const KEY_MARK: &str = "key:";

/// A path key as it is written into a persisted record.
///
/// Every store that keeps a path — bookmarks, favourites, icons, folder views,
/// the tab session — writes through this, so its file becomes unambiguous the
/// first time it is saved.
pub fn persist(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    format!("{KEY_MARK}{key}")
}

/// A persisted identity string as a key.
///
/// A marked record is a key and is taken verbatim: no codec runs over it, so
/// there is nothing left to infer. That is the whole point of the mark. The
/// alternative it replaces — re-encoding whatever was read and relying on the
/// codec being idempotent over both spellings — cannot distinguish a legacy raw
/// path that literally contains `%20` from a key that means a path containing a
/// space, and silently answered with the second. For a bookmark, which is a
/// navigation and a drop target, that is the wrong folder rather than a
/// forgotten mark.
///
/// An unmarked record predates the mark and is migrated as before, because the
/// bytes already on disk carry no evidence either way and existing files must
/// keep loading. That residual ambiguity is bounded: it can only affect records
/// written before this change, and one save of the store retires it.
pub fn normalize(stored: &str) -> String {
    match stored.strip_prefix(KEY_MARK) {
        Some(key) => key.to_owned(),
        None => percent::encode(&percent::decode(stored)),
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_str, encode, normalize, persist, KeyError};
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

    #[test]
    fn a_marked_record_is_a_key_and_is_taken_verbatim() {
        for path in [
            "/home/u/mis fotos",
            // The name the codec would otherwise re-read as an escape.
            "/home/u/100%20de descuento",
        ] {
            let key = encode(&PathBuf::from(path));
            let record = persist(&key);
            assert!(record.starts_with("key:"), "{record}");
            assert_eq!(normalize(&record), key);
            assert_eq!(decode_str(&normalize(&record)), Ok(PathBuf::from(path)));
        }
        // Including the case the whole seam exists for.
        let record = persist(&encode(&non_utf8_path()));
        assert_eq!(decode_str(&normalize(&record)), Ok(non_utf8_path()));
        // Writing a record is idempotent in the only sense that matters: the
        // key it reads back as is the key that went in.
        assert_eq!(
            normalize(&persist(&normalize(&record))),
            encode(&non_utf8_path())
        );
        assert_eq!(persist(""), "");
    }

    #[test]
    fn an_unmarked_legacy_record_still_migrates() {
        // Unmarked records predate the mark; existing files must keep loading.
        assert_eq!(normalize("/home/u/mis fotos"), "/home/u/mis%20fotos");
        // The residual ambiguity, recorded rather than claimed closed: a legacy
        // raw path holding a literal `%20` is indistinguishable from a key for
        // a path holding a space, and is still read as the latter. Only a
        // record written before the mark can reach this, and saving retires it.
        assert_eq!(normalize("/home/u/100%20"), "/home/u/100%20");
        assert_eq!(
            decode_str(&normalize("/home/u/100%20")),
            Ok(PathBuf::from("/home/u/100 "))
        );
        // Marked, the same name survives — which is the repair.
        let literal = PathBuf::from("/home/u/100%20");
        assert_eq!(
            decode_str(&normalize(&persist(&encode(&literal)))),
            Ok(literal)
        );
    }
}
