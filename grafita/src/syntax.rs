//! The bridge to the syntax highlighter's C++ side.
//!
//! The colouring itself is KDE's KSyntaxHighlighting, driven from
//! `cpp/highlighter.cpp`: a `QSyntaxHighlighter` subclass, because colouring a
//! Qt text document *without touching its text* means applying formats to its
//! blocks, and overriding that needs a C++ subclass CXX-Qt cannot express. The
//! projection the widget reports back therefore stays byte for byte the one
//! `grafita-core` handed it, which is what keeps a CRLF file from being
//! rewritten.
//!
//! What crosses here is the registration of the QML type and two pure
//! functions the highlighter is built on — which definition a file is coloured
//! by, and where a bracket's partner is — so their rules are tested from Rust
//! without a window.

pub use ffi::register_highlighter;

#[cxx::bridge]
mod ffi {
    /// A bracket beside the caret and its partner, as UTF-16 offsets. Either
    /// is -1 when there is none.
    // Only the tests read these from Rust; the application pairs brackets in
    // C++. (`cfg_attr` is not an attribute the bridge accepts.)
    #[allow(dead_code)]
    struct BracketPair {
        bracket: i32,
        partner: i32,
    }

    /// What opening a block comment above `lines` lines of C cost: how many
    /// separate edits reached the document, and whether the last line ended
    /// up coloured as part of the comment.
    #[allow(dead_code)]
    struct Recolouring {
        edits: u32,
        last_line_quoted_before: bool,
        last_line_quoted: bool,
    }

    unsafe extern "C++" {
        include!("highlighter.h");

        /// Registers the highlighter as a QML type. Called once, before the
        /// QML that instantiates it is loaded.
        #[rust_name = "register_highlighter"]
        fn register_grafita_highlighter();

        /// The name of the definition a file is coloured by, empty for plain
        /// text: by file name first, then by what its first line looks like.
        #[allow(dead_code)]
        fn grafita_definition_name(file_name: &str, first_line: &str) -> String;

        /// The bracket beside `position` in `text` and its partner. A
        /// non-space character in `quoted` marks the same offset as inside a
        /// string or a comment, which a bracket outside one never pairs with.
        #[allow(dead_code)]
        fn grafita_bracket_pair(text: &str, quoted: &str, position: i32) -> BracketPair;

        /// Types `/*` at the top of `lines` lines of C under the highlighter,
        /// in the test process's offscreen application, and reports the cost.
        #[allow(dead_code)]
        fn grafita_recolour_after_comment(lines: u32) -> Recolouring;
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::{
        grafita_bracket_pair, grafita_definition_name, grafita_recolour_after_comment, BracketPair,
    };

    fn pair(text: &str, position: i32) -> (i32, i32) {
        let BracketPair { bracket, partner } = grafita_bracket_pair(text, "", position);
        (bracket, partner)
    }

    // One test for everything that builds Qt objects: Qt's main thread is the
    // first thread that builds one, and the harness runs tests on several.
    #[test]
    fn the_highlighter_picks_definitions_and_recolours_in_one_pass() {
        definitions();
        opening_a_comment_recolours_every_following_line_in_one_edit();
    }

    fn definitions() {
        assert_eq!(grafita_definition_name("config.kdl", ""), "KDL");
        assert_eq!(
            grafita_definition_name("/home/ana/.config/niri/config.kdl", "// niri"),
            "KDL",
            "a whole path names its file just as well"
        );
        assert_eq!(
            grafita_definition_name("org.celestina.Grafita.desktop", "[Desktop Entry]"),
            ".desktop"
        );
        assert_eq!(grafita_definition_name("Cargo.toml", ""), "TOML");
        assert_eq!(grafita_definition_name("README.md", ""), "Markdown");
        assert_eq!(
            grafita_definition_name("arranque", "#!/bin/sh"),
            "Bash",
            "a script without an extension is known by its first line"
        );
        assert_eq!(grafita_definition_name("notas.xyz123", "hola"), "");
        assert_eq!(grafita_definition_name("notas.txt", "hola"), "");
        assert_eq!(
            grafita_definition_name("", ""),
            "",
            "a new document is plain"
        );
    }

    // KSyntaxHighlighting's own SyntaxHighlighter re-coloured each line after
    // a state change as a queued edit of its own; the widget reported every
    // one as a text change, and Grafita reconciles the whole text on each.
    fn opening_a_comment_recolours_every_following_line_in_one_edit() {
        let cost = grafita_recolour_after_comment(5_000);
        assert!(
            !cost.last_line_quoted_before,
            "the last line is code before the comment opens"
        );
        assert!(cost.last_line_quoted, "the comment reaches the last line");
        assert!(cost.edits <= 2, "{} edits for one keystroke", cost.edits);
    }

    #[test]
    fn a_bracket_after_the_caret_is_paired_before_one_before_it() {
        // `(a[b]c)`: the caret before `(` pairs it with the closing `)`.
        assert_eq!(pair("(a[b]c)", 0), (0, 6));
        // Between `[` and `b` the bracket before the caret is `[`.
        assert_eq!(pair("(a[b]c)", 3), (2, 4));
        // Before `]` that one wins over the `b` before it, which is no bracket.
        assert_eq!(pair("(a[b]c)", 4), (4, 2));
        // After the last `)` it is still beside the caret.
        assert_eq!(pair("(a[b]c)", 7), (6, 0));
    }

    #[test]
    fn nested_brackets_of_the_same_kind_pair_by_depth() {
        let text = "{ f(g(x), [1, {2}]) }";
        assert_eq!(pair(text, 0), (0, 20));
        assert_eq!(pair(text, 3), (3, 18));
        assert_eq!(pair(text, 14), (14, 16));
        assert_eq!(pair(text, 21), (20, 0));
    }

    #[test]
    fn brackets_pair_across_lines() {
        let text = "node {\n    child 1\n}\n";
        assert_eq!(pair(text, 5), (5, 19));
        assert_eq!(pair(text, 20), (19, 5));
    }

    #[test]
    fn an_unmatched_bracket_or_no_bracket_pairs_with_nothing() {
        assert_eq!(pair("(a(b)", 0), (-1, -1));
        assert_eq!(pair("a)", 1), (-1, -1));
        assert_eq!(pair("abc", 1), (-1, -1));
        assert_eq!(pair("", 0), (-1, -1));
        assert_eq!(
            pair("()", -4),
            (-1, -1),
            "an offset off the text is no bracket"
        );
        assert_eq!(pair("()", 40), (-1, -1));
    }

    #[test]
    fn a_bracket_in_a_string_or_comment_is_not_a_partner() {
        // `f(")")` — the `)` inside the quotes is text, not code.
        let text = "f(\")\")";
        let quoted = "  ''' ";
        let found = grafita_bracket_pair(text, quoted, 1);
        assert_eq!((found.bracket, found.partner), (1, 5));
        // A bracket inside a string still pairs with one beside it there.
        let found = grafita_bracket_pair("\"(x)\"", "'''''", 1);
        assert_eq!((found.bracket, found.partner), (1, 3));
    }

    #[test]
    fn the_scan_for_a_partner_is_bounded() {
        let text = format!("({})", "x".repeat(200_000));
        assert_eq!(
            pair(&text, 0),
            (-1, -1),
            "a partner past the bound is not looked for"
        );
        let near = format!("({})", "x".repeat(1_000));
        assert_eq!(pair(&near, 0), (0, 1_001));
    }
}
