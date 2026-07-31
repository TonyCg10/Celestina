//! Caret positions and selections, and the typed refusals they produce.
//!
//! A position is a line index plus a byte offset inside that line's UTF-8
//! content. Byte offsets are exact and cheap; every entry point validates them
//! against the buffer and returns [`PositionError`] instead of panicking, so a
//! host that lags behind an edit cannot bring the document down.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

/// A caret location: `line` indexes the buffer, `column` indexes the line's
/// UTF-8 bytes and must fall on a character boundary.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// The first position of a buffer, which always exists.
    pub const START: Self = Self { line: 0, column: 0 };
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        self.line
            .cmp(&other.line)
            .then_with(|| self.column.cmp(&other.column))
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.line, self.column)
    }
}

/// An ordered region of the buffer, possibly empty.
///
/// Selections arrive from hosts as anchor/head pairs in either direction, so
/// the only constructor sorts its ends. That removes "reversed span" from the
/// error surface entirely rather than leaving it for every caller to check.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Span {
    start: Position,
    end: Position,
}

impl Span {
    /// Builds a span from a selection's two ends, in whichever order they came.
    #[must_use]
    pub fn ordered(anchor: Position, head: Position) -> Self {
        if anchor <= head {
            Self {
                start: anchor,
                end: head,
            }
        } else {
            Self {
                start: head,
                end: anchor,
            }
        }
    }

    /// The empty span at a caret.
    #[must_use]
    pub const fn empty(at: Position) -> Self {
        Self { start: at, end: at }
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Position {
        self.end
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

impl fmt::Display for Span {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}..{}", self.start, self.end)
    }
}

/// Why a position could not be used against the current buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionError {
    /// The line index is past the last line.
    LineOutOfRange { line: usize, lines: usize },
    /// The byte offset is past the end of that line's content.
    ColumnOutOfRange {
        position: Position,
        line_length: usize,
    },
    /// The byte offset splits a multi-byte character.
    NotCharBoundary { position: Position },
}

impl fmt::Display for PositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineOutOfRange { line, lines } => write!(
                formatter,
                "line {line} does not exist in a document of {lines} lines"
            ),
            Self::ColumnOutOfRange {
                position,
                line_length,
            } => write!(
                formatter,
                "position {position} is past the {line_length} bytes of its line"
            ),
            Self::NotCharBoundary { position } => {
                write!(formatter, "position {position} splits a character")
            }
        }
    }
}

impl Error for PositionError {}

#[cfg(test)]
mod tests {
    use super::{Position, Span};

    #[test]
    fn positions_order_by_line_then_column() {
        assert!(Position::new(0, 9) < Position::new(1, 0));
        assert!(Position::new(2, 1) < Position::new(2, 4));
        assert_eq!(Position::START, Position::new(0, 0));
    }

    #[test]
    fn spans_sort_their_ends_whichever_way_the_selection_was_dragged() {
        let forward = Span::ordered(Position::new(1, 2), Position::new(3, 0));
        let backward = Span::ordered(Position::new(3, 0), Position::new(1, 2));

        assert_eq!(forward, backward);
        assert_eq!(forward.start(), Position::new(1, 2));
        assert_eq!(forward.end(), Position::new(3, 0));
        assert!(!forward.is_empty());
        assert!(Span::empty(Position::new(4, 4)).is_empty());
    }
}
