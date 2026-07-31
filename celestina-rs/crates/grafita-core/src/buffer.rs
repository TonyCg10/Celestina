//! The byte- and newline-preserving text buffer.
//!
//! Lines hold their own content and their own terminator, so nothing here
//! normalises the user's file. The buffer's only editing primitive is
//! [`TextBuffer::replace`], which returns the exact fragment it removed; that
//! fragment is what makes undo an exact inverse rather than a re-parse.

use crate::newline::{split_lines, Newline, NewlineCounts, Terminator};
use crate::position::{Position, PositionError, Span};

/// One line: its content without any terminator, plus the terminator that ends
/// it. Only the final line of a buffer or fragment may carry
/// [`Terminator::None`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Line {
    text: String,
    terminator: Terminator,
}

impl Line {
    #[must_use]
    pub fn new(text: impl Into<String>, terminator: Terminator) -> Self {
        Self {
            text: text.into(),
            terminator,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn terminator(&self) -> Terminator {
        self.terminator
    }
}

/// A contiguous piece of a document: what an edit removed, or what it inserted.
///
/// A fragment obeys the same invariant as the buffer — at least one line, and
/// only the last one unterminated — so removing and re-inserting it is lossless
/// even when it starts or ends in the middle of a line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fragment {
    lines: Vec<Line>,
}

impl Fragment {
    /// Builds the fragment a host's text becomes, adopting `newline` for every
    /// line break it contains. This is where "inserted lines use the document's
    /// dominant newline" is enforced, and the only place it is.
    #[must_use]
    pub fn inserted(text: &str, newline: Newline) -> Self {
        let pieces = split_lines(text);
        let last = pieces.len() - 1;
        let lines = pieces
            .into_iter()
            .enumerate()
            .map(|(index, (content, _))| {
                let terminator = if index == last {
                    Terminator::None
                } else {
                    newline.terminator()
                };
                Line::new(content, terminator)
            })
            .collect();
        Self { lines }
    }

    /// Builds a fragment that keeps the terminators it is given, used to record
    /// removed content exactly as it stood.
    fn verbatim(lines: Vec<Line>) -> Self {
        // Every caller builds this from a validated span, which always yields
        // at least the head piece; the assertion documents that invariant
        // without adding a release-time branch.
        debug_assert!(!lines.is_empty());
        Self { lines }
    }

    /// The empty fragment: one empty, unterminated line.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            lines: vec![Line::new("", Terminator::None)],
        }
    }

    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].text.is_empty()
    }

    /// The fragment rendered with its terminators, exactly as it reads on disk.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(&line.text);
            text.push_str(line.terminator.as_str());
        }
        text
    }
}

/// Text as Grafita holds it while editing: lines, their terminators, and the
/// newline that inserted text adopts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextBuffer {
    lines: Vec<Line>,
    counts: NewlineCounts,
    dominant: Newline,
}

impl TextBuffer {
    /// Parses decoded text into lines.
    ///
    /// The dominant newline is decided here and then stays fixed for the
    /// buffer's life: recomputing it while the user types would make the same
    /// keystroke insert different bytes from one moment to the next.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut counts = NewlineCounts::default();
        let lines: Vec<Line> = split_lines(text)
            .into_iter()
            .map(|(content, terminator)| {
                if let Some(newline) = terminator.newline() {
                    counts.record(newline);
                }
                Line::new(content, terminator)
            })
            .collect();
        Self {
            lines,
            counts,
            dominant: counts.dominant(),
        }
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn line(&self, index: usize) -> Option<&Line> {
        self.lines.get(index)
    }

    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The newline inserted text adopts.
    #[must_use]
    pub const fn dominant_newline(&self) -> Newline {
        self.dominant
    }

    /// The newline forms counted when the buffer was parsed.
    #[must_use]
    pub const fn newline_counts(&self) -> NewlineCounts {
        self.counts
    }

    /// The position just past the last character.
    ///
    /// A buffer always holds at least one line — parsing yields a final
    /// unterminated piece and replacement always writes a block back — so the
    /// last index exists.
    #[must_use]
    pub fn end_position(&self) -> Position {
        let line = self.lines.len() - 1;
        Position::new(line, self.lines[line].text.len())
    }

    /// The whole buffer rendered back to text, terminators included.
    #[must_use]
    pub fn to_text(&self) -> String {
        let capacity = self
            .lines
            .iter()
            .map(|line| line.text.len() + line.terminator.as_str().len())
            .sum();
        let mut text = String::with_capacity(capacity);
        for line in &self.lines {
            text.push_str(&line.text);
            text.push_str(line.terminator.as_str());
        }
        text
    }

    /// Checks that a position names a real character boundary.
    pub fn validate(&self, position: Position) -> Result<(), PositionError> {
        let line = self
            .lines
            .get(position.line)
            .ok_or(PositionError::LineOutOfRange {
                line: position.line,
                lines: self.lines.len(),
            })?;
        if position.column > line.text.len() {
            return Err(PositionError::ColumnOutOfRange {
                position,
                line_length: line.text.len(),
            });
        }
        if !line.text.is_char_boundary(position.column) {
            return Err(PositionError::NotCharBoundary { position });
        }
        Ok(())
    }

    /// Moves a position onto the nearest valid one, for hosts that only have a
    /// stale caret to offer. It never fails, which is exactly why editing
    /// entry points do not use it.
    #[must_use]
    pub fn clamp(&self, position: Position) -> Position {
        let line = position.line.min(self.lines.len() - 1);
        let text = &self.lines[line].text;
        let mut column = position.column.min(text.len());
        while !text.is_char_boundary(column) {
            column -= 1;
        }
        Position::new(line, column)
    }

    /// The text inside a span, terminators included.
    pub fn slice(&self, span: Span) -> Result<String, PositionError> {
        Ok(self.extract(span)?.to_text())
    }

    /// Replaces `span` with `fragment` and reports what stood there before.
    ///
    /// Nothing is mutated until both ends validate, so a refusal leaves the
    /// buffer exactly as it was.
    pub fn replace(
        &mut self,
        span: Span,
        fragment: &Fragment,
    ) -> Result<Replacement, PositionError> {
        let removed = self.extract(span)?;
        let start = span.start();
        let end = span.end();

        let head = self.lines[start.line].text[..start.column].to_owned();
        let tail = self.lines[end.line].text[end.column..].to_owned();
        let tail_terminator = self.lines[end.line].terminator;

        let inserted = fragment.lines();
        let last_index = inserted.len() - 1;
        let mut block = Vec::with_capacity(inserted.len());
        if last_index == 0 {
            block.push(Line::new(
                format!("{head}{}{tail}", inserted[0].text),
                tail_terminator,
            ));
        } else {
            block.push(Line::new(
                format!("{head}{}", inserted[0].text),
                inserted[0].terminator,
            ));
            block.extend(inserted[1..last_index].iter().cloned());
            block.push(Line::new(
                format!("{}{tail}", inserted[last_index].text),
                tail_terminator,
            ));
        }

        let end_column = if last_index == 0 {
            head.len() + inserted[0].text.len()
        } else {
            inserted[last_index].text.len()
        };
        let inserted_end = Position::new(start.line + last_index, end_column);

        self.lines.splice(start.line..=end.line, block);
        Ok(Replacement {
            removed,
            inserted_end,
        })
    }

    fn extract(&self, span: Span) -> Result<Fragment, PositionError> {
        self.validate(span.start())?;
        self.validate(span.end())?;
        let start = span.start();
        let end = span.end();

        if start.line == end.line {
            return Ok(Fragment::verbatim(vec![Line::new(
                &self.lines[start.line].text[start.column..end.column],
                Terminator::None,
            )]));
        }

        let mut lines = Vec::with_capacity(end.line - start.line + 1);
        lines.push(Line::new(
            &self.lines[start.line].text[start.column..],
            self.lines[start.line].terminator,
        ));
        lines.extend(self.lines[start.line + 1..end.line].iter().cloned());
        lines.push(Line::new(
            &self.lines[end.line].text[..end.column],
            Terminator::None,
        ));
        Ok(Fragment::verbatim(lines))
    }
}

/// What a [`TextBuffer::replace`] did: the exact content it took out, and where
/// the inserted content ends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Replacement {
    pub removed: Fragment,
    pub inserted_end: Position,
}

#[cfg(test)]
mod tests {
    use super::{Fragment, TextBuffer};
    use crate::newline::{Newline, Terminator};
    use crate::position::{Position, PositionError, Span};

    fn buffer(text: &str) -> TextBuffer {
        TextBuffer::from_text(text)
    }

    #[test]
    fn parsing_and_rendering_round_trip_mixed_newlines() {
        for text in ["", "a", "a\n", "a\r\nb\rc\n", "\n\n\n", "sin salto final"] {
            assert_eq!(buffer(text).to_text(), text, "{text:?}");
        }

        let mixed = buffer("a\r\nb\rc\n");
        assert_eq!(mixed.line_count(), 4);
        assert_eq!(
            mixed.line(0).map(|line| line.terminator()),
            Some(Terminator::CrLf)
        );
        assert_eq!(
            mixed.line(1).map(|line| line.terminator()),
            Some(Terminator::Cr)
        );
        assert_eq!(
            mixed.line(2).map(|line| line.terminator()),
            Some(Terminator::Lf)
        );
        assert_eq!(
            mixed.line(3).map(|line| line.terminator()),
            Some(Terminator::None)
        );
        assert!(mixed.newline_counts().is_mixed());
    }

    #[test]
    fn replacement_reports_the_exact_removed_fragment_and_new_end() {
        let cases: [(&str, Span, &str, &str, &str, Position); 5] = [
            (
                "hola mundo",
                Span::ordered(Position::new(0, 5), Position::new(0, 10)),
                "",
                "hola ",
                "mundo",
                Position::new(0, 5),
            ),
            (
                "uno\ndos\n",
                Span::ordered(Position::new(0, 3), Position::new(1, 0)),
                "",
                "unodos\n",
                "\n",
                Position::new(0, 3),
            ),
            (
                "abc",
                Span::empty(Position::new(0, 1)),
                "XY",
                "aXYbc",
                "",
                Position::new(0, 3),
            ),
            (
                "abc",
                Span::empty(Position::new(0, 3)),
                "\nsiguiente",
                "abc\nsiguiente",
                "",
                Position::new(1, 9),
            ),
            (
                "a\r\nb\r\nc",
                Span::ordered(Position::new(0, 1), Position::new(2, 0)),
                "",
                "ac",
                "\r\nb\r\n",
                Position::new(0, 1),
            ),
        ];

        for (text, span, insert, expected, removed, end) in cases {
            let mut buffer = buffer(text);
            let newline = buffer.dominant_newline();
            let outcome = buffer
                .replace(span, &Fragment::inserted(insert, newline))
                .expect("valid span");

            assert_eq!(buffer.to_text(), expected, "{text:?}");
            assert_eq!(outcome.removed.to_text(), removed, "{text:?}");
            assert_eq!(outcome.inserted_end, end, "{text:?}");
        }
    }

    #[test]
    fn removing_and_reinserting_the_fragment_restores_the_original_bytes() {
        let original = "uno\r\ndos\rtres\ncuatro";
        let span = Span::ordered(Position::new(0, 2), Position::new(3, 3));
        let mut buffer = buffer(original);

        let outcome = buffer
            .replace(span, &Fragment::empty())
            .expect("valid span");
        assert_eq!(buffer.to_text(), "untro");

        buffer
            .replace(Span::empty(outcome.inserted_end), &outcome.removed)
            .expect("valid span");
        assert_eq!(buffer.to_text(), original);
    }

    #[test]
    fn inserted_text_adopts_the_dominant_newline_and_leaves_the_rest_mixed() {
        let mut buffer = buffer("a\r\nb\r\nc\n");
        assert_eq!(buffer.dominant_newline(), Newline::CrLf);

        buffer
            .replace(
                Span::empty(buffer.end_position()),
                &Fragment::inserted("x\ny", buffer.dominant_newline()),
            )
            .expect("valid span");

        assert_eq!(buffer.to_text(), "a\r\nb\r\nc\nx\r\ny");
    }

    #[test]
    fn invalid_positions_are_typed_refusals_that_change_nothing() {
        let mut buffer = buffer("café\nx");
        let newline = buffer.dominant_newline();

        let cases = [
            (
                Position::new(9, 0),
                PositionError::LineOutOfRange { line: 9, lines: 2 },
            ),
            (
                Position::new(0, 99),
                PositionError::ColumnOutOfRange {
                    position: Position::new(0, 99),
                    line_length: 5,
                },
            ),
            (
                Position::new(0, 4),
                PositionError::NotCharBoundary {
                    position: Position::new(0, 4),
                },
            ),
        ];

        for (position, expected) in cases {
            let error = buffer
                .replace(Span::empty(position), &Fragment::inserted("!", newline))
                .expect_err("must refuse");
            assert_eq!(error, expected, "{position}");
        }
        assert_eq!(buffer.to_text(), "café\nx");
    }

    #[test]
    fn clamping_lands_on_a_character_boundary() {
        let buffer = buffer("café\nx");

        assert_eq!(buffer.clamp(Position::new(0, 4)), Position::new(0, 3));
        assert_eq!(buffer.clamp(Position::new(7, 7)), Position::new(1, 1));
        assert_eq!(buffer.clamp(Position::new(0, 0)), Position::START);
    }
}
