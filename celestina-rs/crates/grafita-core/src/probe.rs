//! Deciding whether bytes are text, by looking at the bytes.
//!
//! Nothing in this module consults a filename, an extension or a MIME value.
//! A dotfile, a `.rs`, a `.kdl` and a file with no name suffix at all take the
//! same path; only their content decides. The answer is deliberately one of
//! three truths rather than a boolean, so a host can say "this is text I cannot
//! safely map back" instead of pretending the file is binary.

use crate::encoding::{DecodeError, Encoding, EncodingGuess};
use crate::import::Imported;

/// How much of a file [`classify`] needs to answer for the whole file.
///
/// A probe reads a prefix so pressing `Space` stays cheap on a large file. The
/// prefix answers "offer the editor?"; opening re-runs the same classification
/// over the complete bytes, and that answer is the authoritative one.
pub const DEFAULT_PROBE_BYTES: usize = 64 * 1024;

/// What a byte stream is, as far as Grafita can prove.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    /// Text in a reversible encoding: editable and safe to save.
    EditableText { encoding: Encoding },
    /// Not text, but a document whose text can be carried in and out of it: a
    /// `.docx`, an `.odt`, an `.epub`, a `.rtf`, a PDF, a `.txt.gz`. Editable,
    /// under the imported contract rather than the byte-preserving one.
    ImportedDocument,
    /// Text-shaped bytes with no reversible mapping yet. Showable, but never
    /// advertised as editable, because saving could not reproduce them.
    UnsupportedEncoding { reason: DecodeError },
    /// Not text.
    Binary { reason: BinaryReason },
}

impl Classification {
    /// Whether this content may be opened for editing.
    ///
    /// True for both kinds of document. A host asking "may I offer the editor"
    /// gets one answer; which contract the document is under is the document's
    /// business, not the question's.
    #[must_use]
    pub const fn is_editable(&self) -> bool {
        matches!(self, Self::EditableText { .. } | Self::ImportedDocument)
    }

    /// The encoding, when there is a reversible one.
    #[must_use]
    pub const fn encoding(&self) -> Option<Encoding> {
        match self {
            Self::EditableText { encoding } => Some(*encoding),
            _ => None,
        }
    }
}

/// Why a byte stream was judged to be non-text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryReason {
    /// A NUL byte appeared. No supported text encoding produces one, and
    /// unmarked UTF-16 lands here rather than being guessed at.
    EmbeddedNul { offset: usize },
    /// Enough non-text control bytes appeared that the stream is not prose,
    /// source or configuration.
    ControlBytes { seen: usize, inspected: usize },
}

/// The percentage of control characters above which a decodable stream is
/// still called binary. Tabs, newlines and form feeds do not count; a handful
/// of stray control characters in a real text file must not disqualify it.
const CONTROL_PERCENT_LIMIT: usize = 5;

/// Classifies a prefix of a file.
///
/// `complete` states whether `bytes` is the entire file. It only affects
/// truncated multi-byte sequences at the very end: in a prefix they are
/// expected, in a complete file they are a genuine encoding failure.
#[must_use]
pub fn classify(bytes: &[u8], complete: bool) -> Classification {
    classify_with(bytes, complete, None)
}

/// [`classify`] with an optional guesser for bytes that are not UTF-8.
///
/// Without one, such bytes are `UnsupportedEncoding`, exactly as in
/// [`classify`]. With one, they may become text in a legacy encoding that
/// reproduces them byte for byte.
#[must_use]
pub fn classify_with(bytes: &[u8], complete: bool, guess: Option<EncodingGuess>) -> Classification {
    // Asked before anything else, because every one of these formats would
    // otherwise be called binary by the very next check and never reach the
    // reader that understands it. The marks are the formats' own first bytes,
    // so this is still content deciding and never a name.
    if Imported::looks_importable(bytes) {
        return Classification::ImportedDocument;
    }
    match Encoding::from_byte_order_mark(bytes) {
        Some(encoding @ (Encoding::Utf16Le | Encoding::Utf16Be)) => {
            classify_utf16(bytes, encoding, complete)
        }
        // Only a UTF-16 mark makes zero bytes legitimate. On every other path a
        // NUL means binary before any decoding is attempted, which is also what
        // keeps unmarked UTF-16 from being silently reinterpreted.
        Some(encoding) => {
            nul_check(bytes).unwrap_or_else(|| classify_utf8(bytes, encoding, complete, guess))
        }
        None => nul_check(bytes)
            .unwrap_or_else(|| classify_utf8(bytes, Encoding::Utf8, complete, guess)),
    }
}

fn nul_check(bytes: &[u8]) -> Option<Classification> {
    bytes
        .iter()
        .position(|byte| *byte == 0)
        .map(|offset| Classification::Binary {
            reason: BinaryReason::EmbeddedNul { offset },
        })
}

fn classify_utf8(
    bytes: &[u8],
    encoding: Encoding,
    complete: bool,
    guess: Option<EncodingGuess>,
) -> Classification {
    match encoding.decode(bytes) {
        Ok(text) => control_verdict(&text, encoding),
        Err(DecodeError::InvalidUtf8 {
            offset,
            truncated: true,
        }) if !complete => {
            // The prefix cut a character in half. Judge the part that is whole.
            match encoding.decode(&bytes[..offset]) {
                Ok(text) => control_verdict(&text, encoding),
                Err(reason) => Classification::UnsupportedEncoding { reason },
            }
        }
        // Not UTF-8, and not marked as anything else: a legacy 8-bit or CJK
        // file. Only a plain file is guessed at; a file that announced UTF-8
        // with a mark and then broke it is damaged, not legacy.
        //
        // A file that already proved itself UTF-8 is never reinterpreted: any
        // non-ASCII byte before the first error means the author wrote UTF-8
        // and something later broke it, and a cut-off final character of a
        // complete file is the same damage. Both are refused as before.
        Err(
            reason @ DecodeError::InvalidUtf8 {
                offset,
                truncated: false,
            },
        ) if encoding == Encoding::Utf8 && bytes[..offset].is_ascii() => {
            match guess.and_then(|guess| Encoding::detect(bytes, complete, guess)) {
                Some(detected) => detected_verdict(bytes, detected, complete),
                None => Classification::UnsupportedEncoding { reason },
            }
        }
        Err(reason) => Classification::UnsupportedEncoding { reason },
    }
}

/// The control-byte verdict for text read through a detected encoding.
fn detected_verdict(bytes: &[u8], encoding: Encoding, complete: bool) -> Classification {
    let usable = (0..=if complete { 0 } else { 3 })
        .map(|cut| bytes.len().saturating_sub(cut))
        .find(|end| encoding.decode(&bytes[..*end]).is_ok())
        .unwrap_or(bytes.len());
    match encoding.decode(&bytes[..usable]) {
        Ok(text) => control_verdict(&text, encoding),
        Err(reason) => Classification::UnsupportedEncoding { reason },
    }
}

fn classify_utf16(bytes: &[u8], encoding: Encoding, complete: bool) -> Classification {
    // A prefix can end between the two halves of a code unit, and a trailing
    // high surrogate can have its pair just past the cut. Neither is a defect
    // of the file, so a prefix judges only the units it holds complete.
    let usable = if complete {
        bytes.len()
    } else {
        trim_partial_utf16(bytes, encoding)
    };
    match encoding.decode(&bytes[..usable]) {
        Ok(text) => control_verdict(&text, encoding),
        Err(reason) => Classification::UnsupportedEncoding { reason },
    }
}

fn trim_partial_utf16(bytes: &[u8], encoding: Encoding) -> usize {
    let mark = encoding.byte_order_mark().len();
    let mut usable = mark + (bytes.len() - mark) / 2 * 2;
    if usable >= mark + 2 {
        let pair = [bytes[usable - 2], bytes[usable - 1]];
        let unit = if encoding == Encoding::Utf16Le {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        };
        if (0xD800..0xDC00).contains(&unit) {
            usable -= 2;
        }
    }
    usable
}

fn control_verdict(text: &str, encoding: Encoding) -> Classification {
    let mut seen = 0usize;
    let mut inspected = 0usize;
    for character in text.chars() {
        inspected += 1;
        if is_stray_control(character) {
            seen += 1;
        }
    }
    if seen * 100 > inspected * CONTROL_PERCENT_LIMIT {
        return Classification::Binary {
            reason: BinaryReason::ControlBytes { seen, inspected },
        };
    }
    Classification::EditableText { encoding }
}

fn is_stray_control(character: char) -> bool {
    match character {
        '\t' | '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{1B}' => false,
        _ => character.is_control(),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, classify_with, BinaryReason, Classification};
    use crate::encoding::{DecodeError, Encoding, EncodingGuess, MultiByte, SingleByte};

    fn editable(bytes: &[u8]) -> Option<Encoding> {
        classify(bytes, true).encoding()
    }

    #[test]
    fn text_is_accepted_whatever_its_shape_would_suggest() {
        let cases: [(&str, &[u8]); 7] = [
            ("empty", b""),
            ("plain note", "una nota\n".as_bytes()),
            ("rust source", b"fn main() {\n    let x = 1;\n}\n"),
            ("json", b"{\n  \"clave\": [1, 2]\n}\n"),
            ("kdl", b"node prop=1 {\n  child\n}\n"),
            ("dotfile body", b"[user]\n\tname = Toni\n"),
            ("no trailing newline", b"solo una linea"),
        ];

        for (label, bytes) in cases {
            assert_eq!(editable(bytes), Some(Encoding::Utf8), "{label}");
        }
    }

    #[test]
    fn a_terminal_capture_full_of_escapes_is_text_and_a_program_is_not() {
        // The escape itself is exempt from the control count, so a coloured log
        // is prose with punctuation as far as the heuristic is concerned. This
        // is pinned because it is the case that most looks like it should fail.
        let log = "\u{1B}[0;32mOK\u{1B}[0m compilado\n\u{1B}[1;31mERROR\u{1B}[0m dos\n";
        assert_eq!(editable(log.as_bytes()), Some(Encoding::Utf8));

        // What the count is actually for: bytes that decode but are not text.
        let program: Vec<u8> = (1..=8u8).cycle().take(64).collect();
        assert!(matches!(
            classify(&program, true),
            Classification::Binary {
                reason: BinaryReason::ControlBytes { .. }
            }
        ));
    }

    #[test]
    fn marked_streams_are_read_from_their_mark() {
        assert_eq!(
            editable(
                &Encoding::Utf8Bom
                    .encode("con marca\n")
                    .expect("Unicode carries this")
            ),
            Some(Encoding::Utf8Bom)
        );
        assert_eq!(
            editable(
                &Encoding::Utf16Le
                    .encode("ancho\n")
                    .expect("Unicode carries this")
            ),
            Some(Encoding::Utf16Le)
        );
        assert_eq!(
            editable(
                &Encoding::Utf16Be
                    .encode("ancho\n")
                    .expect("Unicode carries this")
            ),
            Some(Encoding::Utf16Be)
        );
    }

    #[test]
    fn binaries_and_unmarked_wide_text_are_refused_as_text() {
        assert_eq!(
            classify(b"ELF\0\x02\x01", true),
            Classification::Binary {
                reason: BinaryReason::EmbeddedNul { offset: 3 }
            }
        );
        // UTF-16 without a byte-order mark: readable-looking, but accepting it
        // would be a guess, so it is refused rather than reinterpreted.
        assert_eq!(
            classify(b"h\0o\0l\0a\0", true),
            Classification::Binary {
                reason: BinaryReason::EmbeddedNul { offset: 1 }
            }
        );
        assert!(matches!(
            classify(&[0x01, 0x02, 0x03, 0x04, 0x05, b'a'], true),
            Classification::Binary {
                reason: BinaryReason::ControlBytes { .. }
            }
        ));
    }

    /// EUC-JP text: not UTF-8, and the detector's answer for it is an encoding
    /// Grafita has no reversible table for, so nothing can be concluded.
    const EUC_JP: &[u8] = &[
        0xa4, 0xb3, 0xa4, 0xf3, 0xa4, 0xcb, 0xa4, 0xc1, 0xa4, 0xcf, 0xa1, 0xa2, 0xc0, 0xa4, 0xb3,
        0xa6, 0xa1, 0xa3, 0x0a,
    ];

    #[test]
    fn malformed_bytes_are_unsupported_rather_than_editable_or_binary() {
        let outcome = classify(EUC_JP, true);

        assert!(matches!(
            outcome,
            Classification::UnsupportedEncoding {
                reason: DecodeError::InvalidUtf8 { offset: 0, .. }
            }
        ));
        assert!(!outcome.is_editable());
    }

    #[test]
    fn a_prefix_that_cuts_a_character_is_still_text() {
        // The final character is two bytes wide, so dropping one byte cuts it
        // in half instead of just shortening the text.
        let complete = "año año añ".as_bytes();
        let cut = &complete[..complete.len() - 1];

        assert_eq!(
            classify(cut, false),
            Classification::EditableText {
                encoding: Encoding::Utf8
            }
        );
        assert!(matches!(
            classify(cut, true),
            Classification::UnsupportedEncoding { .. }
        ));
    }

    #[test]
    fn valid_utf8_followed_by_a_stray_byte_is_refused_not_reinterpreted() {
        let mut bytes = "se\u{f1}or Mu\u{f1}oz wrote this in UTF-8"
            .as_bytes()
            .to_vec();
        bytes.push(0xFF);

        assert!(matches!(
            classify(&bytes, true),
            Classification::UnsupportedEncoding {
                reason: DecodeError::InvalidUtf8 {
                    truncated: false,
                    ..
                }
            }
        ));
        // A complete ASCII file that ends in half a character is damaged too.
        assert!(matches!(
            classify(b"plain ascii then \xC3", true),
            Classification::UnsupportedEncoding { .. }
        ));
    }

    #[test]
    fn a_prefix_that_cuts_a_surrogate_pair_is_still_text() {
        let complete = Encoding::Utf16Le.encode("hola 🜲");
        let complete = complete.expect("a Unicode encoding carries every character");
        let cut = &complete[..complete.len() - 2];

        assert_eq!(
            classify(cut, false),
            Classification::EditableText {
                encoding: Encoding::Utf16Le
            }
        );
        assert!(matches!(
            classify(cut, true),
            Classification::UnsupportedEncoding {
                reason: DecodeError::UnpairedSurrogate { .. }
            }
        ));
    }

    /// Accented Latin prose, spelled with escapes so the fixture is pure ASCII
    /// source: `\u{e9} \u{f1} \u{fc}` is "e acute, n tilde, u diaeresis".
    const LATIN_NOTE: &str = "The caf\u{e9} served se\u{f1}or Mu\u{f1}oz a pingu\u{fc}ino \
        cake, and the na\u{ef}ve waiter wrote \u{ab}fa\u{e7}ade\u{bb} on the bill. \
        Quelle \u{e9}t\u{e9} \u{e0} Z\u{fc}rich: \u{bf}qu\u{e9} d\u{ed}a es hoy?\n";

    // Test guessers: the statistical part lives in the host, so these stand in
    // for it by naming the WHATWG encoding the fixture was written in. What is
    // under test is the mapping and the byte-exact check, not the statistics.
    fn says_1252(_: &[u8], _: bool) -> Option<&'static str> {
        Some("windows-1252")
    }
    fn says_koi8_r(_: &[u8], _: bool) -> Option<&'static str> {
        Some("KOI8-R")
    }
    fn says_shift_jis(_: &[u8], _: bool) -> Option<&'static str> {
        Some("Shift_JIS")
    }
    fn says_gbk(_: &[u8], _: bool) -> Option<&'static str> {
        Some("GBK")
    }
    fn says_euc_jp(_: &[u8], _: bool) -> Option<&'static str> {
        Some("EUC-JP")
    }
    fn says_nothing(_: &[u8], _: bool) -> Option<&'static str> {
        None
    }

    #[test]
    fn a_windows_1252_note_is_detected_and_reproduces_its_bytes() {
        let table = Encoding::SingleByte(SingleByte::Windows1252);
        let bytes = table.encode(LATIN_NOTE).expect("1252 carries these");

        let Classification::EditableText { encoding } =
            classify_with(&bytes, true, Some(says_1252))
        else {
            panic!("a Latin note in Windows-1252 must be editable text");
        };
        let text = encoding.decode(&bytes).expect("detected encoding decodes");
        assert!(text.contains("se\u{f1}or Mu\u{f1}oz"), "{text}");
        assert!(!text.contains('\u{FFFD}'));
        assert_eq!(encoding.encode(&text).expect("re-encodes"), bytes);
    }

    #[test]
    fn a_short_latin_sample_with_accents_decodes_as_text_not_mojibake() {
        let table = Encoding::SingleByte(SingleByte::Windows1252);
        let sample =
            "Test: \u{e9} \u{f1} \u{fc}, se\u{f1}or Mu\u{f1}oz, pingu\u{fc}ino, cami\u{f3}n.\n";
        let bytes = table.encode(sample).expect("1252 carries these");

        let encoding = classify_with(&bytes, true, Some(says_1252))
            .encoding()
            .expect("detected as editable");
        assert_eq!(encoding.decode(&bytes).expect("decodes"), sample);
        assert_eq!(encoding.encode(sample).expect("re-encodes"), bytes);
    }

    #[test]
    fn utf8_and_marked_files_are_never_reinterpreted_by_detection() {
        assert_eq!(editable(LATIN_NOTE.as_bytes()), Some(Encoding::Utf8));
        let mut marked = b"\xEF\xBB\xBF".to_vec();
        marked.extend_from_slice(LATIN_NOTE.as_bytes());
        assert_eq!(editable(&marked), Some(Encoding::Utf8Bom));
    }

    #[test]
    fn a_cyrillic_koi8_note_is_detected_without_losing_a_byte() {
        let table = Encoding::SingleByte(SingleByte::Koi8R);
        let note = "Привет, мир! Это небольшая заметка на русском языке.\n";
        let bytes = table.encode(note).expect("KOI8-R carries these");

        let encoding = classify_with(&bytes, true, Some(says_koi8_r))
            .encoding()
            .expect("editable");
        assert_eq!(encoding, Encoding::SingleByte(SingleByte::Koi8R));
        let text = encoding.decode(&bytes).expect("decodes");
        assert_eq!(text, note);
        assert_eq!(encoding.encode(&text).expect("re-encodes"), bytes);
    }

    #[test]
    fn longer_shift_jis_and_gbk_notes_are_detected_as_themselves() {
        let japanese = "\u{4eca}\u{65e5}\u{306f}\u{3044}\u{3044}\u{5929}\u{6c17}\u{3067}\u{3059}\u{306d}\u{3002}\
            \u{660e}\u{65e5}\u{306f}\u{96e8}\u{304c}\u{964d}\u{308b}\u{305d}\u{3046}\u{3067}\u{3059}\u{3002}\
            \u{65e5}\u{672c}\u{8a9e}\u{306e}\u{30e1}\u{30e2}\u{3092}\u{66f8}\u{3044}\u{3066}\u{3044}\u{307e}\u{3059}\u{3002}\n";
        let chinese = "\u{4eca}\u{5929}\u{5929}\u{6c14}\u{5f88}\u{597d}\u{ff0c}\u{6211}\u{4eec}\u{4e00}\u{8d77}\u{53bb}\u{516c}\u{56ed}\u{6563}\u{6b65}\u{3002}\
            \u{8fd9}\u{662f}\u{4e00}\u{4efd}\u{7528}\u{7b80}\u{4f53}\u{4e2d}\u{6587}\u{5199}\u{6210}\u{7684}\u{7b14}\u{8bb0}\u{3002}\n";
        for (label, text, expected, guess) in [
            (
                "shift-jis",
                japanese,
                Encoding::MultiByte(MultiByte::ShiftJis),
                says_shift_jis as EncodingGuess,
            ),
            (
                "gbk",
                chinese,
                Encoding::MultiByte(MultiByte::Gbk),
                says_gbk as EncodingGuess,
            ),
        ] {
            let bytes = expected.encode(text).expect("the encoding carries these");
            let found = classify_with(&bytes, true, Some(guess)).encoding();
            assert_eq!(found, Some(expected), "{label}");
            assert_eq!(expected.decode(&bytes).as_deref(), Ok(text), "{label}");
        }
    }

    #[test]
    fn without_a_guesser_or_without_a_usable_answer_nothing_is_concluded() {
        let table = Encoding::SingleByte(SingleByte::Windows1252);
        let bytes = table.encode(LATIN_NOTE).expect("1252 carries these");

        // The pre-detection behaviour, exactly: this is what a host that
        // supplies no guesser gets.
        assert!(matches!(
            classify(&bytes, true),
            Classification::UnsupportedEncoding { .. }
        ));
        assert_eq!(
            classify_with(&bytes, true, Some(says_nothing)),
            classify(&bytes, true)
        );
        // An encoding with no reversible table, and EUC-JP bytes.
        assert_eq!(
            classify_with(EUC_JP, true, Some(says_euc_jp)),
            classify(EUC_JP, true)
        );
    }

    #[test]
    fn a_guess_that_would_not_write_the_bytes_back_is_discarded() {
        // 0x81 0x20 is not a valid Shift-JIS sequence, so this guess cannot
        // read the bytes; a wrong guess must fall back to the refusal.
        let bytes = [b'a', 0x81, 0x20, b'z'];
        let refused = classify(&bytes, true);
        let guessed = classify_with(&bytes, true, Some(says_shift_jis));
        assert_eq!(guessed, refused);
    }

    #[test]
    fn a_stray_control_character_does_not_disqualify_real_text() {
        let mut bytes = b"linea uno\n".to_vec();
        bytes.push(0x07);
        bytes.extend_from_slice("y bastante mas texto normal para diluirlo\n".as_bytes());

        assert_eq!(
            classify(&bytes, true),
            Classification::EditableText {
                encoding: Encoding::Utf8
            }
        );
    }
}
