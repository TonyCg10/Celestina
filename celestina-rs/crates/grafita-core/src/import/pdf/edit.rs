//! Correcting a word a PDF already draws.
//!
//! This is the narrowest of the three things "editing a PDF" can mean, and the
//! only one that touches the page. A drawn string is replaced with another,
//! written with the same font, in the same place. Two limits are refusals
//! rather than defects, and both are stated before an edit is accepted:
//!
//! - a character the font has no code for is refused, never written as some
//!   other glyph that happens to be there;
//! - nothing is re-laid-out. Longer text runs past where the old text ended,
//!   because moving what follows would be typesetting the page again, which is
//!   what a word processor does and this is not one.

use std::collections::BTreeMap;

use super::file::Pdf;
use super::object::PdfError;
use super::text::{Extraction, Placement};
use super::update;

/// Why a correction could not be written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditError {
    /// The font drawing this text has no code for a character that was typed.
    NoGlyph { character: char },
    /// The edit crosses two drawn strings, and splitting it between them would
    /// need to know which half belongs to which.
    CrossesStrings,
    /// The document could not be read or written.
    Pdf(PdfError),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGlyph { character } => write!(
                formatter,
                "the font this text is drawn with has no '{character}'"
            ),
            Self::CrossesStrings => formatter.write_str(
                "this change spans two pieces of the page, and Grafita cannot tell which half \
                 belongs to which",
            ),
            Self::Pdf(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for EditError {}

/// Writes `text` back into the document it was extracted from.
///
/// Only the strings whose text differs are rewritten, and each one is written
/// into the content stream it came from. The result is the original file with
/// an incremental update appended.
pub fn apply(pdf: &Pdf, extraction: &Extraction, text: &str) -> Result<Vec<u8>, EditError> {
    let replacements = replacements(pdf, extraction, text)?;
    update::append(pdf, &replacements).map_err(EditError::Pdf)
}

/// The objects a correction rewrites, for a caller that has more to append in
/// the same update — a form field changed in the same save, for instance.
pub fn replacements(
    pdf: &Pdf,
    extraction: &Extraction,
    text: &str,
) -> Result<Vec<(u32, Vec<u8>)>, EditError> {
    if text == extraction.text {
        return Ok(Vec::new());
    }
    let changes = diff(extraction, text)?;
    if changes.is_empty() {
        return Ok(Vec::new());
    }

    // One rewrite per content stream, however many strings inside it changed.
    let mut by_stream: BTreeMap<u32, Vec<(&Placement, String)>> = BTreeMap::new();
    for (placement, replacement) in changes {
        by_stream
            .entry(placement.stream)
            .or_default()
            .push((placement, replacement));
    }

    let mut replacements = Vec::new();
    for (stream, mut edits) in by_stream {
        let object = pdf.object(stream).map_err(EditError::Pdf)?;
        let Some(dictionary) = object.as_dictionary().cloned() else {
            return Err(EditError::Pdf(PdfError::Malformed {
                detail: format!("object {stream} is no longer a stream"),
            }));
        };
        let content = pdf.stream_data(&object).map_err(EditError::Pdf)?;

        // Applied last-first so an earlier span's offsets stay valid.
        edits.sort_by_key(|(placement, _)| std::cmp::Reverse(placement.span.0));
        let mut updated = content;
        for (placement, replacement) in edits {
            let font = extraction
                .fonts
                .get(placement.font)
                .ok_or(EditError::CrossesStrings)?;
            let bytes = font
                .encode(&replacement)
                .map_err(|character| EditError::NoGlyph { character })?;
            updated.splice(placement.span.0..placement.span.1, write_string(&bytes));
        }
        replacements.push((stream, update::stream_object(&dictionary, &updated)));
    }
    Ok(replacements)
}

/// Which drawn strings changed, and what each one now says.
///
/// The comparison is per string rather than per character: a placement either
/// says what it said or it does not. An edit that spans two of them cannot be
/// divided without guessing, so it is refused.
fn diff<'a>(
    extraction: &'a Extraction,
    text: &str,
) -> Result<Vec<(&'a Placement, String)>, EditError> {
    let old = &extraction.text;
    if old.len() == text.len() {
        // Same length: every placement can be compared where it stands.
        let mut changes = Vec::new();
        for placement in &extraction.placements {
            let (start, end) = placement.text;
            let before = &old[start..end];
            let after = text.get(start..end).unwrap_or("");
            if before != after {
                changes.push((placement, after.to_owned()));
            }
        }
        return Ok(changes);
    }

    // Different length: find the one stretch that differs and rewrite every
    // drawn string it touches. A kerned page draws a word as several strings,
    // so requiring the change to fit inside one would refuse almost every real
    // correction. The rule is the one the rest of this checkpoint uses: the new
    // text goes into the first string it touches and the others are emptied.
    let prefix = common_prefix(old, text);
    let suffix = common_suffix(&old[prefix..], &text[prefix..]);
    let old_span = (prefix, old.len() - suffix);
    let new_span = (prefix, text.len() - suffix);

    let touched: Vec<&Placement> = extraction
        .placements
        .iter()
        .filter(|placement| placement.text.0 < old_span.1 && old_span.0 < placement.text.1)
        .collect();
    let (Some(first), Some(last)) = (touched.first(), touched.last()) else {
        return Err(EditError::CrossesStrings);
    };
    // Two strings drawn by different streams cannot be merged: one of them
    // would have to move to the other's page.
    if touched
        .iter()
        .any(|placement| placement.stream != first.stream)
    {
        return Err(EditError::CrossesStrings);
    }

    let mut replacement = String::new();
    replacement.push_str(&old[first.text.0..old_span.0.max(first.text.0)]);
    replacement.push_str(&text[new_span.0..new_span.1]);
    if old_span.1 < last.text.1 {
        replacement.push_str(&old[old_span.1..last.text.1]);
    }

    let mut changes = vec![(*first, replacement)];
    for placement in touched.into_iter().skip(1) {
        changes.push((placement, String::new()));
    }
    Ok(changes)
}

fn common_prefix(left: &str, right: &str) -> usize {
    let mut index = 0;
    for (a, b) in left.chars().zip(right.chars()) {
        if a != b {
            break;
        }
        index += a.len_utf8();
    }
    index
}

fn common_suffix(left: &str, right: &str) -> usize {
    let mut index = 0;
    for (a, b) in left.chars().rev().zip(right.chars().rev()) {
        if a != b {
            break;
        }
        index += a.len_utf8();
    }
    index
}

/// A replacement string as a PDF writes one.
///
/// Written in hexadecimal so no byte needs escaping and no reader has to guess
/// where the string ends.
fn write_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![b'<'];
    for byte in bytes {
        out.extend_from_slice(format!("{byte:02X}").as_bytes());
    }
    out.push(b'>');
    out
}
