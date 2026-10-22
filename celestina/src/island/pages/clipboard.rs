//! The Clipboard page (CEL-2-G design §8): the history's previews newest
//! first, an image entry as its thumbnail, a pause toggle and a clear action
//! with a confirmation step. A click or Enter on a row makes the entry the
//! selection again; Delete on a focused row removes it (the Island turns the
//! key into [`PageAction`] with [`remove_index`]).
//!
//! The page only describes. What it reads is a [`ClipboardPanel`]: bounded
//! previews and image sizes, never an entry's bytes (design §5). Nothing of
//! an entry is translated, logged or printed: the panel's `Debug` shows
//! counts. A row's accessible name is its bounded preview, or for an image a
//! fixed Spanish word with its size ([`row_name`]); the toolkit has no
//! channel to assistive technology yet, so the name is the row's visible
//! text and is pinned here for when the channel exists.

use std::collections::{HashMap, HashSet};
use std::fmt;

use celestina_paint::{PixmapId, Role, Theme, Weight};
use celestina_shell::clipboard::{MAX_PUBLISHED_ROWS, PREVIEW_UNITS};
use celestina_ui::{view, Key, View};
use taffy::JustifyContent;

use super::{note_list, tray::Pixmaps, PageCx, PageView};
use crate::{
    clipboard::{
        thumb::Thumb,
        verbs::ClipboardVerb,
        worker::{Preview, Row},
        ClipboardState,
    },
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, chrome_height, text_style, BUTTON_H},
    },
    notifications::banners::line,
};

/// The most rows the page draws, however tall the surface.
pub const MAX_ROWS: usize = MAX_PUBLISHED_ROWS;
/// The square an image entry's thumbnail is fitted into, logical px.
pub const THUMB_BOX: f32 = 40.0;

/// What one row shows.
#[derive(Clone, PartialEq, Eq)]
pub enum RowBody {
    /// The entry's bounded preview line.
    Text(String),
    /// An image entry, by its size in bytes.
    Image { bytes: usize },
}

impl fmt::Debug for RowBody {
    /// Never the preview: its length, or the image's size.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => f.debug_struct("Text").field("bytes", &text.len()).finish(),
            Self::Image { bytes } => f.debug_struct("Image").field("bytes", bytes).finish(),
        }
    }
}

/// One entry as the page shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct ClipboardRow {
    /// The position `select` and `remove` address.
    pub index: usize,
    pub body: RowBody,
    /// The thumbnail already uploaded to the painter, with its size in
    /// pixels; `None` for text and for an image with none at hand.
    pub thumb: Option<(PixmapId, (u32, u32))>,
}

impl fmt::Debug for ClipboardRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardRow")
            .field("index", &self.index)
            .field("body", &self.body)
            .field("thumb", &self.thumb.is_some())
            .finish()
    }
}

/// Everything the Clipboard page and piece read: the Island builds one from
/// the worker's latest rows ([`Textures::panel`]).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ClipboardPanel {
    /// `duties.clipboard` is `on`.
    pub on: bool,
    pub rows: Vec<ClipboardRow>,
    /// How many entries the history holds in all.
    pub total: usize,
    /// The snapshot [`ClipboardState::snapshot`] gave these rows: the page's
    /// verbs name it, so the worker refuses them once the history changed.
    pub snapshot: u64,
    /// Recording is paused.
    pub paused: bool,
    /// The session offers no way to watch the clipboard.
    pub unavailable: bool,
    /// The person asked to clear and has not confirmed yet.
    pub confirming_clear: bool,
}

impl fmt::Debug for ClipboardPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardPanel")
            .field("on", &self.on)
            .field("rows", &self.rows.len())
            .field("total", &self.total)
            .field("snapshot", &self.snapshot)
            .field("paused", &self.paused)
            .field("unavailable", &self.unavailable)
            .field("confirming_clear", &self.confirming_clear)
            .finish()
    }
}

/// The panel of a shell whose clipboard duty is off, for a context that has
/// no clipboard to show.
pub static NO_CLIPBOARD: ClipboardPanel = ClipboardPanel {
    on: false,
    rows: Vec::new(),
    total: 0,
    snapshot: 0,
    paused: false,
    unavailable: false,
    confirming_clear: false,
};

/// What the page asks of the Island.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageAction {
    /// A verb for the clipboard worker.
    Verb(ClipboardVerb),
    /// Clear was chosen: show the confirmation step.
    AskClear,
    /// The confirmation step was given up.
    CancelClear,
}

/// The row of the entry at `index`.
#[must_use]
pub fn row_key(index: usize) -> Key {
    Key::indexed("clip-row", index as u64)
}

/// The pause / resume button.
#[must_use]
pub fn pause_key() -> Key {
    Key::named("clip-pause")
}

/// The clear button.
#[must_use]
pub fn clear_key() -> Key {
    Key::named("clip-clear")
}

/// The button that confirms clearing.
#[must_use]
pub fn confirm_key() -> Key {
    Key::named("clip-clear-confirm")
}

/// The button that gives up clearing.
#[must_use]
pub fn cancel_key() -> Key {
    Key::named("clip-clear-cancel")
}

/// The accessible name of a row: its preview, bounded to the preview's
/// length, or for an image a fixed word with its size.
#[must_use]
pub fn row_name(row: &ClipboardRow) -> String {
    match &row.body {
        RowBody::Text(text) => bounded(text),
        RowBody::Image { bytes } => copy::clipboard_image_name(*bytes),
    }
}

/// `text` cut to [`PREVIEW_UNITS`] UTF-16 units, as the worker bounds it.
fn bounded(text: &str) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= PREVIEW_UNITS
        })
        .collect()
}

/// The thumbnail's drawn size: fitted into [`THUMB_BOX`], aspect kept.
fn thumb_size((w, h): (u32, u32)) -> (f32, f32) {
    let longest = w.max(h).max(1) as f32;
    let scale = (THUMB_BOX / longest).clamp(f32::EPSILON, 1.0);
    (
        (w as f32 * scale).max(1.0).round(),
        (h as f32 * scale).max(1.0).round(),
    )
}

fn row_height(theme: &Theme, row: &ClipboardRow) -> f32 {
    match row.body {
        RowBody::Text(_) => theme.row_height,
        RowBody::Image { .. } => theme.row_height.max(THUMB_BOX + 2.0 * theme.spacing.xs),
    }
}

/// The toolbar's height and the gap under it.
fn toolbar_block(theme: &Theme) -> f32 {
    BUTTON_H + theme.spacing.sm
}

/// The line that says how many entries the page does not show, and the gap
/// above it.
fn more_block(theme: &Theme) -> f32 {
    line(theme.type_scale.caption) + theme.spacing.sm
}

/// How many leading rows fit in `room` px.
fn rows_fitting(panel: &ClipboardPanel, theme: &Theme, room: f32) -> usize {
    let mut used = 0.0;
    let mut fit = 0;
    for row in panel.rows.iter().take(MAX_ROWS) {
        used += row_height(theme, row) + if fit > 0 { theme.spacing.xs } else { 0.0 };
        if used > room {
            break;
        }
        fit += 1;
    }
    fit
}

/// The entries the history holds that the page does not draw.
fn hidden(panel: &ClipboardPanel, shown: usize) -> usize {
    panel.total.max(panel.rows.len()).saturating_sub(shown)
}

/// The leading rows that fit a page of `max_size`, at most [`MAX_ROWS`]: the
/// rest are not built, so nothing invisible can be focused or clicked. When
/// some entries are left out, room is kept for the line that counts them.
fn drawn<'a>(panel: &'a ClipboardPanel, theme: &Theme, max_size: (f32, f32)) -> &'a [ClipboardRow] {
    let room = (max_size.1 - chrome_height(theme) - toolbar_block(theme)).max(0.0);
    let mut fit = rows_fitting(panel, theme, room);
    if hidden(panel, fit) > 0 {
        fit = rows_fitting(panel, theme, (room - more_block(theme)).max(0.0));
    }
    &panel.rows[..fit]
}

/// Whether the page shows rows and buttons, or only one line saying why not.
fn working(panel: &ClipboardPanel) -> bool {
    panel.on && !panel.unavailable
}

/// The index of the entry whose row is `key`, among the rows the page
/// draws on a page of `max_size`.
#[must_use]
pub fn index_of(
    panel: &ClipboardPanel,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<usize> {
    if !working(panel) {
        return None;
    }
    drawn(panel, theme, max_size)
        .iter()
        .map(|row| row.index)
        .find(|index| row_key(*index) == key)
}

/// What Delete on the focused node `key` removes, if it is a drawn row.
#[must_use]
pub fn remove_index(
    panel: &ClipboardPanel,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<PageAction> {
    index_of(panel, theme, max_size, key).map(|index| {
        PageAction::Verb(ClipboardVerb::RemoveShown {
            index,
            snapshot: panel.snapshot,
        })
    })
}

/// What a click or Enter on the node `key` asks for.
#[must_use]
pub fn action_for(
    panel: &ClipboardPanel,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<PageAction> {
    if !working(panel) {
        return None;
    }
    if key == pause_key() {
        let verb = if panel.paused {
            ClipboardVerb::Resume
        } else {
            ClipboardVerb::Pause
        };
        return Some(PageAction::Verb(verb));
    }
    if panel.confirming_clear {
        if key == confirm_key() {
            return Some(PageAction::Verb(ClipboardVerb::Clear));
        }
        if key == cancel_key() {
            return Some(PageAction::CancelClear);
        }
    } else if key == clear_key() && !panel.rows.is_empty() {
        return Some(PageAction::AskClear);
    }
    index_of(panel, theme, max_size, key).map(|index| {
        PageAction::Verb(ClipboardVerb::SelectShown {
            index,
            snapshot: panel.snapshot,
        })
    })
}

/// The Clipboard page.
#[derive(Clone, Copy, Debug)]
pub struct Clipboard;

impl PageView for Clipboard {
    fn title(&self) -> &str {
        copy::PAGE_CLIPBOARD
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        let panel = cx.clipboard;
        let note = |text: &str| {
            view::col(Key::named("clipboard")).child(note_list(
                theme,
                Key::named("clipboard-rows"),
                text,
            ))
        };
        if !panel.on {
            return note(copy::CLIPBOARD_OFF);
        }
        if panel.unavailable {
            return note(copy::CLIPBOARD_UNAVAILABLE);
        }
        let rows = drawn(panel, theme, cx.max_size);
        let left_out = hidden(panel, rows.len());
        let list = if panel.rows.is_empty() {
            note_list(theme, Key::named("clipboard-rows"), copy::CLIPBOARD_EMPTY)
        } else {
            view::col(Key::named("clipboard-rows"))
                .gap(theme.spacing.xs)
                .children(rows.iter().map(|row| entry(theme, row)))
                .children((left_out > 0).then(|| more_note(theme, left_out)))
        };
        view::col(Key::named("clipboard"))
            .gap(theme.spacing.sm)
            .child(toolbar(theme, panel))
            .child(list)
    }

    /// The default size, taller when the rows need it, up to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        let panel = cx.clipboard;
        if !working(panel) {
            return PAGE_SIZE;
        }
        let rows = drawn(panel, cx.theme, cx.max_size);
        let heights: f32 = rows.iter().map(|r| row_height(cx.theme, r)).sum();
        let gaps = rows.len().saturating_sub(1) as f32 * cx.theme.spacing.xs;
        let more = if hidden(panel, rows.len()) > 0 {
            more_block(cx.theme)
        } else {
            0.0
        };
        let needed = chrome_height(cx.theme) + toolbar_block(cx.theme) + heights + gaps + more;
        (PAGE_SIZE.0, PAGE_SIZE.1.max(needed))
    }
}

/// The line that counts the entries the page leaves out.
fn more_note(theme: &Theme, left_out: usize) -> View {
    view::text(
        Key::named("clip-more"),
        &copy::clipboard_more(left_out),
        text_style(
            theme.type_scale.caption,
            Weight::Regular,
            false,
            Role::TextLo,
        ),
    )
}

/// A text button of the page.
fn button(theme: &Theme, key: Key, label_key: Key, label: &str) -> View {
    centred(view::row(key))
        .height(BUTTON_H)
        .padding_xy(theme.spacing.md, 0.0)
        .radius(theme.radius_button)
        .fill(Role::ControlFill)
        .interactive()
        .focusable()
        .child(view::text(
            label_key,
            label,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
        ))
}

/// The pause toggle and the clear action (or its confirmation), with the
/// paused word at the start.
fn toolbar(theme: &Theme, panel: &ClipboardPanel) -> View {
    let state = panel.paused.then(|| {
        view::text(
            Key::named("clip-paused"),
            copy::CLIPBOARD_PAUSED,
            text_style(
                theme.type_scale.caption,
                Weight::Regular,
                false,
                Role::Accent,
            ),
        )
    });
    let (pause_label, pause_name) = if panel.paused {
        (copy::CLIPBOARD_RESUME, "clip-resume-label")
    } else {
        (copy::CLIPBOARD_PAUSE, "clip-pause-label")
    };
    let mut bar = centred(view::row(Key::named("clip-bar")))
        .height(BUTTON_H)
        .gap(theme.spacing.sm)
        .children(state)
        .child(view::row(Key::named("clip-bar-gap")).grow());
    bar.style.justify_content = JustifyContent::FLEX_END;
    bar = bar.child(button(
        theme,
        pause_key(),
        Key::named(pause_name),
        pause_label,
    ));
    if panel.confirming_clear {
        bar.child(view::text(
            Key::named("clip-ask"),
            copy::CLIPBOARD_CLEAR_ASK,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
        ))
        .child(button(
            theme,
            confirm_key(),
            Key::named("clip-confirm-label"),
            copy::CLIPBOARD_CLEAR_CONFIRM,
        ))
        .child(button(
            theme,
            cancel_key(),
            Key::named("clip-cancel-label"),
            copy::CLIPBOARD_CLEAR_CANCEL,
        ))
    } else if panel.rows.is_empty() {
        bar
    } else {
        bar.child(button(
            theme,
            clear_key(),
            Key::named("clip-clear-label"),
            copy::CLIPBOARD_CLEAR,
        ))
    }
}

/// The width the text of a row may take.
fn text_width(theme: &Theme) -> f32 {
    PAGE_SIZE.0 - 2.0 * theme.spacing.lg - 2.0 * theme.spacing.md
}

/// One entry's row.
fn entry(theme: &Theme, row: &ClipboardRow) -> View {
    let i = row.index as u64;
    let height = row_height(theme, row);
    let shell = centred(view::row(row_key(row.index)))
        .height(height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive()
        .focusable();
    match &row.body {
        RowBody::Text(text) => {
            let size = theme.type_scale.body;
            shell.child(
                view::col(Key::indexed("clip-text-box", i))
                    .height(line(size))
                    .clip()
                    .grow()
                    .child(
                        view::text(
                            Key::indexed("clip-text", i),
                            &bounded(text),
                            text_style(size, Weight::Regular, false, Role::TextHi),
                        )
                        .max_width(text_width(theme)),
                    ),
            )
        }
        RowBody::Image { bytes } => {
            let picture = match row.thumb {
                Some((id, px)) => {
                    let (w, h) = thumb_size(px);
                    view::image(Key::indexed("clip-thumb", i), id, 1.0).size(w, h)
                }
                None => view::icon(Key::indexed("clip-thumb", i), "image", THUMB_BOX / 2.0)
                    .tint(Role::TextLo),
            };
            let mut frame = centred(view::row(Key::indexed("clip-thumb-box", i)))
                .size(THUMB_BOX, THUMB_BOX)
                .child(picture);
            frame.style.justify_content = JustifyContent::CENTER;
            shell.child(frame).child(
                view::text(
                    Key::indexed("clip-text", i),
                    &copy::clipboard_image_name(*bytes),
                    text_style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
                )
                .grow(),
            )
        }
    }
}

/// The painter's textures for the image entries' thumbnails, uploaded once
/// per distinct thumbnail and released when the entry goes.
#[derive(Debug, Default)]
pub struct Textures {
    uploaded: HashMap<usize, (Thumb, PixmapId)>,
}

impl Textures {
    /// The panel for `state` with the duty `on` and the confirmation step
    /// `confirming_clear`. Thumbnails new since the last call are uploaded to
    /// `pixmaps`; the ones no row uses any more are released. A thumbnail the
    /// painter refuses leaves its row with the placeholder icon.
    pub fn panel(
        &mut self,
        on: bool,
        state: &ClipboardState,
        confirming_clear: bool,
        pixmaps: &mut impl Pixmaps,
    ) -> ClipboardPanel {
        let mut seen = HashSet::new();
        let rows = state
            .rows()
            .iter()
            .map(|row| ClipboardRow {
                index: row.index,
                body: body_of(row),
                thumb: self.texture(row, pixmaps, &mut seen),
            })
            .collect();
        let gone: Vec<usize> = self
            .uploaded
            .keys()
            .filter(|key| !seen.contains(*key))
            .copied()
            .collect();
        for key in gone {
            if let Some((_, id)) = self.uploaded.remove(&key) {
                pixmaps.release(id);
            }
        }
        ClipboardPanel {
            on,
            rows,
            total: state.total(),
            snapshot: state.snapshot(),
            paused: state.paused(),
            unavailable: state.unavailable().is_some(),
            confirming_clear,
        }
    }

    fn texture(
        &mut self,
        row: &Row,
        pixmaps: &mut impl Pixmaps,
        seen: &mut HashSet<usize>,
    ) -> Option<(PixmapId, (u32, u32))> {
        let Preview::Png {
            thumb: Some(thumb), ..
        } = &row.preview
        else {
            return None;
        };
        let key = thumb.identity();
        seen.insert(key);
        let size = (thumb.width(), thumb.height());
        if let Some((_, id)) = self.uploaded.get(&key) {
            return Some((*id, size));
        }
        let id = pixmaps.upload(&thumb.argb(), size.0, size.1)?;
        self.uploaded.insert(key, (thumb.clone(), id));
        Some((id, size))
    }

    /// Releases every texture (the Island goes away).
    pub fn release_all(&mut self, pixmaps: &mut impl Pixmaps) {
        for (_, (_, id)) in self.uploaded.drain() {
            pixmaps.release(id);
        }
    }

    /// How many textures are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.uploaded.len()
    }

    /// Whether none is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.uploaded.is_empty()
    }
}

fn body_of(row: &Row) -> RowBody {
    match &row.preview {
        Preview::Text(text) => RowBody::Text(text.clone()),
        Preview::Png { bytes, .. } => RowBody::Image { bytes: *bytes },
    }
}

/// A painter that stores nothing: it hands out ids and counts what it was
/// asked (tests only).
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct FakePixmaps {
    pub next: u32,
    pub live: HashSet<u32>,
    pub uploads: usize,
    /// Refuse every upload.
    pub refuse: bool,
}

#[cfg(test)]
impl Pixmaps for FakePixmaps {
    fn upload(&mut self, argb: &[u8], w: u32, h: u32) -> Option<PixmapId> {
        assert_eq!(argb.len() as u64, u64::from(w) * u64::from(h) * 4);
        if self.refuse {
            return None;
        }
        self.next += 1;
        self.uploads += 1;
        self.live.insert(self.next);
        Some(PixmapId(self.next))
    }

    fn release(&mut self, id: PixmapId) {
        assert!(self.live.remove(&id.0), "released an id twice or unknown");
    }

    fn has_theme_icon(&self, _: &str, _: u32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use celestina_ui::ViewKind;

    use super::*;
    use crate::clipboard::worker::ClipboardEvent;
    use crate::island::pages::{NameOwner, PageCx};

    const SECRET: &str = "hunter2-correct-horse";
    const SNAPSHOT: u64 = 7;

    fn text_row(index: usize, text: &str) -> ClipboardRow {
        ClipboardRow {
            index,
            body: RowBody::Text(text.to_owned()),
            thumb: None,
        }
    }

    fn image_row(index: usize, bytes: usize, thumb: Option<(u32, (u32, u32))>) -> ClipboardRow {
        ClipboardRow {
            index,
            body: RowBody::Image { bytes },
            thumb: thumb.map(|(id, px)| (PixmapId(id), px)),
        }
    }

    fn panel(rows: Vec<ClipboardRow>) -> ClipboardPanel {
        ClipboardPanel {
            on: true,
            total: rows.len(),
            snapshot: SNAPSHOT,
            rows,
            ..ClipboardPanel::default()
        }
    }

    fn build(panel: &ClipboardPanel, max_size: (f32, f32)) -> View {
        Clipboard.view(&cx(panel, max_size, &Theme::dark()))
    }

    fn cx<'a>(panel: &'a ClipboardPanel, max_size: (f32, f32), theme: &'a Theme) -> PageCx<'a> {
        static OWNERS: NameOwner = NameOwner {
            notifications: None,
            tray: None,
        };
        static CONNECTIVITY: std::sync::LazyLock<crate::connectivity::state::ConnectivityState> =
            std::sync::LazyLock::new(Default::default);
        PageCx {
            theme,
            max_size,
            model: None,
            output: "DP-1",
            owners: &OWNERS,
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            clipboard: panel,
            connectivity: &CONNECTIVITY,
            phone: &celestina_shell::phone::PhoneState::Absent,
        }
    }

    use celestina_paint::Theme;

    fn walk(v: &View) -> Vec<&View> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(n.children.iter().rev());
        }
        out
    }

    fn texts(v: &View) -> Vec<String> {
        walk(v)
            .into_iter()
            .filter_map(|n| match &n.kind {
                ViewKind::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn keys(v: &View) -> Vec<Key> {
        walk(v).into_iter().map(|n| n.key).collect()
    }

    fn find(v: &View, key: Key) -> Option<&View> {
        walk(v).into_iter().find(|n| n.key == key)
    }

    const ROOM: (f32, f32) = (760.0, 560.0);

    // ---- States.

    #[test]
    fn off_and_unavailable_say_why_and_offer_nothing() {
        let off = ClipboardPanel::default();
        let v = build(&off, ROOM);
        assert_eq!(texts(&v), [copy::CLIPBOARD_OFF]);
        assert!(!walk(&v).iter().any(|n| n.focusable));
        let broken = ClipboardPanel {
            on: true,
            unavailable: true,
            rows: vec![text_row(0, SECRET)],
            ..ClipboardPanel::default()
        };
        let v = build(&broken, ROOM);
        assert_eq!(texts(&v), [copy::CLIPBOARD_UNAVAILABLE]);
        assert!(!walk(&v).iter().any(|n| n.focusable));
        for panel in [&off, &broken] {
            assert_eq!(Clipboard.wants(&cx(panel, ROOM, &Theme::dark())), PAGE_SIZE);
            assert_eq!(
                action_for(panel, &Theme::dark(), ROOM, pause_key()),
                None,
                "nothing acts while the page only explains"
            );
        }
    }

    #[test]
    fn an_empty_history_says_so_and_can_still_be_paused_but_not_cleared() {
        let v = build(&panel(Vec::new()), ROOM);
        let said = texts(&v);
        assert!(said.contains(&copy::CLIPBOARD_EMPTY.to_owned()), "{said:?}");
        assert!(said.contains(&copy::CLIPBOARD_PAUSE.to_owned()));
        assert!(find(&v, pause_key()).is_some_and(|n| n.interactive && n.focusable));
        assert!(find(&v, clear_key()).is_none());
        let theme = Theme::dark();
        assert_eq!(
            action_for(&panel(Vec::new()), &theme, ROOM, clear_key()),
            None
        );
    }

    #[test]
    fn rows_are_interactive_focusable_and_newest_first() {
        let rows = vec![
            text_row(0, "newest"),
            text_row(1, "older"),
            text_row(2, "oldest"),
        ];
        let v = build(&panel(rows), ROOM);
        let order: Vec<Key> = (0..3).map(row_key).collect();
        let found: Vec<Key> = keys(&v).into_iter().filter(|k| order.contains(k)).collect();
        assert_eq!(found, order);
        for key in order {
            assert!(find(&v, key).is_some_and(|n| n.interactive && n.focusable));
        }
        let said = texts(&v);
        let at = |t: &str| said.iter().position(|s| s == t);
        assert!(at("newest") < at("older") && at("older") < at("oldest"));
    }

    #[test]
    fn an_image_row_shows_its_thumbnail_or_a_placeholder_and_a_fixed_label() {
        let rows = vec![
            image_row(0, 340 * 1024, Some((7, (64, 32)))),
            image_row(1, 2 * 1024 * 1024, None),
        ];
        let v = build(&panel(rows), ROOM);
        let thumb = find(&v, Key::indexed("clip-thumb", 0)).expect("thumbnail");
        assert!(matches!(
            thumb.kind,
            ViewKind::Image {
                id: PixmapId(7),
                ..
            }
        ));
        let placeholder = find(&v, Key::indexed("clip-thumb", 1)).expect("placeholder");
        assert!(matches!(placeholder.kind, ViewKind::Icon { .. }));
        let said = texts(&v);
        assert!(said.contains(&"Imagen, 340 KB".to_owned()), "{said:?}");
        assert!(said.contains(&"Imagen, 2,0 MB".to_owned()), "{said:?}");
    }

    #[test]
    fn a_thumbnail_is_fitted_into_its_box_keeping_the_aspect() {
        assert_eq!(thumb_size((64, 32)), (40.0, 20.0));
        assert_eq!(thumb_size((32, 64)), (20.0, 40.0));
        assert_eq!(thumb_size((10, 10)), (10.0, 10.0), "never enlarged");
        assert_eq!(thumb_size((64, 1)), (40.0, 1.0));
        assert_eq!(thumb_size((0, 0)), (1.0, 1.0), "never empty");
    }

    #[test]
    fn pausing_changes_the_toggle_and_says_so() {
        let mut p = panel(vec![text_row(0, "x")]);
        let theme = Theme::dark();
        let running = texts(&build(&p, ROOM));
        assert!(running.contains(&copy::CLIPBOARD_PAUSE.to_owned()));
        assert!(!running.contains(&copy::CLIPBOARD_PAUSED.to_owned()));
        assert_eq!(
            action_for(&p, &theme, ROOM, pause_key()),
            Some(PageAction::Verb(ClipboardVerb::Pause))
        );
        p.paused = true;
        let paused = texts(&build(&p, ROOM));
        assert!(paused.contains(&copy::CLIPBOARD_RESUME.to_owned()));
        assert!(paused.contains(&copy::CLIPBOARD_PAUSED.to_owned()));
        assert_eq!(
            action_for(&p, &theme, ROOM, pause_key()),
            Some(PageAction::Verb(ClipboardVerb::Resume))
        );
    }

    // ---- Actions.

    #[test]
    fn a_row_selects_and_delete_removes_that_entry() {
        let p = panel(vec![text_row(0, "a"), text_row(1, "b")]);
        let theme = Theme::dark();
        assert_eq!(
            action_for(&p, &theme, ROOM, row_key(1)),
            Some(PageAction::Verb(ClipboardVerb::SelectShown {
                index: 1,
                snapshot: SNAPSHOT
            }))
        );
        assert_eq!(
            remove_index(&p, &theme, ROOM, row_key(0)),
            Some(PageAction::Verb(ClipboardVerb::RemoveShown {
                index: 0,
                snapshot: SNAPSHOT
            }))
        );
        // A row that is not drawn, a button and a stranger remove nothing.
        assert_eq!(remove_index(&p, &theme, ROOM, row_key(2)), None);
        assert_eq!(remove_index(&p, &theme, ROOM, pause_key()), None);
        assert_eq!(remove_index(&p, &theme, ROOM, Key::named("close")), None);
        assert_eq!(action_for(&p, &theme, ROOM, row_key(2)), None);
        assert_eq!(action_for(&p, &theme, ROOM, Key::named("close")), None);
    }

    #[test]
    fn clearing_takes_two_steps_and_the_first_sends_no_verb() {
        let theme = Theme::dark();
        let mut p = panel(vec![text_row(0, "a")]);
        let first = build(&p, ROOM);
        assert!(find(&first, clear_key()).is_some());
        assert!(find(&first, confirm_key()).is_none());
        assert_eq!(
            action_for(&p, &theme, ROOM, clear_key()),
            Some(PageAction::AskClear)
        );
        // The confirm and cancel keys do nothing before the question.
        assert_eq!(action_for(&p, &theme, ROOM, confirm_key()), None);
        assert_eq!(action_for(&p, &theme, ROOM, cancel_key()), None);

        p.confirming_clear = true;
        let second = build(&p, ROOM);
        assert!(find(&second, clear_key()).is_none());
        assert!(find(&second, confirm_key()).is_some_and(|n| n.focusable));
        assert!(find(&second, cancel_key()).is_some_and(|n| n.focusable));
        let said = texts(&second);
        for want in [
            copy::CLIPBOARD_CLEAR_ASK,
            copy::CLIPBOARD_CLEAR_CONFIRM,
            copy::CLIPBOARD_CLEAR_CANCEL,
        ] {
            assert!(said.contains(&want.to_owned()), "{said:?}");
        }
        assert_eq!(
            action_for(&p, &theme, ROOM, confirm_key()),
            Some(PageAction::Verb(ClipboardVerb::Clear))
        );
        assert_eq!(
            action_for(&p, &theme, ROOM, cancel_key()),
            Some(PageAction::CancelClear)
        );
        assert_eq!(action_for(&p, &theme, ROOM, clear_key()), None);
    }

    // ---- Size and bounds.

    #[test]
    fn the_page_draws_only_the_rows_that_fit_and_never_more_than_the_published_bound() {
        let theme = Theme::dark();
        let rows: Vec<ClipboardRow> = (0..80).map(|i| text_row(i, "row")).collect();
        let p = panel(rows);
        let tall = (760.0, 4000.0);
        let drawn_tall = drawn(&p, &theme, tall).len();
        assert_eq!(drawn_tall, MAX_ROWS);
        let short = drawn(&p, &theme, ROOM).len();
        assert!(0 < short && short < MAX_ROWS, "{short}");
        // An undrawn row can be neither focused nor clicked nor removed.
        let v = build(&p, ROOM);
        assert!(find(&v, row_key(short - 1)).is_some());
        assert!(find(&v, row_key(short)).is_none());
        assert_eq!(action_for(&p, &theme, ROOM, row_key(short)), None);
        assert_eq!(remove_index(&p, &theme, ROOM, row_key(short)), None);
        // The page asks for no more than the surface lets it have.
        let (w, h) = Clipboard.wants(&cx(&p, ROOM, &theme));
        assert_eq!(w, PAGE_SIZE.0);
        assert!(h <= ROOM.1, "{h}");
        assert_eq!(drawn(&p, &theme, (760.0, 0.0)).len(), 0);
    }

    #[test]
    fn rows_that_do_not_fit_are_counted_in_a_line_and_the_page_makes_room_for_it() {
        let theme = Theme::dark();
        let rows: Vec<ClipboardRow> = (0..80).map(|i| text_row(i, "row")).collect();
        let mut p = panel(rows);
        p.total = 200;
        let v = build(&p, ROOM);
        let shown = drawn(&p, &theme, ROOM).len();
        let said = texts(&v);
        assert!(
            said.contains(&copy::clipboard_more(200 - shown)),
            "total less rows drawn: {said:?}"
        );
        // The line is kept inside the surface: the page asks for no more.
        let (_, h) = Clipboard.wants(&cx(&p, ROOM, &theme));
        assert!(h <= ROOM.1, "{h}");
        // One more row would have fit without the line: it was given up.
        assert!(
            shown
                < rows_fitting(
                    &p,
                    &theme,
                    ROOM.1 - chrome_height(&theme) - toolbar_block(&theme)
                ) + 1
        );
        // Nothing hidden, no line.
        let all = panel(vec![text_row(0, "a"), text_row(1, "b")]);
        assert!(find(&build(&all, ROOM), Key::named("clip-more")).is_none());
        // A page too short for any row still says how many there are.
        let none = build(&p, (760.0, 10.0));
        assert!(texts(&none).contains(&copy::clipboard_more(200)));
        assert!(!texts(&none).contains(&copy::CLIPBOARD_EMPTY.to_owned()));
    }

    #[test]
    fn a_page_with_a_few_rows_keeps_the_default_size() {
        let p = panel(vec![text_row(0, "a")]);
        assert_eq!(Clipboard.wants(&cx(&p, ROOM, &Theme::dark())), PAGE_SIZE);
    }

    // ---- Accessible names and privacy.

    #[test]
    fn a_rows_name_is_its_bounded_preview_or_a_fixed_label_with_a_size() {
        let long = "\u{f1}".repeat(PREVIEW_UNITS * 3);
        let name = row_name(&text_row(0, &long));
        assert_eq!(name.encode_utf16().count(), PREVIEW_UNITS);
        assert_eq!(row_name(&text_row(0, "short")), "short");
        assert_eq!(
            row_name(&image_row(0, 512, None)),
            "Imagen, 512 B",
            "no pixel data, only the word and the size"
        );
        // A preview with a character of two units is cut on a boundary.
        let astral = "😀".repeat(PREVIEW_UNITS);
        assert!(row_name(&text_row(0, &astral)).encode_utf16().count() <= PREVIEW_UNITS);
    }

    #[test]
    fn nothing_prints_an_entrys_contents() {
        let p = panel(vec![
            text_row(0, SECRET),
            image_row(1, 99, Some((3, (4, 4)))),
        ]);
        for printed in [format!("{p:?}"), format!("{:?}", p.rows)] {
            assert!(!printed.contains("hunter"), "{printed}");
        }
        assert!(format!("{:?}", p.rows[0]).contains(&SECRET.len().to_string()));
        assert!(!format!("{:?}", p.rows[0].body).contains("hunter"));
        let theme = Theme::dark();
        let printed = format!("{:?}", cx(&p, ROOM, &theme));
        assert!(!printed.contains("hunter"), "{printed}");
    }

    // ---- Thumbnails on the painter.

    fn png(red: u8) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(8, 4, image::Rgba([red, 0, 0, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut out, image::ImageFormat::Png)
            .expect("encode a png");
        out.into_inner()
    }

    fn state_of(rows: Vec<Row>) -> ClipboardState {
        let mut state = ClipboardState::default();
        let total = rows.len();
        state.apply(ClipboardEvent::Changed {
            rows,
            total,
            snapshot: 1,
        });
        state
    }

    fn image_state(reds: &[u8]) -> ClipboardState {
        state_of(
            reds.iter()
                .enumerate()
                .map(|(index, red)| {
                    let bytes = png(*red);
                    Row {
                        index,
                        preview: Preview::Png {
                            bytes: bytes.len(),
                            thumb: Thumb::from_png(&bytes).ok(),
                        },
                    }
                })
                .collect(),
        )
    }

    #[test]
    fn thumbnails_are_uploaded_once_and_released_with_their_entries() {
        let mut textures = Textures::default();
        let mut pixmaps = FakePixmaps::default();
        let state = image_state(&[10, 20]);
        let first = textures.panel(true, &state, false, &mut pixmaps);
        assert_eq!(pixmaps.uploads, 2);
        assert_eq!(textures.len(), 2);
        assert!(first
            .rows
            .iter()
            .all(|r| r.thumb.is_some_and(|(_, px)| px == (8, 4))));
        // The same rows again upload nothing.
        let again = textures.panel(true, &state, false, &mut pixmaps);
        assert_eq!(pixmaps.uploads, 2);
        assert_eq!(first, again);
        // A history without them releases them.
        let none = textures.panel(true, &ClipboardState::default(), false, &mut pixmaps);
        assert!(none.rows.is_empty());
        assert!(textures.is_empty() && pixmaps.live.is_empty());
    }

    #[test]
    fn a_painter_that_refuses_leaves_the_placeholder_and_release_all_frees_the_rest() {
        let mut textures = Textures::default();
        let mut pixmaps = FakePixmaps {
            refuse: true,
            ..FakePixmaps::default()
        };
        let state = image_state(&[1]);
        let refused = textures.panel(true, &state, false, &mut pixmaps);
        assert!(refused.rows[0].thumb.is_none());
        assert!(matches!(refused.rows[0].body, RowBody::Image { .. }));
        pixmaps.refuse = false;
        textures.panel(true, &state, false, &mut pixmaps);
        assert_eq!(pixmaps.live.len(), 1);
        textures.release_all(&mut pixmaps);
        assert!(pixmaps.live.is_empty() && textures.is_empty());
    }

    #[test]
    fn the_panel_carries_the_state_and_the_confirmation_step() {
        let mut textures = Textures::default();
        let mut pixmaps = FakePixmaps::default();
        let mut state = state_of(vec![Row {
            index: 0,
            preview: Preview::Text(SECRET.to_owned()),
        }]);
        state.apply(ClipboardEvent::Paused(true));
        let panel = textures.panel(true, &state, true, &mut pixmaps);
        assert!(panel.on && panel.paused && panel.confirming_clear && !panel.unavailable);
        assert_eq!(panel.total, 1);
        assert_eq!(panel.snapshot, 1, "the page names the rows it shows");
        assert_eq!(panel.rows[0].body, RowBody::Text(SECRET.to_owned()));
        state.apply(ClipboardEvent::Unavailable("no data control".to_owned()));
        let panel = textures.panel(false, &state, false, &mut pixmaps);
        assert!(!panel.on && panel.unavailable);
    }
}
