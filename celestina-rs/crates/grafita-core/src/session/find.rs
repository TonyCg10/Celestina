//! Finding and replacing inside the open document.
//!
//! Split out of the session itself because it is the one group of session
//! methods that answers a different question from the rest: the session is
//! about a document's life — open it, edit it, save it, refuse to lose it —
//! while this is about locating text within the document that life already
//! produced. It reads and writes the same private state as its parent, which is
//! why it is a child module rather than a free-standing type: the search cursor
//! only means anything beside the buffer it points into.

use crate::document::Document;
use crate::search::Query;

use super::{DocumentSession, Outcome};

impl DocumentSession {
    /// Sets what to look for and selects the first occurrence.
    pub fn set_search(&mut self, pattern: &str, query: Query) -> Outcome {
        self.search
            .set(pattern, query, self.document.as_ref().map(Document::buffer));
        self.publish_search();
        self.select_current()
    }

    /// Moves to the occurrence after the selected one, wrapping at the end.
    pub fn find_next(&mut self) -> Outcome {
        self.search.step(1);
        self.publish_search();
        self.select_current()
    }

    /// Moves to the occurrence before it, wrapping at the start.
    pub fn find_previous(&mut self) -> Outcome {
        self.search.step(-1);
        self.publish_search();
        self.select_current()
    }

    /// Replaces the selected occurrence and moves to the next one.
    ///
    /// The index is kept rather than advanced: removing a match shifts the
    /// following ones down by one, so staying put *is* moving on.
    pub fn replace_current(&mut self, replacement: &str) -> Outcome {
        let Some(found) = self.search.current() else {
            return Outcome::nothing();
        };
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        if document.replace_match(found, replacement).is_err() {
            return Outcome::nothing();
        }
        self.refresh();
        self.refresh_search();
        let mut outcome = self.push_projection();
        outcome.event = self.select_current().event.or(outcome.event);
        outcome
    }

    /// Replaces every occurrence as a single undoable action.
    pub fn replace_all(&mut self, replacement: &str) -> Outcome {
        let pattern = self.search.pattern().to_owned();
        let query = self.search.query();
        let Some(document) = self.document.as_mut() else {
            return Outcome::nothing();
        };
        if document.replace_all(&pattern, replacement, query).is_err() {
            return Outcome::nothing();
        }
        self.refresh();
        self.search.deselect();
        self.refresh_search();
        self.push_projection()
    }
}
