//! What clipboard text is worth syncing: the one rule both directions of
//! the clipboard share, whatever wire carries it.

/// The largest clipboard text either end will accept. A person's copied text
/// does not reach this; a payload that does is a document or a mis-typed binary
/// selection, and syncing it only floods the peer's clipboard history.
pub const MAX_CLIPBOARD_BYTES: usize = 64 * 1024;

/// Whether `text` is clipboard content worth syncing.
///
/// Empty carries nothing. A NUL means some layer decoded bytes that were never
/// text — a lossy UTF-8 decode of an image selection produces exactly that, a
/// string of replacement characters that still carries the original NULs — and
/// such a value must not reach a peer under any framing. Oversized content is
/// refused rather than truncated: half a clipboard is not what was copied.
pub fn is_syncable(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_CLIPBOARD_BYTES && !text.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lossily_decoded_binary() -> String {
        "\u{fffd}PNG\r\n\u{1a}\n\0\0\0\rIHDR\0\0\0@".to_owned()
    }

    #[test]
    fn ordinary_copied_text_is_syncable() {
        assert!(is_syncable("hola mundo"));
        assert!(is_syncable("varias\nlíneas\ty tabuladores"));
        assert!(is_syncable(&"a".repeat(MAX_CLIPBOARD_BYTES)));
    }

    #[test]
    fn an_empty_selection_is_not_syncable() {
        assert!(!is_syncable(""));
    }

    #[test]
    fn content_carrying_a_nul_is_not_syncable() {
        assert!(!is_syncable("\0"));
        assert!(!is_syncable("texto\0con nul"));
        assert!(!is_syncable(&lossily_decoded_binary()));
    }

    #[test]
    fn oversized_content_is_refused_not_truncated() {
        assert!(!is_syncable(&"a".repeat(MAX_CLIPBOARD_BYTES + 1)));
    }
}
