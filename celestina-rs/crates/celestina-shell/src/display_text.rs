//! Text a stranger wrote, made safe to paint.
//!
//! Polkit relays text a local user chose (pkexec builds its message from the
//! caller's argv), and PAM relays text a module chose. Both are hostile input
//! that a prompt then shows in front of a person who is about to type a
//! password, so every such string goes through [`sanitise_display`] before it
//! is shown (CEL-2-I design, section 6.2 row D).
//!
//! Reference set: `fluorita-core::metadata::claimed_text` is the suite's other
//! sanitiser of attacker-sized strings. The set here is a strict superset of
//! its invisible-format list (it adds tag characters, variation selectors,
//! U+FFF9-FFFC, the Hangul and Khmer fillers and the deprecated format
//! characters) and differs in two deliberate ways: a line break becomes a
//! space instead of vanishing, and the cut is by grapheme with an ellipsis.
//! This module is polkit-local only because the owners of that function sit
//! outside the `celestina:` commit roots; converging both on one owner is a
//! later suite unit (design Q14, its second option).
//!
//! The function takes a `&str`, so the order "decode, then sanitise" is
//! enforced by the types: text that must be decoded first (the helper's
//! escaped lines) only becomes a `&str` once it has been decoded and checked
//! as UTF-8, and invalid UTF-8 never reaches this point.

use unicode_segmentation::UnicodeSegmentation;

/// What a cut shows in place of the text it removed.
const ELLIPSIS: char = '\u{2026}';

/// How much of a text may be shown: a count of graphemes and a count of
/// bytes, whichever binds first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    graphemes: usize,
    bytes: usize,
}

impl Limit {
    /// A limit of `graphemes` user-perceived characters and `bytes` UTF-8
    /// bytes, the ellipsis of a cut included in both.
    #[must_use]
    pub const fn new(graphemes: usize, bytes: usize) -> Self {
        Self { graphemes, bytes }
    }

    /// The most graphemes shown.
    #[must_use]
    pub const fn graphemes(self) -> usize {
        self.graphemes
    }

    /// The most bytes shown.
    #[must_use]
    pub const fn bytes(self) -> usize {
        self.bytes
    }
}

/// What the sanitiser does with one character.
enum Class {
    Keep,
    Space,
    Drop,
}

fn classify(character: char) -> Class {
    match character {
        '\n' | '\r' | '\t' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
            Class::Space
        }
        other if other.is_control() || is_invisible(other) => Class::Drop,
        _ => Class::Keep,
    }
}

/// Characters that paint nothing or change how their neighbours read: the
/// bidirectional controls, zero-width and format characters, the fillers that
/// look blank, variation selectors, tag characters and the annotation marks.
const fn is_invisible(character: char) -> bool {
    matches!(
        character,
        '\u{00AD}' // soft hyphen
            | '\u{034F}' // combining grapheme joiner
            | '\u{061C}' // Arabic letter mark
            | '\u{115F}' | '\u{1160}' // Hangul choseong and jungseong fillers
            | '\u{17B4}' | '\u{17B5}' // Khmer inherent vowels
            | '\u{180B}'..='\u{180E}' // Mongolian selectors and separator
            | '\u{200B}'..='\u{200F}' // zero-width characters and directional marks
            | '\u{202A}'..='\u{202E}' // bidirectional embeddings and overrides
            | '\u{2060}'..='\u{206F}' // word joiner, invisible operators, deprecated formats
            | '\u{3164}' // Hangul filler
            | '\u{FE00}'..='\u{FE0F}' // variation selectors
            | '\u{FEFF}' // byte-order mark
            | '\u{FFA0}' // halfwidth Hangul filler
            | '\u{FFF9}'..='\u{FFFC}' // interlinear annotation and object replacement
            | '\u{1D173}'..='\u{1D17A}' // musical format controls
            | '\u{E0000}'..='\u{E007F}' // tag characters
            | '\u{E0100}'..='\u{E01EF}' // variation selectors supplement
    )
}

/// `text` made safe to paint and bounded by `limit`.
///
/// Controls are removed (a line break or tab becomes a space), as are the
/// invisible and direction-changing characters of [`is_invisible`]; the ends
/// are trimmed; and a text longer than `limit` is cut at a grapheme boundary
/// and closed with an ellipsis, so the result never exceeds either count and a
/// combining sequence or a flag is never split. A single grapheme that alone
/// exceeds the byte limit (a base letter under thousands of combining marks)
/// is dropped whole, leaving only the ellipsis. A limit too small to hold the
/// ellipsis yields an empty string when a cut is needed.
///
/// Zero-width joiners are removed, so a joined emoji sequence shows as its
/// parts: a lesser evil than letting the joiner hide characters.
///
/// Work and memory are bounded by the byte limit, not by the length of `text`.
#[must_use]
pub fn sanitise_display(text: &str, limit: Limit) -> String {
    let mut cleaned = String::new();
    let mut overflow = false;
    for character in text.chars() {
        let kept = match classify(character) {
            Class::Keep => character,
            Class::Space => ' ',
            Class::Drop => continue,
        };
        if cleaned.is_empty() && kept.is_whitespace() {
            continue;
        }
        cleaned.push(kept);
        if cleaned.len() > limit.bytes {
            overflow = true;
            break;
        }
    }
    if !overflow {
        cleaned.truncate(cleaned.trim_end().len());
        if cleaned.graphemes(true).count() <= limit.graphemes {
            return cleaned;
        }
    }
    if limit.graphemes == 0 || limit.bytes < ELLIPSIS.len_utf8() {
        return String::new();
    }
    // On overflow the last grapheme of `cleaned` may be incomplete, but it
    // ends past the byte budget, so the loop below never keeps it.
    let max_graphemes = limit.graphemes - 1;
    let max_bytes = limit.bytes - ELLIPSIS.len_utf8();
    let mut kept_end = 0;
    for (count, (start, grapheme)) in cleaned.grapheme_indices(true).enumerate() {
        let end = start + grapheme.len();
        if count >= max_graphemes || end > max_bytes {
            break;
        }
        kept_end = end;
    }
    let mut cut = cleaned[..kept_end].trim_end().to_owned();
    cut.push(ELLIPSIS);
    cut
}

#[cfg(test)]
mod tests {
    use super::{sanitise_display, Limit};

    const WIDE: Limit = Limit::new(1000, 4000);

    fn clean(text: &str) -> String {
        sanitise_display(text, WIDE)
    }

    /// Every character the suite's other sanitiser removes, so this one is a
    /// superset of it (`fluorita-core::metadata::claimed_text`).
    fn claimed_text_set() -> Vec<char> {
        let mut set = vec!['\u{00AD}', '\u{061C}', '\u{180E}', '\u{FEFF}'];
        set.extend('\u{200B}'..='\u{200F}');
        set.extend('\u{202A}'..='\u{202E}');
        set.extend('\u{2060}'..='\u{2064}');
        set.extend('\u{2066}'..='\u{2069}');
        set
    }

    #[test]
    fn removes_bidi_overrides_embeddings_isolates_and_marks() {
        assert_eq!(clean("a\u{202E}b\u{202C}c"), "abc");
        assert_eq!(clean("\u{2066}x\u{2069}\u{200E}\u{200F}\u{061C}y"), "xy");
    }

    #[test]
    fn newline_becomes_a_space_and_other_line_breaks_too() {
        assert_eq!(clean("one\ntwo"), "one two");
        assert_eq!(clean("one\r\ntwo"), "one  two");
        assert_eq!(
            clean("a\tb\u{0B}c\u{0C}d\u{85}e\u{2028}f\u{2029}g"),
            "a b c d e f g"
        );
    }

    #[test]
    fn removes_escape_nul_delete_and_c1_controls() {
        assert_eq!(clean("\u{1b}[31mred\u{1b}[0m"), "[31mred[0m");
        assert_eq!(clean("a\u{0}b\u{7f}c\u{9b}d"), "abcd");
    }

    #[test]
    fn strips_tag_characters() {
        assert_eq!(clean("a\u{E0041}\u{E007F}\u{E0000}b"), "ab");
    }

    #[test]
    fn strips_variation_selectors() {
        assert_eq!(
            clean("a\u{FE00}b\u{FE0F}c\u{E0100}d\u{E01EF}e\u{180B}f"),
            "abcdef"
        );
    }

    #[test]
    fn strips_object_replacement_and_interlinear_marks() {
        assert_eq!(clean("a\u{FFF9}b\u{FFFA}c\u{FFFB}d\u{FFFC}e"), "abcde");
        // The visible replacement character is not invisible and stays.
        assert_eq!(clean("a\u{FFFD}b"), "a\u{FFFD}b");
    }

    #[test]
    fn strips_hangul_fillers_and_other_blank_looking_letters() {
        assert_eq!(clean("a\u{115F}b\u{1160}c\u{3164}d\u{FFA0}e"), "abcde");
        assert_eq!(clean("a\u{17B4}b\u{17B5}c\u{034F}d"), "abcd");
    }

    #[test]
    fn strips_zero_width_and_invisible_format_characters() {
        assert_eq!(
            clean("a\u{200B}b\u{200C}c\u{200D}d\u{2060}e\u{FEFF}f\u{00AD}g"),
            "abcdefg"
        );
        assert_eq!(clean("1\u{2061}2\u{2062}3\u{2063}4\u{2064}5"), "12345");
        assert_eq!(clean("a\u{206A}b\u{206F}c\u{1D173}d"), "abcd");
    }

    #[test]
    fn is_a_superset_of_the_claimed_text_set() {
        for character in claimed_text_set() {
            let text = format!("a{character}b");
            assert_eq!(clean(&text), "ab", "U+{:04X}", u32::from(character));
        }
    }

    #[test]
    fn keeps_ordinary_accented_and_emoji_text() {
        let text =
            "Autenticaci\u{f3}n de \u{d1}and\u{fa} \u{1F600} \u{65E5}\u{672C}\u{8A9E} e\u{301}";
        assert_eq!(clean(text), text);
        // A regional-indicator pair (a flag) is one grapheme and survives whole.
        assert_eq!(clean("\u{1F1EA}\u{1F1F8}"), "\u{1F1EA}\u{1F1F8}");
    }

    #[test]
    fn trims_the_ends_and_an_all_invisible_text_is_empty() {
        assert_eq!(clean("  \u{200B} hello \n"), "hello");
        assert_eq!(clean("\u{202E}\u{200B}\u{E0041}"), "");
        assert_eq!(clean(""), "");
    }

    #[test]
    fn cuts_by_grapheme_at_the_limit_with_an_ellipsis() {
        let limit = Limit::new(4, 100);
        assert_eq!(sanitise_display("abcdef", limit), "abc\u{2026}");
        // Exactly at the limit is not cut.
        assert_eq!(sanitise_display("abcd", limit), "abcd");
        // A combining sequence is one grapheme and is never split.
        assert_eq!(
            sanitise_display("e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}", limit),
            "e\u{301}e\u{301}e\u{301}\u{2026}"
        );
        // A flag is one grapheme and is never split.
        assert_eq!(
            sanitise_display("\u{1F1EA}\u{1F1F8}\u{1F1EA}\u{1F1F8}\u{1F1EA}\u{1F1F8}\u{1F1EA}\u{1F1F8}\u{1F1EA}\u{1F1F8}", limit),
            "\u{1F1EA}\u{1F1F8}\u{1F1EA}\u{1F1F8}\u{1F1EA}\u{1F1F8}\u{2026}"
        );
    }

    #[test]
    fn cuts_by_bytes_when_the_byte_limit_binds_first() {
        // Two bytes per letter: seven bytes leave room for two letters (four
        // bytes) and the three-byte ellipsis.
        let limit = Limit::new(100, 7);
        let out = sanitise_display("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}", limit);
        assert_eq!(out, "\u{e9}\u{e9}\u{2026}");
        assert!(out.len() <= 7);
    }

    #[test]
    fn whitespace_before_the_ellipsis_is_trimmed() {
        assert_eq!(
            sanitise_display("ab cdef", Limit::new(4, 100)),
            "ab\u{2026}"
        );
    }

    #[test]
    fn a_limit_too_small_for_the_ellipsis_yields_nothing() {
        assert_eq!(sanitise_display("abcdef", Limit::new(3, 2)), "");
        assert_eq!(sanitise_display("abcdef", Limit::new(0, 100)), "");
    }

    #[test]
    fn ten_kilobytes_of_input_stay_within_the_limits() {
        let limit = Limit::new(200, 512);
        for filler in ["a", "\u{e9}", "\u{1F600}", "a\u{200B}", "\n", "\u{202E}x"] {
            let big = filler.repeat(10_000);
            let out = sanitise_display(&big, limit);
            assert!(out.len() <= 512, "{filler:?}: {}", out.len());
            assert!(out.chars().count() <= 200, "{filler:?}");
        }
        let big = "a".repeat(10_000);
        let out = sanitise_display(&big, limit);
        assert!(out.ends_with('\u{2026}'));
        assert_eq!(out.chars().count(), 200);
    }

    #[test]
    fn a_combining_storm_is_cut_without_a_dangling_mark() {
        let storm = format!("ok e{}", "\u{301}".repeat(10_000));
        let out = sanitise_display(&storm, Limit::new(300, 512));
        assert!(out.len() <= 512);
        assert!(out.starts_with("ok"));
        assert!(!out.starts_with('\u{301}'));
        // The storm alone: one huge grapheme that does not fit becomes the
        // ellipsis, never a partial grapheme.
        let only = format!("e{}", "\u{301}".repeat(10_000));
        assert_eq!(sanitise_display(&only, Limit::new(300, 512)), "\u{2026}");
    }

    #[test]
    fn output_never_contains_a_removed_character() {
        let hostile = "x\u{202E}\u{200B}\u{E0041}\u{FE0F}\u{FFF9}\u{3164}\n\u{1b}y".repeat(50);
        let out = sanitise_display(&hostile, Limit::new(300, 512));
        assert!(out.chars().all(|c| !c.is_control()));
        assert!(!out.contains(['\u{202E}', '\u{200B}', '\u{E0041}', '\u{FE0F}', '\u{3164}']));
    }
}
