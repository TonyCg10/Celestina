//! Deciding whether bytes are text, by looking at the bytes.
//!
//! Nothing in this module consults a filename, an extension or a MIME value.
//! A dotfile, a `.rs`, a `.kdl` and a file with no name suffix at all take the
//! same path; only their content decides. The answer is deliberately one of
//! three truths rather than a boolean, so a host can say "this is text I cannot
//! safely map back" instead of pretending the file is binary.

use crate::encoding::{DecodeError, Encoding};
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
            nul_check(bytes).unwrap_or_else(|| classify_utf8(bytes, encoding, complete))
        }
        None => nul_check(bytes).unwrap_or_else(|| classify_utf8(bytes, Encoding::Utf8, complete)),
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

fn classify_utf8(bytes: &[u8], encoding: Encoding, complete: bool) -> Classification {
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
    use super::{classify, BinaryReason, Classification};
    use crate::encoding::{DecodeError, Encoding};

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

    #[test]
    fn malformed_bytes_are_unsupported_rather_than_editable_or_binary() {
        let outcome = classify(&[b'h', b'o', b'l', b'a', 0xFF, 0xFE], true);

        assert!(matches!(
            outcome,
            Classification::UnsupportedEncoding {
                reason: DecodeError::InvalidUtf8 { offset: 4, .. }
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
