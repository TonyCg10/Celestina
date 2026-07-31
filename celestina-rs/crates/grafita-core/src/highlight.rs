//! Colouring code enough to read it, and no more.
//!
//! This is a lexer, not a parser. It recognises four things — comments,
//! strings, numbers and keywords — because those are what separate code from
//! prose at a glance. It will never colour a type differently from a variable,
//! and that is the deliberate limit: Grafita's stated non-goal is being an IDE,
//! and the measurement behind this choice is recorded in the roadmap. A grammar
//! library cost three orders of magnitude more on every axis and still did not
//! cover QML, the language most edited in this repository.
//!
//! Two rules keep it safe:
//!
//! - **An unknown language stays plain text.** Never a refusal, never a guess:
//!   a file Grafita cannot colour is still a file Grafita can edit.
//! - **Spans land on character boundaries**, so a host can slice the line it was
//!   given without splitting a character.
//!
//! Colouring runs per line and carries a [`LineState`] across, which is what a
//! block comment needs and what lets a host re-colour only the lines that
//! changed rather than the whole document.

use std::path::Path;

/// What a run of characters is, for the purpose of colouring it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Token {
    Comment,
    Text,
    Number,
    Keyword,
}

/// A run of one kind of token within a single line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    /// Byte offset of the run's first character within the line.
    pub start: usize,
    /// Byte offset one past its last character.
    pub end: usize,
    pub token: Token,
}

/// What a line leaves behind for the next one.
///
/// Only block comments survive a line break here. Multi-line string literals —
/// Rust's `r#"…"#`, Python's `"""` — are deliberately not tracked: getting them
/// half right would colour the rest of a file as a string, which is worse than
/// not colouring it at all.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineState {
    #[default]
    Normal,
    InBlockComment,
}

/// The lexical shape of one language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Syntax {
    /// What starts a comment that runs to the end of the line.
    line_comment: Option<&'static str>,
    /// What opens and closes a comment that may span lines.
    block_comment: Option<(&'static str, &'static str)>,
    /// Which quote characters open a string.
    quotes: &'static [char],
    /// Words coloured as keywords.
    keywords: &'static [&'static str],
}

/// A language Grafita knows how to colour.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Language {
    /// No colouring: every line is one plain run. Always editable.
    #[default]
    Plain,
    Rust,
    /// QML and JavaScript share enough shape to share a lexer.
    QmlJs,
    Json,
    Toml,
    C,
    Python,
    Shell,
    Markdown,
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while",
];

const QMLJS_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "bool",
    "break",
    "case",
    "catch",
    "class",
    "color",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "double",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "int",
    "let",
    "new",
    "null",
    "on",
    "property",
    "readonly",
    "real",
    "required",
    "return",
    "signal",
    "string",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "yield",
];

const JSON_KEYWORDS: &[&str] = &["true", "false", "null"];
const TOML_KEYWORDS: &[&str] = &["true", "false"];

const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while",
];

const PYTHON_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield",
];

const SHELL_KEYWORDS: &[&str] = &[
    "case", "do", "done", "elif", "else", "esac", "exit", "export", "fi", "for", "function", "if",
    "in", "local", "read", "readonly", "return", "then", "until", "while",
];

impl Language {
    /// The lexical rules for this language, or `None` for plain text.
    const fn syntax(self) -> Option<Syntax> {
        let syntax = match self {
            Self::Plain => return None,
            Self::Rust => Syntax {
                line_comment: Some("//"),
                block_comment: Some(("/*", "*/")),
                quotes: &['"'],
                keywords: RUST_KEYWORDS,
            },
            Self::QmlJs => Syntax {
                line_comment: Some("//"),
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                keywords: QMLJS_KEYWORDS,
            },
            Self::Json => Syntax {
                line_comment: None,
                block_comment: None,
                quotes: &['"'],
                keywords: JSON_KEYWORDS,
            },
            Self::Toml => Syntax {
                line_comment: Some("#"),
                block_comment: None,
                quotes: &['"', '\''],
                keywords: TOML_KEYWORDS,
            },
            Self::C => Syntax {
                line_comment: Some("//"),
                block_comment: Some(("/*", "*/")),
                quotes: &['"', '\''],
                keywords: C_KEYWORDS,
            },
            Self::Python => Syntax {
                line_comment: Some("#"),
                block_comment: None,
                quotes: &['"', '\''],
                keywords: PYTHON_KEYWORDS,
            },
            Self::Shell => Syntax {
                line_comment: Some("#"),
                block_comment: None,
                quotes: &['"', '\''],
                keywords: SHELL_KEYWORDS,
            },
            Self::Markdown => Syntax {
                line_comment: None,
                block_comment: None,
                quotes: &['`'],
                keywords: &[],
            },
        };
        Some(syntax)
    }

    /// Picks a language from a path.
    ///
    /// Extension first, then the whole file name for the extensionless files
    /// that are nonetheless well known. Anything unrecognised is [`Self::Plain`]
    /// — the file still opens and still edits, it simply is not coloured.
    ///
    /// This is the one place a name is allowed to decide anything, and it only
    /// decides *colour*. Whether a file can be opened at all is still settled by
    /// its bytes, in [`crate::probe`].
    #[must_use]
    pub fn for_path(path: &Path) -> Self {
        if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
            let language = match extension.to_ascii_lowercase().as_str() {
                "rs" => Self::Rust,
                "qml" | "js" | "mjs" | "jsx" | "ts" => Self::QmlJs,
                "json" => Self::Json,
                "toml" => Self::Toml,
                "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" => Self::C,
                "py" | "pyi" => Self::Python,
                "sh" | "bash" | "zsh" => Self::Shell,
                "md" | "markdown" => Self::Markdown,
                _ => Self::Plain,
            };
            if language != Self::Plain {
                return language;
            }
        }
        match path.file_name().and_then(|value| value.to_str()) {
            Some(".bashrc" | ".bash_profile" | ".zshrc" | ".profile" | "PKGBUILD") => Self::Shell,
            Some("Cargo.lock") => Self::Toml,
            _ => Self::Plain,
        }
    }
}

/// Colours one line, given what the previous line left behind.
///
/// Returns the runs to colour and the state the next line starts in. Runs are
/// in order, never overlap, and always sit on character boundaries; the gaps
/// between them are ordinary text the host leaves alone.
#[must_use]
pub fn line(text: &str, language: Language, incoming: LineState) -> (Vec<Span>, LineState) {
    let Some(syntax) = language.syntax() else {
        return (Vec::new(), LineState::Normal);
    };
    let mut spans = Vec::new();
    let mut state = incoming;
    let bytes = text.as_bytes();
    let mut index = 0;

    // A block comment opened on an earlier line owns this one until it closes.
    if state == LineState::InBlockComment {
        let Some((_, close)) = syntax.block_comment else {
            state = LineState::Normal;
            return (spans, state);
        };
        match text.find(close) {
            Some(at) => {
                let end = at + close.len();
                spans.push(Span {
                    start: 0,
                    end,
                    token: Token::Comment,
                });
                index = end;
                state = LineState::Normal;
            }
            None => {
                spans.push(Span {
                    start: 0,
                    end: text.len(),
                    token: Token::Comment,
                });
                return (spans, LineState::InBlockComment);
            }
        }
    }

    while index < bytes.len() {
        if let Some(marker) = syntax.line_comment {
            if text[index..].starts_with(marker) {
                spans.push(Span {
                    start: index,
                    end: text.len(),
                    token: Token::Comment,
                });
                return (spans, LineState::Normal);
            }
        }
        if let Some((open, close)) = syntax.block_comment {
            if text[index..].starts_with(open) {
                let after_open = index + open.len();
                match text[after_open..].find(close) {
                    Some(at) => {
                        let end = after_open + at + close.len();
                        spans.push(Span {
                            start: index,
                            end,
                            token: Token::Comment,
                        });
                        index = end;
                        continue;
                    }
                    None => {
                        spans.push(Span {
                            start: index,
                            end: text.len(),
                            token: Token::Comment,
                        });
                        return (spans, LineState::InBlockComment);
                    }
                }
            }
        }

        let character = text[index..].chars().next().unwrap_or('\u{0}');
        if syntax.quotes.contains(&character) {
            let start = index;
            index += character.len_utf8();
            while index < bytes.len() {
                let next = text[index..].chars().next().unwrap_or('\u{0}');
                if next == '\\' {
                    index += next.len_utf8();
                    index += text[index..].chars().next().map_or(0, char::len_utf8);
                    continue;
                }
                index += next.len_utf8();
                if next == character {
                    break;
                }
            }
            spans.push(Span {
                start,
                end: index.min(text.len()),
                token: Token::Text,
            });
            continue;
        }

        if character.is_ascii_digit() {
            let start = index;
            while index < bytes.len() {
                let next = text[index..].chars().next().unwrap_or('\u{0}');
                if next.is_ascii_alphanumeric() || next == '.' || next == '_' {
                    index += next.len_utf8();
                } else {
                    break;
                }
            }
            spans.push(Span {
                start,
                end: index,
                token: Token::Number,
            });
            continue;
        }

        if character.is_alphabetic() || character == '_' {
            let start = index;
            while index < bytes.len() {
                let next = text[index..].chars().next().unwrap_or('\u{0}');
                if next.is_alphanumeric() || next == '_' {
                    index += next.len_utf8();
                } else {
                    break;
                }
            }
            if syntax.keywords.contains(&&text[start..index]) {
                spans.push(Span {
                    start,
                    end: index,
                    token: Token::Keyword,
                });
            }
            continue;
        }

        index += character.len_utf8();
    }

    (spans, state)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{line, Language, LineState, Span, Token};

    fn colour(text: &str, language: Language) -> Vec<Span> {
        line(text, language, LineState::Normal).0
    }

    fn slices<'a>(text: &'a str, spans: &[Span]) -> Vec<(&'a str, Token)> {
        spans
            .iter()
            .map(|span| (&text[span.start..span.end], span.token))
            .collect()
    }

    #[test]
    fn an_unknown_language_is_plain_text_and_never_a_refusal() {
        let text = "cualquier cosa \"con comillas\" y 42";

        assert_eq!(colour(text, Language::Plain), vec![]);
        assert_eq!(
            Language::for_path(Path::new("notas.desconocido")),
            Language::Plain
        );
        assert_eq!(
            Language::for_path(Path::new("SinExtension")),
            Language::Plain
        );
    }

    #[test]
    fn the_four_things_it_knows_are_recognised() {
        let text = r#"let x = 42; // un comentario"#;

        assert_eq!(
            slices(text, &colour(text, Language::Rust)),
            vec![
                ("let", Token::Keyword),
                ("42", Token::Number),
                ("// un comentario", Token::Comment),
            ]
        );
    }

    #[test]
    fn a_string_swallows_what_looks_like_code_inside_it() {
        let text = r#"let s = "let 42 // no"; let y = 1"#;

        assert_eq!(
            slices(text, &colour(text, Language::Rust)),
            vec![
                ("let", Token::Keyword),
                (r#""let 42 // no""#, Token::Text),
                ("let", Token::Keyword),
                ("1", Token::Number),
            ]
        );
    }

    #[test]
    fn an_escaped_quote_does_not_end_a_string() {
        let text = r#""a\"b" fn"#;

        assert_eq!(
            slices(text, &colour(text, Language::Rust)),
            vec![(r#""a\"b""#, Token::Text), ("fn", Token::Keyword)]
        );
    }

    #[test]
    fn an_unterminated_string_ends_with_the_line_rather_than_running_away() {
        let text = r#"let s = "sin cerrar"#;
        let (spans, state) = line(text, Language::Rust, LineState::Normal);

        assert_eq!(spans.last().map(|span| span.end), Some(text.len()));
        // The next line starts clean: a runaway string would colour the rest of
        // the file, which is worse than not colouring it.
        assert_eq!(state, LineState::Normal);
    }

    #[test]
    fn a_block_comment_carries_across_lines_and_closes() {
        let (spans, state) = line("code /* abre", Language::Rust, LineState::Normal);
        assert_eq!(state, LineState::InBlockComment);
        assert_eq!(spans.last().map(|span| span.token), Some(Token::Comment));

        let (spans, state) = line("sigue dentro", Language::Rust, LineState::InBlockComment);
        assert_eq!(state, LineState::InBlockComment);
        assert_eq!(
            spans,
            vec![Span {
                start: 0,
                end: 12,
                token: Token::Comment
            }]
        );

        let text = "cierra */ let";
        let (spans, state) = line(text, Language::Rust, LineState::InBlockComment);
        assert_eq!(state, LineState::Normal);
        assert_eq!(
            slices(text, &spans),
            vec![("cierra */", Token::Comment), ("let", Token::Keyword)]
        );
    }

    #[test]
    fn a_block_comment_that_opens_and_closes_on_one_line_leaves_no_state() {
        let text = "a /* dentro */ fn";
        let (spans, state) = line(text, Language::Rust, LineState::Normal);

        assert_eq!(state, LineState::Normal);
        assert_eq!(
            slices(text, &spans),
            vec![("/* dentro */", Token::Comment), ("fn", Token::Keyword)]
        );
    }

    #[test]
    fn spans_land_on_character_boundaries_in_multibyte_text() {
        let text = "let año = \"cañón\"; // ñ";

        for span in colour(text, Language::Rust) {
            assert!(text.is_char_boundary(span.start), "{span:?}");
            assert!(text.is_char_boundary(span.end), "{span:?}");
        }
    }

    #[test]
    fn a_keyword_inside_a_longer_word_is_not_a_keyword() {
        let text = "letra fnord selfie";

        assert_eq!(colour(text, Language::Rust), vec![]);
    }

    #[test]
    fn languages_are_chosen_by_extension_and_by_well_known_name() {
        let cases = [
            ("main.rs", Language::Rust),
            ("Main.QML", Language::QmlJs),
            ("app.js", Language::QmlJs),
            ("datos.json", Language::Json),
            ("Cargo.toml", Language::Toml),
            ("shim.cpp", Language::C),
            ("script.py", Language::Python),
            ("run.sh", Language::Shell),
            (".bashrc", Language::Shell),
            ("Cargo.lock", Language::Toml),
            ("LEEME", Language::Plain),
            ("captura.png", Language::Plain),
        ];

        for (name, expected) in cases {
            assert_eq!(Language::for_path(Path::new(name)), expected, "{name}");
        }
    }

    #[test]
    fn json_has_no_comments_so_a_slash_is_just_text() {
        let text = r#"{"ruta": "//servidor", "n": 42}"#;

        assert_eq!(
            slices(text, &colour(text, Language::Json)),
            vec![
                (r#""ruta""#, Token::Text),
                (r#""//servidor""#, Token::Text),
                (r#""n""#, Token::Text),
                ("42", Token::Number),
            ]
        );
    }

    #[test]
    fn every_line_of_a_real_file_is_coloured_without_panicking() {
        // Ragged input on purpose: unbalanced quotes and comment markers.
        let awkward = [
            "", "\"", "/*", "*/", "//", "'", "\\", "42.", "0x", "ñ\"ñ", "/*/",
        ];
        let mut state = LineState::Normal;
        for text in awkward {
            let (spans, next) = line(text, Language::Rust, state);
            for span in &spans {
                assert!(
                    span.start <= span.end && span.end <= text.len(),
                    "{text:?} {span:?}"
                );
                assert!(text.is_char_boundary(span.start) && text.is_char_boundary(span.end));
            }
            state = next;
        }
    }
}
