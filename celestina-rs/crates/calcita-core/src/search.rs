//! Walking a document's search hits: which hit Enter, Shift+Enter, F3 and
//! Shift+F3 land on. QtPdf finds the hits; this decides the order.

/// Which way a step walks the hits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Forward,
    Backward,
}

/// One search as the search card asks for it: the text QtPdf looks for.
/// QtPdf's search ignores case and offers no other option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRequest {
    pub query: String,
}

impl SearchRequest {
    /// The request for the text typed in the card, trimmed; blank text
    /// searches nothing.
    #[must_use]
    pub fn new(text: &str) -> Option<Self> {
        let query = text.trim();
        if query.is_empty() {
            return None;
        }
        Some(Self {
            query: query.to_owned(),
        })
    }
}

/// The hit (0-based) a step from `current` lands on among `hits` hits, or
/// `None` when there are none. Without a current hit, or with one past the
/// end of a list that shrank, the walk starts at the first hit going forward
/// and at the last going backward. Without `wrap` the ends stay put.
#[must_use]
pub fn next_hit(
    current: Option<usize>,
    hits: usize,
    direction: Direction,
    wrap: bool,
) -> Option<usize> {
    let last = hits.checked_sub(1)?;
    let fresh = match direction {
        Direction::Forward => 0,
        Direction::Backward => last,
    };
    let Some(current) = current.filter(|&current| current <= last) else {
        return Some(fresh);
    };
    let next = match direction {
        Direction::Forward if current == last => {
            if wrap {
                0
            } else {
                last
            }
        }
        Direction::Forward => current + 1,
        Direction::Backward if current == 0 => {
            if wrap {
                last
            } else {
                0
            }
        }
        Direction::Backward => current - 1,
    };
    Some(next)
}
