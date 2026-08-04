//! The projection a text widget edits, and the way its result comes back.
//!
//! A Qt `TextArea` — and every other toolkit text widget — stores line breaks
//! its own way. Letting one own the document would silently rewrite CRLF files,
//! which is the exact loss this crate exists to prevent. So the widget never
//! owns the text: it is shown a line-feed-only *projection*, and whatever it
//! reports back is reconciled against that projection into the one splice that
//! explains the difference.
//!
//! That reconciliation is what keeps the guarantee. Untouched lines never enter
//! the difference, so their terminators are never rewritten, and a newline the
//! user actually typed goes through [`Fragment::inserted`] like any other
//! insertion and adopts the document's dominant terminator.

use crate::buffer::TextBuffer;
use crate::position::{Location, Position, Span};

/// A document rendered for a text widget: every terminator shown as `\n`.
///
/// Line content is untouched, so a byte offset inside a projected line is the
/// same offset inside the buffer's line. Only the terminators differ, and each
/// is exactly one character either way.
#[must_use]
pub fn project(buffer: &TextBuffer) -> String {
    let lines = buffer.lines();
    let capacity = lines.iter().map(|line| line.text().len() + 1).sum();
    let mut text = String::with_capacity(capacity);
    for (index, line) in lines.iter().enumerate() {
        text.push_str(line.text());
        if index + 1 < lines.len() {
            text.push('\n');
        }
    }
    text
}

/// Turns a projection offset into a buffer position.
///
/// Offsets past the end clamp to the end rather than failing: a widget can
/// report a caret from a moment ago, and refusing to place a caret is worse
/// than placing it at the nearest real spot.
#[must_use]
pub fn position_at(buffer: &TextBuffer, offset: usize) -> Position {
    let mut consumed = 0;
    for (index, line) in buffer.lines().iter().enumerate() {
        let length = line.text().len();
        if offset <= consumed + length {
            return buffer.clamp(Position::new(index, offset - consumed));
        }
        // The projected newline between this line and the next.
        consumed += length + 1;
    }
    buffer.end_position()
}

/// Turns a buffer position into a projection offset.
#[must_use]
pub fn offset_at(buffer: &TextBuffer, position: Position) -> usize {
    let mut offset = 0;
    for line in buffer.lines().iter().take(position.line) {
        offset += line.text().len() + 1;
    }
    offset + position.column
}

/// Turns a buffer position into the UTF-16 code-unit offset Qt's text widgets
/// count in, which is neither a byte offset nor a character index.
#[must_use]
pub fn utf16_offset_at(buffer: &TextBuffer, position: Position) -> usize {
    let lines = buffer.lines();
    let mut offset = 0;
    for line in lines.iter().take(position.line) {
        offset += line.text().encode_utf16().count() + 1;
    }
    match lines.get(position.line) {
        Some(line) => {
            let column = position.column.min(line.text().len());
            offset + line.text()[..column].encode_utf16().count()
        }
        None => offset,
    }
}

/// Turns a Qt caret offset into the location a status line reports.
///
/// The inverse of [`utf16_offset_at`], and the only correct way to answer
/// "which line and column is the caret on" from what a Qt widget knows. Doing
/// it in the host would be a second, quietly different implementation of the
/// same rule, and one of the two would be wrong on non-ASCII text.
///
/// The column is counted in **characters**, not in the UTF-8 bytes
/// [`Position`] uses. Byte columns are exact for editing and wrong to show a
/// person: an accented letter would read as two columns.
///
/// Both numbers count from one, because that is what a caret readout means by
/// line 1, column 1. An offset past the end clamps to the end, for the same
/// reason [`position_at`] does.
///
/// Cost is proportional to the offset, not to the document: the walk stops at
/// the caret. That matches the projection this crate already rebuilds per
/// edit, so it adds no new order of work to a keystroke.
#[must_use]
pub fn location_at_utf16(buffer: &TextBuffer, offset: usize) -> Location {
    let lines = buffer.lines();
    let mut consumed = 0;
    for (index, line) in lines.iter().enumerate() {
        let units = line.text().encode_utf16().count();
        if offset <= consumed + units {
            let into_line = offset - consumed;
            // Walk the line's characters until their UTF-16 units reach the
            // caret. A caret cannot land inside a surrogate pair, but one
            // reported from before an edit could, so the walk stops at the
            // first character that reaches or passes it rather than assuming
            // it will land exactly.
            let mut units_seen = 0;
            let mut characters = 0;
            for character in line.text().chars() {
                if units_seen >= into_line {
                    break;
                }
                units_seen += character.len_utf16();
                characters += 1;
            }
            return Location {
                line: index + 1,
                column: characters + 1,
            };
        }
        // The projected newline between this line and the next.
        consumed += units + 1;
    }
    let end = buffer.end_position();
    Location {
        line: end.line + 1,
        column: lines
            .get(end.line)
            .map_or(0, |line| line.text().chars().count())
            + 1,
    }
}

/// The single replacement that turns `current` into `proposed`.
///
/// The common prefix and suffix are trimmed on character boundaries, so what
/// remains is the smallest region that genuinely changed. Identical texts
/// produce `None`, which is what makes pushing the document's own projection
/// back into a widget a no-op instead of a recorded edit.
#[must_use]
pub fn reconcile(buffer: &TextBuffer, current: &str, proposed: &str) -> Option<Edit> {
    if current == proposed {
        return None;
    }

    // The prefix is measured in bytes and must be pulled back to a character
    // boundary before anything slices there. Both strings share those bytes, so
    // a boundary in one is a boundary in the other.
    let mut prefix = common_prefix(current, proposed);
    while prefix > 0 && !current.is_char_boundary(prefix) {
        prefix -= 1;
    }

    let limit = current.len().min(proposed.len()) - prefix;
    let mut suffix = common_suffix(&current[prefix..], &proposed[prefix..], limit);
    while suffix > 0
        && (!current.is_char_boundary(current.len() - suffix)
            || !proposed.is_char_boundary(proposed.len() - suffix))
    {
        suffix -= 1;
    }

    Some(Edit {
        span: Span::ordered(
            position_at(buffer, prefix),
            position_at(buffer, current.len() - suffix),
        ),
        text: proposed[prefix..proposed.len() - suffix].to_owned(),
        caret_offset: proposed.len() - suffix,
    })
}

/// The replacement [`reconcile`] derived, in buffer coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    /// The region of the document the widget replaced.
    pub span: Span,
    /// What it put there, with line breaks still projected as `\n`.
    pub text: String,
    /// Where the caret ends up, as a projection offset.
    pub caret_offset: usize,
}

fn common_prefix(left: &str, right: &str) -> usize {
    let limit = left.len().min(right.len());
    let (left, right) = (left.as_bytes(), right.as_bytes());
    let mut index = 0;
    while index < limit && left[index] == right[index] {
        index += 1;
    }
    index
}

fn common_suffix(left: &str, right: &str, limit: usize) -> usize {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    let mut count = 0;
    while count < limit && left[left.len() - 1 - count] == right[right.len() - 1 - count] {
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::{location_at_utf16, offset_at, position_at, project, reconcile, utf16_offset_at};
    use crate::buffer::{Fragment, TextBuffer};
    use crate::position::{Position, Span};

    fn buffer(text: &str) -> TextBuffer {
        TextBuffer::from_text(text)
    }

    #[test]
    fn the_projection_shows_every_terminator_as_one_line_feed() {
        let cases = [
            ("uno\r\ndos\rtres\n", "uno\ndos\ntres\n"),
            ("sin salto", "sin salto"),
            ("", ""),
            ("\r\n\r\n", "\n\n"),
        ];

        for (stored, projected) in cases {
            assert_eq!(project(&buffer(stored)), projected, "{stored:?}");
        }
    }

    #[test]
    fn offsets_and_positions_are_inverses_inside_the_projection() {
        let buffer = buffer("uno\r\ndos\rtres");

        for (offset, position) in [
            (0, Position::new(0, 0)),
            (3, Position::new(0, 3)),
            (4, Position::new(1, 0)),
            (7, Position::new(1, 3)),
            (8, Position::new(2, 0)),
            (12, Position::new(2, 4)),
        ] {
            assert_eq!(position_at(&buffer, offset), position, "offset {offset}");
            assert_eq!(offset_at(&buffer, position), offset, "{position}");
        }

        // Past the end clamps rather than failing.
        assert_eq!(position_at(&buffer, 999), buffer.end_position());
    }

    #[test]
    fn utf16_offsets_count_the_units_qt_counts() {
        let buffer = buffer("añ🜲\nx");

        // 'a' is one unit, 'ñ' one, '🜲' two: the line is four UTF-16 units
        // even though it is seven bytes.
        assert_eq!(utf16_offset_at(&buffer, Position::new(0, 0)), 0);
        assert_eq!(utf16_offset_at(&buffer, Position::new(0, 1)), 1);
        assert_eq!(utf16_offset_at(&buffer, Position::new(0, 3)), 2);
        assert_eq!(utf16_offset_at(&buffer, Position::new(0, 7)), 4);
        assert_eq!(utf16_offset_at(&buffer, Position::new(1, 1)), 6);
    }

    #[test]
    fn a_caret_reports_its_line_and_character_column_counted_from_one() {
        let buffer = buffer("one\r\ntwo\rfour");

        for (offset, line, column) in [
            (0, 1, 1),
            (3, 1, 4),
            (4, 2, 1),
            (7, 2, 4),
            (8, 3, 1),
            (12, 3, 5),
        ] {
            let location = location_at_utf16(&buffer, offset);
            assert_eq!(
                (location.line, location.column),
                (line, column),
                "at {offset}"
            );
        }

        // Past the end clamps to the end, like every other caret entry point.
        let end = location_at_utf16(&buffer, 999);
        assert_eq!((end.line, end.column), (3, 5));
    }

    #[test]
    fn the_reported_column_counts_characters_not_bytes_or_utf16_units() {
        // 'a' is one unit, 'λ' one, '🜲' two, and 'λ' alone is two *bytes*: a
        // column measured in either of the other two units would be wrong here.
        // A Greek letter rather than an accented Latin one so the fixture is
        // not mistaken for Spanish prose by the language ratchet.
        let buffer = buffer("aλ🜲x");

        for (offset, column) in [(0, 1), (1, 2), (2, 3), (4, 4), (5, 5)] {
            assert_eq!(
                location_at_utf16(&buffer, offset).column,
                column,
                "at {offset}"
            );
        }
    }

    #[test]
    fn a_caret_offset_inside_a_surrogate_pair_lands_on_a_whole_character() {
        // A host can report a caret from before an edit. Splitting '🜲' must
        // still answer a real column rather than run past it.
        let buffer = buffer("a🜲b");

        assert_eq!(location_at_utf16(&buffer, 2).column, 3);
    }

    #[test]
    fn an_unchanged_projection_reconciles_to_nothing() {
        let buffer = buffer("uno\r\ndos");

        assert_eq!(reconcile(&buffer, "uno\ndos", "uno\ndos"), None);
    }

    #[test]
    fn reconciliation_finds_the_smallest_region_that_changed() {
        let cases: [(&str, &str, &str, Span, &str); 5] = [
            (
                "typing at the end",
                "uno\r\ndos",
                "uno\ndos!",
                Span::empty(Position::new(1, 3)),
                "!",
            ),
            (
                "backspace in the middle",
                "uno\r\ndos",
                "uo\ndos",
                Span::ordered(Position::new(0, 1), Position::new(0, 2)),
                "",
            ),
            (
                "replacing a selection across lines",
                "uno\r\ndos\rtres",
                "uX\ntres",
                Span::ordered(Position::new(0, 1), Position::new(1, 3)),
                "X",
            ),
            (
                "pressing return",
                "uno\r\ndos",
                "uno\n\ndos",
                Span::empty(Position::new(1, 0)),
                "\n",
            ),
            (
                "deleting everything",
                "uno\r\ndos",
                "",
                Span::ordered(Position::START, Position::new(1, 3)),
                "",
            ),
        ];

        for (label, stored, proposed, span, inserted) in cases {
            let buffer = buffer(stored);
            let edit = reconcile(&buffer, &project(&buffer), proposed).expect(label);

            assert_eq!(edit.span, span, "{label}");
            assert_eq!(edit.text, inserted, "{label}");
        }
    }

    #[test]
    fn reconciliation_never_splits_a_character() {
        let buffer = buffer("añañ");
        let edit = reconcile(&buffer, "añañ", "añXañ").expect("an edit");

        assert_eq!(edit.text, "X");
        assert!(buffer.validate(edit.span.start()).is_ok());
        assert!(buffer.validate(edit.span.end()).is_ok());
    }

    #[test]
    fn applying_a_reconciled_edit_leaves_untouched_terminators_alone() {
        let mut buffer = buffer("uno\r\ndos\rtres\n");
        let edit =
            reconcile(&buffer, &project(&buffer), "uno\ndos EDITADO\ntres\n").expect("an edit");

        buffer
            .replace(
                edit.span,
                &Fragment::inserted(&edit.text, buffer.dominant_newline()),
            )
            .expect("apply");

        assert_eq!(buffer.to_text(), "uno\r\ndos EDITADO\rtres\n");
    }

    #[test]
    fn a_typed_newline_adopts_the_dominant_terminator() {
        let mut buffer = buffer("uno\r\ndos\r\n");
        let edit = reconcile(&buffer, &project(&buffer), "uno\ndos\npartido\n").expect("an edit");

        buffer
            .replace(
                edit.span,
                &Fragment::inserted(&edit.text, buffer.dominant_newline()),
            )
            .expect("apply");

        assert_eq!(buffer.to_text(), "uno\r\ndos\r\npartido\r\n");
    }
}
