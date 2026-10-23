//! The source scanners of the lock's structural guards cut test code out with
//! this one helper (round 6, m-d). It lives here once; the guard tests of
//! `celestina-lock` and `celestina-lock-verify` include this very file with
//! `#[path]`. Test-only code: it is not part of any product binary.

/// `text` without its `#[cfg(test)]` items (round 6, m-d). The scanners used
/// to cut the file at the first `#[cfg(test)]`, so production code after a
/// test-gated `use` or helper escaped every scan. Each gated item is removed
/// on its own: up to its closing brace (a module or function; braces inside
/// strings, characters and comments do not count) or its `;`. Round 7: the
/// gate is only looked for outside comments, strings, raw strings and
/// characters, at the top level as inside an item, so the text `#[cfg(test)]`
/// in a production string or doc comment cuts nothing.
pub fn without_test_blocks(text: &str) -> String {
    const GATE: &str = "#[cfg(test)]";
    let chars: Vec<char> = text.chars().collect();
    let gate: Vec<char> = GATE.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(end) = lexeme_end(&chars, i) {
            out.extend(&chars[i..end]);
            i = end;
        } else if chars[i..].starts_with(&gate) {
            i = end_of_item(&chars, i + gate.len());
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Where the comment, string, raw string or character literal that starts at
/// `i` ends; `None` when none starts there (a lifetime is not one).
fn lexeme_end(chars: &[char], i: usize) -> Option<usize> {
    match chars[i] {
        '/' if chars.get(i + 1) == Some(&'/') => {
            let mut j = i;
            while j < chars.len() && chars[j] != '\n' {
                j += 1;
            }
            Some(j)
        }
        '/' if chars.get(i + 1) == Some(&'*') => {
            let mut nested = 0usize;
            let mut j = i;
            while j < chars.len() {
                if chars[j..].starts_with(&['/', '*']) {
                    nested += 1;
                    j += 2;
                } else if chars[j..].starts_with(&['*', '/']) {
                    nested -= 1;
                    j += 2;
                    if nested == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            Some(j)
        }
        'r' | 'b' if raw_string_hashes(chars, i).is_some() => {
            let hashes = raw_string_hashes(chars, i).unwrap_or(0);
            let mut closing = vec!['"'];
            closing.extend(std::iter::repeat_n('#', hashes));
            let mut j = i + hashes + 2 + usize::from(chars[i] == 'b');
            while j < chars.len() && !chars[j..].starts_with(&closing) {
                j += 1;
            }
            Some((j + closing.len()).min(chars.len()))
        }
        '"' => {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                j += if chars[j] == '\\' { 2 } else { 1 };
            }
            Some((j + 1).min(chars.len()))
        }
        '\'' => {
            // A character literal, else a lifetime.
            if chars.get(i + 1) == Some(&'\\') {
                let mut j = i + 2;
                while j < chars.len() && chars[j] != '\'' {
                    j += 1;
                }
                Some((j + 1).min(chars.len()))
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 3)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Where the item that starts at `from` ends: after its `;`, or after the
/// brace that closes its first `{`.
fn end_of_item(chars: &[char], from: usize) -> usize {
    let mut depth = 0usize;
    let mut i = from;
    while i < chars.len() {
        if let Some(end) = lexeme_end(chars, i) {
            i = end;
            continue;
        }
        match chars[i] {
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                i += 1;
                if depth == 0 {
                    return i;
                }
            }
            ';' if depth == 0 => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// The number of `#` of a raw string literal that starts at `at` (`r"`,
/// `r#"`, `br#"`), if one does.
fn raw_string_hashes(chars: &[char], at: usize) -> Option<usize> {
    let mut i = at;
    if chars[i] == 'b' {
        i += 1;
    }
    if chars.get(i) != Some(&'r') {
        return None;
    }
    // Not the end of an identifier such as `for` or `bar`.
    if at > 0 && (chars[at - 1].is_alphanumeric() || chars[at - 1] == '_') {
        return None;
    }
    i += 1;
    let mut hashes = 0;
    while chars.get(i) == Some(&'#') {
        hashes += 1;
        i += 1;
    }
    (chars.get(i) == Some(&'"')).then_some(hashes)
}

#[test]
fn m_d_the_scanners_drop_each_test_gated_item_and_keep_the_production_after_it() {
    let text = r##"
#[cfg(test)]
use helper::only_in_tests;
fn production_one() { let _ = "}"; }
#[cfg(test)]
fn gated_helper() -> &'static str { "{ unbalanced" }
fn production_two() { /* a } in a comment */ let c = '}'; }
#[cfg(test)]
mod tests {
    const RAW: &str = r#"} }"#;
    fn a<'x>(v: &'x str) { if v == "{" { } }
}
fn production_three() {}
const MARK: &str = "#[cfg(test)]";
fn production_four() { let _ = '"'; }
//! A doc comment that names #[cfg(test)] in passing; it is not a gate.
/* nor is #[cfg(test)] in a block comment */
const HASHED_MARK: &str = r#"#[cfg(test)] fn nope() {"#;
fn production_five() {}
"##;
    let kept = without_test_blocks(text);
    for production in [
        "production_one",
        "production_two",
        "production_three",
        "production_four",
        "production_five",
        "MARK: &str",
        "A doc comment that names",
        "nor is",
        "HASHED_MARK",
        "fn nope",
    ] {
        assert!(kept.contains(production), "{production} survives: {kept}");
    }
    for gone in [
        "only_in_tests",
        "gated_helper",
        "unbalanced",
        "RAW",
        "fn a<",
    ] {
        assert!(!kept.contains(gone), "{gone} is cut: {kept}");
    }
    // A file with no gate is untouched.
    assert_eq!(without_test_blocks("fn x() {}\n"), "fn x() {}\n");
}
