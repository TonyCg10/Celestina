//! What counts as an image the suite is willing to open.
//!
//! Cover art reaches this desktop two ways — over KDE Connect from a phone, and
//! from whatever the session's media player wrote to disk — and neither source
//! is trusted. Both answer the same two questions before anything decodes a
//! byte: is it small enough to be a cover rather than a file, and does it start
//! like an image at all.
//!
//! Neither check makes a file safe to decode on its own; they make it *bounded*
//! and *plausible*, which is what keeps a renamed archive or a file-sized
//! declaration out of an image decoder.

/// Covers ordinary embedded artwork while refusing file-sized declarations.
/// Real covers are tens to hundreds of kilobytes; anything past this is not a
/// cover, whatever it claims.
pub const MAX_ARTWORK_BYTES: i64 = 8 * 1024 * 1024;

/// Whether a declared byte count is a plausible cover.
#[must_use]
pub fn is_artwork_size(size: i64) -> bool {
    (1..=MAX_ARTWORK_BYTES).contains(&size)
}

/// Whether these leading bytes are one of the image formats the suite shows.
///
/// PNG, JPEG and WebP cover everything a player or a phone sends in practice.
/// A format not listed here is not rejected because it is dangerous — it is
/// rejected because nothing here has ever needed it, and an unknown header is a
/// better thing to refuse than to hand to a decoder.
#[must_use]
pub fn is_supported_image_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
}

/// How many leading bytes [`is_supported_image_header`] needs.
pub const IMAGE_HEADER_BYTES: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_common_image_signatures_are_accepted() {
        assert!(is_supported_image_header(b"\x89PNG\r\n\x1a\n----"));
        assert!(is_supported_image_header(&[0xff, 0xd8, 0xff, 0xe0]));
        assert!(is_supported_image_header(b"RIFF____WEBPVP8 "));

        assert!(!is_supported_image_header(b"GIF89a"));
        assert!(!is_supported_image_header(b"PK\x03\x04"));
        assert!(!is_supported_image_header(b""));
        // A RIFF container that is not WebP — a wave file, say — is not one.
        assert!(!is_supported_image_header(b"RIFF____WAVEfmt "));
    }

    #[test]
    fn a_cover_is_bounded_at_both_ends() {
        assert!(is_artwork_size(1));
        assert!(is_artwork_size(MAX_ARTWORK_BYTES));

        // Nothing, and file-sized: neither is a cover.
        assert!(!is_artwork_size(0));
        assert!(!is_artwork_size(-1));
        assert!(!is_artwork_size(MAX_ARTWORK_BYTES + 1));
    }
}
