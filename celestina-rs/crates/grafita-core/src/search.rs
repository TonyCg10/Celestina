//! Finding text in a document, and replacing what was found.
//!
//! Deliberately literal. A pattern is the characters the user typed, matched
//! against the buffer's own lines — no regular expressions, no wildcards, and
//! nothing that turns a stray `.` or `*` into a surprise. That is the whole
//! feature: an editor's find box should find what is in the box.
//!
//! Matches never cross a line. The document keeps each line's terminator as its
//! own, so a pattern spanning lines would have to decide which bytes `\n` means
//! before it could match — and the answer differs per line in a mixed file.
//! Searching within lines needs no such decision.
//!
//! Replacement is not a special operation. A replacement is the ordinary splice
//! [`crate::Document::replace`] already performs, so undo, the savepoint and the
//! dirty flag cover it exactly as they cover typing.

use crate::buffer::TextBuffer;
use crate::position::{Position, Span};

/// How a pattern is compared against the text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Query {
    /// Match upper and lower case as the same character.
    pub ignore_case: bool,
    /// Require the match to stand alone rather than sit inside a longer word.
    pub whole_word: bool,
}

/// One found occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Match {
    /// The line the match sits on.
    pub line: usize,
    /// Byte offset of its first character within that line.
    pub start: usize,
    /// Byte offset one past its last character.
    pub end: usize,
}

impl Match {
    /// The region of the document this match covers.
    #[must_use]
    pub fn span(self) -> Span {
        Span::ordered(
            Position::new(self.line, self.start),
            Position::new(self.line, self.end),
        )
    }
}

/// Every occurrence of `pattern`, in document order.
///
/// An empty pattern matches nothing: an editor that reports a hit between every
/// pair of characters is reporting noise, and "next match" would never move.
#[must_use]
pub fn find_all(buffer: &TextBuffer, pattern: &str, query: Query) -> Vec<Match> {
    if pattern.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (index, line) in buffer.lines().iter().enumerate() {
        collect_line(line.text(), pattern, query, index, &mut found);
    }
    found
}

/// How many times `pattern` occurs.
#[must_use]
pub fn count(buffer: &TextBuffer, pattern: &str, query: Query) -> usize {
    find_all(buffer, pattern, query).len()
}

/// The first match at or after `from`, wrapping to the top of the document.
///
/// Wrapping is what makes "next" usable: a search that stops dead at the last
/// match makes the user scroll back by hand to continue.
#[must_use]
pub fn next(buffer: &TextBuffer, pattern: &str, query: Query, from: Position) -> Option<Match> {
    let matches = find_all(buffer, pattern, query);
    matches
        .iter()
        .find(|found| after(**found, from))
        .or_else(|| matches.first())
        .copied()
}

/// The last match strictly before `from`, wrapping to the bottom.
#[must_use]
pub fn previous(buffer: &TextBuffer, pattern: &str, query: Query, from: Position) -> Option<Match> {
    let matches = find_all(buffer, pattern, query);
    matches
        .iter()
        .rev()
        .find(|found| before(**found, from))
        .or_else(|| matches.last())
        .copied()
}

/// Which of `matches` is the one currently at `caret`, if any.
///
/// A host uses this to say "3 de 12" without searching again.
#[must_use]
pub fn index_at(matches: &[Match], caret: Position) -> Option<usize> {
    matches.iter().position(|found| {
        found.line == caret.line && caret.column >= found.start && caret.column <= found.end
    })
}

/// A match starts at or after `position`.
const fn after(found: Match, position: Position) -> bool {
    found.line > position.line || (found.line == position.line && found.start >= position.column)
}

/// A match ends at or before `position`.
const fn before(found: Match, position: Position) -> bool {
    found.line < position.line || (found.line == position.line && found.end <= position.column)
}

/// Finds every occurrence within one line.
fn collect_line(text: &str, pattern: &str, query: Query, line: usize, found: &mut Vec<Match>) {
    // Case folding is done once per line rather than per candidate position.
    // `to_lowercase` can change a string's length, which would make offsets in
    // the folded text meaningless, so the folded form is only used when it maps
    // one byte-length to the same byte-length.
    let (haystack, needle) = match query.ignore_case {
        true => {
            let folded = text.to_lowercase();
            let pattern = pattern.to_lowercase();
            if folded.len() == text.len() {
                (Some(folded), pattern)
            } else {
                // A fold that changes the byte length (ẞ → ss, İ → i̇) cannot be
                // used for offsets. Fall back to the exact comparison rather
                // than report a match at a position that does not exist.
                (None, pattern)
            }
        }
        false => (None, pattern.to_owned()),
    };
    let subject = haystack.as_deref().unwrap_or(text);
    if query.ignore_case && haystack.is_none() {
        // Case-insensitive was asked for but cannot be answered safely on this
        // line; comparing the raw text would silently make it case-sensitive,
        // so this line simply reports nothing.
        return;
    }

    let mut offset = 0;
    while let Some(hit) = subject[offset..].find(&needle) {
        let start = offset + hit;
        let end = start + needle.len();
        // Only report boundaries the original text agrees with, so a caller can
        // splice at them.
        if text.is_char_boundary(start)
            && text.is_char_boundary(end)
            && (!query.whole_word || is_whole_word(subject, start, end))
        {
            found.push(Match { line, start, end });
        }
        // Advance by one character so overlapping occurrences are all seen but
        // an empty step can never loop forever.
        offset = match subject[start..].chars().next() {
            Some(character) => start + character.len_utf8(),
            None => break,
        };
        if offset > subject.len() {
            break;
        }
    }
}

/// Whether the match at `start..end` stands alone rather than inside a word.
fn is_whole_word(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    !before.is_some_and(is_word_character) && !after.is_some_and(is_word_character)
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// The live search a host drives: what to look for, what was found, and which
/// occurrence is selected.
///
/// It is held rather than recomputed per keystroke, and it is rescanned after
/// every edit — matches found before an edit describe a document that no longer
/// exists, and acting on them would splice at the wrong offsets.
#[derive(Clone, Debug, Default)]
pub struct LiveSearch {
    pattern: String,
    query: Query,
    matches: Vec<Match>,
    index: Option<usize>,
}

impl LiveSearch {
    /// Sets what to look for and selects the first occurrence.
    ///
    /// An empty pattern clears the search rather than matching everywhere.
    pub fn set(&mut self, pattern: &str, query: Query, buffer: Option<&TextBuffer>) {
        self.pattern = pattern.to_owned();
        self.query = query;
        self.rescan(buffer);
        self.index = (!self.matches.is_empty()).then_some(0);
    }

    /// Recomputes the matches against the document as it stands now.
    pub fn rescan(&mut self, buffer: Option<&TextBuffer>) {
        self.matches = match (buffer, self.pattern.is_empty()) {
            (Some(buffer), false) => find_all(buffer, &self.pattern, self.query),
            _ => Vec::new(),
        };
        if self.matches.is_empty() {
            self.index = None;
        } else if let Some(index) = self.index {
            self.index = Some(index.min(self.matches.len() - 1));
        }
    }

    /// Moves the selection by `delta` occurrences, wrapping at both ends.
    ///
    /// With nothing selected — after a replace-all, say — stepping selects an
    /// end of the list rather than moving away from an occurrence that was
    /// never selected: forwards lands on the first match, backwards on the
    /// last. Counting from a phantom index zero would skip the first one.
    pub fn step(&mut self, delta: isize) {
        if self.matches.is_empty() {
            self.index = None;
            return;
        }
        let total = self.matches.len() as isize;
        self.index = Some(match self.index {
            Some(current) => (current as isize + delta).rem_euclid(total) as usize,
            None if delta < 0 => self.matches.len() - 1,
            None => 0,
        });
    }

    /// Drops the selection while keeping the pattern.
    pub fn deselect(&mut self) {
        self.index = None;
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.pattern.is_empty()
    }

    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    #[must_use]
    pub const fn query(&self) -> Query {
        self.query
    }

    #[must_use]
    pub fn total(&self) -> usize {
        self.matches.len()
    }

    #[must_use]
    pub const fn index(&self) -> Option<usize> {
        self.index
    }

    /// The occurrence currently selected.
    #[must_use]
    pub fn current(&self) -> Option<Match> {
        self.index
            .and_then(|index| self.matches.get(index).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::{count, find_all, index_at, next, previous, LiveSearch, Match, Query};
    use crate::buffer::TextBuffer;
    use crate::position::Position;

    fn buffer(text: &str) -> TextBuffer {
        TextBuffer::from_text(text)
    }

    fn plain() -> Query {
        Query::default()
    }

    #[test]
    fn a_pattern_is_found_literally_and_in_document_order() {
        let buffer = buffer("uno dos uno\r\ntres uno\n");

        assert_eq!(
            find_all(&buffer, "uno", plain()),
            vec![
                Match {
                    line: 0,
                    start: 0,
                    end: 3
                },
                Match {
                    line: 0,
                    start: 8,
                    end: 11
                },
                Match {
                    line: 1,
                    start: 5,
                    end: 8
                },
            ]
        );
    }

    #[test]
    fn special_characters_are_literal_not_a_pattern_language() {
        let buffer = buffer("a.c abc a*c\n");

        assert_eq!(count(&buffer, ".", plain()), 1);
        assert_eq!(count(&buffer, "a.c", plain()), 1);
        assert_eq!(count(&buffer, "a*c", plain()), 1);
        assert_eq!(count(&buffer, ".*", plain()), 0);
    }

    #[test]
    fn an_empty_pattern_finds_nothing() {
        let buffer = buffer("cualquier cosa\n");

        assert_eq!(find_all(&buffer, "", plain()), vec![]);
    }

    #[test]
    fn a_match_never_crosses_a_line() {
        let buffer = buffer("uno\ndos\n");

        // "uno\ndos" is not a match: the terminator is the line's own, not text.
        assert_eq!(count(&buffer, "uno\ndos", plain()), 0);
    }

    #[test]
    fn case_insensitive_matches_without_moving_the_offsets() {
        let buffer = buffer("Uno UNO uno\n");

        let query = Query {
            ignore_case: true,
            ..Query::default()
        };
        assert_eq!(count(&buffer, "uno", query), 3);
        assert_eq!(count(&buffer, "uno", plain()), 1);
    }

    #[test]
    fn whole_word_refuses_a_match_inside_a_longer_word() {
        let buffer = buffer("un uno unos un_o\n");
        let query = Query {
            whole_word: true,
            ..Query::default()
        };

        assert_eq!(
            find_all(&buffer, "un", query),
            vec![Match {
                line: 0,
                start: 0,
                end: 2
            }]
        );
        assert_eq!(count(&buffer, "un", plain()), 4);
    }

    #[test]
    fn overlapping_occurrences_are_all_reported() {
        let buffer = buffer("aaaa\n");

        assert_eq!(count(&buffer, "aa", plain()), 3);
    }

    #[test]
    fn matches_land_on_character_boundaries_in_multibyte_text() {
        let buffer = buffer("añañaño\n");

        let found = find_all(&buffer, "ña", plain());
        assert_eq!(found.len(), 2);
        for hit in found {
            assert!(buffer.validate(hit.span().start()).is_ok());
            assert!(buffer.validate(hit.span().end()).is_ok());
        }
    }

    #[test]
    fn next_and_previous_move_from_the_caret_and_wrap() {
        let buffer = buffer("uno\ndos uno\nuno\n");
        let all = find_all(&buffer, "uno", plain());
        assert_eq!(all.len(), 3);

        // Forward from the top, then on past the last one, which wraps.
        assert_eq!(next(&buffer, "uno", plain(), Position::START), Some(all[0]));
        assert_eq!(
            next(&buffer, "uno", plain(), Position::new(0, 1)),
            Some(all[1])
        );
        assert_eq!(
            next(&buffer, "uno", plain(), Position::new(2, 1)),
            Some(all[0]),
            "past the last match it wraps to the first"
        );

        // Backward, and wrapping the other way.
        assert_eq!(
            previous(&buffer, "uno", plain(), Position::new(2, 0)),
            Some(all[1])
        );
        assert_eq!(
            previous(&buffer, "uno", plain(), Position::START),
            Some(all[2]),
            "before the first match it wraps to the last"
        );
    }

    #[test]
    fn a_pattern_that_is_absent_has_no_next_or_previous() {
        let buffer = buffer("nada que ver\n");

        assert_eq!(next(&buffer, "ausente", plain(), Position::START), None);
        assert_eq!(previous(&buffer, "ausente", plain(), Position::START), None);
    }

    #[test]
    fn the_caret_reports_which_match_it_sits_in() {
        let buffer = buffer("uno dos uno\n");
        let all = find_all(&buffer, "uno", plain());

        assert_eq!(index_at(&all, Position::new(0, 1)), Some(0));
        assert_eq!(index_at(&all, Position::new(0, 9)), Some(1));
        assert_eq!(index_at(&all, Position::new(0, 5)), None);
    }

    #[test]
    fn stepping_with_nothing_selected_lands_on_an_end_of_the_list() {
        let buffer = buffer("uno dos uno\ntres uno\n");
        let mut search = LiveSearch::default();
        search.set("uno", plain(), Some(&buffer));
        assert_eq!(search.index(), Some(0));

        // Deselected, forwards must find the *first* occurrence again rather
        // than the second.
        search.deselect();
        search.step(1);
        assert_eq!(search.index(), Some(0));

        search.deselect();
        search.step(-1);
        assert_eq!(search.index(), Some(2));
    }
}
