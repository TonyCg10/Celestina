//! The lexer, reachable from the C++ side of the highlighter.
//!
//! `QSyntaxHighlighter` is the one way to colour a Qt text document *without
//! touching its text* — it applies formats to a block, leaving the characters
//! alone. That matters more here than anywhere: the projection the widget
//! reports back has to stay byte-for-byte the projection `grafita-core` handed
//! it, or the reconciliation that keeps a CRLF file intact would be comparing
//! against markup.
//!
//! Overriding `highlightBlock` needs a C++ subclass, which CXX-Qt cannot
//! express, so the subclass lives in `cpp/highlighter.cpp` and calls back into
//! this bridge for the actual lexing. No colouring rule lives in C++: it asks
//! what the runs are and paints them.
//!
//! The runs cross already measured in the UTF-16 code units a Qt block counts
//! in, converted by `grafita-core` in one pass over the line. The C++ side
//! used to map each run's byte offsets by re-decoding the line's prefix, which
//! made a long minified line quadratic and gave the UTF-16 rule a second owner.

use grafita_core::highlight::{self, Language, LineState, Token};

pub use ffi::register_highlighter;

#[cxx::bridge]
mod ffi {
    /// One coloured run, flattened for the C++ side, in the UTF-16 code units
    /// `QSyntaxHighlighter::setFormat` takes.
    struct Run {
        start: u32,
        length: u32,
        token: u8,
    }

    /// What one line's colouring produced.
    struct Coloured {
        runs: Vec<Run>,
        state: u8,
    }

    unsafe extern "C++" {
        include!("highlighter.h");

        /// Registers the highlighter as a QML type. Called once, before the
        /// QML that instantiates it is loaded.
        #[rust_name = "register_highlighter"]
        fn register_grafita_highlighter();
    }

    extern "Rust" {
        /// Colours one line. `language` and `state` are the numeric forms of
        /// `Language` and `LineState`; an unknown value for either is treated as
        /// plain text rather than as an error, so a mismatch between the two
        /// sides degrades to "no colour" instead of misbehaving.
        fn grafita_colour_line(text: &str, language: u8, state: u8) -> Coloured;

        /// The numeric language for a path, so the shim never has to know how
        /// languages are chosen.
        fn grafita_language_for_path(path: &str) -> u8;
    }
}

/// Numeric forms shared with `cpp/highlighter.cpp`. Kept in one place on this
/// side so the mapping is written once.
const LANGUAGES: &[Language] = &[
    Language::Plain,
    Language::Rust,
    Language::QmlJs,
    Language::Json,
    Language::Toml,
    Language::C,
    Language::Python,
    Language::Shell,
    Language::Markdown,
];

#[must_use]
pub fn language_code(language: Language) -> u8 {
    LANGUAGES
        .iter()
        .position(|candidate| *candidate == language)
        .and_then(|index| u8::try_from(index).ok())
        .unwrap_or(0)
}

fn language_from_code(code: u8) -> Language {
    LANGUAGES
        .get(code as usize)
        .copied()
        .unwrap_or(Language::Plain)
}

const fn token_code(token: Token) -> u8 {
    match token {
        Token::Comment => 0,
        Token::Text => 1,
        Token::Number => 2,
        Token::Keyword => 3,
    }
}

fn grafita_colour_line(text: &str, language: u8, state: u8) -> ffi::Coloured {
    let incoming = match state {
        1 => LineState::InBlockComment,
        _ => LineState::Normal,
    };
    let (runs, outgoing) = highlight::line_utf16(text, language_from_code(language), incoming);
    ffi::Coloured {
        runs: runs
            .into_iter()
            // A Qt block cannot hold 2^32 units, so a run that does not fit is
            // one no block could have produced; it is skipped, not clamped.
            .filter_map(|run| {
                Some(ffi::Run {
                    start: u32::try_from(run.start).ok()?,
                    length: u32::try_from(run.len).ok()?,
                    token: token_code(run.token),
                })
            })
            .collect(),
        state: u8::from(outgoing == LineState::InBlockComment),
    }
}

fn grafita_language_for_path(path: &str) -> u8 {
    language_code(Language::for_path(std::path::Path::new(path)))
}

#[cfg(test)]
mod tests {
    use grafita_core::highlight::Language;

    use std::time::{Duration, Instant};

    use super::{grafita_colour_line, grafita_language_for_path, language_code};

    #[test]
    fn every_language_survives_the_round_trip_through_a_number() {
        for language in [
            Language::Plain,
            Language::Rust,
            Language::QmlJs,
            Language::Json,
            Language::Toml,
            Language::C,
            Language::Python,
            Language::Shell,
            Language::Markdown,
        ] {
            let code = language_code(language);
            assert_eq!(super::language_from_code(code), language, "{language:?}");
        }
    }

    #[test]
    fn a_number_the_other_side_does_not_know_is_plain_text_not_an_error() {
        let coloured = grafita_colour_line("let x = 1; // c", 250, 0);

        assert!(coloured.runs.is_empty());
        assert_eq!(coloured.state, 0);
    }

    #[test]
    fn the_bridge_reports_the_same_runs_the_lexer_does() {
        let rust = grafita_language_for_path("/tmp/modulo.rs");
        let coloured = grafita_colour_line("let x = 42; // nota", rust, 0);

        assert_eq!(coloured.runs.len(), 3);
        assert_eq!(coloured.runs[0].token, 3, "let is a keyword");
        assert_eq!(coloured.runs[1].token, 2, "42 is a number");
        assert_eq!(coloured.runs[2].token, 0, "the tail is a comment");
    }

    #[test]
    fn block_comment_state_crosses_the_bridge_in_both_directions() {
        let rust = grafita_language_for_path("x.rs");

        let opened = grafita_colour_line("code /* abre", rust, 0);
        assert_eq!(opened.state, 1);

        let still_inside = grafita_colour_line("dentro", rust, 1);
        assert_eq!(still_inside.state, 1);

        let closed = grafita_colour_line("cierra */", rust, 1);
        assert_eq!(closed.state, 0);
    }

    #[test]
    fn runs_cross_the_bridge_in_utf16_units() {
        let rust = grafita_language_for_path("x.rs");
        // `ø` and `ß` are two bytes and one unit each; `😀` is four and two.
        let coloured = grafita_colour_line("øß😀 fn", rust, 0);

        assert_eq!(coloured.runs.len(), 1);
        assert_eq!(coloured.runs[0].start, 5, "1 + 1 + 2 units, then a space");
        assert_eq!(coloured.runs[0].length, 2);
        assert_eq!(coloured.runs[0].token, 3);
    }

    #[test]
    fn a_five_megabyte_single_line_colours_within_a_time_bound() {
        // Minified JavaScript: one line, hundreds of thousands of runs. The
        // highlighter runs this on the GUI thread, so it must stay linear.
        let piece = "var a=\"ø\",b=42;/*😀*/if(a)b=null;";
        let line = piece.repeat(5 * 1024 * 1024 / piece.len() + 1);
        let js = grafita_language_for_path("app.min.js");

        let started = Instant::now();
        let coloured = grafita_colour_line(&line, js, 0);
        let elapsed = started.elapsed();

        assert!(coloured.runs.len() > 500_000, "{}", coloured.runs.len());
        let last = coloured.runs.last().expect("a last run");
        let units = line.encode_utf16().count();
        assert_eq!((last.start + last.length) as usize, units - 1);
        assert!(
            elapsed < Duration::from_secs(10),
            "colouring a 5 MB line took {elapsed:?}"
        );
    }
}
