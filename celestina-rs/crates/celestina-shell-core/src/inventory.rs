//! What a listing tool answered, and what a panel may show while it is not
//! answering.
//!
//! The summary readings beside this one — [`crate::network`] and
//! [`crate::bluetooth`] — each already distinguish "I looked and there is
//! nothing" from "I could not look". A list has one more state than a summary
//! does: the tool may not be installed at all, and on that machine an empty
//! list is not a failure but the truth, permanently.
//!
//! So three answers, and the rule that a poll which saw nothing publishes the
//! last thing that was seen rather than an invented emptiness. Both inventories
//! want exactly that, which is why it is stated once here instead of twice
//! beside two different parsers.

/// What one run of an external listing tool produced.
///
/// Deliberately not `Option<String>`: the panel behaves differently for a tool
/// that is absent and one that is merely slow, and collapsing the two is what
/// makes a missing program look like a broken one forever.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The program is not installed on this session.
    Missing,
    /// The program exists and this run did not produce usable output — it
    /// exceeded its deadline, failed, or could not be started.
    Unreadable,
    /// It ran and this is what it said. The text may still be empty, hostile
    /// or malformed; that is the parser's problem, not this type's.
    Text(String),
}

impl Answer {
    /// The text, when there is any. Parsers take this; policy takes the enum.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Missing | Self::Unreadable => None,
        }
    }

    /// The same, owned, for a caller that only ever wanted the output — every
    /// reading that predates this distinction and does not need it.
    #[must_use]
    pub fn into_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            Self::Missing | Self::Unreadable => None,
        }
    }
}

/// What one poll concluded about a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading<T> {
    /// The tool this list comes from is not on this machine. Nothing will ever
    /// be listed, so an empty list is a conclusion rather than a gap.
    Unavailable,
    /// This poll could not read the list. Nothing is concluded, and in
    /// particular nothing is concluded about the list being empty.
    Unreadable,
    /// The tool answered. An empty vector here means it listed nothing, which
    /// is a fact and is published as one.
    Listed(Vec<T>),
}

/// What a payload should say about a list, after this poll.
///
/// Four states rather than a vector, because collapsing them is exactly how a
/// panel lies about a list. An empty vector is a claim — "there is nothing" —
/// and a session that has not finished its first poll, or whose tool is not
/// installed, has not made that claim. The surface needs to tell those apart to
/// choose its words, and choosing them is not the surface's decision to make
/// from a length.
#[derive(Debug, PartialEq, Eq)]
pub enum Published<'a, T> {
    /// Nothing conclusive has ever been read. There is no list yet, and in
    /// particular there is not an empty one.
    Pending,
    /// The tool this list comes from is not installed. There will never be a
    /// list, which is a conclusion about the session rather than a gap.
    Unavailable,
    /// This poll read it. An empty slice here is a confirmed empty list.
    Fresh(&'a [T]),
    /// An earlier poll read it and this one could not. The rows are real and
    /// they are older than this poll.
    Held(&'a [T]),
}

// Derived `Copy` would demand `T: Copy`, which is wrong: this borrows its rows
// and never owns one, so it is copyable whatever they are.
impl<T> Clone for Published<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Published<'_, T> {}

impl<'a, T> Published<'a, T> {
    /// The protocol token. What a person reads is the surface's business; what
    /// state the reading is in is not.
    #[must_use]
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Unavailable => "unavailable",
            Self::Fresh(_) => "fresh",
            Self::Held(_) => "held",
        }
    }

    /// The rows, when there are any to publish. `None` is the absence of a
    /// list, never an empty one.
    #[must_use]
    pub fn rows(self) -> Option<&'a [T]> {
        match self {
            Self::Fresh(rows) | Self::Held(rows) => Some(rows),
            Self::Pending | Self::Unavailable => None,
        }
    }

    /// Whether this state carries a list a surface can act on. The one thing
    /// worth publishing a provider for, when nothing else about it is known.
    #[must_use]
    pub fn is_conclusive(self) -> bool {
        self.rows().is_some()
    }
}

/// The last conclusive listing, held across polls that could not read it.
///
/// The same rule the link tracker follows, for the same reason: a probe that
/// saw nothing is not evidence that there is nothing, and any number of them
/// still is not. Only a tool that answered replaces a list, and only a tool
/// that is absent withdraws one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held<T> {
    listed: Option<Vec<T>>,
    /// Whether the last conclusive thing learned was that the tool is not
    /// installed. Distinct from an empty list, which is a tool that answered.
    unavailable: bool,
    /// Polls since the last conclusive reading. Reporting only — it never
    /// expires anything.
    unconfirmed: u32,
}

impl<T> Default for Held<T> {
    fn default() -> Self {
        Self {
            listed: None,
            unavailable: false,
            unconfirmed: 0,
        }
    }
}

impl<T> Held<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What to publish after this reading.
    pub fn observe(&mut self, reading: Reading<T>) -> Published<'_, T> {
        match reading {
            // The tool went away, so whatever it last listed describes nothing
            // that is still true. The rows go with it rather than being held
            // against a program that is no longer there to contradict them.
            Reading::Unavailable => {
                self.listed = None;
                self.unavailable = true;
                self.unconfirmed = 0;
            }
            Reading::Listed(rows) => {
                self.listed = Some(rows);
                self.unavailable = false;
                self.unconfirmed = 0;
            }
            // Held, however long this goes on, and it concludes nothing. A
            // reading that is still absent stays absent: an unreadable poll
            // cannot promote "I have never looked" into "there is nothing".
            Reading::Unreadable => self.unconfirmed = self.unconfirmed.saturating_add(1),
        }

        match (self.listed.as_deref(), self.unconfirmed, self.unavailable) {
            (Some(rows), 0, _) => Published::Fresh(rows),
            (Some(rows), _, _) => Published::Held(rows),
            (None, _, true) => Published::Unavailable,
            (None, _, false) => Published::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_yields_text_only_when_there_was_a_run_that_produced_some() {
        assert_eq!(Answer::Text("rows".to_owned()).text(), Some("rows"));
        assert_eq!(Answer::Missing.text(), None);
        assert_eq!(Answer::Unreadable.text(), None);
    }

    /// The defect this contract was rewritten for. A first poll that could not
    /// read anything used to be indistinguishable from a tool that answered
    /// with nothing, and the panel would have shown a confirmed empty list on
    /// the strength of a reading that never happened.
    #[test]
    fn a_first_unreadable_poll_is_not_a_confirmed_empty_list() {
        let mut held: Held<u8> = Held::new();

        let published = held.observe(Reading::Unreadable);
        assert_eq!(published, Published::Pending);
        assert_eq!(published.as_token(), "pending");
        // No rows at all — not an empty list, which would be a claim.
        assert_eq!(published.rows(), None);
        assert!(!published.is_conclusive());

        // And it stays that way however long the tool keeps not answering.
        for _ in 0..1_000 {
            assert_eq!(held.observe(Reading::Unreadable), Published::Pending);
        }
    }

    #[test]
    fn a_tool_that_answered_nothing_publishes_an_empty_list_as_a_fact() {
        let mut held: Held<u8> = Held::new();

        let published = held.observe(Reading::Listed(Vec::new()));
        assert_eq!(published, Published::Fresh(&[]));
        assert_eq!(published.as_token(), "fresh");
        // An empty list that is present, which is the claim "there is nothing".
        assert_eq!(published.rows(), Some(&[][..]));
        assert!(published.is_conclusive());
    }

    /// A tool that is not installed is its own answer, and it is not a timeout.
    #[test]
    fn a_tool_that_is_not_installed_says_so_rather_than_timing_out() {
        let mut held: Held<u8> = Held::new();
        held.observe(Reading::Listed(vec![1, 2]));

        let published = held.observe(Reading::Unavailable);
        assert_eq!(published, Published::Unavailable);
        assert_eq!(published.as_token(), "unavailable");
        // The rows it used to list describe nothing that is still true.
        assert_eq!(published.rows(), None);
        assert!(!published.is_conclusive());

        // An unreadable poll afterwards does not promote it back to pending:
        // the last thing conclusively learned is still that the tool is gone.
        assert_eq!(held.observe(Reading::Unreadable), Published::Unavailable);
    }

    /// The behaviour this type exists for. An unreadable poll must never turn
    /// a list into an empty one, however many of them there are.
    #[test]
    fn an_unreadable_run_holds_a_list_and_says_that_it_is_holding_it() {
        let mut held = Held::new();
        assert_eq!(
            held.observe(Reading::Listed(vec![7, 8])),
            Published::Fresh(&[7, 8])
        );

        for _ in 0..10_000 {
            let published = held.observe(Reading::Unreadable);
            assert_eq!(published, Published::Held(&[7, 8]));
            assert_eq!(published.as_token(), "held");
            assert!(published.is_conclusive());
        }

        // And a reading that did answer replaces it and clears the hold.
        assert_eq!(
            held.observe(Reading::Listed(vec![9])),
            Published::Fresh(&[9])
        );
    }
}
