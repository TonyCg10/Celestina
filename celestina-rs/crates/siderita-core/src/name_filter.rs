/// Shell-style name patterns — the `*.png` a file chooser's "Images" filter is
/// made of.
///
/// Deliberately small: `*` (any run, including none), `?` (one character) and
/// literal text, matched case-insensitively over the display name. No character
/// classes, no `**`, no regex — a file chooser's filter list has never needed
/// them, and every construct added here is one more way to hide a file the user
/// can see with their own eyes.
///
/// The safety property that shapes the API: an **empty** pattern set matches
/// everything, and a pattern nobody can interpret is the caller's problem to
/// avoid sending — `matches_any` never answers "no" for an empty set. Hiding a
/// file the asking application would have accepted is the failure that matters;
/// showing one it will reject is merely untidy.
#[must_use]
pub fn matches_any(name: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return true;
    }
    patterns.iter().any(|pattern| matches(name, pattern))
}

/// Whether `name` matches one pattern.
#[must_use]
pub fn matches(name: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    let name: Vec<char> = name.to_lowercase().chars().collect();
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    glob(&name, &pattern)
}

/// Iterative backtracking match: linear in the common case, and it cannot
/// recurse deeply on a pattern full of stars the way the naive version does.
fn glob(name: &[char], pattern: &[char]) -> bool {
    let (mut n, mut p) = (0usize, 0usize);
    // Where to resume if the current `*` turns out to have eaten too little.
    let mut star: Option<usize> = None;
    let mut resume = 0usize;

    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some(p);
                resume = n;
                p += 1;
            }
            Some('?') => {
                p += 1;
                n += 1;
            }
            Some(c) if *c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => match star {
                // Backtrack: let the last `*` swallow one more character.
                Some(index) => {
                    p = index + 1;
                    resume += 1;
                    n = resume;
                }
                None => return false,
            },
        }
    }

    // Trailing stars may match nothing.
    while pattern.get(p) == Some(&'*') {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns(list: &[&str]) -> Vec<String> {
        list.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn an_empty_set_matches_everything() {
        assert!(matches_any("anything.at.all", &[]));
    }

    #[test]
    fn extensions_match_case_insensitively() {
        let images = patterns(&["*.png", "*.jpg"]);
        assert!(matches_any("photo.png", &images));
        assert!(matches_any("PHOTO.PNG", &images));
        assert!(matches_any("holiday.jpg", &images));
        assert!(!matches_any("notes.txt", &images));
        assert!(!matches_any("png", &images));
    }

    #[test]
    fn question_marks_match_exactly_one_character() {
        assert!(matches("a1.txt", "a?.txt"));
        assert!(!matches("a12.txt", "a?.txt"));
        assert!(!matches("a.txt", "a?.txt"));
    }

    #[test]
    fn stars_match_across_the_whole_name() {
        assert!(matches("report-2026-final.pdf", "*2026*"));
        assert!(matches("anything", "*"));
        assert!(matches("", "*"));
        assert!(matches("prefix-mid-suffix", "prefix*suffix"));
        assert!(!matches("prefix-mid", "prefix*suffix"));
    }

    #[test]
    fn a_pattern_of_many_stars_does_not_blow_up() {
        // The naive recursive matcher goes exponential here.
        let name = "a".repeat(64);
        assert!(matches(&name, "*a*a*a*a*a*a*a*a*a*b") == false);
        assert!(matches(&name, "*a*a*a*a*a*a*a*a*a*a"));
    }

    #[test]
    fn a_literal_pattern_is_an_exact_match() {
        assert!(matches("Makefile", "makefile"));
        assert!(!matches("Makefile.in", "makefile"));
    }
}
