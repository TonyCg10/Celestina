//! Turning what a desktop handler passes into a local path.
//!
//! A `.desktop` entry's `%u` hands over a `file://` URL, a `%f` a plain path,
//! and a shell hands over whatever the user typed. All three arrive at the same
//! place, so the conversion lives here rather than being guessed at each call
//! site — and anything that is not a local file is refused rather than
//! half-understood.

use std::path::PathBuf;

/// The local path behind `argument`, or `None` when it does not name one.
///
/// A plain path is taken as-is. A `file://` URL is percent-decoded, and only
/// an empty or `localhost` authority counts as local: `file://otra-maquina/x`
/// names someone else's filesystem, which Grafita cannot write back to
/// atomically and therefore will not pretend to edit.
#[must_use]
pub fn local_path(argument: &str) -> Option<PathBuf> {
    let Some(rest) = argument.strip_prefix("file://") else {
        return (!argument.is_empty()).then(|| PathBuf::from(argument));
    };
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(index) if matches!(&rest[..index], "localhost") => &rest[index..],
        // A non-empty, non-localhost authority, or no path at all.
        _ => return None,
    };
    let decoded = percent_decode(path)?;
    (!decoded.is_empty()).then(|| PathBuf::from(decoded))
}

/// Decodes `%XX` escapes. Returns `None` for a truncated or non-hex escape
/// rather than passing a malformed name through to the filesystem.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes.get(index + 1..index + 3)?;
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            out.push((high * 16 + low) as u8);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::local_path;

    #[test]
    fn plain_paths_and_local_urls_both_arrive_as_paths() {
        let cases = [
            ("/home/toni/notas.txt", Some("/home/toni/notas.txt")),
            ("relativo/nota", Some("relativo/nota")),
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
        ] {
            assert_eq!(local_path(argument), None, "{argument}");
        }
    }

    #[test]
    fn a_name_that_is_not_utf8_after_decoding_is_refused() {
        // %FF is not valid UTF-8; accepting it would invent a filename.
        assert_eq!(local_path("file:///home/toni/%FF"), None);
    }
}
