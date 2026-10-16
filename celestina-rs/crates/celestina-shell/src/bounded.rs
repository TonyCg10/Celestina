// Copied from `celestina-rs/crates/celestina-shell-core/src/lib.rs` (the
// `bounded` helper and its tests) and `MAX_TEXT_UNITS` from `snapshot.rs` at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9, adapted: the reason for counting
// UTF-16 code units is restated for an in-process shell, and the test sample
// is English.

//! Truncating hostile or accidental text to a bounded prefix.
//!
//! Everything that crosses into the shell from another process — a window
//! title, a notification body, an application's name — is capped before it is
//! shown or kept. The cap is one rule, owned here, so two modules can never
//! disagree about what a limit counts.

/// The longest free text any one field carries, in UTF-16 code units.
///
/// Titles and names arrive from other processes and are shown; they are capped
/// before they are ever kept. The value is the halted shell's field bound,
/// copied unchanged so every bound derived from it (a notification body, a
/// window title) keeps the meaning it was tested with.
pub const MAX_TEXT_UNITS: usize = 512;

/// Truncates hostile or accidental text to a bounded prefix.
///
/// The limit counts UTF-16 code units. The halted shell counted them because
/// its Qt host revalidated every field with Qt's own string length; this shell
/// keeps the unit so a bound measured against astral-plane text (emoji, which
/// are two units each) means the same thing it always did. Characters are
/// taken whole, so neither a multi-byte boundary nor a surrogate pair is ever
/// split, and the result can therefore be one unit shorter than the limit.
#[must_use]
pub fn bounded(text: &str, limit: usize) -> String {
    let mut kept = String::new();
    let mut units = 0;
    for character in text.chars() {
        units += character.len_utf16();
        if units > limit {
            break;
        }
        kept.push(character);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::bounded;

    #[test]
    fn bounding_counts_characters_not_bytes() {
        // "na\u{ef}ve" carries one two-byte character among single-byte ones.
        assert_eq!(bounded("na\u{ef}ve", 3), "na\u{ef}");
        assert_eq!(bounded("short", 50), "short");
    }

    #[test]
    fn bounding_counts_utf16_units() {
        // Every one of these is a single character and two UTF-16 code units,
        // so a limit of four admits two of them and no more.
        let astral = "\u{1f600}\u{1f600}\u{1f600}";
        assert_eq!(bounded(astral, 4), "\u{1f600}\u{1f600}");
        assert_eq!(bounded(astral, 6), astral);
        // A limit that would land inside a surrogate pair keeps the character
        // out rather than splitting it.
        assert_eq!(bounded(astral, 5), "\u{1f600}\u{1f600}");
        assert_eq!(bounded(astral, 1), "");
    }
}
