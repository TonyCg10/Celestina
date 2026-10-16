// Copied from `celestina-rs/crates/celestina-shell-core/src/launcher.rs` at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9, adapted: the accented letters
// `fold` maps are written as Unicode escapes with their names, and the test
// fixtures are English names (with escaped accents where a test needs one),
// so the copy carries no non-English prose; the ranking is unchanged.

//! Finding an application by typing part of its name.
//!
//! The index is what the session's `.desktop` files say — read by
//! `celestina_core::desktop_entry`, the same reader the file manager uses — and
//! the matching is the part worth testing: which entries a query names, and in
//! what order a person expects to see them.
//!
//! Nothing here touches the filesystem. A caller hands in the entries it read
//! and gets back the ones that answer, so every rule below is testable without
//! a desktop.

use celestina_core::desktop_entry::DesktopEntry;

/// How well an entry answers a query. Only the ordering matters; the numbers
/// exist to make "why is this first" answerable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Match {
    /// The query is a subsequence of something the entry says.
    Loose,
    /// A word of the entry's name starts with the query.
    WordStart,
    /// The entry's name starts with the query.
    Prefix,
    /// The query is the entry's whole name.
    Exact,
}

/// One entry that answered, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit<'a> {
    pub entry: &'a DesktopEntry,
    pub quality: Match,
}

/// Folds a query or a name into what typing can reasonably be compared to:
/// lowercase, and without the accents a Spanish keyboard makes optional. An
/// application named with an accented vowel and the same name typed without it
/// are the same search.
#[must_use]
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(|character| character.to_lowercase())
        .map(|character| match character {
            // a with acute, grave, diaeresis and circumflex.
            '\u{e1}' | '\u{e0}' | '\u{e4}' | '\u{e2}' => 'a',
            // e with acute, grave, diaeresis and circumflex.
            '\u{e9}' | '\u{e8}' | '\u{eb}' | '\u{ea}' => 'e',
            // i with acute, grave, diaeresis and circumflex.
            '\u{ed}' | '\u{ec}' | '\u{ef}' | '\u{ee}' => 'i',
            // o with acute, grave, diaeresis and circumflex.
            '\u{f3}' | '\u{f2}' | '\u{f6}' | '\u{f4}' => 'o',
            // u with acute, grave, diaeresis and circumflex.
            '\u{fa}' | '\u{f9}' | '\u{fc}' | '\u{fb}' => 'u',
            // n with tilde.
            '\u{f1}' => 'n',
            // c with cedilla.
            '\u{e7}' => 'c',
            other => other,
        })
        .collect()
}

/// Whether `query` appears in `text` in order, letter by letter, with anything
/// between. This is what makes `gimp` find `GNU Image Manipulation Program`.
fn is_subsequence(text: &str, query: &str) -> bool {
    let mut wanted = query.chars();
    let mut next = wanted.next();
    for character in text.chars() {
        match next {
            None => return true,
            Some(target) if target == character => next = wanted.next(),
            Some(_) => {}
        }
    }
    next.is_none()
}

fn quality_of(name: &str, query: &str) -> Option<Match> {
    if name == query {
        return Some(Match::Exact);
    }
    if name.starts_with(query) {
        return Some(Match::Prefix);
    }
    if name
        .split(|character: char| !character.is_alphanumeric())
        .any(|word| !word.is_empty() && word.starts_with(query))
    {
        return Some(Match::WordStart);
    }
    is_subsequence(name, query).then_some(Match::Loose)
}

/// How well one entry answers a query, or `None` when it does not.
///
/// The name is what a person is typing at; everything else — the generic name,
/// the keywords, the command — can only ever produce a loose answer, so a
/// keyword never outranks the application actually called that.
#[must_use]
pub fn score(entry: &DesktopEntry, query: &str) -> Option<Match> {
    let query = fold(query);
    if query.is_empty() {
        return Some(Match::Loose);
    }

    if let Some(quality) = quality_of(&fold(&entry.name), &query) {
        return Some(quality);
    }

    let elsewhere = std::iter::once(&entry.generic_name)
        .chain(std::iter::once(&entry.comment))
        .chain(entry.keywords.iter())
        .chain(std::iter::once(&entry.exec));
    for text in elsewhere {
        if quality_of(&fold(text), &query).is_some() {
            return Some(Match::Loose);
        }
    }

    None
}

/// The entries that answer a query, best first.
///
/// Ties are broken by name so the list never reorders itself between two
/// keystrokes that mean the same thing.
#[must_use]
pub fn rank<'a>(entries: &'a [DesktopEntry], query: &str) -> Vec<Hit<'a>> {
    let mut hits: Vec<Hit<'a>> = entries
        .iter()
        .filter_map(|entry| score(entry, query).map(|quality| Hit { entry, quality }))
        .collect();

    hits.sort_by(|left, right| {
        right
            .quality
            .cmp(&left.quality)
            .then_with(|| fold(&left.entry.name).cmp(&fold(&right.entry.name)))
    });
    hits
}

/// What a bounded query answered, truthful about whether more existed.
///
/// The same contract `siderita::search`'s [`SearchOutcome`] uses for a
/// filesystem walk: a caller must never learn only the count of a truncated
/// list and infer it was complete.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RankOutcome {
    pub ids: Vec<String>,
    /// More entries answered than `limit` allowed through.
    pub truncated: bool,
}

/// Ranks and caps the result at `limit`, in one pass over what
/// [`rank`] already sorted best-first — so truncation always drops the
/// *worst* matches, never an arbitrary suffix.
#[must_use]
pub fn rank_bounded(entries: &[DesktopEntry], query: &str, limit: usize) -> RankOutcome {
    let hits = rank(entries, query);
    let truncated = hits.len() > limit;
    let ids = hits
        .into_iter()
        .take(limit)
        .map(|hit| hit.entry.id.clone())
        .collect();
    RankOutcome { ids, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> DesktopEntry {
        DesktopEntry {
            id: format!("{}.desktop", fold(name)),
            name: name.to_owned(),
            is_application: true,
            ..DesktopEntry::default()
        }
    }

    #[test]
    fn typing_without_accents_still_finds_them() {
        // "Caf\u{e9}" is the name with an e-acute; the query omits the accent.
        let cafe = entry("Caf\u{e9}");

        assert_eq!(score(&cafe, "cafe"), Some(Match::Exact));
        // An upper-case accented query folds the same way.
        assert_eq!(score(&cafe, "CAF\u{c9}"), Some(Match::Exact));
        assert_eq!(score(&cafe, "CA"), Some(Match::Prefix));
    }

    #[test]
    fn letters_in_order_are_enough_to_find_something() {
        let gimp = DesktopEntry {
            name: "GNU Image Manipulation Program".to_owned(),
            ..entry("gimp-placeholder")
        };

        assert_eq!(score(&gimp, "gimp"), Some(Match::Loose));
        assert_eq!(score(&gimp, "image"), Some(Match::WordStart));
        assert_eq!(score(&gimp, "zzz"), None);
    }

    #[test]
    fn what_the_entry_is_called_outranks_what_it_mentions() {
        let files = entry("Files");
        let editor = DesktopEntry {
            keywords: vec!["files".to_owned()],
            ..entry("Zed")
        };

        // Both answer, but only one is called that.
        let entries = [editor, files];
        let hits = rank(&entries, "files");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].entry.name, "Files");
        assert_eq!(hits[0].quality, Match::Exact);
        assert_eq!(hits[1].quality, Match::Loose);
    }

    #[test]
    fn truncation_drops_the_worst_matches_and_says_so() {
        let entries = [entry("Aa"), entry("Ab"), entry("Ac"), entry("Ad")];

        let outcome = rank_bounded(&entries, "a", 2);
        assert!(outcome.truncated);
        assert_eq!(outcome.ids.len(), 2);
        // Ties break by name, so the two kept are deterministic — the first
        // two alphabetically, not an arbitrary pair.
        assert_eq!(outcome.ids, [entries[0].id.clone(), entries[1].id.clone()]);

        let untruncated = rank_bounded(&entries, "a", 10);
        assert!(!untruncated.truncated);
        assert_eq!(untruncated.ids.len(), 4);
    }

    #[test]
    fn a_command_or_a_description_can_answer_too() {
        let terminal = DesktopEntry {
            exec: "kitty".to_owned(),
            generic_name: "Terminal emulator".to_owned(),
            ..entry("Kitty")
        };
        let unrelated = entry("Calculator");

        assert_eq!(score(&terminal, "terminal"), Some(Match::Loose));
        assert_eq!(score(&unrelated, "terminal"), None);
    }

    #[test]
    fn an_empty_query_is_the_whole_list_in_name_order() {
        // The middle name carries an i-diaeresis, which folds to "naive" and
        // therefore sorts between "Files" and "Zed".
        let entries = [entry("Zed"), entry("Files"), entry("na\u{ef}ve")];

        let hits = rank(&entries, "");
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].entry.name, "Files");
        assert_eq!(hits[1].entry.name, "na\u{ef}ve");
        assert_eq!(hits[2].entry.name, "Zed");
    }

    #[test]
    fn the_order_never_depends_on_the_order_the_files_were_read() {
        let forwards = [entry("Aa"), entry("Ab")];
        let backwards = [entry("Ab"), entry("Aa")];

        let names = |hits: Vec<Hit<'_>>| {
            hits.iter()
                .map(|hit| hit.entry.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(rank(&forwards, "a")), names(rank(&backwards, "a")));
    }
}
