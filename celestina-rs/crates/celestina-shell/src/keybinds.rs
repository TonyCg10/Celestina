//! What niri's `config.kdl` binds, as groups of chord and label for the
//! shortcuts page.
//!
//! The file is the person's own, but it is still text from outside the
//! program: it is bounded before it is parsed (size), while it is read (groups,
//! binds) and before it is shown (chord and label lengths). Nothing here
//! touches the filesystem. `include` nodes are only *reported*; the reader
//! resolves them inside the config's directory and calls [`parse`] again for
//! each file, never deeper than [`MAX_INCLUDE_DEPTH`].
//!
//! A bind whose title is `null` (`hotkey-overlay-title=null`) is hidden by its
//! owner and is skipped here, as niri's own overlay skips it.
//!
//! The model keeps words apart by who wrote them: the `hotkey-overlay-title`
//! is the person's own text, the action is niri's raw token
//! (`focus-column-left`), and a `spawn` keeps only its program's basename.
//! Nothing here turns a token into prose; the product's copy decides how each
//! is shown.
//!
//! The `kdl` parser is recursive, so nesting is bounded *before* it runs:
//! [`check_nesting`] refuses text whose `{` blocks or `/* */` comments nest
//! deeper than [`MAX_NESTING`], which would otherwise exhaust the stack. That
//! scan is only a cheap first defence: it follows the lexical shapes that nest
//! braces and comments, and the parser has other recursive paths (slashdash
//! comments, type annotations, property chains, failed escapes) that small
//! crafted text still drives into a stack overflow, which aborts the process.
//! A program that parses a file it does not control must therefore run
//! [`parse_file`] where an abort costs nothing: the shell runs it only in a
//! child process (`celestina --keybinds-probe`).

use crate::bounded::bounded;
use kdl::{KdlDocument, KdlNode, KdlValue};
use std::fmt;

/// The largest config file this module reads, in bytes.
pub const MAX_FILE_BYTES: usize = 256 * 1024;
/// The most binds kept across all groups; later ones are dropped.
pub const MAX_BINDS: usize = 512;
/// The most groups kept (the normal binds plus binding modes).
pub const MAX_GROUPS: usize = 24;
/// The longest chord kept, in UTF-16 code units (see [`bounded`]).
pub const MAX_CHORD_UNITS: usize = 48;
/// The longest label kept, in UTF-16 code units (see [`bounded`]).
pub const MAX_LABEL_UNITS: usize = 96;
/// How many levels of `include` a reader follows. The root file is depth 0, so
/// a file read at depth [`MAX_INCLUDE_DEPTH`] has its own includes ignored.
pub const MAX_INCLUDE_DEPTH: usize = 2;
/// The most `include` paths one file reports.
pub const MAX_INCLUDES: usize = 32;
/// The deepest nesting of `{` blocks, and separately of `/* */` comments,
/// offered to the parser. niri's own config nests three or four levels. The
/// bound is set by the smallest stack the parser may run on: an unoptimised
/// build spends about 35 KiB of stack per level, so 64 levels already
/// overflow a 2 MiB test thread while 32 leave room. The shell parses only in
/// a child process, on that process's main stack.
pub const MAX_NESTING: usize = 32;

/// One shortcut: the chord as written and what it does, as the config says
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind {
    /// The chord as written in the config, such as `Mod+Shift+A`.
    pub chord: String,
    /// The bind's `hotkey-overlay-title`, the person's own words, when set.
    pub title: Option<String>,
    /// The action's raw token as written (`focus-column-left`, `spawn`);
    /// empty only for a bind that has a title and no action.
    pub action: String,
    /// For `spawn`, the basename of the program it starts, when it names one.
    pub program: Option<String>,
}

/// A set of binds that are live together.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BindGroup {
    /// `None` for the normal `binds` block, the mode name for a binding mode.
    pub name: Option<String>,
    /// For a binding mode, the chord of the first bind that runs
    /// `enter-binding-mode` for it; `None` when nothing enters it (or for the
    /// normal binds).
    pub enter: Option<String>,
    /// The binds, in file order.
    pub binds: Vec<Bind>,
}

/// What one file said: its groups and the `include` paths it names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    /// The groups, normal binds first when the file lists them first.
    pub groups: Vec<BindGroup>,
    /// The `include` paths as written, never followed here.
    pub includes: Vec<String>,
}

/// Why a file could not be read as a niri config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindError {
    /// The file is larger than [`MAX_FILE_BYTES`].
    TooLarge {
        /// The size that was offered, in bytes.
        bytes: usize,
    },
    /// The text is not KDL.
    Syntax(String),
}

impl fmt::Display for KeybindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { bytes } => {
                write!(
                    f,
                    "config is {bytes} bytes, over the {MAX_FILE_BYTES} limit"
                )
            }
            Self::Syntax(reason) => write!(f, "config is not valid KDL: {reason}"),
        }
    }
}

impl std::error::Error for KeybindError {}

/// Reads the groups of one config file. See [`parse_file`] for the includes.
///
/// # Errors
///
/// [`KeybindError::TooLarge`] over the size bound, [`KeybindError::Syntax`]
/// when the text is not KDL.
pub fn parse(text: &str) -> Result<Vec<BindGroup>, KeybindError> {
    parse_file(text).map(|parsed| parsed.groups)
}

/// Reads one config file: its groups and the `include` paths it reports.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_file(text: &str) -> Result<Parsed, KeybindError> {
    if text.len() > MAX_FILE_BYTES {
        return Err(KeybindError::TooLarge { bytes: text.len() });
    }
    check_nesting(text)?;
    let document: KdlDocument = text
        .parse()
        .map_err(|error: kdl::KdlError| KeybindError::Syntax(bounded(&error.to_string(), 160)))?;

    let mut parsed = Parsed::default();
    let mut total = 0usize;
    for node in document.nodes() {
        match node.name().value() {
            "include" => {
                if parsed.includes.len() < MAX_INCLUDES {
                    if let Some(path) = first_string(node) {
                        parsed.includes.push(path.to_owned());
                    }
                }
            }
            "binds" => push_group(&mut parsed.groups, &mut total, None, node),
            "binding-mode" => {
                if let Some(name) = first_string(node) {
                    let name = bounded(name, MAX_LABEL_UNITS);
                    push_group(&mut parsed.groups, &mut total, Some(name), node);
                }
            }
            _ => {}
        }
    }
    link_entries(&mut parsed.groups, &document);
    Ok(parsed)
}

/// Names, for each binding mode, the chord that enters it.
fn link_entries(groups: &mut [BindGroup], document: &KdlDocument) {
    let mut entries: Vec<(String, String)> = Vec::new();
    for node in document
        .nodes()
        .iter()
        .filter(|n| n.name().value() == "binds")
    {
        let Some(children) = node.children() else {
            continue;
        };
        for bind in children.nodes() {
            let target = bind
                .children()
                .and_then(|c| c.nodes().first())
                .filter(|action| action.name().value() == "enter-binding-mode")
                .and_then(first_string);
            if let Some(target) = target {
                entries.push((
                    target.to_owned(),
                    bounded(bind.name().value(), MAX_CHORD_UNITS),
                ));
            }
        }
    }
    for group in groups {
        if let Some(name) = &group.name {
            group.enter = entries
                .iter()
                .find(|(target, _)| bounded(target, MAX_LABEL_UNITS) == *name)
                .map(|(_, chord)| chord.clone());
        }
    }
}

fn push_group(
    groups: &mut Vec<BindGroup>,
    total: &mut usize,
    name: Option<String>,
    node: &KdlNode,
) {
    if groups.len() >= MAX_GROUPS {
        return;
    }
    let mut group = BindGroup {
        name,
        enter: None,
        binds: Vec::new(),
    };
    if let Some(children) = node.children() {
        for bind in children.nodes() {
            if *total >= MAX_BINDS {
                break;
            }
            if let Some(bind) = read_bind(bind) {
                group.binds.push(bind);
                *total += 1;
            }
        }
    }
    groups.push(group);
}

fn read_bind(node: &KdlNode) -> Option<Bind> {
    let chord = node.name().value();
    if chord.is_empty() {
        return None;
    }
    let title = node
        .entries()
        .iter()
        .find(|entry| {
            entry
                .name()
                .is_some_and(|name| name.value() == "hotkey-overlay-title")
        })
        .map(|entry| entry.value());
    let title = match title {
        Some(KdlValue::Null) => return None,
        Some(KdlValue::String(text)) if !text.trim().is_empty() => {
            Some(bounded(text, MAX_LABEL_UNITS))
        }
        _ => None,
    };
    let action = node
        .children()
        .and_then(|children| children.nodes().first());
    let (action, program) = match action {
        Some(action) => {
            let token = action.name().value();
            if token.is_empty() {
                return None;
            }
            let program = (token == "spawn")
                .then(|| first_string(action))
                .flatten()
                .and_then(|program| program.rsplit('/').next())
                .filter(|base| !base.is_empty())
                .map(|base| bounded(base, MAX_LABEL_UNITS));
            (bounded(token, MAX_LABEL_UNITS), program)
        }
        None if title.is_some() => (String::new(), None),
        None => return None,
    };
    Some(Bind {
        chord: bounded(chord, MAX_CHORD_UNITS),
        title,
        action,
        program,
    })
}

/// Refuses `text` whose `{` blocks or `/* */` comments nest deeper than
/// [`MAX_NESTING`], before the recursive parser sees it.
///
/// The scan follows KDL's lexical shapes well enough to skip braces inside
/// strings (escaped, raw `#"…"#`, multi-line `"""…"""`) and comments. Where
/// it could misread exotic text it errs towards counting more, never fewer:
/// a `}` lowers the count only where the scan is sure it is code, and a line
/// comment ends at every newline KDL knows. A KDL v1 raw string (`r"…"`,
/// `r#"…"#`) is the one exception to skipping: the v2 parser, which runs
/// first, reads its body as code while it fails, and recurses on the braces
/// there, so every `{` inside one counts (and no `}` does).
///
/// # Errors
///
/// [`KeybindError::Syntax`] when either nesting passes the bound.
pub fn check_nesting(text: &str) -> Result<(), KeybindError> {
    #[derive(Clone, Copy)]
    enum Scan {
        Code,
        Line,
        Block(usize),
        Quoted,
        Multiline,
        /// A raw string closed by `"` and this many `#`; `true` for v1 (`r…`).
        Raw(usize, bool),
    }
    let too_deep = || {
        KeybindError::Syntax(format!(
            "nesting deeper than {MAX_NESTING} levels is refused"
        ))
    };
    let bytes = text.as_bytes();
    let at = |index: usize| bytes.get(index).copied();
    let mut depth = 0usize;
    let mut scan = Scan::Code;
    let mut index = 0usize;
    while let Some(byte) = at(index) {
        match scan {
            Scan::Code => match byte {
                b'{' => {
                    depth += 1;
                    if depth > MAX_NESTING {
                        return Err(too_deep());
                    }
                }
                b'}' => depth = depth.saturating_sub(1),
                b'/' if at(index + 1) == Some(b'/') => {
                    scan = Scan::Line;
                    index += 1;
                }
                b'/' if at(index + 1) == Some(b'*') => {
                    scan = Scan::Block(1);
                    index += 1;
                }
                b'"' if bytes[index..].starts_with(b"\"\"\"") => {
                    scan = Scan::Multiline;
                    index += 2;
                }
                b'"' => scan = Scan::Quoted,
                b'r' | b'#' => {
                    let from = if byte == b'r' { index + 1 } else { index };
                    let hashes = bytes[from..].iter().take_while(|b| **b == b'#').count();
                    if at(from + hashes) == Some(b'"') && (byte == b'r' || hashes > 0) {
                        scan = Scan::Raw(hashes, byte == b'r');
                        index = from + hashes;
                    } else if hashes > 0 {
                        // Not a raw string: the whole run of `#` is plain
                        // code, so it is stepped over at once. Looking at it
                        // again from each `#` would make a long run cost the
                        // square of its length.
                        index = from + hashes - 1;
                    }
                }
                _ => {}
            },
            Scan::Line => {
                let ends = matches!(byte, b'\n' | b'\r' | 0x0B | 0x0C)
                    || bytes[index..].starts_with("\u{85}".as_bytes())
                    || bytes[index..].starts_with("\u{2028}".as_bytes())
                    || bytes[index..].starts_with("\u{2029}".as_bytes());
                if ends {
                    scan = Scan::Code;
                }
            }
            Scan::Block(level) => {
                if byte == b'/' && at(index + 1) == Some(b'*') {
                    if level + 1 > MAX_NESTING {
                        return Err(too_deep());
                    }
                    scan = Scan::Block(level + 1);
                    index += 1;
                } else if byte == b'*' && at(index + 1) == Some(b'/') {
                    scan = if level <= 1 {
                        Scan::Code
                    } else {
                        Scan::Block(level - 1)
                    };
                    index += 1;
                }
            }
            Scan::Quoted => match byte {
                b'\\' => index += 1,
                b'"' => scan = Scan::Code,
                _ => {}
            },
            Scan::Multiline => match byte {
                b'\\' => index += 1,
                b'"' if bytes[index..].starts_with(b"\"\"\"") => {
                    scan = Scan::Code;
                    index += 2;
                }
                _ => {}
            },
            Scan::Raw(hashes, v1) => {
                if v1 && byte == b'{' {
                    depth += 1;
                    if depth > MAX_NESTING {
                        return Err(too_deep());
                    }
                } else if byte == b'"'
                    && bytes
                        .get(index + 1..index + 1 + hashes)
                        .is_some_and(|tail| tail.iter().all(|b| *b == b'#'))
                {
                    scan = Scan::Code;
                    index += hashes;
                }
            }
        }
        index += 1;
    }
    Ok(())
}

fn first_string(node: &KdlNode) -> Option<&str> {
    node.entries()
        .iter()
        .find(|entry| entry.name().is_none())
        .and_then(|entry| entry.value().as_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/niri-binds.kdl");

    fn groups(text: &str) -> Vec<BindGroup> {
        parse(text).unwrap_or_default()
    }

    #[test]
    fn the_fixture_reads_as_modes_then_normal_binds() {
        let groups = groups(FIXTURE);
        let names: Vec<_> = groups.iter().map(|g| g.name.as_deref()).collect();
        assert_eq!(names, [Some("monitor"), Some("size"), None]);
        assert_eq!(groups[0].binds[0].chord, "Left");
        assert_eq!(groups[0].binds[0].action, "focus-monitor-left");
        assert_eq!(groups[0].binds[1].chord, "Shift+Right");
        assert_eq!(groups[1].binds[1].action, "set-column-width");
    }

    #[test]
    fn titles_actions_and_programs_are_kept_apart_and_verbatim() {
        let binds = &groups(FIXTURE)[2].binds;
        let bind = |chord: &str| binds.iter().find(|b| b.chord == chord);
        let parts = |chord: &str| {
            bind(chord).map(|b| (b.title.as_deref(), b.action.as_str(), b.program.as_deref()))
        };
        assert_eq!(parts("Mod+Q"), Some((None, "spawn", Some("kitty"))));
        assert_eq!(
            parts("Mod+T"),
            Some((Some("Open a terminal"), "spawn", Some("kitty")))
        );
        assert_eq!(
            parts("Mod+1"),
            Some((None, "spawn", Some("blackout-toggle")))
        );
        assert_eq!(parts("Mod+R"), Some((None, "enter-binding-mode", None)));
        assert_eq!(parts("XF86AudioMute"), Some((None, "spawn-sh", None)));
        assert!(
            bind("Mod+Space").is_none(),
            "a commented-out bind is not a bind"
        );
    }

    #[test]
    fn a_spawn_without_a_program_keeps_no_program() {
        let text = "binds {\nMod+A { spawn; }\nMod+B { spawn \"/usr/bin/\"; }\n}";
        let binds = &groups(text)[0].binds;
        assert_eq!(binds.len(), 2);
        assert!(binds
            .iter()
            .all(|b| b.action == "spawn" && b.program.is_none()));
    }

    #[test]
    fn a_mode_names_the_chord_that_enters_it() {
        let groups = groups(FIXTURE);
        assert_eq!(groups[1].enter.as_deref(), Some("Mod+R"));
        assert_eq!(groups[0].enter, None);
        assert_eq!(groups[2].enter, None);
    }

    #[test]
    fn a_null_title_hides_the_bind() {
        let text = "binds {\nMod+H hotkey-overlay-title=null { quit; }\nMod+Q { quit; }\n}";
        let binds = &groups(text)[0].binds;
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].chord, "Mod+Q");
    }

    #[test]
    fn a_bind_with_no_action_and_no_title_is_skipped() {
        let text = "binds {\nMod+A { }\nMod+B\nMod+C hotkey-overlay-title=\"Kept\" { }\n}";
        let binds = &groups(text)[0].binds;
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].title.as_deref(), Some("Kept"));
        assert_eq!(binds[0].action, "");
    }

    #[test]
    fn unknown_nodes_are_ignored() {
        let text = "input { keyboard { numlock; } }\nlayout { gaps 8 }\nbinds { Mod+Q { quit; } }";
        let groups = groups(text);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].binds.len(), 1);
    }

    #[test]
    fn includes_are_reported_and_not_followed() {
        let text = "include \"extra.kdl\"\ninclude \"sub/more.kdl\"\nbinds { Mod+Q { quit; } }";
        let parsed = parse_file(text).ok().unwrap_or_default();
        assert_eq!(parsed.includes, ["extra.kdl", "sub/more.kdl"]);
        assert_eq!(parsed.groups.len(), 1);
    }

    #[test]
    fn too_large_input_is_refused_before_parsing() {
        let text = " ".repeat(MAX_FILE_BYTES + 1);
        assert_eq!(
            parse(&text),
            Err(KeybindError::TooLarge {
                bytes: MAX_FILE_BYTES + 1
            })
        );
    }

    #[test]
    fn garbage_is_a_syntax_error_not_a_panic() {
        for hostile in [
            "binds {",
            "\u{0}\u{1}{{{",
            "}}}}",
            "binds { Mod+Q { quit; }",
            "\"unterminated",
        ] {
            assert!(
                matches!(parse(hostile), Err(KeybindError::Syntax(_))),
                "{hostile:?}"
            );
        }
    }

    #[test]
    fn over_long_chords_and_labels_are_cut() {
        let chord = "A".repeat(200);
        let title = "T".repeat(400);
        let text = format!("binds {{ {chord} hotkey-overlay-title=\"{title}\" {{ quit; }} }}");
        let bind = &groups(&text)[0].binds[0];
        assert_eq!(bind.chord.chars().count(), MAX_CHORD_UNITS);
        assert_eq!(
            bind.title.as_ref().map(|t| t.chars().count()),
            Some(MAX_LABEL_UNITS)
        );
    }

    #[test]
    fn binds_over_the_bound_are_dropped() {
        let mut text = String::from("binds {\n");
        for index in 0..(MAX_BINDS + 40) {
            text.push_str(&format!("K{index} {{ quit; }}\n"));
        }
        text.push_str("}\n");
        let groups = groups(&text);
        let total: usize = groups.iter().map(|g| g.binds.len()).sum();
        assert_eq!(total, MAX_BINDS);
    }

    #[test]
    fn the_bind_bound_spans_groups() {
        let mut text = String::new();
        for group in 0..4 {
            text.push_str(&format!("binding-mode \"m{group}\" {{\n"));
            for index in 0..200 {
                text.push_str(&format!("K{index} {{ quit; }}\n"));
            }
            text.push_str("}\n");
        }
        let total: usize = groups(&text).iter().map(|g| g.binds.len()).sum();
        assert_eq!(total, MAX_BINDS);
    }

    #[test]
    fn groups_over_the_bound_are_dropped() {
        let mut text = String::new();
        for group in 0..(MAX_GROUPS + 10) {
            text.push_str(&format!("binding-mode \"m{group}\" {{ A {{ quit; }} }}\n"));
        }
        assert_eq!(groups(&text).len(), MAX_GROUPS);
    }

    #[test]
    fn many_includes_are_capped() {
        let text: String = (0..100)
            .map(|i| format!("include \"f{i}.kdl\"\n"))
            .collect();
        let parsed = parse_file(&text).ok().unwrap_or_default();
        assert_eq!(parsed.includes.len(), MAX_INCLUDES);
    }

    #[test]
    fn deep_brace_nesting_is_refused_before_the_parser_recurses() {
        // ~1.5 KB that overflows the recursive parser's stack.
        let hostile = "a{".repeat(500);
        assert!(matches!(parse(&hostile), Err(KeybindError::Syntax(_))));
        let closed = format!("{}{}", "a{".repeat(500), "}".repeat(500));
        assert!(matches!(parse(&closed), Err(KeybindError::Syntax(_))));
    }

    #[test]
    fn deep_comment_nesting_is_refused_before_the_parser_recurses() {
        let hostile = "/*".repeat(1000);
        assert!(matches!(parse(&hostile), Err(KeybindError::Syntax(_))));
        let closed = format!("{}{}", "/*".repeat(1000), "*/".repeat(1000));
        assert!(matches!(parse(&closed), Err(KeybindError::Syntax(_))));
    }

    #[test]
    fn nesting_at_the_bound_still_parses() {
        let text = format!("{}{}", "a{".repeat(MAX_NESTING), "}".repeat(MAX_NESTING));
        assert_eq!(check_nesting(&text), Ok(()));
        assert!(parse(&text).is_ok());
        let over = format!(
            "{}{}",
            "a{".repeat(MAX_NESTING + 1),
            "}".repeat(MAX_NESTING + 1)
        );
        assert!(check_nesting(&over).is_err());
    }

    #[test]
    fn braces_in_strings_and_comments_are_not_nesting() {
        let many = "{".repeat(200);
        for text in [
            format!("binds {{ Mod+Q {{ spawn \"sh\" \"-c\" \"{many}\"; }} }}"),
            format!("binds {{ Mod+Q {{ spawn \"a\\\"{many}\"; }} }}"),
            format!("// {many}\nbinds {{ Mod+Q {{ quit; }} }}"),
            format!("/* {many} /* {many} */ */\nbinds {{ Mod+Q {{ quit; }} }}"),
        ] {
            assert_eq!(check_nesting(&text), Ok(()), "{text}");
            assert!(parse(&text).is_ok(), "{text}");
        }
        // A v2 raw and a multi-line string skip their braces too.
        assert_eq!(check_nesting(&format!("a #\"{many}\"#")), Ok(()));
        assert_eq!(
            check_nesting(&format!("a \"\"\"\n\" {many}\n\"\"\"")),
            Ok(())
        );
    }

    #[test]
    fn braces_in_a_v1_raw_string_count_because_the_v2_parser_reads_them() {
        // The v2 parser, tried first, overflows on these before the v1 one
        // reads them as strings.
        for text in [
            format!("a r\"{}\"", "{".repeat(100)),
            format!("a r#\"{}\"#", "{".repeat(100)),
            format!("a r\"{}", "{".repeat(500)),
        ] {
            assert!(matches!(parse(&text), Err(KeybindError::Syntax(_))));
        }
        // KDL v1, as niri writes it: a newline ends each node.
        let few = "binds {\n    Mod+Q { spawn-sh r\"awk '{print $1}'\"; }\n}\n";
        assert_eq!(check_nesting(few), Ok(()));
        assert!(parse(few).is_ok());
    }

    #[test]
    fn a_long_run_of_hashes_is_scanned_once() {
        // Each `#` used to look at the rest of its run: 256 KiB of them took
        // the square of that. Stepping over the run makes it linear, so this
        // ends at once in any build; the bytes count, not the clock.
        let text = "#".repeat(MAX_FILE_BYTES);
        assert_eq!(check_nesting(&text), Ok(()));
        // A run that does open a raw string still does, and one that does
        // not leaves the braces after it counted.
        let raw = format!(
            "a {}\"{}\"{}",
            "#".repeat(3),
            "{".repeat(100),
            "#".repeat(3)
        );
        assert_eq!(check_nesting(&raw), Ok(()));
        let plain = format!("{} {}", "#".repeat(50), "a{".repeat(MAX_NESTING + 1));
        assert!(check_nesting(&plain).is_err());
    }

    #[test]
    fn a_carriage_return_ends_a_line_comment_for_the_scan() {
        // The parser reads code after a `\r`; so must the scan.
        let hostile = format!("// x\r{}", "a{".repeat(100));
        assert!(check_nesting(&hostile).is_err());
    }
}
