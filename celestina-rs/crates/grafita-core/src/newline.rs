//! Line terminators as document data rather than a normalisation target.
//!
//! Each line keeps the terminator it was read with, so a mixed file stays mixed
//! and an untouched open/save cycle is byte-identical. Only text the user
//! inserts has to pick a terminator, and it picks the document's dominant one.

use std::fmt;

/// The terminator that actually closes a line, including "nothing".
///
/// Only the final line of a buffer or fragment carries [`Terminator::None`];
/// that is what distinguishes `a\n` (two lines, the last one empty) from `a`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Terminator {
    /// End of content: no bytes follow this line.
    #[default]
    None,
    /// A single line feed.
    Lf,
    /// A carriage return followed by a line feed.
    CrLf,
    /// A lone carriage return.
    Cr,
}

impl Terminator {
    /// The exact characters this terminator contributes to the text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// The newline this terminator is, when it terminates anything at all.
    #[must_use]
    pub const fn newline(self) -> Option<Newline> {
        match self {
            Self::None => None,
            Self::Lf => Some(Newline::Lf),
            Self::CrLf => Some(Newline::CrLf),
            Self::Cr => Some(Newline::Cr),
        }
    }
}

/// A terminator that really separates two lines.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Newline {
    /// `\n`, the default for a document that has no line break yet.
    #[default]
    Lf,
    /// `\r\n`.
    CrLf,
    /// `\r`.
    Cr,
}

impl Newline {
    /// The terminator form of this newline.
    #[must_use]
    pub const fn terminator(self) -> Terminator {
        match self {
            Self::Lf => Terminator::Lf,
            Self::CrLf => Terminator::CrLf,
            Self::Cr => Terminator::Cr,
        }
    }

    /// The exact characters this newline contributes to the text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.terminator().as_str()
    }
}

impl fmt::Display for Newline {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
            Self::Cr => "CR",
        })
    }
}

/// Counts of each newline form seen while reading a document.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NewlineCounts {
    pub line_feed: usize,
    pub carriage_return_line_feed: usize,
    pub carriage_return: usize,
}

impl NewlineCounts {
    pub fn record(&mut self, newline: Newline) {
        match newline {
            Newline::Lf => self.line_feed += 1,
            Newline::CrLf => self.carriage_return_line_feed += 1,
            Newline::Cr => self.carriage_return += 1,
        }
    }

    /// Whether the document mixes more than one newline form.
    #[must_use]
    pub const fn is_mixed(&self) -> bool {
        let forms = (self.line_feed > 0) as u8
            + (self.carriage_return_line_feed > 0) as u8
            + (self.carriage_return > 0) as u8;
        forms > 1
    }

    /// The newline inserted text adopts.
    ///
    /// The most frequent form wins; ties resolve to LF, then CRLF, then CR, so
    /// the answer never depends on iteration order. A document with no line
    /// break at all adopts LF.
    #[must_use]
    pub const fn dominant(&self) -> Newline {
        let mut best = Newline::Lf;
        let mut best_count = self.line_feed;
        if self.carriage_return_line_feed > best_count {
            best = Newline::CrLf;
            best_count = self.carriage_return_line_feed;
        }
        if self.carriage_return > best_count {
            best = Newline::Cr;
        }
        best
    }
}

/// Splits text into `(content, terminator)` pairs.
///
/// The last pair always carries [`Terminator::None`], so text ending in a
/// newline yields a final empty piece and the round trip stays exact.
pub(crate) fn split_lines(text: &str) -> Vec<(&str, Terminator)> {
    let mut pieces = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        let terminator = match bytes[index] {
            b'\n' => Terminator::Lf,
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => Terminator::CrLf,
            b'\r' => Terminator::Cr,
            _ => {
                index += 1;
                continue;
            }
        };
        pieces.push((&text[start..index], terminator));
        index += terminator.as_str().len();
        start = index;
    }
    pieces.push((&text[start..], Terminator::None));
    pieces
}

#[cfg(test)]
mod tests {
    use super::{split_lines, Newline, NewlineCounts, Terminator};

    #[test]
    fn splitting_keeps_every_terminator_and_a_final_piece() {
        let cases: [(&str, &[(&str, Terminator)]); 6] = [
            ("", &[("", Terminator::None)]),
            ("a", &[("a", Terminator::None)]),
            ("a\n", &[("a", Terminator::Lf), ("", Terminator::None)]),
            (
                "a\r\nb",
                &[("a", Terminator::CrLf), ("b", Terminator::None)],
            ),
            (
                "a\rb\n",
                &[
                    ("a", Terminator::Cr),
                    ("b", Terminator::Lf),
                    ("", Terminator::None),
                ],
            ),
            (
                "\n\r\n",
                &[
                    ("", Terminator::Lf),
                    ("", Terminator::CrLf),
                    ("", Terminator::None),
                ],
            ),
        ];

        for (text, expected) in cases {
            assert_eq!(split_lines(text), expected, "{text:?}");
            let rebuilt: String = split_lines(text)
                .iter()
                .map(|(content, terminator)| format!("{content}{}", terminator.as_str()))
                .collect();
            assert_eq!(rebuilt, text, "{text:?}");
        }
    }

    #[test]
    fn dominance_prefers_the_majority_then_a_fixed_order() {
        let cases = [
            (NewlineCounts::default(), Newline::Lf, false),
            (
                NewlineCounts {
                    line_feed: 1,
                    carriage_return_line_feed: 3,
                    carriage_return: 0,
                },
                Newline::CrLf,
                true,
            ),
            (
                NewlineCounts {
                    line_feed: 2,
                    carriage_return_line_feed: 2,
                    carriage_return: 0,
                },
                Newline::Lf,
                true,
            ),
            (
                NewlineCounts {
                    line_feed: 0,
                    carriage_return_line_feed: 0,
                    carriage_return: 5,
                },
                Newline::Cr,
                false,
            ),
        ];

        for (counts, dominant, mixed) in cases {
            assert_eq!(counts.dominant(), dominant, "{counts:?}");
            assert_eq!(counts.is_mixed(), mixed, "{counts:?}");
        }
    }
}
