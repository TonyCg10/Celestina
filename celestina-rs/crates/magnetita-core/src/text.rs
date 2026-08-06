// language-contract: allow-non-english
//
// The tests below cut multi-byte text on a character boundary, which needs
// characters that are not one byte wide. Those literals are international-input
// fixtures, not repository prose.

//! Bounding peer-supplied text at the decode boundary.
//!
//! Every string in a packet is chosen by the peer, and each one ends up in a
//! map key, a log line, a D-Bus property or a label. Bounding them once, where
//! the packet becomes a typed value, is what keeps every layer above from
//! having to re-ask how long a "name" can be.

/// `text` cut to at most `limit` characters, never splitting one.
///
/// Truncation, not rejection: an over-long body is still the notification the
/// person wants to see, and the part that matters is at the front.
pub fn bounded(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((end, _)) => text[..end].to_owned(),
        None => text.to_owned(),
    }
}

/// Whether an identifying value is present and short enough to be used as a
/// map key or path component. An identifier cannot be truncated — that would
/// merge two peers' entries — so an over-long one is refused instead.
pub fn is_bounded_identifier(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.chars().count() <= limit
}

#[cfg(test)]
mod tests {
    use super::{bounded, is_bounded_identifier};

    #[test]
    fn a_short_value_is_returned_unchanged() {
        assert_eq!(bounded("Galaxy S25 Ultra", 64), "Galaxy S25 Ultra");
    }

    #[test]
    fn truncation_counts_characters_and_never_splits_one() {
        // Four-byte characters: a byte-based cut would produce invalid UTF-8.
        let emoji = "\u{1f4f1}".repeat(10);
        assert_eq!(bounded(&emoji, 3).chars().count(), 3);
        assert_eq!(bounded("ñññññ", 2), "ññ");
    }

    #[test]
    fn an_identifier_is_bounded_but_never_shortened() {
        assert!(is_bounded_identifier("abc", 3));
        assert!(!is_bounded_identifier("abcd", 3));
        assert!(!is_bounded_identifier("", 3));
    }
}
