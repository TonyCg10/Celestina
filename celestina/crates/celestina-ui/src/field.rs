//! The text field: a single-line editor with a cursor, a selection, word
//! jumps, grapheme-safe deletion, paste and an IME preedit.
//!
//! The editing state is a `String`, byte offsets for the cursor and the
//! selection anchor, and the preedit. Every [`EditOp`] works on that state
//! alone, with no font system, so keys, paste and IME events edit the field
//! wherever they arrive. The geometry (where the caret and the selection are
//! painted, which index a pointer position names) comes from the shaped
//! [`TextRun`] of the display text (the text with the preedit inserted at
//! the cursor), which [`Field::shape`] rebuilds when the display text, the
//! style or the device ratio changed; `Tree::layout` does it for every field.
//!
//! Motions and deletions move by extended grapheme cluster, so a letter with
//! a combining accent or a flag emoji is one step; word jumps stop at the
//! Unicode word boundaries (UAX 29, the segmentation cosmic-text's
//! `Editor` uses for its own word motions). The text is one line of at most
//! [`MAX_CHARS`] characters: whatever is inserted is stripped of control
//! characters and cut to fit, at a grapheme boundary.
#![forbid(unsafe_code)]

use std::ops::Range;

use celestina_paint::{TextRun, TextStyle, TextSystem};
use unicode_segmentation::UnicodeSegmentation;

/// The longest text a field holds, in characters. Pasted, committed and
/// preedit text is cut to what still fits.
pub const MAX_CHARS: usize = 256;

/// The identity of a field for the product: the key that names its node
/// names it to the tree; this names it to the code that reads its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FieldId(pub u64);

/// One edit of a [`Field`].
#[derive(Clone, Debug, PartialEq)]
pub enum EditOp {
    /// Typed text; replaces the selection.
    Insert(String),
    /// Deletes the selection, else the grapheme before the cursor.
    Backspace,
    /// Deletes the selection, else the grapheme after the cursor.
    Delete,
    /// One grapheme (a word with `word`) to the left; `select` extends the
    /// selection. Without `select`, a selection collapses to its start.
    Left {
        select: bool,
        word: bool,
    },
    /// The mirror of `Left`; a selection collapses to its end.
    Right {
        select: bool,
        word: bool,
    },
    Home {
        select: bool,
    },
    End {
        select: bool,
    },
    SelectAll,
    /// Clipboard text; replaces the selection like `Insert`.
    Paste(String),
    /// The IME's composition in progress, shown at the cursor with the caret
    /// at byte `cursor` of `text`; empty text ends the composition. Starting
    /// a composition deletes the selection.
    Preedit {
        text: String,
        cursor: usize,
    },
    /// The IME's final text: replaces the composition (and the selection).
    CommitPreedit(String),
    /// A press at `x` logical px from the left edge of the text (scroll
    /// included): places the cursor, clears the selection and starts a drag.
    PointerDown(f32),
    /// The pointer moved to `x` with the press held: selects from where the
    /// press placed the cursor.
    PointerDrag(f32),
}

/// A composition in progress, inserted at the field's cursor for display.
#[derive(Clone, Debug, PartialEq)]
struct Preedit {
    text: String,
    /// The caret inside `text`, a character boundary.
    cursor: usize,
}

/// The shaped display text and what it was shaped for.
struct Shaped {
    run: TextRun,
    display: String,
    style: TextStyle,
    dpr: f32,
}

/// A single-line text editor.
#[derive(Default)]
pub struct Field {
    text: String,
    /// Byte offset of the cursor in `text`, on a grapheme boundary.
    cursor: usize,
    /// The other end of the selection, when there is one.
    anchor: Option<usize>,
    preedit: Option<Preedit>,
    /// Where the press that started a drag placed the cursor.
    drag_from: Option<usize>,
    shaped: Option<Shaped>,
    /// How far the text is scrolled left, in logical px, to keep the caret
    /// in view.
    scroll_x: f32,
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("text", &self.text)
            .field("cursor", &self.cursor)
            .field("anchor", &self.anchor)
            .field("preedit", &self.preedit)
            .finish_non_exhaustive()
    }
}

impl Field {
    pub fn new() -> Field {
        Field::default()
    }

    /// The committed text (without the preedit).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the text (sanitised and cut to [`MAX_CHARS`]) with the
    /// cursor at its end; ends the selection and the composition.
    pub fn set_text(&mut self, s: &str) {
        self.text = fit(s, MAX_CHARS);
        self.cursor = self.text.len();
        self.anchor = None;
        self.preedit = None;
        self.drag_from = None;
        self.scroll_x = 0.0;
    }

    /// Applies `op`; whether the text, the cursor, the selection or the
    /// composition changed.
    pub fn edit(&mut self, op: EditOp) -> bool {
        let before = (
            self.text.clone(),
            self.cursor,
            self.anchor,
            self.preedit.clone(),
        );
        // A drag continues only the press that started it: any other edit
        // may move the text under the remembered position.
        if !matches!(op, EditOp::PointerDown(_) | EditOp::PointerDrag(_)) {
            self.drag_from = None;
        }
        match op {
            EditOp::Insert(s) | EditOp::Paste(s) => {
                self.end_preedit();
                self.insert(&s);
            }
            EditOp::Backspace => {
                self.end_preedit();
                if !self.delete_selection() {
                    let start = self.text[..self.cursor]
                        .grapheme_indices(true)
                        .next_back()
                        .map_or(0, |(i, _)| i);
                    self.text.replace_range(start..self.cursor, "");
                    self.cursor = start;
                }
            }
            EditOp::Delete => {
                self.end_preedit();
                if !self.delete_selection() {
                    let len = self.text[self.cursor..]
                        .graphemes(true)
                        .next()
                        .map_or(0, str::len);
                    self.text.replace_range(self.cursor..self.cursor + len, "");
                }
            }
            EditOp::Left { select, word } => {
                self.end_preedit();
                self.step(select, |f| {
                    if word {
                        f.word_before()
                    } else {
                        f.grapheme_before()
                    }
                });
            }
            EditOp::Right { select, word } => {
                self.end_preedit();
                self.step(select, |f| {
                    if word {
                        f.word_after()
                    } else {
                        f.grapheme_after()
                    }
                });
            }
            EditOp::Home { select } => {
                self.end_preedit();
                self.step(select, |_| 0);
            }
            EditOp::End { select } => {
                self.end_preedit();
                self.step(select, |f| f.text.len());
            }
            EditOp::SelectAll => {
                self.end_preedit();
                self.anchor = Some(0);
                self.cursor = self.text.len();
                self.normalise_anchor();
            }
            EditOp::Preedit { text, cursor } => {
                // The selection a composition replaces frees its room first.
                let kept = self.selection().map_or(0, |r| self.text[r].chars().count());
                let room = MAX_CHARS.saturating_sub(self.text.chars().count() - kept);
                let text = fit(&text, room);
                if text.is_empty() {
                    self.preedit = None;
                } else {
                    self.delete_selection();
                    let cursor = char_floor(&text, cursor);
                    self.preedit = Some(Preedit { text, cursor });
                }
            }
            EditOp::CommitPreedit(s) => {
                self.end_preedit();
                self.insert(&s);
            }
            EditOp::PointerDown(x) => {
                let at = self.index_at(x);
                self.end_preedit();
                self.cursor = snap(&self.text, at);
                self.anchor = None;
                self.drag_from = Some(self.cursor);
            }
            EditOp::PointerDrag(x) => {
                let at = self.index_at(x);
                self.end_preedit();
                let from = snap(&self.text, *self.drag_from.get_or_insert(self.cursor));
                self.anchor = Some(from);
                self.cursor = snap(&self.text, at);
                self.normalise_anchor();
            }
        }
        before
            != (
                self.text.clone(),
                self.cursor,
                self.anchor,
                self.preedit.clone(),
            )
    }

    /// The cursor, as a byte offset into [`Field::text`] (on a grapheme
    /// boundary).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selected bytes of [`Field::text`], when any.
    pub fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor?;
        let (a, b) = (anchor.min(self.cursor), anchor.max(self.cursor));
        (a < b).then_some(a..b)
    }

    /// The composition in progress.
    pub fn preedit(&self) -> Option<&str> {
        self.preedit.as_ref().map(|p| p.text.as_str())
    }

    /// The text as shown: the committed text with the composition inserted
    /// at the cursor.
    pub fn display(&self) -> String {
        match &self.preedit {
            Some(p) => {
                let mut s = String::with_capacity(self.text.len() + p.text.len());
                s.push_str(&self.text[..self.cursor]);
                s.push_str(&p.text);
                s.push_str(&self.text[self.cursor..]);
                s
            }
            None => self.text.clone(),
        }
    }

    /// The bytes of [`Field::display`] the composition occupies.
    pub fn preedit_range(&self) -> Option<Range<usize>> {
        self.preedit
            .as_ref()
            .map(|p| self.cursor..self.cursor + p.text.len())
    }

    /// Shapes the display text for `style` at `dpr` unless it already is,
    /// which gives the geometry accessors below something to measure.
    pub fn shape(&mut self, text: &mut TextSystem, style: &TextStyle, dpr: f32) {
        let display = self.display();
        if self
            .shaped
            .as_ref()
            .is_some_and(|s| s.display == display && s.style == *style && s.dpr == dpr)
        {
            return;
        }
        let run = text.layout(&display, style, None, dpr);
        self.shaped = Some(Shaped {
            run,
            display,
            style: *style,
            dpr,
        });
    }

    /// The shaped display text, once [`Field::shape`] ran.
    pub fn run(&self) -> Option<&TextRun> {
        self.shaped.as_ref().map(|s| &s.run)
    }

    /// The caret's x in logical px from the left edge of the text: inside
    /// the composition while there is one.
    pub fn caret_x(&self) -> f32 {
        let at = self.cursor + self.preedit.as_ref().map_or(0, |p| p.cursor);
        self.run().map_or(0.0, |r| r.caret_x(at))
    }

    /// The selection's span in logical px from the left edge of the text;
    /// none while composing.
    pub fn selection_span(&self) -> Option<(f32, f32)> {
        if self.preedit.is_some() {
            return None;
        }
        let range = self.selection()?;
        self.run()?.span(range.start, range.end)
    }

    /// The composition's span in logical px from the left edge of the text.
    pub fn preedit_span(&self) -> Option<(f32, f32)> {
        let range = self.preedit_range()?;
        self.run()?.span(range.start, range.end)
    }

    /// How far the text is scrolled left, in logical px.
    pub fn scroll_x(&self) -> f32 {
        self.scroll_x
    }

    /// Scrolls just enough to keep the caret inside a field `width` logical
    /// px wide, and never further than the text's end.
    pub fn reveal_caret(&mut self, width: f32) {
        let width = if width.is_finite() {
            width.max(0.0)
        } else {
            0.0
        };
        let caret = self.caret_x();
        let text_w = self.run().map_or(0.0, |r| r.size().0);
        let mut scroll = self.scroll_x;
        if caret < scroll {
            scroll = caret;
        } else if caret > scroll + width {
            scroll = caret - width;
        }
        // Do not leave empty room right of the text while it fits again.
        let max = (text_w.max(caret) - width).max(0.0);
        self.scroll_x = scroll.clamp(0.0, max);
    }

    /// The committed-text index of a pointer at `x` logical px from the
    /// left edge of the text, from the last shaping (0 before one).
    fn index_at(&self, x: f32) -> usize {
        let Some(shaped) = &self.shaped else {
            return 0;
        };
        let at = shaped.run.index_at(x);
        match self.preedit_range() {
            Some(r) if at >= r.end => at - r.len(),
            Some(r) if at > r.start => r.start,
            _ => at,
        }
    }

    /// Ends the composition, leaving the committed text as it was.
    fn end_preedit(&mut self) {
        self.preedit = None;
    }

    /// Replaces the selection with `s`, cut to what fits.
    fn insert(&mut self, s: &str) {
        self.delete_selection();
        let room = MAX_CHARS.saturating_sub(self.text.chars().count());
        let s = fit(s, room);
        self.text.insert_str(self.cursor, &s);
        self.cursor += s.len();
    }

    /// Removes the selection, leaving the cursor at its start; whether
    /// there was one.
    fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.text.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.anchor = None;
        true
    }

    /// Moves the cursor to `target(self)`: extending the selection with
    /// `select`, else collapsing a selection to the edge the move heads for.
    fn step(&mut self, select: bool, target: impl Fn(&Field) -> usize) {
        let to = target(self);
        if select {
            self.anchor.get_or_insert(self.cursor);
            self.cursor = to;
            self.normalise_anchor();
        } else if let Some(range) = self.selection() {
            self.cursor = if to < self.cursor {
                range.start
            } else {
                range.end
            };
            self.anchor = None;
        } else {
            self.cursor = to;
            self.anchor = None;
        }
    }

    fn normalise_anchor(&mut self) {
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
    }

    fn grapheme_before(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn grapheme_after(&self) -> usize {
        self.cursor
            + self.text[self.cursor..]
                .graphemes(true)
                .next()
                .map_or(0, str::len)
    }

    /// The start of the word before the cursor (0 when there is none).
    fn word_before(&self) -> usize {
        self.text
            .unicode_word_indices()
            .map(|(i, _)| i)
            .rev()
            .find(|&i| i < self.cursor)
            .unwrap_or(0)
    }

    /// The end of the word after the cursor (the text's end when none).
    fn word_after(&self) -> usize {
        self.text
            .unicode_word_indices()
            .map(|(i, w)| i + w.len())
            .find(|&e| e > self.cursor)
            .unwrap_or(self.text.len())
    }
}

/// `s` as one line of at most `room` characters: control characters other
/// than newline and tab are dropped (the two become spaces, a carriage
/// return nothing), and the cut falls on a grapheme boundary. Looks at only
/// the first `room` plus a few characters, so hostile input stays cheap.
fn fit(s: &str, room: usize) -> String {
    let clean: String = s
        .chars()
        .filter_map(|c| match c {
            '\r' => None,
            '\n' | '\t' => Some(' '),
            c if c.is_control() => None,
            c => Some(c),
        })
        .take(room.saturating_add(32))
        .collect();
    let mut out = String::new();
    let mut chars = 0;
    for g in clean.graphemes(true) {
        let n = g.chars().count();
        if chars + n > room {
            break;
        }
        chars += n;
        out.push_str(g);
    }
    out
}

/// `index` moved down to a character boundary of `s` (at most its length).
fn char_floor(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// `index` moved down to the start of the grapheme it falls in (at most the
/// length of `s`): a hit-test against a stale shaping still lands on a
/// boundary of the current text.
fn snap(s: &str, index: usize) -> usize {
    let index = char_floor(s, index);
    if index == s.len() {
        return index;
    }
    s.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|&i| i <= index)
        .last()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use celestina_paint::{default_font_dirs, Role, TextSystem, Weight};

    use super::*;

    fn typed(s: &str) -> Field {
        let mut f = Field::new();
        f.edit(EditOp::Insert(s.to_owned()));
        f
    }

    fn left(select: bool, word: bool) -> EditOp {
        EditOp::Left { select, word }
    }

    fn right(select: bool, word: bool) -> EditOp {
        EditOp::Right { select, word }
    }

    fn style() -> TextStyle {
        TextStyle {
            size: 14.0,
            weight: Weight::Regular,
            tabular: false,
            role: Role::TextHi,
        }
    }

    /// One `TextSystem` for the module: loading the system's fonts is slow.
    fn system() -> MutexGuard<'static, TextSystem> {
        static SYSTEM: OnceLock<Mutex<TextSystem>> = OnceLock::new();
        let lock = SYSTEM.get_or_init(|| {
            Mutex::new(
                TextSystem::new(&default_font_dirs()).expect("Inter Variable in the checkout"),
            )
        });
        lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn backspace_deletes_a_whole_grapheme() {
        // A letter with a combining accent is two code points, one grapheme.
        let mut f = typed("ae\u{301}");
        assert!(f.edit(EditOp::Backspace));
        assert_eq!(f.text(), "a");
        // A flag is two regional indicators, one grapheme.
        let mut f = typed("x\u{1f1ea}\u{1f1f8}");
        assert!(f.edit(EditOp::Backspace));
        assert_eq!(f.text(), "x");
        // Delete removes the grapheme after the cursor the same way.
        let mut f = typed("\u{1f1ea}\u{1f1f8}y");
        f.edit(EditOp::Home { select: false });
        assert!(f.edit(EditOp::Delete));
        assert_eq!(f.text(), "y");
        // Nothing to delete is not a change.
        assert!(!f.edit(EditOp::Backspace));
        f.edit(EditOp::End { select: false });
        assert!(!f.edit(EditOp::Delete));
        // Arrows step over the same clusters.
        let mut f = typed("e\u{301}!");
        f.edit(left(false, false));
        f.edit(left(false, false));
        assert_eq!(f.cursor(), 0);
        f.edit(right(false, false));
        assert_eq!(f.cursor(), "e\u{301}".len());
    }

    #[test]
    fn word_jumps_stop_at_word_boundaries() {
        let mut f = typed("hello big world");
        assert_eq!(f.cursor(), 15);
        let mut seen = Vec::new();
        for _ in 0..3 {
            f.edit(left(false, true));
            seen.push(f.cursor());
        }
        assert_eq!(seen, [10, 6, 0]);
        let mut seen = Vec::new();
        for _ in 0..3 {
            f.edit(right(false, true));
            seen.push(f.cursor());
        }
        assert_eq!(seen, [5, 9, 15]);
        // Punctuation is not a word: the jump skips it.
        let mut f = typed("a, b");
        f.edit(left(false, true));
        assert_eq!(f.cursor(), 3);
        f.edit(left(false, true));
        assert_eq!(f.cursor(), 0);
        // Ctrl+Shift extends the selection by a word.
        let mut f = typed("one two");
        f.edit(left(true, true));
        assert_eq!(f.selection(), Some(4..7));
    }

    #[test]
    fn shift_arrows_extend_the_selection_and_typing_replaces_it() {
        let mut f = typed("abcd");
        f.edit(left(true, false));
        f.edit(left(true, false));
        assert_eq!(f.selection(), Some(2..4));
        assert_eq!(f.cursor(), 2);
        // Shrinking back to the anchor leaves no selection.
        f.edit(right(true, false));
        f.edit(right(true, false));
        assert_eq!(f.selection(), None);
        // Typing replaces the selection.
        f.edit(left(true, false));
        f.edit(left(true, false));
        assert!(f.edit(EditOp::Insert("X".into())));
        assert_eq!(f.text(), "abX");
        assert_eq!(f.selection(), None);
        // Without Shift a selection collapses to the edge the move heads for.
        f.edit(EditOp::SelectAll);
        assert_eq!(f.selection(), Some(0..3));
        f.edit(left(false, false));
        assert_eq!((f.cursor(), f.selection()), (0, None));
        f.edit(EditOp::SelectAll);
        f.edit(EditOp::Home { select: false });
        assert_eq!(f.cursor(), 0);
        f.edit(EditOp::End { select: true });
        assert_eq!(f.selection(), Some(0..3));
        f.edit(EditOp::Backspace);
        assert_eq!(f.text(), "");
    }

    #[test]
    fn paste_inserts_at_the_cursor_and_is_clamped_to_256_chars() {
        assert_eq!(MAX_CHARS, 256);
        let mut f = typed("ad");
        f.edit(left(false, false));
        assert!(f.edit(EditOp::Paste("bc".into())));
        assert_eq!(f.text(), "abcd");
        assert_eq!(f.cursor(), 3);
        // Newlines become spaces and other control characters vanish.
        let mut f = Field::new();
        f.edit(EditOp::Paste("a\r\nb\tc\u{7}d".into()));
        assert_eq!(f.text(), "a b cd");
        // The total never exceeds 256 characters, and a cut never splits a
        // grapheme.
        let mut f = typed(&"x".repeat(250));
        f.edit(EditOp::Paste("y".repeat(1000)));
        assert_eq!(f.text().chars().count(), 256);
        assert!(f.text().ends_with("yyyyyy"));
        let mut f = typed(&"x".repeat(255));
        f.edit(EditOp::Paste("e\u{301}".into()));
        assert_eq!(f.text().chars().count(), 255, "the accent stays whole");
        // A huge clipboard is bounded before it is processed.
        let mut f = Field::new();
        f.edit(EditOp::Paste("w".repeat(1 << 20)));
        assert_eq!(f.text().len(), 256);
        // A selection's room is freed first.
        let mut f = typed(&"x".repeat(256));
        f.edit(EditOp::SelectAll);
        f.edit(EditOp::Paste("new".into()));
        assert_eq!(f.text(), "new");
        // `set_text` obeys the same bound.
        f.set_text(&"q".repeat(300));
        assert_eq!(f.text().chars().count(), 256);
        assert_eq!(f.cursor(), 256);
    }

    #[test]
    fn preedit_shows_and_commit_replaces_it() {
        let mut f = typed("ab");
        f.edit(left(false, false));
        assert!(f.edit(EditOp::Preedit {
            text: "ni".into(),
            cursor: 1
        }));
        assert_eq!(f.preedit(), Some("ni"));
        assert_eq!(f.text(), "ab", "the composition is not committed");
        assert_eq!(f.display(), "anib");
        assert_eq!(f.preedit_range(), Some(1..3));
        // A new composition state replaces the old one.
        assert!(f.edit(EditOp::Preedit {
            text: "nih".into(),
            cursor: 3
        }));
        assert_eq!(f.display(), "anihb");
        // The commit replaces the composition with the final text.
        assert!(f.edit(EditOp::CommitPreedit("\u{4f60}\u{597d}".into())));
        assert_eq!(f.preedit(), None);
        assert_eq!(f.text(), "a\u{4f60}\u{597d}b");
        assert_eq!(f.cursor(), "a\u{4f60}\u{597d}".len());
        // Empty preedit text ends a composition; a char-boundary cursor.
        f.edit(EditOp::Preedit {
            text: "\u{e9}".into(),
            cursor: 1,
        });
        assert_eq!(f.preedit(), Some("\u{e9}"));
        assert!(f.edit(EditOp::Preedit {
            text: String::new(),
            cursor: 0
        }));
        assert_eq!(f.preedit(), None);
        // Starting a composition replaces the selection.
        let mut f = typed("abc");
        f.edit(EditOp::SelectAll);
        f.edit(EditOp::Preedit {
            text: "x".into(),
            cursor: 1,
        });
        assert_eq!((f.text(), f.display().as_str()), ("", "x"));
        // A composition is bounded like pasted text.
        let mut f = typed(&"a".repeat(250));
        f.edit(EditOp::Preedit {
            text: "b".repeat(100),
            cursor: 0,
        });
        assert_eq!(f.display().chars().count(), 256);
    }

    #[test]
    fn pointer_down_places_the_cursor_by_x() {
        let mut ts = system();
        let mut f = typed("hello world");
        // Nothing shaped yet: a press cannot name an index.
        f.edit(EditOp::PointerDown(100.0));
        assert_eq!(f.cursor(), 0);
        f.shape(&mut ts, &style(), 1.0);
        let run = f.run().expect("shaped");
        let mid = run.caret_x(5);
        let end = run.size().0;
        assert!(f.edit(EditOp::PointerDown(mid + 0.5)));
        assert_eq!(f.cursor(), 5);
        assert!(f.edit(EditOp::PointerDown(end + 40.0)));
        assert_eq!(f.cursor(), 11);
        f.edit(EditOp::PointerDown(-10.0));
        assert_eq!(f.cursor(), 0);
        // Dragging selects from where the press landed.
        f.edit(EditOp::PointerDown(mid + 0.5));
        assert!(f.edit(EditOp::PointerDrag(end + 40.0)));
        assert_eq!(f.selection(), Some(5..11));
        f.edit(EditOp::PointerDrag(0.0));
        assert_eq!(f.selection(), Some(0..5));
        f.edit(EditOp::PointerDrag(mid + 0.5));
        assert_eq!(f.selection(), None);
        // A press mid-composition ends it and maps back to the committed text.
        let mut f = typed("abcd");
        f.edit(left(false, false));
        f.edit(left(false, false));
        f.edit(EditOp::Preedit {
            text: "XYZ".into(),
            cursor: 3,
        });
        f.shape(&mut ts, &style(), 1.0);
        let after = f.run().expect("shaped").caret_x(8);
        f.edit(EditOp::PointerDown(after + 0.5));
        assert_eq!((f.preedit(), f.cursor()), (None, 4));
    }

    #[test]
    fn a_drag_after_the_text_shrank_never_selects_past_the_end() {
        let mut ts = system();
        let mut f = typed("hello world");
        f.shape(&mut ts, &style(), 1.0);
        let end = f.run().expect("shaped").size().0;
        f.edit(EditOp::PointerDown(end + 40.0));
        for _ in 0..6 {
            f.edit(EditOp::Backspace);
        }
        assert_eq!(f.text(), "hello");
        f.shape(&mut ts, &style(), 1.0);
        f.edit(EditOp::PointerDrag(0.0));
        assert!(f.selection().is_none_or(|r| r.end <= 5));
        f.edit(EditOp::Backspace);
        f.edit(EditOp::Insert("x".into()));
        // The same with multibyte text: the old anchor falls inside a letter.
        let mut f = typed("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}");
        f.shape(&mut ts, &style(), 1.0);
        let end = f.run().expect("shaped").size().0;
        f.edit(EditOp::PointerDown(end + 40.0));
        f.edit(EditOp::Insert("ab".into()));
        f.edit(EditOp::SelectAll);
        f.edit(EditOp::Insert("\u{e9}".into()));
        f.shape(&mut ts, &style(), 1.0);
        f.edit(EditOp::PointerDrag(0.0));
        f.edit(EditOp::Delete);
        assert!(f.text().len() <= 2);
    }

    #[test]
    fn a_composition_replacing_a_full_selection_keeps_its_room() {
        let mut f = typed(&"a".repeat(256));
        f.edit(EditOp::SelectAll);
        f.edit(EditOp::Preedit {
            text: "xyz".into(),
            cursor: 3,
        });
        assert_eq!((f.text(), f.preedit()), ("", Some("xyz")));
    }

    #[test]
    fn geometry_follows_the_shaped_text() {
        let mut ts = system();
        let mut f = typed("hello world");
        assert_eq!(f.caret_x(), 0.0, "unshaped");
        f.shape(&mut ts, &style(), 1.0);
        let end = f.caret_x();
        assert!(end > 20.0);
        f.edit(left(true, true));
        f.shape(&mut ts, &style(), 1.0);
        let (lo, hi) = f.selection_span().expect("a span");
        assert!(lo > 0.0 && (hi - end).abs() < 1.0, "{lo}..{hi} of {end}");
        // A composition is drawn inside the text and hides the selection.
        f.edit(EditOp::Preedit {
            text: "ab".into(),
            cursor: 1,
        });
        f.shape(&mut ts, &style(), 1.0);
        assert_eq!(f.selection_span(), None);
        let (plo, phi) = f.preedit_span().expect("a span");
        assert!(phi > plo && f.caret_x() > plo && f.caret_x() < phi);
        // Reshaping happens only when something changed.
        let before = f.run().map(|r| r.text().to_owned());
        f.shape(&mut ts, &style(), 1.0);
        assert_eq!(f.run().map(|r| r.text().to_owned()), before);
    }

    #[test]
    fn the_caret_is_kept_in_view() {
        let mut ts = system();
        let mut f = typed("a long line of text that is wider than the field");
        f.shape(&mut ts, &style(), 1.0);
        f.reveal_caret(100.0);
        let caret = f.caret_x();
        assert!(caret > 100.0);
        assert!((f.scroll_x() - (caret - 100.0)).abs() < 0.01);
        // Moving home scrolls back.
        f.edit(EditOp::Home { select: false });
        f.shape(&mut ts, &style(), 1.0);
        f.reveal_caret(100.0);
        assert_eq!(f.scroll_x(), 0.0);
        // A wide field never scrolls; a NaN width counts as 0 without a panic.
        f.reveal_caret(10_000.0);
        assert_eq!(f.scroll_x(), 0.0);
        f.reveal_caret(f32::NAN);
        assert!(f.scroll_x().is_finite());
    }
}
