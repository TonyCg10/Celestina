//! The appearance file: `reduced_motion` and `text_scale`.
//!
//! The file is TOML, but this module reads and rewrites only its two keys in
//! the root table, line by line. That is what lets [`save`] keep every other
//! line — an unknown key a later release added, a comment the author wrote —
//! byte for byte, which a parse-and-serialise round trip through a TOML
//! library would not. Lines this module does not recognise are kept and
//! ignored; a known key with a value it cannot read makes the whole file
//! malformed, and [`load`] then answers the defaults without touching it.

use std::path::{Path, PathBuf};

use celestina_core::atomic_file;
use celestina_core::xdg;

use crate::SettingsError;

/// The variable that forces reduced motion on, whatever the file says: the
/// suite's development and accessibility override since before the file
/// existed.
const REDUCED_MOTION_VARIABLE: &str = "CELESTINA_REDUCED_MOTION";

/// The file is two lines; anything near this is not an appearance file.
const READ_LIMIT: u64 = 64 * 1024;

const REDUCED_MOTION_KEY: &str = "reduced_motion";
const TEXT_SCALE_KEY: &str = "text_scale";

/// How large the suite draws its text, as a factor on the theme's type roles.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum TextScale {
    Compact,
    #[default]
    Normal,
    Large,
    Larger,
}

impl TextScale {
    /// The factor `CelestinaTheme.textScale` takes.
    #[must_use]
    pub fn factor(self) -> f64 {
        match self {
            Self::Compact => 0.9,
            Self::Normal => 1.0,
            Self::Large => 1.15,
            Self::Larger => 1.3,
        }
    }

    /// The value's spelling in the file.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Normal => "normal",
            Self::Large => "large",
            Self::Larger => "larger",
        }
    }

    /// The value a file spelling names, if it names one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "compact" => Some(Self::Compact),
            "normal" => Some(Self::Normal),
            "large" => Some(Self::Large),
            "larger" => Some(Self::Larger),
            _ => None,
        }
    }
}

/// The suite's appearance choices.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Appearance {
    pub reduced_motion: bool,
    pub text_scale: TextScale,
}

/// `$XDG_CONFIG_HOME/celestina/appearance.toml`.
///
/// When neither `$XDG_CONFIG_HOME` nor `$HOME` is absolute there is no
/// configuration directory; this then answers the relative
/// `celestina/appearance.toml` for messages only, and [`load`], [`save`] and
/// [`watch`](crate::watch) refuse to use it.
#[must_use]
pub fn path() -> PathBuf {
    located().unwrap_or_else(|| PathBuf::from("celestina").join("appearance.toml"))
}

pub(crate) fn located() -> Option<PathBuf> {
    xdg::config_home().map(|home| home.join("celestina").join("appearance.toml"))
}

/// Whether `CELESTINA_REDUCED_MOTION` is set (to anything).
///
/// While it is, reduced motion is on: the file cannot turn it off.
#[must_use]
pub fn env_forces_reduced_motion() -> bool {
    std::env::var_os(REDUCED_MOTION_VARIABLE).is_some()
}

/// The current appearance: the file's values, or the defaults for a missing,
/// unreadable or malformed file (logged, and the file is left as it is), with
/// the environment override applied.
#[must_use]
pub fn load() -> Appearance {
    located().map_or_else(with_override, |path| load_from(&path))
}

/// [`load`] for a path already located: the watcher's re-read.
pub(crate) fn load_from(path: &Path) -> Appearance {
    let mut value = read_file(path);
    if env_forces_reduced_motion() {
        value.reduced_motion = true;
    }
    value
}

fn with_override() -> Appearance {
    Appearance {
        reduced_motion: env_forces_reduced_motion(),
        ..Appearance::default()
    }
}

fn read_file(path: &Path) -> Appearance {
    let bytes = match atomic_file::read_bounded(path, READ_LIMIT) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Appearance::default(),
        Err(error) => {
            eprintln!("celestina-settings: using the default appearance: {error}");
            return Appearance::default();
        }
    };
    let Ok(text) = String::from_utf8(bytes) else {
        eprintln!(
            "celestina-settings: using the default appearance: {} is not UTF-8",
            path.display()
        );
        return Appearance::default();
    };
    match parse(&text) {
        Ok(value) => value,
        Err(problem) => {
            eprintln!(
                "celestina-settings: using the default appearance: {} line {}: {}",
                path.display(),
                problem.line,
                problem.reason
            );
            Appearance::default()
        }
    }
}

/// Writes both values, atomically, keeping every other line of the file.
///
/// # Errors
///
/// [`SettingsError::NoConfigHome`] without a configuration directory,
/// [`SettingsError::Read`] or [`SettingsError::NotText`] when the existing
/// file cannot be read as text (it is then not replaced), and [`SettingsError::Write`] when the new bytes cannot be
/// published.
pub fn save(value: &Appearance) -> Result<(), SettingsError> {
    let path = located().ok_or(SettingsError::NoConfigHome)?;
    let existing = match atomic_file::read_bounded(&path, READ_LIMIT) {
        Ok(Some(bytes)) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => return Err(SettingsError::NotText { path }),
        },
        Ok(None) => String::new(),
        Err(source) => return Err(SettingsError::Read { path, source }),
    };
    let text = rewrite(&existing, value);
    atomic_file::replace(&path, text.as_bytes())
        .map_err(|source| SettingsError::Write { path, source })
}

/// Where and why the file could not be read.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Malformed {
    line: usize,
    reason: &'static str,
}

/// One of the two keys this module owns.
#[derive(Clone, Copy)]
enum Known {
    ReducedMotion,
    TextScale,
}

/// What one line of the file is, as far as this module cares.
enum Line<'a> {
    /// A real `[table]` or `[[array]]` header: the root table ends here.
    Header,
    /// A known key of the root table, with the text after its `=`.
    Known(Known, &'a str),
    /// Anything else, kept as it is: comments, blanks, other keys and the
    /// continuation lines of their multi-line values.
    Other,
}

/// A multi-line value still open at the end of a line.
#[derive(Clone, Copy)]
enum Open {
    Nothing,
    /// Brackets or braces not yet closed, counted across lines.
    Brackets(i64),
    /// A `"""` or `'''` string not yet closed.
    Text(&'static str),
}

/// Where a `#` comment starts in `text`, ignoring one inside a string.
fn comment_start(text: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, character) in text.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if character == '\\' => escaped = true,
            Some(open) if character == open => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == '#' => return Some(index),
            None => {}
        }
    }
    None
}

/// `text` without its comment, trimmed.
fn code(text: &str) -> &str {
    comment_start(text)
        .map_or(text, |start| &text[..start])
        .trim()
}

/// Opening minus closing brackets and braces outside strings and comments.
fn bracket_balance(text: &str) -> i64 {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut balance = 0;
    for character in code(text).chars() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if character == '\\' => escaped = true,
            Some(open) if character == open => quote = None,
            Some(_) => {}
            None => match character {
                '"' | '\'' => quote = Some(character),
                '[' | '{' => balance += 1,
                ']' | '}' => balance -= 1,
                _ => {}
            },
        }
    }
    balance
}

/// A `[table]` or `[[array]]` header line (outside any multi-line value,
/// which [`classify`] checks first).
fn is_table_header(line: &str) -> bool {
    let line = code(line);
    line.len() > 2 && line.starts_with('[') && line.ends_with(']')
}

/// The key of a `key = value` line, unquoted, and the text after the `=`.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    let key = key
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(key);
    Some((key, value))
}

/// Classifies every line (without its line ending), following multi-line
/// values so that none of their lines is taken for a header or a key.
fn classify<'a>(lines: &[&'a str]) -> Vec<Line<'a>> {
    let mut open = Open::Nothing;
    let mut kinds = Vec::with_capacity(lines.len());
    for line in lines {
        match open {
            Open::Text(delimiter) => {
                if line.contains(delimiter) {
                    open = Open::Nothing;
                }
                kinds.push(Line::Other);
                continue;
            }
            Open::Brackets(depth) => {
                let depth = depth + bracket_balance(line);
                open = if depth > 0 {
                    Open::Brackets(depth)
                } else {
                    Open::Nothing
                };
                kinds.push(Line::Other);
                continue;
            }
            Open::Nothing => {}
        }
        if line.trim_start().starts_with('#') {
            kinds.push(Line::Other);
            continue;
        }
        if is_table_header(line) {
            kinds.push(Line::Header);
            continue;
        }
        let Some((key, value)) = key_value(line) else {
            kinds.push(Line::Other);
            continue;
        };
        match key {
            REDUCED_MOTION_KEY => kinds.push(Line::Known(Known::ReducedMotion, value)),
            TEXT_SCALE_KEY => kinds.push(Line::Known(Known::TextScale, value)),
            _ => {
                let trimmed = value.trim_start();
                for delimiter in ["\"\"\"", "'''"] {
                    if let Some(rest) = trimmed.strip_prefix(delimiter) {
                        if !rest.contains(delimiter) {
                            open = Open::Text(delimiter);
                        }
                    }
                }
                if matches!(open, Open::Nothing) {
                    let depth = bracket_balance(value);
                    if depth > 0 {
                        open = Open::Brackets(depth);
                    }
                }
                kinds.push(Line::Other);
            }
        }
    }
    kinds
}

/// A known key's value without its comment: a string's contents, or a bare
/// word.
enum Scalar<'a> {
    Text(&'a str),
    Bare(&'a str),
}

fn scalar(value: &str) -> Scalar<'_> {
    let value = code(value);
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            if !inner.contains(quote) {
                return Scalar::Text(inner);
            }
        }
    }
    Scalar::Bare(value)
}

pub(crate) fn parse(text: &str) -> Result<Appearance, Malformed> {
    let lines: Vec<&str> = text.lines().collect();
    let mut value = Appearance::default();
    let mut seen_motion = false;
    let mut seen_scale = false;
    for (index, kind) in classify(&lines).into_iter().enumerate() {
        let line = index + 1;
        let (key, raw) = match kind {
            Line::Header => break,
            Line::Other => continue,
            Line::Known(key, raw) => (key, raw),
        };
        match key {
            Known::ReducedMotion => {
                if std::mem::replace(&mut seen_motion, true) {
                    return Err(Malformed {
                        line,
                        reason: "reduced_motion is repeated",
                    });
                }
                value.reduced_motion = match scalar(raw) {
                    Scalar::Bare("true") => true,
                    Scalar::Bare("false") => false,
                    _ => {
                        return Err(Malformed {
                            line,
                            reason: "reduced_motion is not true or false",
                        })
                    }
                };
            }
            Known::TextScale => {
                if std::mem::replace(&mut seen_scale, true) {
                    return Err(Malformed {
                        line,
                        reason: "text_scale is repeated",
                    });
                }
                value.text_scale = match scalar(raw) {
                    Scalar::Text(name) => TextScale::from_name(name),
                    Scalar::Bare(_) => None,
                }
                .ok_or(Malformed {
                    line,
                    reason: "text_scale is not \"compact\", \"normal\", \"large\" or \"larger\"",
                })?;
            }
        }
    }
    Ok(value)
}

/// The comment after a known key's value, with the space before it, if any.
fn trailing_comment(value: &str) -> &str {
    match comment_start(value) {
        Some(start) => &value[value[..start].trim_end().len()..],
        None => "",
    }
}

/// `existing` with the two known lines of the root table replaced (a repeat is
/// dropped, a trailing comment kept), and any missing one added at the end of
/// the root table, before the first real header. The rewritten lines use the
/// file's own line ending.
pub(crate) fn rewrite(existing: &str, value: &Appearance) -> String {
    let eol = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let motion = format!("{REDUCED_MOTION_KEY} = {}", value.reduced_motion);
    let scale = format!("{TEXT_SCALE_KEY} = \"{}\"", value.text_scale.as_str());
    let pieces: Vec<&str> = existing.split_inclusive('\n').collect();
    let bodies: Vec<&str> = pieces
        .iter()
        .map(|piece| {
            piece
                .strip_suffix('\n')
                .map_or(*piece, |body| body.strip_suffix('\r').unwrap_or(body))
        })
        .collect();
    let mut output = String::with_capacity(existing.len() + motion.len() + scale.len() + 8);
    let mut wrote_motion = false;
    let mut wrote_scale = false;
    let mut in_root = true;
    for (piece, kind) in pieces.iter().zip(classify(&bodies)) {
        match kind {
            Line::Header if in_root => {
                in_root = false;
                append_missing(
                    &mut output,
                    eol,
                    [(wrote_motion, &motion), (wrote_scale, &scale)],
                );
            }
            Line::Known(key, raw) if in_root => {
                let (written, line) = match key {
                    Known::ReducedMotion => (&mut wrote_motion, &motion),
                    Known::TextScale => (&mut wrote_scale, &scale),
                };
                if !std::mem::replace(written, true) {
                    output.push_str(line);
                    output.push_str(trailing_comment(raw));
                    output.push_str(eol);
                }
                continue;
            }
            _ => {}
        }
        output.push_str(piece);
    }
    if in_root {
        append_missing(
            &mut output,
            eol,
            [(wrote_motion, &motion), (wrote_scale, &scale)],
        );
    }
    output
}

/// Appends each line not yet written, on a line of its own.
fn append_missing(output: &mut String, eol: &str, lines: [(bool, &String); 2]) {
    if lines.iter().any(|(written, _)| !written) && !output.is_empty() && !output.ends_with('\n') {
        output.push_str(eol);
    }
    for (written, line) in lines {
        if !written {
            output.push_str(line);
            output.push_str(eol);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, rewrite, Appearance, TextScale};

    #[test]
    fn a_trailing_comment_and_a_literal_string_are_read() {
        let value = parse("reduced_motion = true # mine\ntext_scale = 'large' # bigger\n")
            .expect("a valid file");
        assert_eq!(
            value,
            Appearance {
                reduced_motion: true,
                text_scale: TextScale::Large
            }
        );
    }

    #[test]
    fn keys_inside_a_table_are_not_the_root_keys() {
        let value = parse("[other]\ntext_scale = \"larger\"\n").expect("a valid file");
        assert_eq!(value, Appearance::default());
    }

    #[test]
    fn a_missing_key_goes_before_the_first_table() {
        let text = rewrite(
            "# head\n[other]\nx = 1\n",
            &Appearance {
                reduced_motion: true,
                text_scale: TextScale::Compact,
            },
        );
        assert_eq!(
            text,
            "# head\nreduced_motion = true\ntext_scale = \"compact\"\n[other]\nx = 1\n"
        );
    }

    #[test]
    fn a_repeated_key_is_malformed_and_rewritten_once() {
        assert!(parse("reduced_motion = true\nreduced_motion = false\n").is_err());
        let text = rewrite(
            "reduced_motion = true\nreduced_motion = false\n",
            &Appearance::default(),
        );
        assert_eq!(text, "reduced_motion = false\ntext_scale = \"normal\"\n");
    }

    #[test]
    fn a_trailing_comment_on_a_known_line_is_kept() {
        let text = rewrite(
            "text_scale = \"large\"  # mine\nreduced_motion = false\n",
            &Appearance {
                reduced_motion: true,
                text_scale: TextScale::Compact,
            },
        );
        assert_eq!(
            text,
            "text_scale = \"compact\"  # mine\nreduced_motion = true\n"
        );
    }

    #[test]
    fn a_crlf_file_stays_crlf() {
        let text = rewrite(
            "# head\r\ntext_scale = \"large\"\r\n",
            &Appearance::default(),
        );
        assert_eq!(
            text,
            "# head\r\ntext_scale = \"normal\"\r\nreduced_motion = false\r\n"
        );
        assert!(!text.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn a_multi_line_array_is_not_a_header() {
        let existing = "paths = [\n  [1, 2],\n  \"x\",\n]\n[other]\nx = 1\n";
        let text = rewrite(existing, &Appearance::default());
        assert_eq!(
            text,
            "paths = [\n  [1, 2],\n  \"x\",\n]\nreduced_motion = false\ntext_scale = \"normal\"\n[other]\nx = 1\n"
        );
        let after = "list = [\n  [3],\n]\ntext_scale = \"larger\"\n";
        assert_eq!(parse(after).expect("valid").text_scale, TextScale::Larger);
    }

    #[test]
    fn a_multi_line_string_hides_what_looks_like_a_key() {
        let text = "note = \"\"\"\ntext_scale = \"huge\"\n[x]\n\"\"\"\ntext_scale = \"large\"\n";
        assert_eq!(parse(text).expect("valid").text_scale, TextScale::Large);
    }

    #[test]
    fn an_array_of_tables_header_ends_the_root() {
        let value = parse("[[things]]\ntext_scale = \"large\"\n").expect("valid");
        assert_eq!(value, Appearance::default());
    }

    #[test]
    fn no_spaces_around_the_equals_sign() {
        let value = parse("text_scale=\"large\"\nreduced_motion=true\n").expect("valid");
        assert_eq!(
            value,
            Appearance {
                reduced_motion: true,
                text_scale: TextScale::Large
            }
        );
    }

    #[test]
    fn an_unknown_string_with_signs_beside_a_valid_key() {
        let value =
            parse("accent = \"a = b # [c]\"\ntext_scale = \"larger\" # big\n").expect("valid");
        assert_eq!(value.text_scale, TextScale::Larger);
    }
}
