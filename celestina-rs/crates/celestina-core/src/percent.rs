//! Percent-encoding of path bytes — one codec instead of four.
//!
//! The Trash spec (`siderita-ops`) percent-encodes the original path, and the
//! app's portal/D-Bus glue percent-encodes `file://` URIs; both had their own
//! copy of the same `%XX` codec, and the decode half was written twice inside
//! this workspace's core alone. This is the single implementation they share.
//!
//! It works on raw bytes so a non-UTF-8 path round-trips, and the unreserved
//! set (`alnum` + `-_.~/`) and uppercase hex match what the four copies emitted,
//! so encoded output is byte-for-byte identical — existing `.trashinfo` files
//! and URIs keep decoding.

use std::path::{Path, PathBuf};

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Percent-encodes `bytes`: the unreserved set — ASCII alphanumerics plus `-`,
/// `_`, `.`, `~` and `/` — is kept raw; every other byte becomes an uppercase
/// `%XX` escape.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

/// Percent-encodes absolute path bytes the way `QUrl::fromLocalFile().toEncoded()`
/// does: RFC 3986's `pchar` set (`alnum`, `-._~`, `!$&'()*+,;=`, `:@`) plus the
/// path separator `/` stay raw, and every other byte becomes an uppercase `%XX`
/// escape.
///
/// This is a genuinely different preserved set from [`encode`], not a duplicated
/// recipe: [`encode`] escapes `!$&'()*+,;=:@`, which is right for the Trash spec
/// and wrong for a key another Qt consumer already computes. The freedesktop
/// thumbnail cache keys on this exact spelling, so Fluorita and Siderita must
/// agree byte for byte; the rule was recovered from a Qt 6 probe rather than
/// from documentation. Known interop limit: GLib escapes `;` where Qt keeps it
/// raw, so a filename containing `;` lands on two different cache keys.
pub fn encode_qt_path(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &byte in bytes {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
                    | b'/'
            )
        {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

/// Decodes `%XX` escapes to bytes, keeping a malformed escape verbatim so a
/// stray `%` never drops the rest of the input. For URIs off the bus/clipboard,
/// where salvaging the readable part beats discarding the whole path.
pub fn decode(value: &str) -> Vec<u8> {
    let raw = value.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' {
            if let (Some(high), Some(low)) = (
                raw.get(index + 1).and_then(|b| hex_value(*b)),
                raw.get(index + 2).and_then(|b| hex_value(*b)),
            ) {
                out.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        out.push(raw[index]);
        index += 1;
    }
    out
}

/// Like [`decode`] but returns `None` on a malformed escape — for a spec file
/// (`.trashinfo`) where a corrupt entry should be skipped, not salvaged.
pub fn decode_strict(value: &str) -> Option<Vec<u8>> {
    let raw = value.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        match raw[index] {
            b'%' => {
                let high = hex_value(*raw.get(index + 1)?)?;
                let low = hex_value(*raw.get(index + 2)?)?;
                out.push((high << 4) | low);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// A path's raw bytes: exact on Unix, lossy UTF-8 elsewhere.
#[cfg(unix)]
pub fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
pub fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

/// Bytes back to a path: byte-exact on Unix, lossy UTF-8 elsewhere.
#[cfg(unix)]
pub fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
pub fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{decode, decode_strict, encode, encode_qt_path};

    #[test]
    fn encode_keeps_the_unreserved_set_and_escapes_the_rest() {
        assert_eq!(encode(b"/home/u/a b"), "/home/u/a%20b");
        assert_eq!(encode(b"/x/y.txt"), "/x/y.txt");
        assert_eq!(encode(b"a-_.~/z"), "a-_.~/z");
        // Uppercase hex, like every copy this replaces.
        assert_eq!(encode(b"#"), "%23");
    }

    #[test]
    fn qt_path_encoding_keeps_pchar_and_escapes_the_rest() {
        // The sub-delims and `:@` that `encode` escapes stay raw here.
        assert_eq!(
            encode_qt_path(b"/home/u/plus+amp&eq=semi;.flac"),
            "/home/u/plus+amp&eq=semi;.flac"
        );
        assert_eq!(
            encode_qt_path(b"/home/u/at@colon:comma,.opus"),
            "/home/u/at@colon:comma,.opus"
        );
        assert_eq!(encode_qt_path(b"/home/u/a b.mp4"), "/home/u/a%20b.mp4");
        assert_eq!(
            encode_qt_path(b"/home/u/weird#hash?q.png"),
            "/home/u/weird%23hash%3Fq.png"
        );
        // Uppercase escapes, including for a byte that is not valid UTF-8.
        assert_eq!(encode_qt_path(&[b'/', 0xff]), "/%FF");
    }

    #[test]
    fn qt_path_encoding_differs_from_the_trash_codec() {
        assert_eq!(encode(b"a;b"), "a%3Bb");
        assert_eq!(encode_qt_path(b"a;b"), "a;b");
    }

    #[test]
    fn decode_round_trips_and_keeps_a_stray_percent() {
        assert_eq!(decode("/home/u/a%20b"), b"/home/u/a b");
        // Lenient: a truncated escape survives verbatim.
        assert_eq!(decode("/bad%2"), b"/bad%2");
    }

    #[test]
    fn strict_decode_rejects_a_truncated_escape() {
        assert_eq!(decode_strict("/home/u/a%20b").unwrap(), b"/home/u/a b");
        assert!(decode_strict("/bad%2").is_none());
    }

    #[test]
    fn non_utf8_bytes_round_trip() {
        let raw = &[b'/', 0xff, b'a'];
        assert_eq!(decode(&encode(raw)), raw);
    }
}
