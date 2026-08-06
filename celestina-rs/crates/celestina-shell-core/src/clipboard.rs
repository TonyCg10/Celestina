//! What counts as clipboard history worth keeping, and how much of it.
//!
//! Watching the desktop clipboard over `ext-data-control-v1` is the IO layer's
//! business; deciding whether a change belongs in history, and how the list
//! behaves once it does, is this file's. It is pure so every rule is testable
//! without a compositor: a caller hands in the text it read and gets back
//! whether it changed anything.
//!
//! `magnetita_core::clipboard::is_syncable` answers a related but different
//! question — whether text is small enough to hand a *phone* over the network
//! — and its bound exists to protect that peer. This bound protects a local
//! history list from a decode error or an accidental image-as-text selection,
//! so it is wider and lives here rather than being reused from a package about
//! a different transport.

/// A selection this large is not something a person copied on purpose; it is a
/// document, or a lossy decode of something that was never text. Kept ten times
/// generous against the largest plausible paragraph, because the cost of being
/// wrong here is a history entry nobody reads, not a peer's flooded clipboard.
pub const MAX_ENTRY_BYTES: usize = 256 * 1024;
/// How many entries the history keeps. Bounded so persistence stays a small
/// file and a drawer stays a list, not a scrollbar into the past.
pub const MAX_ENTRIES: usize = 200;
/// The most a persisted history may be read back from. A file this shell wrote
/// cannot exceed the two bounds above plus its JSON punctuation; a larger one
/// was written by something else, and reading it whole to find that out is the
/// allocation the bound exists to refuse.
pub const MAX_PERSISTED_BYTES: u64 = (MAX_ENTRIES * (MAX_ENTRY_BYTES + 8)) as u64;

/// The one mime type this suite treats as a signal to remember nothing: the
/// convention a password manager adds to a selection so a clipboard history
/// tool knows not to keep it. Skipping it means never even looking at the
/// text — the safest thing this list can do with a password is not touch it.
pub const SENSITIVE_MIME: &str = "x-kde-passwordManagerHint";

/// Whether `text` is worth remembering at all: not empty, not larger than a
/// person could plausibly have selected on purpose, and not carrying a NUL —
/// the mark of a lossy decode of something that was never text, the same
/// signal `magnetita_core::clipboard::is_syncable` treats as disqualifying.
#[must_use]
pub fn is_recordable(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_ENTRY_BYTES && !text.contains('\0')
}

/// Whether a selection's offered mime types mark it as one this list must
/// never store, regardless of what the text looks like.
#[must_use]
pub fn is_sensitive(mimes: &[String]) -> bool {
    mimes.iter().any(|mime| mime == SENSITIVE_MIME)
}

/// The desktop clipboard's remembered text, most recent first.
///
/// A ring only in the sense that it is bounded: it is a list, not a queue,
/// because copying something already in it must move it to the front rather
/// than adding a second copy — that is what makes re-copying the same thing
/// twice not grow the list.
#[derive(Debug, Default)]
pub struct ClipboardHistory {
    entries: Vec<String>,
}

impl ClipboardHistory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restores a history from what was persisted, oldest-last as saved,
    /// truncating to the current cap — a cap lowered since the file was
    /// written must still be honoured.
    ///
    /// A restored entry passes the same test a newly copied one does. The file
    /// is ordinary state on disk that anything running as this person may have
    /// written, so trusting its contents because this shell wrote it once would
    /// let a bound that holds for every live selection be bypassed by editing a
    /// file. Anything that would not be recorded now is dropped rather than
    /// loaded and re-persisted.
    #[must_use]
    pub fn from_entries(entries: Vec<String>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .filter(|entry| is_recordable(entry))
                .take(MAX_ENTRIES)
                .collect(),
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Records a new selection. Returns whether the visible list changed — a
    /// copy of the text already at the front changes nothing, so the caller
    /// knows not to republish or persist.
    ///
    /// The caller is expected to have already refused what [`is_recordable`]
    /// and [`is_sensitive`] would refuse; this only orders and bounds what it
    /// is given.
    pub fn record(&mut self, text: String) -> bool {
        if self.entries.first() == Some(&text) {
            return false;
        }

        if let Some(position) = self.entries.iter().position(|entry| entry == &text) {
            self.entries.remove(position);
        }
        self.entries.insert(0, text);
        self.entries.truncate(MAX_ENTRIES);
        true
    }

    /// Removes one entry by its current position. Returns whether anything was
    /// there to remove.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.entries.remove(index);
        true
    }

    /// Returns whether there was anything to clear.
    pub fn clear(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        self.entries.clear();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recordable_text_is_bounded_and_never_carries_a_nul() {
        assert!(is_recordable("copied text"));
        assert!(!is_recordable(""));
        assert!(!is_recordable(&"x".repeat(MAX_ENTRY_BYTES + 1)));
        // A lossy decode of an image selection is a string of these.
        assert!(!is_recordable("bad\0decode"));
    }

    #[test]
    fn a_password_managers_hint_marks_a_selection_untouchable() {
        assert!(is_sensitive(&[SENSITIVE_MIME.to_owned()]));
        assert!(is_sensitive(&[
            "text/plain".to_owned(),
            SENSITIVE_MIME.to_owned(),
        ]));
        assert!(!is_sensitive(&["text/plain".to_owned()]));
        assert!(!is_sensitive(&[]));
    }

    #[test]
    fn recording_the_same_text_again_moves_it_to_the_front_not_beside_itself() {
        let mut history = ClipboardHistory::new();
        assert!(history.record("one".to_owned()));
        assert!(history.record("two".to_owned()));
        assert_eq!(history.entries(), ["two", "one"]);

        assert!(history.record("one".to_owned()));
        assert_eq!(history.entries(), ["one", "two"]);
        assert_eq!(history.entries().len(), 2);
    }

    #[test]
    fn recording_the_same_text_twice_in_a_row_is_not_a_change() {
        let mut history = ClipboardHistory::new();
        assert!(history.record("one".to_owned()));
        assert!(!history.record("one".to_owned()));
        assert_eq!(history.entries().len(), 1);
    }

    #[test]
    fn the_list_never_grows_past_its_cap() {
        let mut history = ClipboardHistory::new();
        for index in 0..MAX_ENTRIES + 10 {
            history.record(format!("entry-{index}"));
        }
        assert_eq!(history.entries().len(), MAX_ENTRIES);
        // The newest survives; the oldest was evicted.
        assert_eq!(history.entries()[0], format!("entry-{}", MAX_ENTRIES + 9));
    }

    #[test]
    fn restoring_a_history_honours_the_current_cap_even_if_lowered_since() {
        let saved: Vec<String> = (0..MAX_ENTRIES + 5).map(|n| n.to_string()).collect();
        let history = ClipboardHistory::from_entries(saved);
        assert_eq!(history.entries().len(), MAX_ENTRIES);
    }

    // The state file is ordinary bytes on disk. Trusting them because this
    // shell wrote them once would let the bound that holds for every live
    // selection be bypassed by editing a file.
    #[test]
    fn restoring_a_history_refuses_what_it_would_never_have_recorded() {
        let history = ClipboardHistory::from_entries(vec![
            "keep me".to_owned(),
            String::new(),
            "x".repeat(MAX_ENTRY_BYTES + 1),
            "wide\0load".to_owned(),
            "keep me too".to_owned(),
        ]);

        assert_eq!(history.entries(), ["keep me", "keep me too"]);
    }

    #[test]
    fn removing_and_clearing_report_whether_anything_happened() {
        let mut history = ClipboardHistory::new();
        assert!(!history.remove(0));
        assert!(!history.clear());

        history.record("one".to_owned());
        history.record("two".to_owned());
        assert!(history.remove(1));
        assert_eq!(history.entries(), ["two"]);
        assert!(!history.remove(5));

        assert!(history.clear());
        assert!(history.entries().is_empty());
    }
}
