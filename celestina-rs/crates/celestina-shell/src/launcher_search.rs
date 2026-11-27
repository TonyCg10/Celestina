//! The launcher's matcher: words in any order, one typo, aliases.
//!
//! Each entry is prepared once per index ([`Prepared::new`]) so a keystroke
//! only folds the query. A query is cut into at most [`MAX_TERMS`] terms of at
//! most [`MAX_TERM_CHARS`] characters; an entry answers when every term scores
//! in some tier, and its score is the mean of the term scores.
//!
//! Nothing here touches the filesystem. The `Mod+K` selector keeps
//! [`crate::launcher::score`]; this is the launcher's own ranking.

use celestina_core::desktop_entry::{DesktopEntry, Locale};

use crate::launcher::fold;

/// The most terms a query is cut to.
pub const MAX_TERMS: usize = 8;
/// The most characters one term keeps.
pub const MAX_TERM_CHARS: usize = 64;
/// Terms shorter than this never match by typo.
const MIN_TYPO_CHARS: usize = 4;

/// One entry, folded and split once.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub id: String,
    pub display: String,
    pub detail: String,
    display_folded: String,
    /// Folded whole names: the plain one and the localized one.
    names: Vec<String>,
    name_words: Vec<String>,
    secondary_words: Vec<String>,
    secondary_texts: Vec<String>,
    exec: String,
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !value.is_empty() && !list.contains(&value) {
        list.push(value);
    }
}

/// Splits folded text on anything that is neither alphanumeric nor `+`.
fn words(folded: &str) -> impl Iterator<Item = &str> {
    folded
        .split(|c: char| !(c.is_alphanumeric() || c == '+'))
        .filter(|word| !word.is_empty())
}

impl Prepared {
    #[must_use]
    pub fn new(entry: &DesktopEntry, locale: Option<&Locale>, aliases: &[&str]) -> Prepared {
        let local_name = locale.and_then(|l| entry.localized_name(l));
        let local_generic = locale.and_then(|l| entry.localized_generic_name(l));
        let local_keywords = locale.and_then(|l| entry.localized_keywords(l));

        let display = local_name.unwrap_or(&entry.name).to_owned();
        let detail = local_generic.unwrap_or(&entry.generic_name).to_owned();

        let mut names = Vec::new();
        push_unique(&mut names, fold(&entry.name));
        if let Some(name) = local_name {
            push_unique(&mut names, fold(name));
        }

        let mut secondary_texts = Vec::new();
        push_unique(&mut secondary_texts, fold(&entry.generic_name));
        if let Some(generic) = local_generic {
            push_unique(&mut secondary_texts, fold(generic));
        }
        for keyword in entry
            .keywords
            .iter()
            .chain(local_keywords.into_iter().flatten())
        {
            push_unique(&mut secondary_texts, fold(keyword));
        }
        for alias in aliases {
            push_unique(&mut secondary_texts, fold(alias));
        }

        let mut name_words = Vec::new();
        for name in &names {
            for word in words(name) {
                push_unique(&mut name_words, word.to_owned());
            }
        }
        let mut secondary_words = Vec::new();
        for text in &secondary_texts {
            for word in words(text) {
                push_unique(&mut secondary_words, word.to_owned());
            }
        }

        Prepared {
            id: entry.id.clone(),
            display_folded: fold(&display),
            display,
            detail,
            names,
            name_words,
            secondary_words,
            secondary_texts,
            exec: fold(&entry.exec),
        }
    }
}

fn is_subsequence(text: &str, term: &str) -> bool {
    let mut wanted = term.chars().peekable();
    for c in text.chars() {
        if wanted.peek() == Some(&c) {
            wanted.next();
        }
    }
    wanted.peek().is_none()
}

/// Optimal string alignment distance of at most one between two char slices.
fn osa_at_most_one(a: &[char], b: &[char]) -> bool {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    if long.len() - short.len() > 1 {
        return false;
    }
    let common = long.iter().zip(short).take_while(|(x, y)| x == y).count();
    if common == long.len() && common == short.len() {
        return true;
    }
    let rest_long = &long[common..];
    let rest_short = &short[common..];
    if long.len() == short.len() {
        // Substitution, or an adjacent swap.
        rest_long[1..] == rest_short[1..]
            || (rest_long.len() >= 2
                && rest_long[0] == rest_short[1]
                && rest_long[1] == rest_short[0]
                && rest_long[2..] == rest_short[2..])
    } else {
        // One insertion or deletion.
        rest_long[1..] == *rest_short
    }
}

/// Whether `word` is within one edit of `term`: the whole word or its prefix
/// of the term's length minus one, equal to or plus one.
#[cfg(test)]
fn within_one_edit(word: &str, term: &str) -> bool {
    let term: Vec<char> = term.chars().collect();
    let mut buffer = Vec::new();
    within_one_edit_with(word, &term, &mut buffer)
}

fn within_one_edit_with(word: &str, term: &[char], buffer: &mut Vec<char>) -> bool {
    buffer.clear();
    buffer.extend(word.chars().take(term.len() + 1));
    let whole = buffer.len() <= term.len() + 1 && word.chars().nth(term.len() + 1).is_none();
    let low = term.len().saturating_sub(1);
    for length in [low, term.len(), term.len() + 1] {
        if length <= buffer.len() && osa_at_most_one(&buffer[..length], term) {
            return true;
        }
    }
    whole && osa_at_most_one(buffer, term)
}

/// The best tier a term reaches on an entry, or `None`.
fn term_score(entry: &Prepared, term: &str, chars: &[char], buffer: &mut Vec<char>) -> Option<u32> {
    if entry.names.iter().any(|n| n == term) {
        return Some(60);
    }
    if entry.names.iter().any(|n| n.starts_with(term)) {
        return Some(50);
    }
    if entry.name_words.iter().any(|w| w.starts_with(term)) {
        return Some(40);
    }
    if entry.secondary_words.iter().any(|w| w.starts_with(term)) {
        return Some(30);
    }
    let typo = chars.len() >= MIN_TYPO_CHARS;
    if typo
        && entry
            .name_words
            .iter()
            .any(|w| within_one_edit_with(w, chars, buffer))
    {
        return Some(20);
    }
    if typo
        && entry
            .secondary_words
            .iter()
            .any(|w| within_one_edit_with(w, chars, buffer))
    {
        return Some(15);
    }
    if entry.names.iter().any(|n| is_subsequence(n, term)) {
        return Some(10);
    }
    if entry
        .secondary_texts
        .iter()
        .chain(std::iter::once(&entry.exec))
        .any(|t| is_subsequence(t, term))
    {
        return Some(5);
    }
    None
}

/// Indices into `entries` that answer `query`, best first: score descending,
/// then `use_score(id)` descending, then the folded display name. An empty or
/// all-separator query returns nothing; the caller shows its own list.
pub fn search(entries: &[Prepared], query: &str, use_score: &dyn Fn(&str) -> u64) -> Vec<usize> {
    let folded = fold(query);
    let terms: Vec<(String, Vec<char>)> = words(&folded)
        .take(MAX_TERMS)
        .map(|word| {
            let cut: String = word.chars().take(MAX_TERM_CHARS).collect();
            let chars = cut.chars().collect();
            (cut, chars)
        })
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let whole = folded.trim();
    let mut buffer = Vec::new();
    let mut hits: Vec<(u32, u64, usize)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let mut sum = 0;
        let mut matched = true;
        for (term, chars) in &terms {
            match term_score(entry, term, chars, &mut buffer) {
                Some(score) => sum += score,
                None => {
                    matched = false;
                    break;
                }
            }
        }
        if !matched {
            continue;
        }
        let mut score = sum / u32::try_from(terms.len()).unwrap_or(1);
        if entry.display_folded == whole {
            score = 60;
        }
        hits.push((score, use_score(&entry.id), index));
    }
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then_with(|| {
            entries[a.2]
                .display_folded
                .cmp(&entries[b.2].display_folded)
        })
    });
    hits.into_iter().map(|(_, _, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, generic: &str, keywords: &[&str], exec: &str) -> DesktopEntry {
        DesktopEntry {
            id: id.to_owned(),
            name: name.to_owned(),
            generic_name: generic.to_owned(),
            keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
            exec: exec.to_owned(),
            ..DesktopEntry::default()
        }
    }

    fn prep(e: &DesktopEntry) -> Prepared {
        Prepared::new(e, None, &[])
    }

    fn none(_: &str) -> u64 {
        0
    }

    fn ids(list: &[Prepared], query: &str, usage: &dyn Fn(&str) -> u64) -> Vec<String> {
        search(list, query, usage)
            .into_iter()
            .map(|i| list[i].id.clone())
            .collect()
    }

    #[test]
    fn typo_matches_name_word() {
        let list = [prep(&entry("krita", "Krita", "", &[], "krita"))];
        assert_eq!(search(&list, "krtia", &none), vec![0]);
        assert_eq!(
            term_score(
                &list[0],
                "krtia",
                &"krtia".chars().collect::<Vec<_>>(),
                &mut Vec::new()
            ),
            Some(20)
        );
    }

    #[test]
    fn short_terms_do_not_typo_match() {
        let list = [prep(&entry("a", "Krita", "", &[], "zzz"))];
        assert!(search(&list, "krz", &none).is_empty());
        assert!(search(&list, "krx", &none).is_empty());
    }

    #[test]
    fn keyword_word_start_beats_name_subsequence() {
        let kw = entry("kw", "Zed", "", &["browser"], "zed");
        let sub = entry("sub", "Big Rain Open Web", "", &[], "x");
        let list = [prep(&sub), prep(&kw)];
        assert_eq!(ids(&list, "brow", &none), vec!["kw", "sub"]);
        assert_eq!(
            term_score(&list[0], "brow", &['b', 'r', 'o', 'w'], &mut Vec::new()),
            Some(10)
        );
        assert_eq!(
            term_score(&list[1], "brow", &['b', 'r', 'o', 'w'], &mut Vec::new()),
            Some(30)
        );
    }

    #[test]
    fn word_order_is_free() {
        let list = [prep(&entry("a", "Foo", "Image Editor", &[], "foo"))];
        assert_eq!(search(&list, "editor image", &none), vec![0]);
        assert_eq!(search(&list, "image editor", &none), vec![0]);
    }

    #[test]
    fn alias_matches() {
        let e = entry("a", "Foo", "", &[], "foo");
        let list = [Prepared::new(&e, None, &["navegador"])];
        assert_eq!(search(&list, "naveg", &none), vec![0]);
    }

    #[test]
    fn localized_name_and_keywords_match() {
        let mut e = entry("a", "Image Viewer", "", &[], "iv");
        e.name_l10n = vec![("es".into(), "Visor de im\u{e1}genes".into())];
        e.keywords_l10n = vec![("es".into(), vec!["foto".into()])];
        let es = Locale::parse("es_ES.UTF-8").unwrap();
        let list = [Prepared::new(&e, Some(&es), &[])];
        assert_eq!(list[0].display, "Visor de im\u{e1}genes");
        assert_eq!(search(&list, "imagenes visor", &none), vec![0]);
        assert_eq!(search(&list, "foto", &none), vec![0]);
        assert_eq!(search(&list, "viewer", &none), vec![0]);
    }

    #[test]
    fn use_breaks_ties_then_name() {
        let list = [
            prep(&entry("b", "Beta Tool", "", &[], "b")),
            prep(&entry("a", "Alpha Tool", "", &[], "a")),
        ];
        assert_eq!(ids(&list, "tool", &none), vec!["a", "b"]);
        let usage = |id: &str| if id == "b" { 5 } else { 1 };
        assert_eq!(ids(&list, "tool", &usage), vec!["b", "a"]);
    }

    #[test]
    fn bounds_hold() {
        let list = [prep(&entry("a", "Foo", "", &[], "foo"))];
        let long = "a".repeat(10_000);
        assert!(search(&list, &long, &none).is_empty());
        let many = (0..50).map(|_| "foo").collect::<Vec<_>>().join(" ");
        assert_eq!(search(&list, &many, &none), vec![0]);
        // Only the first 8 terms count: a miss in the 9th is ignored.
        let ninth = "foo foo foo foo foo foo foo foo zzzz";
        assert_eq!(search(&list, ninth, &none), vec![0]);
        // A term is cut to 64 chars.
        let cut = format!("{}{}", "f".repeat(64), "z".repeat(100));
        let name = prep(&entry("b", &"f".repeat(64), "", &[], "x"));
        assert_eq!(search(&[name], &cut, &none), vec![0]);
    }

    #[test]
    fn whole_name_rule() {
        // Two words, each a word start (40), so the mean is 40 ...
        let exact = prep(&entry("exact", "Visual Studio", "", &[], "x"));
        // ... and the other entry names both terms as full words too, but
        // its display differs.
        let other = prep(&entry("other", "Visual Studio Code", "", &[], "x"));
        let list = [other, exact];
        assert_eq!(
            ids(&list, "visual studio", &|id| if id == "other" {
                9
            } else {
                0
            }),
            vec!["exact", "other"]
        );
    }

    #[test]
    fn empty_and_punctuation_queries_return_nothing() {
        let list = [prep(&entry("a", "Foo", "", &[], "foo"))];
        assert!(search(&list, "", &none).is_empty());
        assert!(search(&list, "   ", &none).is_empty());
        assert!(search(&list, " -- ,, ", &none).is_empty());
    }

    #[test]
    fn plus_stays_in_terms() {
        let list = [prep(&entry("a", "Notepad++", "", &[], "np"))];
        assert_eq!(search(&list, "notepad++", &none), vec![0]);
    }

    #[test]
    fn within_one_edit_cases() {
        assert!(within_one_edit("krita", "krta")); // deletion in term
        assert!(within_one_edit("krita", "kritaa")); // insertion in term
        assert!(within_one_edit("krita", "krito")); // substitution
        assert!(within_one_edit("krita", "krtia")); // swap
        assert!(!within_one_edit("krita", "kxtix")); // two edits
        assert!(!within_one_edit("krita", "ktria")); // two edits
                                                     // A prefix of a long word counts.
        assert!(within_one_edit("firefox", "fierf"));
        assert!(!within_one_edit("firefox", "fxrxfo"));
    }
}
