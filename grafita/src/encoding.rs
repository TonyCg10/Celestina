//! The statistical half of automatic encoding detection.
//!
//! `grafita-core` decides what is safe: it maps a WHATWG encoding name onto an
//! encoding it can write back byte for byte, and discards the guess unless
//! re-encoding reproduces the file. What it does not carry is the detector,
//! because Siderita's embedded editor shares that crate and has no use for one.
//! This module is Grafita's guesser, handed to the core through `Limits`.

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};

/// Names the encoding `bytes` most resembles, as a WHATWG label.
///
/// `complete` is false for a prefix of a file, so a character cut at the end is
/// not held against the guess. UTF-8 is denied: the core only asks about bytes
/// that already failed to be UTF-8.
pub fn guess(bytes: &[u8], complete: bool) -> Option<&'static str> {
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
    detector.feed(bytes, complete);
    Some(detector.guess(None, Utf8Detection::Deny).name())
}

#[cfg(test)]
mod tests {
    use grafita_core::probe::classify_with;
    use grafita_core::{Classification, Encoding, SingleByte};

    use super::guess;

    #[test]
    fn the_real_detector_reads_a_windows_1252_note_end_to_end() {
        let table = Encoding::SingleByte(SingleByte::Windows1252);
        let note =
            "Test: \u{e9} \u{f1} \u{fc}, se\u{f1}or Mu\u{f1}oz, pingu\u{fc}ino, cami\u{f3}n.\n";
        let bytes = table.encode(note).expect("1252 carries these");

        let Classification::EditableText { encoding } = classify_with(&bytes, true, Some(guess))
        else {
            panic!("a Windows-1252 note must be read as text");
        };
        assert_eq!(encoding.decode(&bytes).as_deref(), Ok(note));
        assert_eq!(encoding.encode(note), Ok(bytes));
    }

    #[test]
    fn utf8_text_is_never_handed_to_the_detector_to_reinterpret() {
        let note = "se\u{f1}or Mu\u{f1}oz\n";
        assert_eq!(
            classify_with(note.as_bytes(), true, Some(guess)).encoding(),
            Some(Encoding::Utf8)
        );
    }
}
