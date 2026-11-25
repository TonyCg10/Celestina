//! The `shell.json` files the project's scripts write name every duty
//! (CEL-2-M review, I1 and M4; re-review m-1). Since the cutover a key left
//! out of `duties` reads as on, and a nest without fakes (the wallpaper
//! backdrop probe) or with the host's runtime directory would start the
//! worker it did not name against the host's PipeWire, monitors or Melibea
//! socket. The key list comes from the domain's own `Duties`, so a duty added
//! there (CEL-2-I's `polkit`, say) fails this test until every script names
//! it; the shell lists in the scripts cannot get a compile error the way the
//! Rust `Duties::all_off()` does.
//!
//! The scan is inverted so that nothing slips past it silently: every use of
//! the word `duties` in any file under `scripts/` (subdirectories included)
//! must be one of a literal `"duties": {...}` object (single- or multi-line,
//! quotes escaped or not), the smoke's `$(duties ...)` helper (as a value, on
//! its own, or as the argument a `printf` format's `"duties":%s` takes), the
//! helper's own definition, or a comment line. Anything else fails the test
//! with its file and line, so an escaped literal missing a key, a `printf`
//! fed another argument, a `'"$var"'` or a `$(cat file)` is a failure, not a
//! form the scan did not know to look at.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use celestina_shell::settings::Duties;

fn scripts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts")
}

/// Every regular file under `dir`, recursively, as its path relative to
/// `dir` and its text, sorted by path.
fn scripts_under(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, found: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("{}: {err}", dir.display())) {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, found);
                continue;
            }
            let text =
                fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            let name = path
                .strip_prefix(root)
                .expect("under the root")
                .to_string_lossy()
                .into_owned();
            found.push((name, text));
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    assert!(
        !found.is_empty(),
        "no scripts found under {}",
        dir.display()
    );
    found
}

/// Every file under `scripts/`, with its text.
fn scripts() -> Vec<(String, String)> {
    scripts_under(&scripts_dir())
}

/// The keys of `Duties` as the shell writes them.
fn duty_keys() -> BTreeSet<String> {
    let value = serde_json::to_value(Duties::default()).expect("duties serialize");
    let keys: BTreeSet<String> = value
        .as_object()
        .expect("an object")
        .keys()
        .cloned()
        .collect();
    assert!(
        keys.contains("clipboard") && keys.contains("lock"),
        "{keys:?}"
    );
    keys
}

/// A use of the word `duties` the scan does not accept.
#[derive(Debug, PartialEq, Eq)]
struct Violation {
    line: usize,
    text: String,
    why: &'static str,
}

/// What a scan of one script found: the literal `duties` objects (their
/// text, with `\"` unescaped so the keys read the same either way) and the
/// uses of the word that are none of the accepted forms.
#[derive(Debug, Default, PartialEq, Eq)]
struct Scan {
    objects: Vec<String>,
    violations: Vec<Violation>,
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Whether `rest` starts with a call of the helper: `$(duties` followed by
/// its closing parenthesis or an argument.
fn starts_with_helper_call(rest: &str) -> bool {
    rest.strip_prefix("$(duties")
        .is_some_and(|after| after.starts_with(')') || after.starts_with(char::is_whitespace))
}

/// The text of a literal object starting at the `{` that opens `value`,
/// braces balanced across lines, or `None` when it never closes.
fn literal_object(value: &str) -> Option<String> {
    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(value[..=index].replace("\\\"", "\""));
                }
            }
            _ => {}
        }
    }
    None
}

/// The words of a shell command line, quotes and `$( ... )` kept whole so an
/// argument like `"$(duties phone bubbles)"` is one word.
fn shell_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut single, mut double, mut escaped) = (false, false, false);
    let mut parens = 0usize;
    for ch in text.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if !single => {
                word.push(ch);
                escaped = true;
            }
            '\'' if !double && parens == 0 => {
                single = !single;
                word.push(ch);
            }
            '"' if !single => {
                double = !double;
                word.push(ch);
            }
            '(' if !single => {
                parens += 1;
                word.push(ch);
            }
            ')' if !single && parens > 0 => {
                parens -= 1;
                word.push(ch);
            }
            ch if ch.is_whitespace() && !single && !double && parens == 0 => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            ch => word.push(ch),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// Whether `line` is a `printf` whose format carries `"duties":%s` and whose
/// argument for that conversion is the helper, `"$(duties ...)"`.
fn printf_feeds_the_helper(line: &str) -> bool {
    let Some(at) = line.find("printf") else {
        return false;
    };
    let words = shell_words(&line[at + "printf".len()..]);
    let Some(format) = words.first() else {
        return false;
    };
    let Some(key) = format.find("\"duties\"") else {
        return false;
    };
    let after_key = format[key + "\"duties\"".len()..].trim_start();
    let Some(after_colon) = after_key.strip_prefix(':') else {
        return false;
    };
    if !after_colon.trim_start().starts_with("%s") {
        return false;
    }
    // The conversions before this one, `%%` being a literal percent.
    let mut conversions = 0usize;
    let mut chars = format[..key].chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            if chars.peek() == Some(&'%') {
                chars.next();
            } else {
                conversions += 1;
            }
        }
    }
    words
        .get(1 + conversions)
        .map(|argument| {
            let bare = argument
                .strip_prefix('"')
                .and_then(|a| a.strip_suffix('"'))
                .unwrap_or(argument);
            starts_with_helper_call(bare)
        })
        .unwrap_or(false)
}

/// Classifies every use of the word `duties` in `text`.
fn scan(text: &str) -> Scan {
    let mut found = Scan::default();
    for (at, _) in text.match_indices("duties") {
        let before = &text[..at];
        let after = &text[at + "duties".len()..];
        if before.chars().next_back().is_some_and(is_word_char)
            || after.chars().next().is_some_and(is_word_char)
        {
            continue;
        }
        let line_start = before.rfind('\n').map_or(0, |nl| nl + 1);
        let line_end = after
            .find('\n')
            .map_or(text.len(), |nl| at + "duties".len() + nl);
        let line = &text[line_start..line_end];
        let line_number = before.matches('\n').count() + 1;
        let on_line_before = &text[line_start..at];
        let violation = |why| Violation {
            line: line_number,
            text: line.trim().to_owned(),
            why,
        };
        if line.trim_start().starts_with('#') {
            continue;
        }
        if on_line_before.trim().is_empty() && after.starts_with("()") {
            continue;
        }
        if on_line_before.ends_with("$(")
            && (after.starts_with(')') || after.starts_with(char::is_whitespace))
        {
            continue;
        }
        let quoted_key =
            on_line_before.ends_with('"') && (after.starts_with('"') || after.starts_with("\\\""));
        if !quoted_key {
            found.violations.push(violation(
                "the word duties outside a \"duties\": key, the helper or a comment",
            ));
            continue;
        }
        let after_quote = after
            .strip_prefix("\\\"")
            .or_else(|| after.strip_prefix('"'))
            .expect("a closing quote");
        let Some(after_colon) = after_quote.trim_start().strip_prefix(':') else {
            found
                .violations
                .push(violation("a \"duties\" string that is not a key"));
            continue;
        };
        let value = after_colon.trim_start();
        if value.starts_with('{') {
            match literal_object(value) {
                Some(object) => found.objects.push(object),
                None => found
                    .violations
                    .push(violation("a duties object that never closes")),
            }
            continue;
        }
        // `'"$(duties)"'` closes a single-quoted piece to splice the helper.
        let bare = value.trim_start_matches(['\'', '"']);
        if starts_with_helper_call(bare) {
            continue;
        }
        if value.starts_with("%s") {
            if !printf_feeds_the_helper(line) {
                found.violations.push(violation(
                    "a printf whose \"duties\":%s is not fed \"$(duties ...)\"",
                ));
            }
            continue;
        }
        found.violations.push(violation(
            "a duties value that is neither a literal object nor $(duties ...)",
        ));
    }
    found
}

/// The quoted keys of a JSON object literal, in order of appearance.
fn keys_of(object: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = object;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else { break };
        let word = &after[..close];
        let following = after[close + 1..].trim_start();
        if following.starts_with(':') {
            keys.push(word.to_owned());
        }
        rest = &after[close + 1..];
    }
    keys
}

/// `scripts/smoke.sh` plus every `*.sh` of `scripts/smoke/`, sorted, as
/// (path relative to `scripts/`, text). Fails when the phase directory is
/// missing or holds fewer than the 15 files of the layout (`common.sh` and
/// the 14 phases).
fn smoke_sources() -> Vec<(String, String)> {
    let all = scripts();
    let mut found: Vec<(String, String)> = all
        .into_iter()
        .filter(|(name, _)| {
            name == "smoke.sh" || (name.starts_with("smoke/") && name.ends_with(".sh"))
        })
        .collect();
    found.sort();
    let phase_files = found
        .iter()
        .filter(|(n, _)| n.starts_with("smoke/"))
        .count();
    assert!(
        phase_files >= 15,
        "scripts/smoke/ holds {phase_files} .sh files; the layout needs common.sh and 14 phases"
    );
    assert!(found.iter().any(|(n, _)| n == "smoke.sh"), "smoke.sh");
    found
}

/// The names in the smoke's `duties` helper (`for d_name in ...; do`, kept in
/// `smoke/common.sh`), which prints the object every run of the smoke writes.
fn smoke_helper_names(text: &str) -> BTreeSet<String> {
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("for d_name in "))
        .expect("the smoke keeps its duties helper");
    let names = line
        .trim_start()
        .strip_prefix("for d_name in ")
        .and_then(|rest| rest.strip_suffix("; do"))
        .expect("the helper's list ends the line");
    names.split_whitespace().map(str::to_owned).collect()
}

fn describe(name: &str, violations: &[Violation]) -> String {
    violations
        .iter()
        .map(|v| format!("{name}:{}: {} ({})", v.line, v.why, v.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_use_of_the_word_duties_in_a_script_is_a_literal_object_or_the_helper() {
    let mut problems = Vec::new();
    let mut uses = 0;
    for (name, text) in scripts() {
        let found = scan(&text);
        uses += found.objects.len();
        if !found.violations.is_empty() {
            problems.push(describe(&name, &found.violations));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(
        uses >= 5,
        "the scripts hold {uses} duties objects; the scan is broken"
    );
}

#[test]
fn every_duties_object_a_script_writes_names_every_duty() {
    let wanted = duty_keys();
    let mut seen = 0;
    for (name, text) in scripts() {
        for object in scan(&text).objects {
            seen += 1;
            let keys: BTreeSet<String> = keys_of(&object).into_iter().collect();
            let missing: Vec<&String> = wanted.difference(&keys).collect();
            assert!(
                missing.is_empty(),
                "{name} writes a duties object without {missing:?}: {object}"
            );
        }
    }
    assert!(
        seen >= 5,
        "the scripts hold {seen} duties objects; the scan is broken"
    );
}

#[test]
fn the_smoke_helper_lists_every_duty_and_nothing_else() {
    let sources = smoke_sources();
    let (_, common) = sources
        .iter()
        .find(|(name, _)| name == "smoke/common.sh")
        .expect("smoke/common.sh");
    assert_eq!(smoke_helper_names(common), duty_keys());
    // The helper is the only one: no other file of the smoke repeats the loop.
    for (name, text) in &sources {
        if name != "smoke/common.sh" {
            assert!(
                !text
                    .lines()
                    .any(|line| line.trim_start().starts_with("for d_name in ")),
                "{name} repeats the duties list of smoke/common.sh"
            );
        }
    }
}

#[test]
fn exactly_one_file_of_the_smoke_defines_the_duties_helper() {
    let definers: Vec<String> = smoke_sources()
        .into_iter()
        .filter(|(_, text)| {
            text.lines()
                .any(|line| line.trim_start().starts_with("duties()"))
        })
        .map(|(name, _)| name)
        .collect();
    assert_eq!(definers, ["smoke/common.sh"]);
}

#[test]
fn no_script_writes_a_bar_duty_any_more() {
    // The bar is not a duty since the cutover; a `"bar"` key inside `duties`
    // would be an unknown key, read past and dropped on the next write.
    for (name, text) in scripts() {
        for object in scan(&text).objects {
            assert!(
                !keys_of(&object).iter().any(|key| key == "bar"),
                "{name} writes a bar duty: {object}"
            );
        }
        assert!(
            !text.contains(r#"\"bar\":\"on\""#),
            "{name} writes a bar duty"
        );
        assert!(!text.contains(r#""bar":"on""#), "{name} writes a bar duty");
        assert!(!text.contains(r#""bar": "on""#), "{name} writes a bar duty");
    }
}

#[test]
fn the_scan_accepts_every_form_the_scripts_use() {
    // Single- and multi-line literal objects, escaped or not; the helper as
    // a value, inside `'"..."'`, on its own, and as the argument of a
    // `printf` whose format has `"duties":%s` (first or a later conversion);
    // the helper's definition; comment lines.
    let text = r#"
# the duties helper prints the duties object
duties() {
    for d_name in a b; do :; done
}
printf '{"schema_version":1,"duties":%s}\n' "$(duties)" > f
printf '{"schema_version":1,"duties":%s,"osd":{"timeout_ms":4000}}\n' "$(duties osd)" \
    > g
printf '{"a":"%s","b":%%,"duties":%s}\n' "$x" "$(duties phone bubbles)" > h
cat > g <<JSON
{
  "schema_version": 1,
  "duties": {
    "idle": "on", "caffeine": "off",
    "lock": "on"
  },
  "bar": { "end": ["tray"] }
}
JSON
printf '%s\n' '{"duties":{"lock":"on","idle":"off"},"wallpaper":{"folder":"x"}}'
printf '{"duties":{"lock":"on","idle":"%s"}}\n' "$state"
"duties": $(duties)
run '{"schema_version":1,"duties":'"$(duties)"',"bar":{}}'
off="{\"schema_version\":1,\"duties\":$(duties)}"
esc="{\"duties\":{\"lock\":\"on\",\"idle\":\"off\"}}"
alone=$(duties idle)
"#;
    let found = scan(text);
    assert_eq!(
        found.violations,
        [],
        "{}",
        describe("text", &found.violations)
    );
    assert_eq!(found.objects.len(), 4, "{:?}", found.objects);
    assert_eq!(keys_of(&found.objects[0]), ["idle", "caffeine", "lock"]);
    assert_eq!(keys_of(&found.objects[1]), ["lock", "idle"]);
    assert_eq!(keys_of(&found.objects[2]), ["lock", "idle"]);
    assert_eq!(
        keys_of(&found.objects[3]),
        ["lock", "idle"],
        "escaped quotes read as keys"
    );
    assert_eq!(
        smoke_helper_names("x\n    for d_name in a b c; do\n"),
        ["a", "b", "c"].into_iter().map(str::to_owned).collect()
    );
}

#[test]
fn the_scan_fails_every_use_that_is_neither_a_literal_object_nor_the_helper() {
    // The evasions of the re-review (m-1), each named with its line.
    for (text, why) in [
        (
            r#"printf '{"duties":%s}' '{"idle":"off"}'"#,
            "a printf whose \"duties\":%s is not fed \"$(duties ...)\"",
        ),
        (
            r#"printf '{"a":"%s","duties":%s}' "$(duties)" "$x""#,
            "a printf whose \"duties\":%s is not fed \"$(duties ...)\"",
        ),
        (
            r#"printf '{"duties":%s}' "$(duties_evil)""#,
            "a printf whose \"duties\":%s is not fed \"$(duties ...)\"",
        ),
        (
            r#"printf '{"duties":%s}'"#,
            "a printf whose \"duties\":%s is not fed \"$(duties ...)\"",
        ),
        (
            r#"run '{"duties":'"$var"'}'"#,
            "a duties value that is neither a literal object nor $(duties ...)",
        ),
        (
            r#"  "duties": $(cat some-file),"#,
            "a duties value that is neither a literal object nor $(duties ...)",
        ),
        (
            r#"  "duties": $(duties_evil),"#,
            "a duties value that is neither a literal object nor $(duties ...)",
        ),
        (
            r#"  "duties": "off","#,
            "a duties value that is neither a literal object nor $(duties ...)",
        ),
        (
            r#"x="{\"duties\":$(cat f)}""#,
            "a duties value that is neither a literal object nor $(duties ...)",
        ),
        (
            r#"echo "every duties object""#,
            "the word duties outside a \"duties\": key, the helper or a comment",
        ),
        (
            r#"grep '"duties"' file"#,
            "a \"duties\" string that is not a key",
        ),
        (
            "x=\"{\\\"duties\\\":{\\\"idle\\\":\\\"off\\\"\"",
            "a duties object that never closes",
        ),
    ] {
        let found = scan(&format!("# a comment\n{text}\n"));
        assert_eq!(found.violations.len(), 1, "{text}: {found:?}");
        assert_eq!(found.violations[0].why, why, "{text}");
        assert_eq!(found.violations[0].line, 2, "{text}");
        assert_eq!(found.violations[0].text, text.trim(), "{text}");
    }
    // The escaped literal of the review is read as an object: it then fails
    // the key check, not the scan.
    let found = scan(r#"x="{\"duties\":{\"idle\":\"off\"}}""#);
    assert_eq!(found.violations, []);
    assert_eq!(found.objects, [r#"{"idle":"off"}"#]);
    assert_eq!(keys_of(&found.objects[0]), ["idle"]);
    assert!(!duty_keys().is_subset(&keys_of(&found.objects[0]).into_iter().collect()));
}

#[test]
fn scripts_in_subdirectories_are_scanned_too() {
    let dir = std::env::temp_dir().join(format!("celestina-duty-files-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("nested/deeper")).expect("the directories");
    fs::write(dir.join("top.sh"), "# duties\n").expect("write");
    fs::write(
        dir.join("nested/deeper/hidden.py"),
        "write('{\"duties\": \"off\"}')\n",
    )
    .expect("write");
    let found = scripts_under(&dir);
    let _ = fs::remove_dir_all(&dir);
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["nested/deeper/hidden.py", "top.sh"]);
    let (_, hidden) = &found[0];
    let scanned = scan(hidden);
    assert_eq!(scanned.violations.len(), 1, "{scanned:?}");
    assert_eq!(scanned.violations[0].line, 1);
    assert!(scan(&found[1].1).violations.is_empty());
}
