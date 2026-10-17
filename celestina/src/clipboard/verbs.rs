//! The clipboard verbs of `org.celestina.Shell1`: `clipboard select N`,
//! `remove N`, `clear`, `pause` and `resume`.
//!
//! `select N` and `remove N` act on entry `N` of the history as it is when
//! the worker takes the verb: the socket has no listing verb, so a caller
//! there cannot know any older snapshot, and "the Nth entry now" is the least
//! surprising reading. The Island page instead names the snapshot of the rows
//! it showed ([`ClipboardVerb::SelectShown`], [`ClipboardVerb::RemoveShown`],
//! spelled `select N S` / `remove N S`), and the worker refuses such a verb
//! when the history changed since, rather than acting on whatever entry now
//! sits at that position.
//!
//! The bus answers `(true, "")` as soon as a verb is queued, and no verb
//! returns an entry, a preview, a list or a byte of content (design §5): how
//! a verb ended is an [`Outcome`], logged by the loop, and it names the verb
//! and an index at most. There is no listing verb, on purpose.

use crate::clipboard::ClipboardDuty;
use crate::command::Verb;

/// The longest index a verb spells; the history holds far fewer entries.
const MAX_INDEX_DIGITS: usize = 4;
/// The longest snapshot number a verb spells: any `u64`.
const MAX_SNAPSHOT_DIGITS: usize = 20;

/// One clipboard verb. Indices are the positions the Island page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardVerb {
    /// Make entry `N` of the latest rows the selection again.
    Select(usize),
    /// Make entry `index` of the rows of `snapshot`
    /// ([`crate::clipboard::ClipboardState::snapshot`]) the selection again;
    /// refused by the worker when the history changed since.
    SelectShown { index: usize, snapshot: u64 },
    /// Remove entry `N` of the latest rows.
    Remove(usize),
    /// Remove entry `index` of the rows of `snapshot`; refused when the
    /// history changed since.
    RemoveShown { index: usize, snapshot: u64 },
    /// Empty the history.
    Clear,
    /// Stop reading selections.
    Pause,
    /// Read selections again.
    Resume,
}

impl ClipboardVerb {
    /// The verb the words after `clipboard` spell.
    #[must_use]
    pub fn parse(words: &[&str]) -> Option<Self> {
        match words {
            ["select", index] => index_from_word(index).map(Self::Select),
            ["select", index, snapshot] => Some(Self::SelectShown {
                index: index_from_word(index)?,
                snapshot: snapshot_from_word(snapshot)?,
            }),
            ["remove", index] => index_from_word(index).map(Self::Remove),
            ["remove", index, snapshot] => Some(Self::RemoveShown {
                index: index_from_word(index)?,
                snapshot: snapshot_from_word(snapshot)?,
            }),
            ["clear"] => Some(Self::Clear),
            ["pause"] => Some(Self::Pause),
            ["resume"] => Some(Self::Resume),
            _ => None,
        }
    }

    /// The words that spell the whole verb, `clipboard` included.
    #[must_use]
    pub fn words(self) -> Vec<String> {
        let own = |words: &[&str]| words.iter().map(|w| (*w).to_owned()).collect();
        match self {
            Self::Select(index) => own(&["clipboard", "select", &index.to_string()]),
            Self::Remove(index) => own(&["clipboard", "remove", &index.to_string()]),
            Self::SelectShown { index, snapshot } => own(&[
                "clipboard",
                "select",
                &index.to_string(),
                &snapshot.to_string(),
            ]),
            Self::RemoveShown { index, snapshot } => own(&[
                "clipboard",
                "remove",
                &index.to_string(),
                &snapshot.to_string(),
            ]),
            Self::Clear => own(&["clipboard", "clear"]),
            Self::Pause => own(&["clipboard", "pause"]),
            Self::Resume => own(&["clipboard", "resume"]),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Select(_) | Self::SelectShown { .. } => "select",
            Self::Remove(_) | Self::RemoveShown { .. } => "remove",
            Self::Clear => "clear",
            Self::Pause => "pause",
            Self::Resume => "resume",
        }
    }
}

/// A bounded run of ASCII digits; anything else is not an index.
fn index_from_word(word: &str) -> Option<usize> {
    if word.is_empty() || word.len() > MAX_INDEX_DIGITS || !word.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    word.parse().ok()
}

/// A bounded run of ASCII digits that fits a `u64`.
fn snapshot_from_word(word: &str) -> Option<u64> {
    if word.is_empty()
        || word.len() > MAX_SNAPSHOT_DIGITS
        || !word.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    word.parse().ok()
}

/// How a clipboard verb ended. Success or failure and a reason, never
/// content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The worker took the verb; whether the entry existed is reported
    /// later as a [`crate::clipboard::worker::ClipboardEvent`].
    Requested(ClipboardVerb),
    /// The duty is off: nothing was done and nothing is kept.
    DutyOff(ClipboardVerb),
    /// The worker did not take the verb.
    NotRequested(ClipboardVerb, String),
}

impl Outcome {
    /// Whether the verb was queued.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        matches!(self, Self::Requested(_))
    }

    /// Writes the outcome to the journal.
    pub fn log(&self) {
        match self {
            Self::Requested(verb) => tracing::info!("clipboard {}: requested", verb.name()),
            Self::DutyOff(verb) => tracing::warn!(
                "clipboard {}: nothing was done: the duty is off (set duties.clipboard to \"on\" in shell.json)",
                verb.name()
            ),
            Self::NotRequested(verb, why) => {
                tracing::warn!("clipboard {}: not requested: {why}", verb.name());
            }
        }
    }
}

/// Runs a clipboard verb; `None` for any other verb.
pub fn run(verb: &Verb, duty: &ClipboardDuty) -> Option<Outcome> {
    let Verb::Clipboard(verb) = verb else {
        return None;
    };
    let verb = *verb;
    Some(match duty.command(verb) {
        Ok(()) => Outcome::Requested(verb),
        Err(Refused::DutyOff) => Outcome::DutyOff(verb),
        Err(Refused::Worker(why)) => Outcome::NotRequested(verb, why),
    })
}

/// Why the duty did not pass a verb to its worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No worker runs.
    DutyOff,
    /// The worker did not take it.
    Worker(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_round_trip() {
        for verb in [
            ClipboardVerb::Select(0),
            ClipboardVerb::Select(1234),
            ClipboardVerb::Remove(7),
            ClipboardVerb::SelectShown {
                index: 2,
                snapshot: 41,
            },
            ClipboardVerb::RemoveShown {
                index: 0,
                snapshot: u64::MAX,
            },
            ClipboardVerb::Clear,
            ClipboardVerb::Pause,
            ClipboardVerb::Resume,
        ] {
            let words = verb.words();
            let rest: Vec<&str> = words[1..].iter().map(String::as_str).collect();
            assert_eq!(ClipboardVerb::parse(&rest), Some(verb));
        }
    }

    #[test]
    fn malformed_verbs_are_refused() {
        for bad in [
            &[][..],
            &["select"],
            &["select", ""],
            &["select", "-1"],
            &["select", "+1"],
            &["select", "1.0"],
            &["select", "one"],
            &["select", "12345"],
            &["select", "99999999999999999999"],
            &["select", "1", "2", "3"],
            &["select", "1", "-2"],
            &["select", "1", "x"],
            &["select", "1", "184467440737095516160"],
            &["remove", "1", ""],
            &["remove"],
            &["clear", "all"],
            &["list"],
            &["show", "0"],
            &["Pause"],
            &["resume", "now"],
        ] {
            assert_eq!(ClipboardVerb::parse(bad), None, "{bad:?}");
        }
    }
}
