// language-contract: allow-non-english
//
// A module that folds accents has to contain them: the table below is the
// accented characters themselves, and the tests must search for words that
// carry one. The prose here is English like everywhere else; the marker exists
// for the letters, exactly as it does in the checker that reads this line.

//! Finding something in a library by what it is called.
//!
//! A library of mapped folders is only navigable by walking it, which stops
//! being navigation somewhere around the second thousand file. This module owns
//! the one rule that matters for making that searchable in Spanish: **a query
//! matches regardless of accents and case**, so `cancion` finds `Canción` and
//! `arbol` finds `Árbol`.
//!
//! That is not a nicety. On this desktop the person typing is the person whose
//! files these are, and a search that demands the accent is a search that finds
//! nothing most of the time — while a search that quietly ignores the accent
//! only ever finds *more*, never something wrong.
//!
//! Everything is bounded: the query is capped before it is folded, and matching
//! is a substring test over names the catalogue already holds. Nothing here
//! opens a file, reads a tag, or builds an index.

/// The longest query this will act on. Past this a person is pasting, not
/// searching, and the cap keeps the fold from being run over a large string on
/// every keystroke.
pub const MAX_QUERY_CHARACTERS: usize = 128;

/// A prepared query: folded once, then used against many names.
///
/// Building it separately from matching is the whole performance story. A grid
/// of two thousand rows folds the *query* once and each name once per keystroke;
/// folding both inside the comparison would do it two thousand times over.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Query {
    folded: String,
}

impl Query {
    /// Prepares what a person typed.
    ///
    /// The ends are trimmed and the inside is not: `de cuna` is two words a
    /// person meant, and a leading space is one they are about to type over.
    /// Without the trim, deleting a word back to a single space would leave a
    /// query that matches nothing and a library that looks empty.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            folded: fold(text.trim(), MAX_QUERY_CHARACTERS),
        }
    }

    /// Whether this query asks for anything at all. An empty one matches
    /// everything, which is how clearing the box restores the whole library
    /// without a second code path.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.folded.is_empty()
    }

    /// Whether `name` matches.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        if self.folded.is_empty() {
            return true;
        }
        fold(name, usize::MAX).contains(&self.folded)
    }

    /// The folded text, for a caller that has to store or compare queries.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.folded
    }
}

/// Lowercases and strips the accents Spanish and the languages around it use.
///
/// Deliberately not a full Unicode normalisation: that is a dependency and a
/// table, and what this library needs is the Latin letters its file names are
/// written in. A character this does not know is kept as it is, so a name in an
/// alphabet with no accents to strip is matched exactly and never mangled.
fn fold(text: &str, limit: usize) -> String {
    text.chars()
        .take(limit)
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ý' | 'ÿ' => 'y',
            'ç' => 'c',
            // `ñ` is a letter of its own and not an accented `n`. Folding it
            // would make `ano` find `año`, which is a different word and a
            // worse joke than a missed search.
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Query, MAX_QUERY_CHARACTERS};

    #[test]
    fn a_query_ignores_accents_and_case_in_both_directions() {
        assert!(Query::new("cancion").matches("Canción de cuna.flac"));
        assert!(Query::new("CANCIÓN").matches("cancion de cuna.flac"));
        assert!(Query::new("arbol").matches("Árbol.jpg"));
        assert!(Query::new("Ángel").matches("angel.png"));
    }

    #[test]
    fn n_with_a_tilde_is_its_own_letter() {
        assert!(
            !Query::new("ano").matches("año nuevo.mp4"),
            "folding ñ would make one word find a different one"
        );
        assert!(Query::new("año").matches("Año nuevo.mp4"));
    }

    #[test]
    fn an_empty_query_matches_everything_so_clearing_the_box_restores_the_library() {
        let empty = Query::new("   ");
        assert!(empty.is_empty());
        assert!(empty.matches("cualquier cosa.png"));
        assert!(empty.matches(""));
    }

    #[test]
    fn the_ends_are_trimmed_and_the_inside_is_not() {
        assert!(Query::new("  cuna  ").matches("Canción de cuna.flac"));
        assert!(Query::new("de cuna").matches("Canción de cuna.flac"));
        assert!(!Query::new("de cuna").matches("Canción decuna.flac"));
    }

    #[test]
    fn a_query_matches_anywhere_in_the_name() {
        let query = Query::new("cuna");
        assert!(query.matches("Canción de cuna.flac"));
        assert!(!query.matches("Canción de amor.flac"));
    }

    #[test]
    fn a_pasted_wall_of_text_is_bounded_before_it_is_folded() {
        let query = Query::new(&"a".repeat(MAX_QUERY_CHARACTERS * 4));
        assert_eq!(query.as_str().chars().count(), MAX_QUERY_CHARACTERS);
    }

    #[test]
    fn a_name_in_an_alphabet_with_no_accents_is_matched_exactly() {
        assert!(Query::new("привет").matches("привет.mp3"));
        assert!(Query::new("東京").matches("東京 2019.jpg"));
        assert!(!Query::new("東京").matches("kyoto.jpg"));
    }
}
