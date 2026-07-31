//! Undo, redo and the savepoint that defines "dirty".
//!
//! Every recorded change stores the fragment it removed and the fragment it
//! inserted, so undo replays an exact inverse instead of re-deriving one. The
//! savepoint is pinned to a change's identity rather than to a stack depth: a
//! user who undoes past the last save and then types something different has a
//! dirty document again, even though the stack is the same height.

use std::collections::VecDeque;

use crate::buffer::Fragment;
use crate::position::{Position, Span};

/// How many changes stay undoable. Bounded, because an editing session must
/// not grow without limit while a large file sits open.
pub const DEFAULT_UNDO_LIMIT: usize = 512;

/// Counts applied mutations of a document, undo and redo included.
///
/// A save carries the revision it wrote, so a reply that comes back after a
/// newer keystroke can be recognised as stale instead of clearing fresh work.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Revision(u64);

impl Revision {
    /// The revision a freshly opened document carries.
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// The next revision, saturating rather than wrapping.
    ///
    /// Saturation is safe where wrapping is not: at `u64::MAX` two different
    /// document states would otherwise compare equal and a stale save would be
    /// accepted as current.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// One applied replacement, kept in both directions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    sequence: u64,
    /// Changes sharing a group id were one action to the user and are undone
    /// and redone together. A replace-all is dozens of splices and exactly one
    /// thing the user did.
    group: Option<u64>,
    span_before: Span,
    removed: Fragment,
    inserted: Fragment,
    inserted_end: Position,
    caret_before: Position,
}

impl Change {
    /// The span this change replaced, and therefore the span redo replaces.
    /// The action this change belongs to, when it was part of one.
    #[must_use]
    pub const fn group(&self) -> Option<u64> {
        self.group
    }

    #[must_use]
    pub const fn span_before(&self) -> Span {
        self.span_before
    }

    /// The span the inserted content now occupies, and therefore the span undo
    /// replaces.
    #[must_use]
    pub fn span_after(&self) -> Span {
        Span::ordered(self.span_before.start(), self.inserted_end)
    }

    #[must_use]
    pub const fn removed(&self) -> &Fragment {
        &self.removed
    }

    #[must_use]
    pub const fn inserted(&self) -> &Fragment {
        &self.inserted
    }

    #[must_use]
    pub const fn caret_before(&self) -> Position {
        self.caret_before
    }
}

/// Where the last save left the document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Savepoint {
    /// The document was last saved with nothing on the undo stack.
    AtOrigin,
    /// The document was last saved right after this change.
    AtChange(u64),
    /// The saved state fell off the bounded undo stack. It can no longer be
    /// returned to, so the document must be reported dirty from here on.
    Unreachable,
}

/// The bounded undo/redo stacks plus the savepoint marker.
#[derive(Clone, Debug)]
pub struct History {
    undo: VecDeque<Change>,
    redo: Vec<Change>,
    next_sequence: u64,
    next_group: u64,
    open_group: Option<u64>,
    savepoint: Savepoint,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::with_limit(DEFAULT_UNDO_LIMIT)
    }
}

impl History {
    /// A history bounded to `limit` undoable changes. A limit of zero is
    /// treated as one, so recording a change never immediately discards it.
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            next_sequence: 0,
            next_group: 0,
            open_group: None,
            savepoint: Savepoint::AtOrigin,
            limit: limit.max(1),
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Whether the buffer matches the last saved state.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        match self.savepoint {
            Savepoint::AtOrigin => self.undo.is_empty(),
            Savepoint::AtChange(sequence) => {
                self.undo.back().map(|change| change.sequence) == Some(sequence)
            }
            Savepoint::Unreachable => false,
        }
    }

    /// Records a new change, which discards any redo branch.
    pub fn record(
        &mut self,
        span_before: Span,
        removed: Fragment,
        inserted: Fragment,
        inserted_end: Position,
        caret_before: Position,
    ) {
        self.redo.clear();
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.undo.push_back(Change {
            sequence,
            group: self.open_group,
            span_before,
            removed,
            inserted,
            inserted_end,
            caret_before,
        });
        while self.undo.len() > self.limit {
            let evicted = self.undo.pop_front();
            let lost_savepoint = match (self.savepoint, evicted) {
                (Savepoint::AtOrigin, _) => true,
                (Savepoint::AtChange(marked), Some(change)) => change.sequence == marked,
                _ => false,
            };
            if lost_savepoint {
                self.savepoint = Savepoint::Unreachable;
            }
        }
    }

    /// Opens a group: every change recorded until [`History::close_group`]
    /// undoes and redoes as one action.
    pub fn open_group(&mut self) {
        self.open_group = Some(self.next_group);
        self.next_group = self.next_group.saturating_add(1);
    }

    pub fn close_group(&mut self) {
        self.open_group = None;
    }

    /// The group of the change `take_undo` would return next, so a caller can
    /// keep undoing until the action is fully reversed.
    #[must_use]
    pub fn peek_undo_group(&self) -> Option<u64> {
        self.undo.back().and_then(Change::group)
    }

    /// The same, for the redo direction.
    #[must_use]
    pub fn peek_redo_group(&self) -> Option<u64> {
        self.redo.last().and_then(Change::group)
    }

    /// Takes the change to invert. The caller applies it and then calls
    /// [`History::finish_undo`], so a buffer that refuses the replacement never
    /// desynchronises the stacks.
    pub fn take_undo(&mut self) -> Option<Change> {
        self.undo.pop_back()
    }

    pub fn finish_undo(&mut self, change: Change) {
        self.redo.push(change);
    }

    /// Puts a change back when the caller could not apply it.
    pub fn restore_undo(&mut self, change: Change) {
        self.undo.push_back(change);
    }

    pub fn take_redo(&mut self) -> Option<Change> {
        self.redo.pop()
    }

    pub fn finish_redo(&mut self, change: Change) {
        self.undo.push_back(change);
    }

    pub fn restore_redo(&mut self, change: Change) {
        self.redo.push(change);
    }

    /// Pins the savepoint to the current top of the undo stack.
    pub fn mark_saved(&mut self) {
        self.savepoint = match self.undo.back() {
            Some(change) => Savepoint::AtChange(change.sequence),
            None => Savepoint::AtOrigin,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{History, Revision};
    use crate::buffer::Fragment;
    use crate::newline::Newline;
    use crate::position::{Position, Span};

    fn record(history: &mut History, text: &str) {
        history.record(
            Span::empty(Position::START),
            Fragment::empty(),
            Fragment::inserted(text, Newline::Lf),
            Position::new(0, text.len()),
            Position::START,
        );
    }

    #[test]
    fn revisions_advance_and_never_wrap() {
        assert_eq!(Revision::INITIAL.value(), 0);
        assert_eq!(Revision::INITIAL.next().next().value(), 2);
        assert_eq!(Revision(u64::MAX).next(), Revision(u64::MAX));
    }

    #[test]
    fn a_document_is_clean_at_the_savepoint_and_dirty_either_side_of_it() {
        let mut history = History::default();
        assert!(history.is_clean());

        record(&mut history, "a");
        assert!(!history.is_clean());

        history.mark_saved();
        assert!(history.is_clean());

        record(&mut history, "b");
        assert!(!history.is_clean());

        let change = history.take_undo().expect("one change to undo");
        history.finish_undo(change);
        assert!(history.is_clean());
    }

    #[test]
    fn undoing_past_the_savepoint_and_typing_something_else_stays_dirty() {
        let mut history = History::default();
        record(&mut history, "a");
        history.mark_saved();
        record(&mut history, "b");

        let change = history.take_undo().expect("redoable change");
        history.finish_undo(change);
        let change = history.take_undo().expect("change under the savepoint");
        history.finish_undo(change);
        assert!(!history.is_clean());

        record(&mut history, "c");
        assert!(!history.can_redo());
        assert!(!history.is_clean());
    }

    #[test]
    fn a_savepoint_that_falls_off_the_bounded_stack_keeps_the_document_dirty() {
        let mut history = History::with_limit(2);
        record(&mut history, "a");
        history.mark_saved();

        record(&mut history, "b");
        record(&mut history, "c");
        assert!(!history.is_clean());

        // Undoing back to the same stack height cannot restore the saved
        // bytes, because the change that made them is gone.
        for _ in 0..2 {
            let change = history.take_undo().expect("change");
            history.finish_undo(change);
        }
        assert!(!history.is_clean());
        assert!(!history.can_undo());
    }
}
