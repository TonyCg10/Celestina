//! The source guards of the polkit agent (CEL-2-I; design §6.3, the items
//! tagged `G`).
//!
//! The shared architecture scanner lives outside this project's commit roots,
//! so the guards that belong to the polkit modules are tests here: each reads
//! the production sources and fails on a forbidden construct. Every test names
//! the invariant it enforces (`N-n` of §6.3) in its name and its doc.
//!
//! How the scan sees a file:
//!
//! * comments are blanked first (`//`, `///`, `//!` and nested `/* */`), so a
//!   comment that talks about `unwrap(` or `AuthenticationAgentResponse` never
//!   trips a guard; string and character literals are kept, because a method
//!   name is a string on the bus;
//! * items attributed `#[cfg(test)]` are cut with their bodies (a whole
//!   `mod tests { .. }`, a `mod tests;` declaration, a test-only field, a
//!   test-only statement). The cut is item-wise and not "from the first
//!   `#[cfg(test)]` line to the end of the file", because `agent.rs`,
//!   `providers/polkit/mod.rs` and `polkit.rs` carry test seams in the middle
//!   of production code, so a line cut would hide real code from the guards;
//! * files that exist only for tests are excluded BY LIST
//!   ([`TEST_ONLY_FILES`]). The list is asserted to contain only existing
//!   files, each declared behind `#[cfg(test)]` by its parent, and every other
//!   `.rs` file under the polkit directories must be in [`PRODUCTION_FILES`],
//!   so a new file cannot escape the guards by being forgotten.
//!
//! Each scanner is proven able to fail by a test on the scanner function with a
//! hostile sample (the `scanner_*` tests at the bottom).

use std::fs;
use std::path::{Path, PathBuf};

// ---- the file lists -----------------------------------------------------------------------

/// The polkit production sources, relative to `celestina/` (the crate root).
const PRODUCTION_FILES: &[&str] = &[
    "src/providers/polkit/mod.rs",
    "src/providers/polkit/agent.rs",
    "src/providers/polkit/helper.rs",
    "src/providers/polkit/session.rs",
    "src/polkit/mod.rs",
    "src/polkit/state.rs",
    "src/polkit/view.rs",
    "src/polkit/duty.rs",
    "src/polkit/dumpable.rs",
    "../celestina-rs/crates/celestina-shell/src/polkit.rs",
    "../celestina-rs/crates/celestina-shell/src/display_text.rs",
    "crates/celestina-ui/src/secret.rs",
];

/// Files that exist only to test, excluded from every scan by name. Each is
/// declared by its parent behind `#[cfg(test)]`, which
/// `the_test_only_exclusion_list_names_only_existing_test_files` verifies.
/// `(file, parent that declares it, module name)`.
const TEST_ONLY_FILES: &[(&str, &str, &str)] = &[
    (
        "src/providers/polkit/testing.rs",
        "src/providers/polkit/mod.rs",
        "testing",
    ),
    (
        "src/providers/polkit/tests.rs",
        "src/providers/polkit/mod.rs",
        "tests",
    ),
    (
        "src/providers/polkit/helper/tests.rs",
        "src/providers/polkit/helper.rs",
        "tests",
    ),
    ("src/polkit/state/tests.rs", "src/polkit/state.rs", "tests"),
    ("src/polkit/duty/tests.rs", "src/polkit/duty.rs", "tests"),
];

/// The directories whose every `.rs` file must be classified above.
const POLKIT_DIRECTORIES: &[&str] = &["src/providers/polkit", "src/polkit"];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = crate_root().join(relative);
    match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => panic!("{} is readable: {err}", path.display()),
    }
}

/// Every production file, scanned form: comments blanked, test items cut.
fn production_sources() -> Vec<(&'static str, String)> {
    PRODUCTION_FILES
        .iter()
        .map(|&file| (file, production(&read(file))))
        .collect()
}

// ---- the scanner --------------------------------------------------------------------------

/// What [`lex`] blanks besides comments.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Blank {
    /// Comments only: string and character literals stay, for the guards that
    /// look at literals.
    Comments,
    /// Comments and the inside of every literal, for structural scanning
    /// (brace matching) that a `"{"` in a string must not confuse.
    CommentsAndLiterals,
}

fn is_ident(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

/// Blanks comments (and, on request, literals) with spaces, keeping every
/// newline and every offset, so the output lines up byte for byte with the
/// input.
fn lex(source: &str, blank: Blank) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    let wipe = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in out.iter_mut().take(to).skip(from) {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();
        if b == b'/' && next == Some(b'/') {
            let end = bytes[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(bytes.len(), |p| i + p);
            wipe(&mut out, i, end);
            i = end;
        } else if b == b'/' && next == Some(b'*') {
            let mut depth = 1;
            let mut j = i + 2;
            while j < bytes.len() && depth > 0 {
                if bytes[j] == b'/' && bytes.get(j + 1) == Some(&b'*') {
                    depth += 1;
                    j += 2;
                } else if bytes[j] == b'*' && bytes.get(j + 1) == Some(&b'/') {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            wipe(&mut out, i, j);
            i = j;
        } else if let Some((open, close_hashes)) = raw_string_start(bytes, i) {
            // r"..", r#".."#, br#".."#: ends at a quote and as many hashes.
            let mut j = open;
            let end = loop {
                match bytes[j..].iter().position(|&c| c == b'"') {
                    None => break bytes.len(),
                    Some(p) => {
                        let q = j + p;
                        let tail = &bytes[q + 1..];
                        if tail.iter().take_while(|&&c| c == b'#').count() >= close_hashes {
                            break q + 1 + close_hashes;
                        }
                        j = q + 1;
                    }
                }
            };
            if blank == Blank::CommentsAndLiterals {
                wipe(
                    &mut out,
                    open,
                    end.saturating_sub(1 + close_hashes).max(open),
                );
            }
            i = end;
        } else if b == b'"' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(bytes.len());
            if blank == Blank::CommentsAndLiterals {
                wipe(&mut out, i + 1, j.min(bytes.len()));
            }
            i = end;
        } else if b == b'\'' {
            i = skip_char_literal(bytes, i, &mut out, blank);
        } else {
            i += 1;
        }
    }
    // Only whole comment and literal spans were overwritten with ASCII.
    String::from_utf8(out).unwrap_or_default()
}

/// `(offset of the first byte after the opening quote, hashes)` when a raw
/// string starts at `i`.
fn raw_string_start(bytes: &[u8], i: usize) -> Option<(usize, usize)> {
    if i > 0 && is_ident(bytes[i - 1]) {
        return None;
    }
    let mut j = i;
    if bytes.get(j) == Some(&b'b') {
        j += 1;
    }
    if bytes.get(j) != Some(&b'r') {
        return None;
    }
    j += 1;
    let hashes = bytes[j..].iter().take_while(|&&c| c == b'#').count();
    j += hashes;
    (bytes.get(j) == Some(&b'"')).then_some((j + 1, hashes))
}

/// A `'` starts a character literal (`'a'`, `'\n'`, `'"'`) or a lifetime
/// (`'a`). Returns the offset after what it consumed.
fn skip_char_literal(bytes: &[u8], i: usize, out: &mut [u8], blank: Blank) -> usize {
    let escaped = bytes.get(i + 1) == Some(&b'\\');
    let close = if escaped {
        bytes[i + 2..]
            .iter()
            .position(|&c| c == b'\'')
            .map(|p| i + 2 + p)
    } else {
        // One scalar value, whatever its width, then the closing quote.
        let width = match bytes.get(i + 1) {
            Some(&c) if c >= 0xf0 => 4,
            Some(&c) if c >= 0xe0 => 3,
            Some(&c) if c >= 0xc0 => 2,
            Some(_) => 1,
            None => return i + 1,
        };
        (bytes.get(i + 1 + width) == Some(&b'\'')).then_some(i + 1 + width)
    };
    match close {
        Some(end) => {
            if blank == Blank::CommentsAndLiterals {
                for byte in out.iter_mut().take(end).skip(i + 1) {
                    *byte = b' ';
                }
            }
            end + 1
        }
        None => i + 1, // a lifetime
    }
}

/// Removes every item attributed `#[cfg(test)]`, with its body. `stripped` is
/// [`lex`] with [`Blank::Comments`]; the structure is read from the same text
/// with literals blanked, so offsets agree.
fn cut_test_items(stripped: &str) -> String {
    let structure = lex(stripped, Blank::CommentsAndLiterals);
    let sb = structure.as_bytes();
    let mut keep = vec![true; stripped.len()];
    let mut i = 0;
    while let Some(found) = structure[i..].find("#[") {
        let start = i + found;
        let Some(close) = attribute_end(sb, start) else {
            i = start + 2;
            continue;
        };
        let compact: String = structure[start..close]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if compact != "#[cfg(test)]" {
            i = close;
            continue;
        }
        let end = item_end(sb, close);
        for flag in keep.iter_mut().take(end).skip(start) {
            *flag = false;
        }
        i = end;
    }
    rebuild(stripped, &keep)
}

/// The kept bytes, as text. A cut always lies on item boundaries, which are
/// ASCII, so the kept runs stay valid UTF-8.
fn rebuild(stripped: &str, keep: &[bool]) -> String {
    let bytes: Vec<u8> = stripped
        .bytes()
        .zip(keep)
        .map(
            |(byte, &kept)| {
                if kept || byte == b'\n' {
                    byte
                } else {
                    b' '
                }
            },
        )
        .collect();
    String::from_utf8(bytes).unwrap_or_default()
}

/// The offset after the `]` that closes the attribute opened at `start`.
fn attribute_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, &byte) in bytes[start..].iter().enumerate() {
        match byte {
            b'[' => depth += 1,
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(start + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// The offset after the item that starts at `from`: at the first `;` or `,`
/// outside every bracket, or at the `}` that closes the first top-level
/// brace block (a function, a module, an `if let`).
fn item_end(bytes: &[u8], from: usize) -> usize {
    let (mut paren, mut brace, mut square, mut angle) = (0i32, 0i32, 0i32, 0i32);
    let mut i = from;
    while i < bytes.len() {
        let b = bytes[i];
        let prev = if i > from { bytes[i - 1] } else { b' ' };
        match b {
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'[' => square += 1,
            b']' => square -= 1,
            b'<' => angle += 1,
            b'>' if prev != b'-' && prev != b'=' => angle = (angle - 1).max(0),
            b'{' => brace += 1,
            b'}' => {
                brace -= 1;
                if brace == 0 && paren == 0 && square == 0 {
                    return i + 1;
                }
            }
            b';' | b',' if brace == 0 && paren == 0 && square == 0 && angle == 0 => {
                return i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// What the guards look at: comments blanked, test items cut.
fn production(source: &str) -> String {
    cut_test_items(&lex(source, Blank::Comments))
}

// ---- the matchers -------------------------------------------------------------------------

/// A forbidden token. `whole_word` requires a non-identifier byte on each side
/// that is an identifier byte on the token's own edge, so `unsafe` does not
/// match `unsafe_code` and `expect(` does not match `unexpect(`.
#[derive(Clone, Copy)]
struct Token {
    text: &'static str,
    whole_word: bool,
}

const fn word(text: &'static str) -> Token {
    Token {
        text,
        whole_word: true,
    }
}

const fn substring(text: &'static str) -> Token {
    Token {
        text,
        whole_word: false,
    }
}

/// `(line number, line)` for every occurrence of a forbidden token.
fn hits(code: &str, tokens: &[Token]) -> Vec<(usize, String, &'static str)> {
    let mut found = Vec::new();
    for (index, line) in code.lines().enumerate() {
        for token in tokens {
            if line_has(line, *token) {
                found.push((index + 1, line.trim().to_owned(), token.text));
            }
        }
    }
    found
}

fn line_has(line: &str, token: Token) -> bool {
    let bytes = line.as_bytes();
    let needle = token.text.as_bytes();
    let mut from = 0;
    while let Some(p) = line[from..].find(token.text) {
        let at = from + p;
        let end = at + needle.len();
        let left_ok =
            !token.whole_word || !is_ident(needle[0]) || at == 0 || !is_ident(bytes[at - 1]);
        let right_ok = !token.whole_word
            || !is_ident(needle[needle.len() - 1])
            || end >= bytes.len()
            || !is_ident(bytes[end]);
        if left_ok && right_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

/// Fails with every hit, file and line.
fn assert_none(what: &str, files: &[(&str, String)], tokens: &[Token]) {
    let mut report = String::new();
    for (file, code) in files {
        for (line, text, token) in hits(code, tokens) {
            report.push_str(&format!("\n  {file}:{line}: `{token}` in `{text}`"));
        }
    }
    assert!(report.is_empty(), "{what}:{report}");
}

fn only<'a>(files: &'a [(&'static str, String)], name: &str) -> &'a str {
    match files.iter().find(|(file, _)| *file == name) {
        Some((_, code)) => code,
        None => panic!("{name} is a production file of the list"),
    }
}

// ---- the guards ---------------------------------------------------------------------------

/// N-21 (and the file lists' own integrity): the exclusion list names only
/// existing test files, each declared behind `#[cfg(test)]` by its parent, and
/// every `.rs` file under the polkit directories is classified. A forgotten
/// new file cannot hide from the guards.
#[test]
fn the_test_only_exclusion_list_names_only_existing_test_files() {
    for &(file, parent, module) in TEST_ONLY_FILES {
        assert!(
            crate_root().join(file).is_file(),
            "{file} is in TEST_ONLY_FILES but does not exist"
        );
        let raw = read(parent);
        let kept = production(&raw);
        let declaration = format!("mod {module}");
        assert!(
            raw.contains(&declaration),
            "{parent} does not declare `{declaration}`"
        );
        assert!(
            !kept.contains(&declaration),
            "{parent} declares `{declaration}` outside #[cfg(test)], so {file} is not test-only"
        );
    }
    let mut unclassified = Vec::new();
    for directory in POLKIT_DIRECTORIES {
        for file in rust_files(&crate_root().join(directory)) {
            let relative = match file.strip_prefix(crate_root()) {
                Ok(relative) => relative.to_string_lossy().into_owned(),
                Err(_) => continue,
            };
            let known = PRODUCTION_FILES.contains(&relative.as_str())
                || TEST_ONLY_FILES.iter().any(|(f, _, _)| *f == relative);
            if !known {
                unclassified.push(relative);
            }
        }
    }
    assert!(
        unclassified.is_empty(),
        "classify these in PRODUCTION_FILES or TEST_ONLY_FILES: {unclassified:?}"
    );
}

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    files
}

/// N-9: the agent never decides a success. Nothing calls
/// `AuthenticationAgentResponse*` (the call that would tell polkitd the
/// caller authenticated) and nothing spawns a program.
#[test]
fn n09_no_authentication_response_call_and_no_process_spawn() {
    assert_none(
        "N-9: the agent must not answer for polkit or run a program",
        &production_sources(),
        &[
            substring("AuthenticationAgentResponse"),
            substring("std::process"),
            word("process::Command"),
            word("Command::new"),
        ],
    );
}

/// The cut never hides production code: everything before a file's first
/// `#[cfg(test)]` line survives it, whatever the seams after it do.
#[test]
fn the_cut_keeps_every_line_before_the_first_cfg_test() {
    for (file, kept) in production_sources() {
        let raw = lex(&read(file), Blank::Comments);
        let before: Vec<&str> = raw
            .lines()
            .take_while(|l| !l.contains("#[cfg(test)]"))
            .filter(|l| !l.trim().is_empty())
            .collect();
        let surviving: Vec<&str> = kept.lines().filter(|l| !l.trim().is_empty()).collect();
        assert!(
            surviving.len() >= before.len(),
            "{file}: the cut dropped code that precedes its first #[cfg(test)]"
        );
        for line in before {
            assert!(kept.contains(line.trim_end()), "{file}: lost `{line}`");
        }
    }
}

/// N-21: no `unsafe`, and no production `unwrap`, `expect` or `panic!` (nor
/// the `todo!`/`unimplemented!` that panic as well) in the polkit modules.
#[test]
fn n21_no_unsafe_unwrap_expect_or_panic_in_production() {
    assert_none(
        "N-21: production code must not panic or use unsafe",
        &production_sources(),
        &[
            word("unsafe"),
            word("unwrap("),
            word("expect("),
            word("panic!"),
            word("todo!"),
            word("unimplemented!"),
        ],
    );
}

/// N-13 and decision 22: the agent's connection answers its own two methods
/// from the bus thread's single ordered stream; it exports no interface and no
/// object server, so nothing else can be called on it, and it opens no
/// session-bus connection to export anything on.
#[test]
fn n13_no_interface_attribute_and_no_object_server() {
    assert_none(
        "N-13: the agent must not export an interface",
        &production_sources(),
        &[
            substring("#[interface"),
            substring("zbus::interface"),
            substring("dbus_interface"),
            word("ObjectServer"),
            word("object_server"),
            // Never on the session bus either: the agent's one connection is
            // the system bus's.
            substring("Connection::session"),
            substring("Builder::session"),
        ],
    );
}

/// N-2: the secret and the cookie never enter the loop's `Message`, the bus
/// module or the command surface. (Their types are not named there; the
/// provider keeps the request table behind a mutex, so the cookie never needs
/// to.) The local type-level assertions are cited, not duplicated: see
/// `n01_the_compile_time_assertions_still_exist`.
#[test]
fn n02_secret_and_cookie_are_never_named_in_message_bus_or_command() {
    let files: Vec<(&str, String)> = ["src/message.rs", "src/bus.rs", "src/command.rs"]
        .iter()
        .map(|&file| (file, production(&read(file))))
        .collect();
    assert_none(
        "N-2: the loop's message, bus and command types must not carry a secret",
        &files,
        &[
            word("Secret"),
            word("Cookie"),
            word("Zeroizing"),
            word("SecretField"),
            substring("celestina_ui::secret"),
        ],
    );
}

/// N-5: the secret is never put in a `String`, a `format!` result or a
/// growable buffer. In `secret.rs` the only `String` is the toolkit's own key
/// text that `wipe` zeroes (`fn wipe(text: Option<String>)`), so the count of
/// the word is pinned to that one signature, and nothing that formats, copies
/// or reallocates may appear.
#[test]
fn n05_the_secret_buffer_never_formats_copies_or_grows() {
    let files = production_sources();
    let secret = only(&files, "crates/celestina-ui/src/secret.rs");
    assert_none(
        "N-5: secret.rs must not format, copy or grow",
        &[("secret.rs", secret.to_owned())],
        &[
            word("format!"),
            word("push_str"),
            word("to_string"),
            word("to_owned"),
            word("to_vec"),
            word("String::"),
            word("clone"),
            word("reserve"),
            word("extend"),
            word("extend_from_slice"),
            word("resize"),
            word("append"),
            word("shrink_to_fit"),
            word("Vec::new"),
            word("Vec::with_capacity"),
        ],
    );
    let strings: Vec<&str> = secret
        .lines()
        .filter(|l| line_has(l, word("String")))
        .collect();
    assert_eq!(
        strings.len(),
        1,
        "N-5: the only String in secret.rs is wipe's parameter: {strings:?}"
    );
    assert!(
        strings
            .first()
            .is_some_and(|l| l.trim() == "fn wipe(text: Option<String>) {"),
        "N-5: the one String must be wipe's parameter: {strings:?}"
    );
    assert_eq!(
        secret.matches("vec![").count(),
        1,
        "N-5: the buffer is allocated once, at its full size"
    );
}

/// N-5 (the paths the secret travels, line by line): in the loop state and
/// in the helper conversation, a line that names the secret must not format,
/// copy or print it. A heuristic by line, which complements the type-level
/// facts above (no `Clone`, no `Debug`).
#[test]
fn n05_lines_that_name_the_secret_never_format_or_copy_it() {
    let files = production_sources();
    let travelling: Vec<(&str, String)> = ["src/polkit/state.rs", "src/providers/polkit/helper.rs"]
        .iter()
        .map(|&name| (name, only(&files, name).to_owned()))
        .map(|(name, code)| {
            let lines: String = code
                .lines()
                .map(|l| {
                    if line_has(l, word("secret")) || line_has(l, word("Secret")) {
                        l
                    } else {
                        ""
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            (name, lines)
        })
        .collect();
    assert_none(
        "N-5: a line naming the secret formats or copies it",
        &travelling,
        &[
            word("format!"),
            word("format_args!"),
            word("push_str"),
            word("to_string"),
            word("to_owned"),
            word("to_vec"),
            word("clone"),
            word("String::from"),
            word("{:?}"),
            word("debug!"),
            word("trace!"),
            word("info!"),
            word("warn!"),
            word("error!"),
            word("println!"),
            word("eprintln!"),
        ],
    );
}

/// N-18: the card's text comes only from the sanitised request. In
/// `view.rs`, the only model fields read are the request's `message`,
/// `action_id` and `account`, PAM's three lines and the state flags; no
/// description is composed (`format!`, `String`, `concat!`, `join`), and every
/// string literal is a view key, never text shown. The labels come from
/// `copy::`.
#[test]
fn n18_the_card_builds_its_description_only_from_the_request_display() {
    let files = production_sources();
    let view = only(&files, "src/polkit/view.rs");
    assert_eq!(
        used_model_fields(view),
        allowed_model_fields(),
        "N-18: the view reads exactly these CardModel fields"
    );
    assert_none(
        "N-18: the card composes no text of its own",
        &[("view.rs", view.to_owned())],
        &[
            word("format!"),
            word("concat!"),
            word("String"),
            word("to_string"),
            word("to_owned"),
            word("push_str"),
            word("join("),
            word("write!"),
        ],
    );
    let stray = shown_literals(view);
    assert!(
        stray.is_empty(),
        "N-18: string literals in view.rs outside view keys and the glyph: {stray:?}"
    );
    // The text arguments of the text-bearing calls are the model's fields or
    // `copy::` constants.
    for model_field in ["message", "action_id", "account"] {
        assert!(
            view.contains(&format!("model.{model_field}")),
            "the card shows {model_field}"
        );
    }
    assert!(
        view.contains("copy::POLKIT_TITLE"),
        "labels come from copy.rs"
    );
}

/// The `CardModel` fields the view may read: polkit's three request fields,
/// PAM's three lines and the state flags.
fn allowed_model_fields() -> Vec<String> {
    let mut fields: Vec<String> = [
        "account",
        "action_id",
        "failure",
        "field",
        "message",
        "pam_line",
        "pam_prompt",
        "phase",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    fields.sort();
    fields
}

/// The sorted, distinct `model.<field>` accessors in `code`.
fn used_model_fields(code: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut rest = code;
    while let Some(p) = rest.find("model.") {
        let tail = &rest[p + "model.".len()..];
        let name: String = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if p == 0 || !is_ident(rest.as_bytes()[p - 1]) {
            fields.push(name);
        }
        rest = tail;
    }
    fields.sort();
    fields.dedup();
    fields
}

/// String literals that are not a view key (`polkit-` and lowercase words
/// with dashes) or the glyph name: text that would be shown without passing
/// through `copy.rs` or the request.
fn shown_literals(code: &str) -> Vec<String> {
    let mut stray = Vec::new();
    for line in code.lines() {
        if line.trim_start().starts_with("pub const LOCK_GLYPH") {
            continue;
        }
        let pieces = line.split('"').skip(1).step_by(2);
        for literal in pieces {
            let key = literal.starts_with("polkit-")
                && literal.bytes().all(|b| b.is_ascii_lowercase() || b == b'-');
            if !key {
                stray.push(line.trim().to_owned());
            }
        }
    }
    stray
}

/// N-3: the answer field is not the shared text field. `celestina_ui::Field`
/// carries a selection, a clipboard path and an input method; the prompt uses
/// `SecretFieldState` only.
#[test]
fn n03_the_prompt_view_does_not_use_the_shared_text_field() {
    let files = production_sources();
    let view = only(&files, "src/polkit/view.rs");
    assert!(
        field_uses(view).is_empty(),
        "N-3: polkit/view.rs uses celestina_ui::Field: {:?}",
        field_uses(view)
    );
    // The same for the state that drives it.
    let state = only(&files, "src/polkit/state.rs");
    assert!(
        field_uses(state).is_empty(),
        "N-3: polkit/state.rs uses celestina_ui::Field: {:?}",
        field_uses(state)
    );
}

/// Where `code` names the toolkit's `Field` type: a path to it, a `use` that
/// imports it, or its constructor. `Focus::Field` (the card's own variant) and
/// `SecretField` are different names.
fn field_uses(code: &str) -> Vec<String> {
    let mut uses = Vec::new();
    for statement in code.split(';') {
        let compact: String = statement.split_whitespace().collect::<Vec<_>>().join(" ");
        let is_use = compact.trim_start().starts_with("use ")
            || compact.contains("\nuse ")
            || compact.contains(" use celestina_ui");
        if is_use && compact.contains("celestina_ui") && line_has(&compact, word("Field")) {
            uses.push(compact.clone());
        }
    }
    for token in [
        "celestina_ui::Field",
        "ui::Field",
        "celestina_ui::field",
        "Field::new",
        "Field::default",
    ] {
        for (line, text, _) in hits(code, &[word(token)]) {
            uses.push(format!("{line}: {text}"));
        }
    }
    uses
}

/// N-1/N-2 (cited): the type-level assertions that `Secret`, `Cookie` and
/// `Request` implement no `Clone`, `Debug`, `Display` or `Serialize` live in
/// the crates that own the types. This guard pins that they are still written,
/// so deleting one fails here rather than silently. They are not duplicated.
#[test]
fn n01_the_compile_time_assertions_still_exist() {
    let shell = read("../celestina-rs/crates/celestina-shell/src/polkit.rs");
    assert!(
        shell.contains("assert_not_impl!(Cookie: Clone, Debug, Display, serde::Serialize)"),
        "N-1: celestina_shell::polkit::tests::cookie_and_request_leak_nothing_through_standard_traits (Cookie)"
    );
    assert!(
        shell.contains("assert_not_impl!(Request: Clone, Debug, Display, serde::Serialize)"),
        "N-1: celestina_shell::polkit::tests::cookie_and_request_leak_nothing_through_standard_traits (Request)"
    );
    let secret = read("crates/celestina-ui/src/secret.rs");
    assert!(
        secret.contains("fn secret_is_not_clone_or_debug()")
            && secret.contains("assert_not_impl!(\n            Secret:"),
        "N-1: celestina_ui::secret::tests::secret_is_not_clone_or_debug"
    );
}

/// N-1 (the part no trait assertion can state): `Secret` has no `derive` and
/// the only trait implemented for it is `Default`. `celestina-ui` has no
/// serde, so `Serialize` cannot be named in an assertion there; this closes it.
#[test]
fn n01_secret_has_no_derive_and_implements_only_default() {
    let files = production_sources();
    let secret = only(&files, "crates/celestina-ui/src/secret.rs");
    let lines: Vec<&str> = secret.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.trim() == "pub struct Secret {")
        .expect("secret.rs declares `pub struct Secret {`");
    let before = lines[..at]
        .iter()
        .rev()
        .find(|l| !l.trim().is_empty())
        .copied()
        .unwrap_or_default();
    assert!(
        !before.contains('#'),
        "N-1: Secret carries an attribute (a derive?): `{before}`"
    );
    let impls: Vec<&str> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.starts_with("impl") && l.contains(" for Secret"))
        .collect();
    assert_eq!(
        impls,
        ["impl Default for Secret {"],
        "N-1: Secret implements nothing but Default"
    );
}

/// N-4: text input is switched on only by `SurfaceCtl::set_text_input_enabled`
/// (and positioned by `set_text_input_rect`); the polkit modules call neither,
/// so the compositor's input method never sees the prompt's surface.
#[test]
fn n04_the_prompt_never_enables_text_input() {
    assert_none(
        "N-4: the prompt must not enable the input method",
        &production_sources(),
        &[
            word("set_text_input_enabled"),
            word("set_text_input_rect"),
            word("TextInput"),
            word("SeatRequest"),
        ],
    );
}

/// N-14: registration is the plain per-session call, once. The agent never
/// uses `RegisterAuthenticationAgentWithOptions` (which carries the fallback
/// flag) and the plain call exists exactly where the agent registers.
#[test]
fn n14_registration_uses_only_the_plain_call() {
    let files = production_sources();
    assert_none(
        "N-14: no registration with options",
        &files,
        &[substring("RegisterAuthenticationAgentWithOptions")],
    );
    let agent = only(&files, "src/providers/polkit/agent.rs");
    assert!(agent.contains("\"RegisterAuthenticationAgent\""));
}

/// N-20: `icon_name` and `details` are never resolved or shown. The bus
/// thread binds them as `_icon` and `_details` and goes no further; the
/// request domain, the state and the view never name either.
#[test]
fn n20_icon_name_and_details_are_discarded_at_the_door() {
    let files = production_sources();
    let agent = only(&files, "src/providers/polkit/agent.rs");
    assert!(
        agent.contains("(action_id, text, _icon, _details, cookie, offered)"),
        "N-20: agent.rs binds icon_name and details unused"
    );
    let downstream: Vec<(&str, String)> = [
        "../celestina-rs/crates/celestina-shell/src/polkit.rs",
        "src/polkit/state.rs",
        "src/polkit/view.rs",
        "src/polkit/mod.rs",
    ]
    .iter()
    .map(|&name| (name, only(&files, name).to_owned()))
    .collect();
    assert_none(
        "N-20: the request, state and card never carry icon_name or details",
        &downstream,
        &[word("icon_name"), word("details"), word("_details")],
    );
}

/// N-16 and the namespace rule: the helper's socket path is one constant, and
/// the only environment variables the polkit modules read are the session
/// id and the two locale variables the session lookup and the card's language
/// need; none of them selects the socket, the PAM service or a program.
#[test]
fn n16_the_helper_path_is_one_constant_and_no_environment_selects_it() {
    let files = production_sources();
    let occurrences: usize = files
        .iter()
        .map(|(_, code)| code.matches("\"/run/polkit/agent-helper.socket\"").count())
        .sum();
    assert_eq!(
        occurrences, 1,
        "N-16: the helper's socket path is written once, as HELPER_SOCKET"
    );
    let mut variables = Vec::new();
    for (file, code) in &files {
        for pattern in ["env::var(", "env::var_os(", "env!(", "option_env!("] {
            let mut rest = code.as_str();
            while let Some(p) = rest.find(pattern) {
                let tail = &rest[p + pattern.len()..];
                let name: String = tail
                    .trim_start()
                    .trim_start_matches('"')
                    .chars()
                    .take_while(|c| *c != '"')
                    .collect();
                variables.push(((*file).to_owned(), name));
                rest = tail;
            }
        }
    }
    for (file, name) in &variables {
        assert!(
            ["XDG_SESSION_ID", "LC_MESSAGES", "LANG"].contains(&name.as_str()),
            "N-16: {file} reads the environment variable `{name}`"
        );
    }
    assert_eq!(
        variables.len(),
        3,
        "N-16: exactly the three documented reads: {variables:?}"
    );
}

/// N-18/N-20 and the niri rule's name: the layer-shell namespace
/// `celestina-polkit-prompt` is written once in production source (as
/// `NAMESPACE`), so a `layer-rule` in the author's niri configuration names a
/// single thing. A behavioural test cites it:
/// `polkit::state::tests::the_card_goes_on_the_focused_output_with_the_exclusive_keyboard_and_the_one_namespace`.
#[test]
fn n24_the_prompt_namespace_literal_appears_once_in_production_and_a_test_cites_it() {
    let mut occurrences = Vec::new();
    for file in rust_files(&crate_root().join("src")) {
        let relative = match file.strip_prefix(crate_root()) {
            Ok(relative) => relative.to_string_lossy().into_owned(),
            Err(_) => continue,
        };
        if TEST_ONLY_FILES.iter().any(|(f, _, _)| *f == relative)
            || relative.ends_with("/tests.rs")
            || relative.ends_with("/testing.rs")
        {
            continue;
        }
        let code = production(&read(&relative));
        for (line, text, _) in hits(&code, &[substring("\"celestina-polkit-prompt\"")]) {
            occurrences.push(format!("{relative}:{line}: {text}"));
        }
    }
    assert_eq!(
        occurrences.len(),
        1,
        "the namespace literal appears once in production source: {occurrences:?}"
    );
    assert!(
        occurrences
            .first()
            .is_some_and(|o| o.starts_with("src/polkit/mod.rs:")),
        "…and it is polkit::NAMESPACE: {occurrences:?}"
    );
    let cited = read("src/polkit/state/tests.rs");
    assert!(
        cited.contains("assert_eq!(NAMESPACE, \"celestina-polkit-prompt\""),
        "a test cites the namespace literal (the niri rule's name)"
    );
}

/// N-25: no surface that takes the keyboard exclusively opens while a prompt
/// holds it. The only production code outside the polkit modules that asks for
/// `Exclusive` is `App::overlay_surface` (the launcher and the bubbles
/// selector, each caller refusing while `polkit_claims_keyboard()` is true,
/// before it closes or replaces anything) and `App::open_overview_surface` (the
/// overview and the shortcut sheet, which refuses before it creates its surface
/// and whose verb refuses too). The prompt's host closes the overview when a
/// prompt activates. A new exclusive surface, or a new caller, fails here until
/// it is decided.
#[test]
fn n25_every_exclusive_keyboard_opener_outside_the_prompt_checks_the_claim() {
    let mut exclusive = Vec::new();
    for file in rust_files(&crate_root().join("src")) {
        let relative = match file.strip_prefix(crate_root()) {
            Ok(relative) => relative.to_string_lossy().into_owned(),
            Err(_) => continue,
        };
        if TEST_ONLY_FILES.iter().any(|(f, _, _)| *f == relative)
            || relative.ends_with("/tests.rs")
            || relative.ends_with("/testing.rs")
            || relative.starts_with("src/polkit/")
            || relative.starts_with("src/providers/polkit/")
        {
            continue;
        }
        let code = production(&read(&relative));
        for (line, text, _) in hits(&code, &[substring("KeyboardInteractivity::Exclusive")]) {
            exclusive.push(format!("{relative}:{line}: {text}"));
        }
    }
    assert_eq!(
        exclusive.len(),
        2,
        "two exclusive keyboard surfaces outside the prompt, in App::overlay_surface and App::open_overview_surface: {exclusive:?}"
    );
    assert!(
        exclusive.iter().all(|e| e.starts_with("src/app.rs:")),
        "…both in src/app.rs: {exclusive:?}"
    );
    let app = production(&read("src/app.rs"));
    // The text of the function containing byte offset `at`, up to `at`.
    let function_head = |at: usize| -> (String, String) {
        let start = app[..at].rfind("\n    fn ").unwrap_or(0);
        let name: String = app[start..]
            .trim_start()
            .strip_prefix("fn ")
            .unwrap_or("")
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        (name, app[start..at].to_owned())
    };
    let mut creators = Vec::new();
    for (at, _) in app.match_indices("KeyboardInteractivity::Exclusive") {
        let (name, head) = function_head(at);
        creators.push(name.clone());
        if name == "open_overview_surface" {
            assert!(
                head.contains("self.polkit_claims_keyboard()"),
                "`{name}` creates an exclusive overlay without refusing while a polkit prompt holds the keyboard"
            );
        }
    }
    creators.sort();
    assert_eq!(
        creators,
        ["open_overview_surface", "overlay_surface"],
        "the two exclusive creators of src/app.rs"
    );
    let mut callers = 0;
    for (at, _) in app.match_indices("self.overlay_surface(") {
        let (name, head) = function_head(at);
        assert!(
            head.contains("self.polkit_claims_keyboard()"),
            "`{name}` opens an exclusive overlay without refusing while a polkit prompt holds the keyboard"
        );
        callers += 1;
    }
    assert!(
        callers >= 2,
        "the launcher and the bubbles selector both open through overlay_surface ({callers})"
    );
    // The overview's verbs refuse as well, and the prompt's host closes the
    // overview when it activates.
    let verb = app
        .find("fn overview_verb(")
        .map(|at| &app[at..at + 700])
        .unwrap_or("");
    assert!(
        verb.contains("self.polkit_claims_keyboard()"),
        "overview_verb refuses while a polkit prompt holds the keyboard"
    );
    let host = app
        .find("fn close_keyboard_surfaces(")
        .map(|at| &app[at..at + 2500])
        .unwrap_or("");
    assert!(
        host.contains("self.overview.panel.take()"),
        "the prompt's host closes the overview"
    );
}

/// N-22: no blocking D-Bus, socket or sleep call on the loop thread. The loop
/// state, the card's view, the duty's switch and the dumpable window run on the
/// loop; the bus and the helper live in `providers/polkit/` on their own
/// threads. A heuristic by name, with review (the design marks N-22 `G/review`).
#[test]
fn n22_the_loop_side_modules_do_no_blocking_io() {
    let files = production_sources();
    let loop_side: Vec<(&str, String)> = [
        "src/polkit/state.rs",
        "src/polkit/view.rs",
        "src/polkit/mod.rs",
        "src/polkit/duty.rs",
        "src/polkit/dumpable.rs",
    ]
    .iter()
    .map(|&name| (name, only(&files, name).to_owned()))
    .collect();
    assert_none(
        "N-22: blocking IO on the loop thread",
        &loop_side,
        &[
            word("zbus"),
            word("UnixStream"),
            word("UnixListener"),
            word("TcpStream"),
            word("sleep("),
            word("recv("),
            word("recv_timeout("),
            word("join("),
            // A bounded wait is still a wait: the loop hands a thread to a
            // reaper instead (`PolkitAgent::retire`, `Helper::cancel`).
            word("join_within("),
            word("read_to_string("),
            word("read_to_end("),
            substring("std::fs::"),
            substring("std::net"),
        ],
    );
}

// ---- the scanner proves it can fail -------------------------------------------------------

/// A source in which every forbidden construct appears once as code, once in
/// a comment and once in a string, plus a test module.
const HOSTILE: &str = r##"
//! unwrap( in a module doc must not trip.
/// panic! in a doc comment must not trip.
fn code() {
    // x.unwrap() in a line comment must not trip.
    /* y.expect("m") in a /* nested */ block comment must not trip */
    let a = real.unwrap();
    let b = real.expect("boom");
    panic!("no");
    unsafe { core::hint::unreachable_unchecked() }
    call("AuthenticationAgentResponse2");
    let c = std::process::Command::new("sh");
    #[zbus::interface(name = "x.y")]
    impl Thing {}
}
#![forbid(unsafe_code)]
fn lookalikes() {
    let ok = value.unwrap_or(0);
    let ok = value.unwrap_or_default();
    let ok = unexpected(1);
}
#[cfg(test)]
mod tests {
    #[test]
    fn t() { x.unwrap(); panic!("tests may"); }
}
"##;

fn names(found: Vec<(usize, String, &'static str)>) -> Vec<&'static str> {
    let mut tokens: Vec<&'static str> = found.into_iter().map(|(_, _, t)| t).collect();
    tokens.sort_unstable();
    tokens.dedup();
    tokens
}

#[test]
fn scanner_n21_sees_code_but_not_comments_tests_or_lookalikes() {
    let code = production(HOSTILE);
    let found = names(hits(
        &code,
        &[
            word("unsafe"),
            word("unwrap("),
            word("expect("),
            word("panic!"),
        ],
    ));
    assert_eq!(found, ["expect(", "panic!", "unsafe", "unwrap("]);
    let lines: Vec<usize> = hits(&code, &[word("unwrap(")])
        .into_iter()
        .map(|(line, _, _)| line)
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "only the real call, not unwrap_or/_default or the test's: {lines:?}"
    );
}

#[test]
fn scanner_n09_and_n13_see_the_response_call_the_spawn_and_the_attribute() {
    let code = production(HOSTILE);
    assert_eq!(
        names(hits(
            &code,
            &[
                substring("AuthenticationAgentResponse"),
                substring("std::process"),
                word("Command::new"),
                substring("zbus::interface"),
            ],
        )),
        [
            "AuthenticationAgentResponse",
            "Command::new",
            "std::process",
            "zbus::interface"
        ]
    );
}

#[test]
fn scanner_cuts_a_cfg_test_item_in_the_middle_of_a_function_without_hiding_the_rest() {
    let source = r#"
fn a() {
    #[cfg(test)]
    let env = seams.environment();
    #[cfg(not(test))]
    let env = Environment::read();
    #[cfg(test)]
    if let Some(hook) = &self.hook {
        hook(member);
    }
    real.unwrap();
}
struct S {
    kept: u8,
    #[cfg(test)]
    hook: Option<Box<dyn Fn(&str)>>,
    also_kept: u8,
}
impl S {
    #[cfg(test)]
    fn only_in_tests(&self) -> &str { &self.0 }
    fn after() { second.unwrap(); }
}
#[cfg(test)]
mod tests;
fn last() { third.unwrap(); }
"#;
    let code = production(source);
    assert!(!code.contains("seams.environment"), "{code}");
    assert!(
        code.contains("Environment::read"),
        "the not(test) arm is production"
    );
    assert!(
        !code.contains("hook"),
        "test-only statement and field are gone: {code}"
    );
    assert!(!code.contains("only_in_tests"));
    assert!(code.contains("kept: u8") && code.contains("also_kept"));
    assert_eq!(
        hits(&code, &[word("unwrap(")]).len(),
        3,
        "code after every cut is still scanned: {code}"
    );
}

#[test]
fn scanner_is_not_fooled_by_braces_and_comment_markers_in_literals() {
    let source = r##"
fn a() {
    let text = "// not a comment { and an open brace";
    let raw = r#"/* nor this " } "#;
    let quote = '"';
    let brace = '{';
    let lifetime: &'static str = "x";
    x.unwrap();
}
#[cfg(test)]
mod tests {
    fn t() { let s = "}"; }
}
fn after() { y.unwrap(); }
"##;
    let code = production(source);
    assert_eq!(hits(&code, &[word("unwrap(")]).len(), 2, "{code}");
    assert!(
        code.contains("// not a comment"),
        "literals are kept for the literal guards"
    );
}

#[test]
fn scanner_n05_sees_a_string_buffer_and_a_growing_push() {
    let source = r#"
fn bad(secret: &Secret) {
    let s = format!("{}", 1);
    buffer.push_str("x");
    v.extend_from_slice(b"y");
    let t = String::from("z");
}
"#;
    let code = production(source);
    assert_eq!(
        names(hits(
            &code,
            &[
                word("format!"),
                word("push_str"),
                word("extend_from_slice"),
                word("String::"),
            ],
        )),
        ["String::", "extend_from_slice", "format!", "push_str"]
    );
}

#[test]
fn scanner_n02_whole_word_matches_the_type_and_not_its_neighbours() {
    let code = production("use x::{Secret, Cookie};\nfn f(_: SecretField, _: CookieJar) {}\n");
    let found = hits(&code, &[word("Secret"), word("Cookie")]);
    assert_eq!(
        found.len(),
        2,
        "Secret and Cookie on line 1 only: {found:?}"
    );
    assert!(found.iter().all(|(line, _, _)| *line == 1));
}

#[test]
fn scanner_n18_sees_an_undeclared_model_field_and_a_shown_literal() {
    let source = r#"
fn card(model: &CardModel) {
    text(model.message);
    text(model.details);
    text("Hello, administrator");
    text(Key::named("polkit-card"));
}
"#;
    let code = production(source);
    assert_eq!(used_model_fields(&code), ["details", "message"]);
    assert_ne!(used_model_fields(&code), allowed_model_fields());
    assert_eq!(shown_literals(&code), ["text(\"Hello, administrator\");"]);
}

#[test]
fn scanner_n03_sees_the_shared_field_by_import_and_by_path() {
    for hostile in [
        "use celestina_ui::{view, Field, Key};\n",
        "use celestina_ui::Field;\n",
        "let f = celestina_ui::Field::new();\n",
        "use celestina_ui::{\n    view,\n    Field,\n};\n",
    ] {
        assert!(!field_uses(&production(hostile)).is_empty(), "{hostile}");
    }
    for fine in [
        "use celestina_ui::{view, Key, SecretFieldState, View};\n",
        "enum Focus { Field }\nfn f() { let _ = Focus::Field; }\n",
    ] {
        assert!(field_uses(&production(fine)).is_empty(), "{fine}");
    }
}

#[test]
fn scanner_n24_finds_the_namespace_literal_only_in_code() {
    let source = r#"
// "celestina-polkit-prompt" in a comment
pub const NAMESPACE: &str = "celestina-polkit-prompt";
#[cfg(test)]
mod tests { const AGAIN: &str = "celestina-polkit-prompt"; }
"#;
    let code = production(source);
    assert_eq!(
        hits(&code, &[substring("\"celestina-polkit-prompt\"")]).len(),
        1
    );
}
