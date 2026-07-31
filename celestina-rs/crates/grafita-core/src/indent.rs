//! What a document indents with, reported rather than guessed.
//!
//! An editor that assumes four spaces will quietly wreck a tab-indented file
//! the first time someone presses Return. So the document is measured, and the
//! answer is allowed to be "it is inconsistent" or "there is none" — both are
//! true things about real files, and both are more useful than a confident
//! wrong number.
//!
//! Only the leading whitespace of lines that *have* leading whitespace counts.
//! Blank lines say nothing, and whitespace after the first non-space character
//! is alignment, not indentation.

use crate::buffer::TextBuffer;

/// What a document appears to indent with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Indentation {
    /// Tab characters.
    Tabs,
    /// A consistent number of spaces per level.
    Spaces { width: usize },
    /// Both appear, and neither dominates enough to call it.
    Mixed,
    /// Nothing in the document is indented.
    None,
}

impl Indentation {
    /// The text one level of this indentation inserts.
    ///
    /// `Mixed` and `None` have no answer of their own, so they fall back to the
    /// caller's preference rather than inventing one.
    #[must_use]
    pub fn unit(self, fallback_width: usize) -> String {
        match self {
            Self::Tabs => "\t".to_owned(),
            Self::Spaces { width } => " ".repeat(width),
            Self::Mixed | Self::None => " ".repeat(fallback_width),
        }
    }
}

/// Measures what `buffer` indents with.
///
/// A file is called tabs or spaces when that style holds at least four fifths
/// of the indented lines: real files carry the odd stray line, and one of them
/// should not turn a clean answer into `Mixed`.
#[must_use]
pub fn detect(buffer: &TextBuffer) -> Indentation {
    let mut tabbed = 0usize;
    let mut spaced = 0usize;
    // Indexed by width; widths beyond 8 are not a level, they are alignment.
    let mut widths = [0usize; 9];

    for line in buffer.lines() {
        let text = line.text();
        let leading: &str = &text[..text.len() - text.trim_start().len()];
        if leading.is_empty() || leading.len() == text.len() {
            // Not indented, or entirely whitespace — a blank line indents
            // nothing and votes for nothing.
            continue;
        }
        if leading.starts_with('\t') {
            tabbed += 1;
        } else if leading.starts_with(' ') {
            spaced += 1;
            let width = leading.chars().take_while(|c| *c == ' ').count();
            if width < widths.len() {
                widths[width] += 1;
            }
        }
    }

    let indented = tabbed + spaced;
    if indented == 0 {
        return Indentation::None;
    }
    if tabbed * 5 >= indented * 4 {
        return Indentation::Tabs;
    }
    if spaced * 5 >= indented * 4 {
        return match step_width(&widths) {
            Some(width) => Indentation::Spaces { width },
            None => Indentation::Mixed,
        };
    }
    Indentation::Mixed
}

/// The smallest width that divides every observed indent — the step the file
/// climbs by, not merely its most common depth.
fn step_width(widths: &[usize; 9]) -> Option<usize> {
    let mut step = 0usize;
    for (width, count) in widths.iter().enumerate() {
        if *count == 0 || width == 0 {
            continue;
        }
        step = gcd(step, width);
    }
    (step > 0).then_some(step)
}

const fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::{detect, Indentation};
    use crate::buffer::TextBuffer;

    fn measure(text: &str) -> Indentation {
        detect(&TextBuffer::from_text(text))
    }

    #[test]
    fn a_document_reports_the_indentation_it_actually_uses() {
        let cases = [
            ("nada indentado\nsegunda\n", Indentation::None),
            ("", Indentation::None),
            ("a\n\tb\n\t\tc\n", Indentation::Tabs),
            ("a\n  b\n    c\n", Indentation::Spaces { width: 2 }),
            ("a\n    b\n        c\n", Indentation::Spaces { width: 4 }),
            ("a\n   b\n      c\n", Indentation::Spaces { width: 3 }),
            // Half and half is genuinely mixed, and saying so is the honest
            // answer rather than picking a winner.
            ("\ta\n  b\n\tc\n  d\n", Indentation::Mixed),
        ];

        for (text, expected) in cases {
            assert_eq!(measure(text), expected, "{text:?}");
        }
    }

    #[test]
    fn one_stray_line_does_not_overturn_a_consistent_file() {
        let mostly_tabs = "\ta\n\tb\n\tc\n\td\n\te\n\tf\n\tg\n\th\n  i\n";

        assert_eq!(measure(mostly_tabs), Indentation::Tabs);
    }

    #[test]
    fn blank_and_whitespace_only_lines_vote_for_nothing() {
        // Only the two tabbed lines carry an opinion.
        assert_eq!(measure("\ta\n\n   \n\tb\n"), Indentation::Tabs);
        assert_eq!(measure("\n   \n\t\n"), Indentation::None);
    }

    #[test]
    fn the_unit_is_what_one_level_inserts() {
        assert_eq!(Indentation::Tabs.unit(4), "\t");
        assert_eq!(Indentation::Spaces { width: 2 }.unit(4), "  ");
        // No opinion of its own: the caller's preference decides.
        assert_eq!(Indentation::None.unit(4), "    ");
        assert_eq!(Indentation::Mixed.unit(2), "  ");
    }
}
