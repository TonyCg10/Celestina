//! The launcher's view: a Haze card with the search field on top and a
//! clipped area of eight rows under it (the list scrolls past them), centred
//! in a transparent surface that fills the output (design §5.5, Launcher; `celestina-style/DESIGN.md`:
//! search fields are pills, rows of the row height). An application's row
//! shows its icon, its name and its generic name; a switch's row its
//! semantic icon, its command and what it does; a minimised window's row
//! (the selector) its icon, its title and its application id. The selected
//! row has the control fill the Island's pages give their current row.

use celestina_paint::{IconKey, Rect, Role, TextStyle, Theme, Weight};
use celestina_ui::{view, FieldId, Key, View, ViewKind};
use std::ops::Range;
use taffy::{AlignItems, JustifyContent, LengthPercentageAuto};

use super::{
    commands::Switch,
    groups::Group,
    selector,
    state::{AppRow, LauncherState, ListView, Row, Source, Tab, GRID_COLUMNS},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    copy,
    island::pages::activity::truncate_title,
    scrolling::{self, THUMB_INSET, THUMB_RADIUS, THUMB_W},
};

/// The card's width, logical px.
pub const LAUNCHER_W: f32 = 560.0;
/// How many rows the area shows at once.
pub const AREA_ROWS: usize = 8;
/// Rows built beyond each edge of the area, so a row that enters with the
/// tree's entrance animation enters out of view.
pub const BUILD_MARGIN: usize = 4;
/// A row's icon.
pub const ICON_SIZE: f32 = 24.0;
/// The search glyph in the field.
const SEARCH_ICON: f32 = 18.0;
/// The clear button's side, its glyph and how far it reaches into the
/// pill's right padding.
const CLEAR_SIZE: f32 = 28.0;
const CLEAR_ICON: f32 = 16.0;
const CLEAR_MARGIN: f32 = -8.0;
/// The Enter key cap: a one-px outline, corner radius and padding.
const KEY_CAP_RADIUS: f32 = 5.0;
const KEY_CAP_PAD_X: f32 = 5.0;
const KEY_CAP_PAD_Y: f32 = 3.0;
/// The icon of an application whose entry names none.
const FALLBACK_ICON: &str = "app-window";
/// The tab row's height.
pub const TAB_ROW_H: f32 = 36.0;
/// A tab's height and corner radius, its glyph, its side padding and the
/// gap between its glyph and its label.
const TAB_H: f32 = 32.0;
const TAB_RADIUS: f32 = 16.0;
const TAB_ICON: f32 = 18.0;
const TAB_PAD_X: f32 = 9.0;
const TAB_LABEL_GAP: f32 = 6.0;
/// The gap between the tabs.
const TAB_GAP: f32 = 4.0;
/// The view button's width (its height is a tab's).
const VIEW_BUTTON_W: f32 = 36.0;
/// The tabs' opacity while a query is typed.
const TABS_DIMMED: f32 = 0.45;
/// A grid cell's height, its icon, the gap under the icon, its side
/// padding.
pub const CELL_H: f32 = 80.0;
const CELL_ICON: f32 = 40.0;
const CELL_GAP: f32 = 6.0;
const CELL_PAD_X: f32 = 4.0;
/// Grid lines built beyond each edge of the area.
pub const GRID_BUILD_MARGIN: usize = 2;

/// Cuts a text in a style to a width: a grid cell's name. The launcher
/// measures with the painter's text system ([`fit_end`]).
pub type Fit<'a> = &'a mut dyn FnMut(&str, &TextStyle, f32) -> String;

/// `text` whole if `width_of` says it fits `max_width`; else its longest
/// start that fits with an ellipsis after it (trailing spaces dropped), or
/// the ellipsis alone.
pub fn fit_end(text: &str, max_width: f32, mut width_of: impl FnMut(&str) -> f32) -> String {
    if width_of(text) <= max_width {
        return text.to_owned();
    }
    let ends: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .skip(1)
        .collect();
    let cut = |end: usize| format!("{}\u{2026}", text[..end].trim_end());
    // The longest start that fits: widths grow with the start's length.
    let (mut lo, mut hi) = (0, ends.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width_of(&cut(ends[mid - 1])) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    match lo {
        0 => "\u{2026}".to_owned(),
        n => cut(ends[n - 1]),
    }
}

/// The launcher's text field.
pub const FIELD: FieldId = FieldId(1);

/// The surface: the root of the view, transparent, holding the card.
#[must_use]
pub fn surface_key() -> Key {
    Key::named("launcher-surface")
}

/// The card.
#[must_use]
pub fn card_key() -> Key {
    Key::named("launcher")
}

/// The search field's node.
#[must_use]
pub fn field_key() -> Key {
    Key::named("launcher-field")
}

/// The clear button in the pill.
#[must_use]
pub fn clear_key() -> Key {
    Key::named("launcher-clear")
}

/// The hold-Backspace sweep in the pill.
#[must_use]
pub fn sweep_key() -> Key {
    Key::named("launcher-sweep")
}

/// The calculator's row, always the first: its own node, so it enters and
/// leaves with its own entrance.
#[must_use]
pub fn calc_key() -> Key {
    Key::named("launcher-calc")
}

/// The clipped box that shows the rows and takes the wheel.
#[must_use]
pub fn area_key() -> Key {
    Key::named("launcher-area")
}

/// The column inside the area that holds the built rows and is offset by
/// the scroll.
#[must_use]
pub fn inner_key() -> Key {
    Key::named("launcher-inner")
}

/// The scroll bar at the area's right edge.
#[must_use]
pub fn thumb_key() -> Key {
    Key::named("launcher-thumb")
}

/// The row (or grid cell) at `index`.
#[must_use]
pub fn row_key(index: usize) -> Key {
    Key::indexed("launcher-row", index as u64)
}

/// The tab row: the tabs, then the view button.
#[must_use]
pub fn tabs_key() -> Key {
    Key::named("launcher-tabs")
}

/// The tabs together, the part that dims while a query is typed.
#[must_use]
pub fn tabset_key() -> Key {
    Key::named("launcher-tabset")
}

/// The tab at visible position `i`.
#[must_use]
pub fn tab_key(i: usize) -> Key {
    Key::indexed("launcher-tab", i as u64)
}

/// The visible position `key` names, if it is one of `count` tabs.
#[must_use]
pub fn tab_of(key: Key, count: usize) -> Option<usize> {
    (0..count).find(|&i| tab_key(i) == key)
}

/// The button that switches between the list and the grid.
#[must_use]
pub fn view_button_key() -> Key {
    Key::named("launcher-view")
}

/// The node of a tab or view switch: the inner column sits in it, so a
/// switch is a new node and the tree crossfades. Keyed by the tab itself,
/// so a group's tab appearing or going leaves the others' content alone.
#[must_use]
pub fn content_key(tab: Tab, grid: bool) -> Key {
    let tab = match tab {
        Tab::Recent => 0,
        Tab::All => 1,
        Tab::Group(group) => 2 + Group::ALL.iter().position(|g| *g == group).unwrap_or(0),
    };
    Key::indexed("launcher-content", tab as u64 * 2 + u64::from(grid))
}

/// The row `key` names, if it is one of `count` rows.
#[must_use]
pub fn row_of(key: Key, count: usize) -> Option<usize> {
    if key == calc_key() {
        return (count > 0).then_some(0);
    }
    (0..count).find(|&i| row_key(i) == key)
}

/// How the list is scrolled when the view is built.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ListScroll {
    /// How far the list is scrolled, logical px.
    pub offset: f32,
    /// The bar's y and height in the area; `None` when nothing scrolls.
    pub thumb: Option<(f32, f32)>,
    /// The bar is shown (opacity 1) or fading out (0).
    pub thumb_shown: bool,
}

impl ListScroll {
    /// The scroll of `state`'s rows in `theme`, `offset` px down, with the
    /// bar shown or not.
    #[must_use]
    pub fn new(theme: &Theme, state: &LauncherState, offset: f32, shown: bool) -> ListScroll {
        let content = content_height(theme, state);
        let viewport = AREA_ROWS as f32 * theme.row_height;
        ListScroll {
            offset,
            thumb: scrolling::thumb(offset, content, viewport),
            thumb_shown: shown,
        }
    }
}

/// The wash that sweeps the query away while Backspace is held: it grows
/// from the text's end towards its start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    /// 0 (just started) to 1 (the whole text).
    pub progress: f32,
    /// The text's end, logical px from the field's left edge.
    pub text_end: f32,
}

/// The wash's distance from the pill's top and bottom.
const SWEEP_INSET: f32 = 10.0;

/// The card's logical size with `theme` over `source`: the card's width;
/// the field, the tab row (the launcher only) and the area of eight rows,
/// each under a small gap, padded by the medium spacing.
#[must_use]
pub fn card_size(theme: &Theme, source: &Source) -> (f32, f32) {
    let tabs = match source {
        Source::Apps => TAB_ROW_H + theme.spacing.sm,
        Source::Bubbles(_) => 0.0,
    };
    let h = 2.0 * theme.spacing.md
        + theme.top_bar
        + theme.spacing.sm
        + tabs
        + AREA_ROWS as f32 * theme.row_height;
    (LAUNCHER_W, h)
}

/// Whether `state` lays its rows out as a grid: the author chose it and the
/// rows are applications (a `/` query lists switches as rows whatever the
/// view; the selector never has a grid).
#[must_use]
pub fn use_grid(state: &LauncherState) -> bool {
    state.view == ListView::Grid
        && matches!(state.source, Source::Apps)
        // A `/` query answers switches only.
        && !matches!(state.rows.first(), Some(Row::Switch(_)))
}

/// The height of the whole list or grid for `state`, logical px.
#[must_use]
pub fn content_height(theme: &Theme, state: &LauncherState) -> f32 {
    let count = state.rows.len();
    if !use_grid(state) {
        return count as f32 * theme.row_height;
    }
    let lead = state.grid_lead();
    let lines = (count - lead).div_ceil(GRID_COLUMNS);
    lead as f32 * theme.row_height + lines as f32 * CELL_H
}

/// The top and the height of item `i` in the list or grid, logical px.
#[must_use]
pub fn item_span(theme: &Theme, state: &LauncherState, i: usize) -> (f32, f32) {
    if !use_grid(state) {
        return (i as f32 * theme.row_height, theme.row_height);
    }
    let lead = state.grid_lead();
    if i < lead {
        return (0.0, theme.row_height);
    }
    let line = state.grid_line(i).0 - lead;
    (
        lead as f32 * theme.row_height + line as f32 * CELL_H,
        CELL_H,
    )
}

/// The items to build with the area `offset` px down: those in view and
/// [`BUILD_MARGIN`] rows (or [`GRID_BUILD_MARGIN`] lines, whole) beyond
/// each edge.
#[must_use]
pub fn built_items(theme: &Theme, state: &LauncherState, offset: f32) -> Range<usize> {
    let count = state.rows.len();
    let viewport = AREA_ROWS as f32 * theme.row_height;
    if !use_grid(state) {
        return scrolling::built(offset, theme.row_height, viewport, count, BUILD_MARGIN);
    }
    let lead = state.grid_lead();
    let lead_h = lead as f32 * theme.row_height;
    let margin = GRID_BUILD_MARGIN as f32 * CELL_H;
    let (top, bottom) = (offset - margin, offset + viewport + margin);
    let cells = scrolling::built(
        offset - lead_h,
        CELL_H,
        viewport,
        (count - lead).div_ceil(GRID_COLUMNS),
        GRID_BUILD_MARGIN,
    );
    let start = if lead == 1 && top < lead_h && bottom > 0.0 {
        0
    } else {
        lead + cells.start * GRID_COLUMNS
    };
    let end = (lead + cells.end * GRID_COLUMNS).min(count);
    start.min(end)..end
}

fn centred(mut v: View) -> View {
    v.style.align_items = AlignItems::CENTER;
    v
}

fn style(size: f32, role: Role) -> TextStyle {
    TextStyle {
        size,
        weight: Weight::Regular,
        tabular: false,
        role,
    }
}

fn icon(key: Key, icon: IconKey, tint: Option<Role>, size: f32) -> View {
    View::new(
        key,
        ViewKind::Icon {
            key: icon,
            tint,
            size,
        },
    )
    .size(size, size)
}

/// Where a card of `card` size sits in a surface of `surface` size: centred,
/// on whole logical pixels (so the layout's rounding cannot move it off the
/// rect the blur region and the hit test use), and never above or left of
/// the origin when the surface is smaller than the card.
#[must_use]
pub fn card_rect(surface: (f32, f32), card: (f32, f32)) -> Rect {
    let offset = |outer: f32, inner: f32| ((outer - inner) / 2.0).floor().max(0.0);
    Rect {
        x: offset(surface.0, card.0),
        y: offset(surface.1, card.1),
        w: card.0,
        h: card.1,
    }
}

/// The launcher for `state` in `theme` on a surface of `surface` size: a
/// transparent root as large as the surface, with the card centred in it.
#[must_use]
pub fn launcher(
    fit: Fit<'_>,
    theme: &Theme,
    state: &LauncherState,
    surface: (f32, f32),
    sweep: Option<Sweep>,
    scroll: &ListScroll,
) -> View {
    let at = card_rect(surface, card_size(theme, &state.source));
    view::col(surface_key())
        .size(surface.0, surface.1)
        .child(card(fit, theme, state, sweep, scroll).absolute(at.x, at.y))
}

/// The card for `state` in `theme`, with the `sweep` over the query while
/// there is one (the launcher of applications only) and the list scrolled
/// as `scroll` says; `fit` cuts the grid's names to their cells.
#[must_use]
pub fn card(
    fit: Fit<'_>,
    theme: &Theme,
    state: &LauncherState,
    sweep: Option<Sweep>,
    scroll: &ListScroll,
) -> View {
    let (w, h) = card_size(theme, &state.source);
    let (placeholder, note) = match &state.source {
        Source::Apps => (copy::LAUNCHER_PLACEHOLDER, None),
        Source::Bubbles(bubbles) => (copy::SELECTOR_PLACEHOLDER, selector::note(bubbles)),
    };
    let mut pill = centred(view::row(Key::named("launcher-pill")))
        .height(theme.top_bar)
        .radius(theme.top_bar / 2.0)
        .fill(Role::ControlFill)
        .padding_xy(theme.spacing.lg, 0.0)
        .gap(theme.spacing.sm)
        .child(icon(
            Key::named("launcher-search"),
            IconKey::Semantic("search"),
            Some(Role::TextLo),
            SEARCH_ICON,
        ))
        .child(
            view::field(
                field_key(),
                FIELD,
                placeholder,
                style(theme.type_scale.row_title, Role::TextHi),
            )
            .grow(),
        );
    let apps = matches!(state.source, Source::Apps);
    if let Some(sweep) = sweep.filter(|_| apps) {
        // First, so the text and the glyphs draw over it.
        let text_end = sweep.text_end.max(0.0);
        let width = text_end * sweep.progress.clamp(0.0, 1.0);
        let field_left = theme.spacing.lg + SEARCH_ICON + theme.spacing.sm;
        let wash = view::col(sweep_key())
            .size(width, theme.top_bar - 2.0 * SWEEP_INSET)
            .fill(Role::AccentSoft)
            .absolute(field_left + text_end - width, SWEEP_INSET);
        pill.children.insert(0, wash);
    }
    if apps {
        pill = pill.child(clear_button(!state.query.text().is_empty()));
    }
    let area_w = w - 2.0 * theme.spacing.md;
    let area = rows_area(fit, theme, state, scroll, area_w, note);
    let mut card = view::col(card_key())
        .haze(theme.radius_lg)
        .size(w, h)
        .padding(theme.spacing.md)
        .gap(theme.spacing.sm)
        .child(pill);
    if apps {
        card = card.child(tab_row(theme, state));
    }
    card.child(area)
}

/// The glyph's name and the label of `tab`.
fn tab_parts(tab: Tab) -> (&'static str, &'static str) {
    match tab {
        Tab::Recent => ("rotate-ccw-clock", copy::LAUNCHER_TAB_RECENT),
        Tab::All => ("arrow-down-a-z", copy::LAUNCHER_TAB_ALL),
        Tab::Group(group) => (group.icon(), group.label()),
    }
}

/// The tab row: one pill per visible tab, a spacer, then the view button.
/// The open tab is filled and carries its label (only it is built, so the
/// tree fades it in while the neighbours slide); the others show a glyph.
/// The tabs dim while a query answers instead of the tab; they stay
/// clickable.
fn tab_row(theme: &Theme, state: &LauncherState) -> View {
    let tabs = state.visible_tabs();
    let mut set = centred(view::row(tabset_key()))
        .gap(TAB_GAP)
        .grow()
        .opacity(if state.searching() { TABS_DIMMED } else { 1.0 });
    for (i, tab) in tabs.iter().enumerate() {
        let (glyph, label) = tab_parts(*tab);
        let open = *tab == state.tab;
        let mut pill = centred(view::row(tab_key(i)))
            .height(TAB_H)
            .radius(TAB_RADIUS)
            .padding_xy(TAB_PAD_X, 0.0)
            .gap(TAB_LABEL_GAP)
            .child(icon(
                Key::indexed("launcher-tab-icon", i as u64),
                IconKey::Semantic(glyph),
                Some(if open { Role::TextHi } else { Role::TextLo }),
                TAB_ICON,
            ));
        if open {
            pill = pill.fill(Role::ControlFill).child(view::text(
                Key::indexed("launcher-tab-label", i as u64),
                label,
                style(theme.type_scale.body, Role::TextHi),
            ));
        }
        // The open tab takes a click only while a query is typed: it then
        // empties the query and shows the tab.
        if !open || state.searching() {
            pill = pill.interactive();
        }
        set = set.child(pill);
    }
    let glyph = match state.view {
        ListView::List => "view-grid",
        ListView::Grid => "view-list",
    };
    let mut button = centred(view::row(view_button_key()))
        .size(VIEW_BUTTON_W, TAB_H)
        .radius(TAB_RADIUS)
        .interactive()
        .child(icon(
            Key::named("launcher-view-icon"),
            IconKey::Semantic(glyph),
            Some(Role::TextLo),
            TAB_ICON,
        ));
    button.style.justify_content = JustifyContent::CENTER;
    centred(view::row(tabs_key()))
        .height(TAB_ROW_H)
        .gap(TAB_GAP)
        .child(set)
        .child(button)
}

/// The area: a clipped box of eight rows' height holding a content node
/// (keyed by the tab and the layout, so a switch crossfades) with the inner
/// column, which is offset up by the scroll and holds the rows or cells near
/// the view, each at its full place; and the scroll bar over it. Only items
/// in view and the build margin beyond are built.
fn rows_area(
    fit: Fit<'_>,
    theme: &Theme,
    state: &LauncherState,
    scroll: &ListScroll,
    width: f32,
    note: Option<&str>,
) -> View {
    let row_h = theme.row_height;
    let viewport = AREA_ROWS as f32 * row_h;
    let count = state.rows.len();
    let grid = use_grid(state);
    let mut area = view::col(area_key()).size(width, viewport).clip();
    if scroll.thumb.is_some() {
        // Rows cover the whole area, so the box itself is never what the
        // pointer lifts: it only needs to be a target when it scrolls.
        area = area.interactive();
    }
    let inner = if count == 0 {
        let text = match note {
            Some(note) => note,
            None if matches!(state.source, Source::Apps)
                && state.tab == Tab::Recent
                && !state.searching() =>
            {
                copy::LAUNCHER_RECENT_EMPTY
            }
            None => copy::LAUNCHER_EMPTY,
        };
        view::col(inner_key())
            .size(width, viewport)
            .absolute(0.0, 0.0)
            .child(
                centred(view::row(Key::named("launcher-empty")))
                    .absolute(0.0, 0.0)
                    .size(width, row_h)
                    .padding_xy(theme.spacing.md, 0.0)
                    .child(view::text(
                        Key::named("launcher-empty-text"),
                        text,
                        style(theme.type_scale.body, Role::TextLo),
                    )),
            )
    } else {
        let built = built_items(theme, state, scroll.offset);
        let cell_w = width / GRID_COLUMNS as f32;
        view::col(inner_key())
            .size(width, content_height(theme, state))
            .absolute(0.0, 0.0 - scroll.offset)
            .children(built.map(|i| {
                let selected = i == state.selected;
                let (top, _) = item_span(theme, state, i);
                match &state.rows[i] {
                    Row::App(app) if grid => {
                        let column = state.grid_line(i).1;
                        cell(fit, theme, i, app, selected, cell_w)
                            .absolute(column as f32 * cell_w, top)
                    }
                    other => row(theme, i, other, selected, state.copied)
                        .width(width)
                        .absolute(0.0, top),
                }
            }))
    };
    let content = view::col(content_key(state.tab, grid))
        .size(width, viewport)
        .absolute(0.0, 0.0)
        .child(inner);
    area = area.child(content);
    if let Some((y, h)) = scroll.thumb {
        area = area.child(
            view::col(thumb_key())
                .size(THUMB_W, h)
                .radius(THUMB_RADIUS)
                .fill(Role::ControlHover)
                .opacity(if scroll.thumb_shown { 1.0 } else { 0.0 })
                .absolute(width - THUMB_INSET - THUMB_W, y),
        );
    }
    area
}

/// The clear button: a round plate with an `x`, at the pill's right end. It
/// fades in with the first character and out with the last, and takes no
/// click while it is out.
fn clear_button(shown: bool) -> View {
    let mut v = centred(view::row(clear_key()))
        .size(CLEAR_SIZE, CLEAR_SIZE)
        .radius(CLEAR_SIZE / 2.0)
        .opacity(if shown { 1.0 } else { 0.0 })
        .child(icon(
            Key::named("launcher-clear-icon"),
            IconKey::Semantic("x"),
            Some(Role::TextLo),
            CLEAR_ICON,
        ));
    v.style.justify_content = JustifyContent::CENTER;
    v.style.margin.right = LengthPercentageAuto::length(CLEAR_MARGIN);
    if shown {
        v = v.interactive();
    }
    v
}

/// An application's icon, and the tint of the fallback glyph when its entry
/// names none.
fn app_glyph(app: &AppRow) -> (IconKey, Option<Role>) {
    match IconKey::desktop_icon(&app.entry.icon) {
        Some(key) => (key, None),
        None => (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo)),
    }
}

/// One grid cell: the icon over the name, centred, in a column of `width`.
fn cell(
    fit: Fit<'_>,
    theme: &Theme,
    index: usize,
    app: &AppRow,
    selected: bool,
    width: f32,
) -> View {
    let i = index as u64;
    let (glyph, tint) = app_glyph(app);
    let name_style = style(theme.type_scale.row_secondary, Role::TextHi);
    let name = fit(&app.name, &name_style, width - 2.0 * CELL_PAD_X);
    let mut v = centred(view::col(row_key(index)))
        .size(width, CELL_H)
        .padding_xy(CELL_PAD_X, 0.0)
        .gap(CELL_GAP)
        .radius(theme.radius_md)
        .interactive();
    v.style.justify_content = JustifyContent::CENTER;
    if selected {
        v = v.fill(Role::ControlFill);
    }
    v.child(icon(
        Key::indexed("launcher-icon", i),
        glyph,
        tint,
        CELL_ICON,
    ))
    .child(view::text(
        Key::indexed("launcher-name", i),
        &name,
        name_style,
    ))
}

/// One row: the icon, the name growing, the secondary text on the right.
fn row(theme: &Theme, index: usize, row: &Row, selected: bool, copied: bool) -> View {
    let i = index as u64;
    if let Row::Calc { result } = row {
        return calc_row(theme, result, selected, copied);
    }
    let (glyph, tint, name, detail) = match row {
        Row::App(app) => {
            let (glyph, tint) = app_glyph(app);
            (
                glyph,
                tint,
                truncate_title(&app.name),
                truncate_title(&app.detail),
            )
        }
        Row::Switch(switch) => switch_parts(*switch),
        Row::Calc { .. } => unreachable!("the calculator's row returned above"),
        Row::Bubble {
            title,
            app_id,
            icon,
            ..
        } => {
            let (glyph, tint) = match icon {
                Some(name) => (IconKey::Theme(name.clone()), None),
                None => (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo)),
            };
            let name = if title.is_empty() {
                copy::WINDOW_UNTITLED
            } else {
                title.as_str()
            };
            (glyph, tint, truncate_title(name), truncate_title(app_id))
        }
    };
    let mut v = centred(view::row(row_key(index)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive();
    if selected {
        v = v.fill(Role::ControlFill);
    }
    v.child(icon(
        Key::indexed("launcher-icon", i),
        glyph,
        tint,
        ICON_SIZE,
    ))
    .child(
        view::text(
            Key::indexed("launcher-name", i),
            &name,
            style(theme.type_scale.body, Role::TextHi),
        )
        .grow(),
    )
    .child(view::text(
        Key::indexed("launcher-detail", i),
        &detail,
        style(theme.type_scale.row_secondary, Role::TextLo),
    ))
}

/// The calculator's row: its glyph (a check once copied), an `=` and the
/// result in the title size, and on the right the action with an Enter key
/// cap, or "Copied" in the success colour once the result is on the
/// clipboard.
fn calc_row(theme: &Theme, result: &str, selected: bool, copied: bool) -> View {
    let (glyph, word, word_role) = if copied {
        ("check", copy::LAUNCHER_CALC_COPIED, Role::Success)
    } else {
        ("calculator", copy::LAUNCHER_CALC_COPY, Role::TextLo)
    };
    let detail_style = style(theme.type_scale.row_secondary, word_role);
    let mut detail = centred(view::row(Key::named("launcher-calc-detail")))
        .gap(theme.spacing.sm)
        .child(view::text(
            Key::named("launcher-calc-word"),
            word,
            detail_style,
        ));
    if !copied {
        detail = detail.child(
            centred(view::row(Key::named("launcher-calc-cap")))
                .radius(KEY_CAP_RADIUS)
                .stroke(Role::Divider)
                .padding_xy(KEY_CAP_PAD_X, KEY_CAP_PAD_Y)
                .child(view::text(
                    Key::named("launcher-calc-key"),
                    copy::LAUNCHER_CALC_KEY,
                    style(theme.type_scale.row_secondary, Role::TextLo),
                )),
        );
    }
    let mut v = centred(view::row(calc_key()))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive();
    if selected {
        v = v.fill(Role::ControlFill);
    }
    v.child(icon(
        Key::named("launcher-calc-icon"),
        IconKey::Semantic(glyph),
        Some(Role::TextLo),
        ICON_SIZE,
    ))
    .child(
        centred(view::row(Key::named("launcher-calc-name")))
            .gap(KEY_CAP_PAD_X)
            .grow()
            .child(view::text(
                Key::named("launcher-calc-equals"),
                "=",
                style(theme.type_scale.row_title, Role::TextLo),
            ))
            .child(view::text(
                Key::named("launcher-calc-result"),
                result,
                TextStyle {
                    tabular: true,
                    ..style(theme.type_scale.row_title, Role::TextHi)
                },
            )),
    )
    .child(detail)
}

fn switch_parts(switch: Switch) -> (IconKey, Option<Role>, String, String) {
    (
        IconKey::Semantic(switch.icon()),
        Some(Role::TextLo),
        switch.command().to_owned(),
        switch.label().to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_core::desktop_entry::DesktopEntry;
    use celestina_ui::Tree;

    use super::*;

    /// Names stay whole in the tests: there is no text system.
    fn whole(s: &str, _: &TextStyle, _: f32) -> String {
        s.to_owned()
    }

    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn entry(id: &str, name: &str) -> DesktopEntry {
        DesktopEntry {
            id: format!("{id}.desktop"),
            name: name.to_owned(),
            generic_name: "Generic".to_owned(),
            icon: id.to_owned(),
            exec: id.to_owned(),
            is_application: true,
            ..DesktopEntry::default()
        }
    }

    /// The rows' area in a card.
    fn area(v: &View) -> &View {
        v.children
            .iter()
            .find(|c| c.key == area_key())
            .expect("the card holds the rows' area")
    }

    /// The content node of the area (keyed by the tab and the layout).
    fn content(v: &View) -> &View {
        area(v)
            .children
            .iter()
            .find(|c| c.key != thumb_key())
            .expect("the area holds the content node")
    }

    /// The column inside the content node that holds the built rows.
    fn inner(v: &View) -> &View {
        content(v)
            .children
            .iter()
            .find(|c| c.key == inner_key())
            .expect("the content node holds the inner column")
    }

    /// The built rows (or the note) of a card.
    fn rows_of(v: &View) -> &Vec<View> {
        &inner(v).children
    }

    fn at(x: f32, y: f32) -> (LengthPercentageAuto, LengthPercentageAuto) {
        (
            LengthPercentageAuto::length(x),
            LengthPercentageAuto::length(y),
        )
    }

    fn many(n: usize) -> (Vec<DesktopEntry>, LauncherState) {
        let index: Vec<_> = (0..n)
            .map(|i| entry(&format!("t{i:02}"), &format!("Tool {i:02}")))
            .collect();
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.update(&index);
        state.set_tab(Tab::All, &index);
        (index, state)
    }

    fn scrolled(theme: &Theme, state: &LauncherState, offset: f32, shown: bool) -> ListScroll {
        ListScroll::new(theme, state, offset, shown)
    }

    #[test]
    fn thirty_answers_build_the_rows_near_the_top_in_a_clipped_area() {
        let theme = Theme::dark();
        let (_, state) = many(30);
        assert_eq!(state.rows.len(), 30);
        let scroll = scrolled(&theme, &state, 0.0, false);
        let v = card(&mut whole, &theme, &state, None, &scroll);
        let a = area(&v);
        assert!(a.clip, "the area clips");
        assert_eq!(
            a.style.size.height,
            taffy::Dimension::length(AREA_ROWS as f32 * theme.row_height)
        );
        let content_w = LAUNCHER_W - 2.0 * theme.spacing.md;
        assert_eq!(a.style.size.width, taffy::Dimension::length(content_w));
        assert!(a.interactive, "a scrolling area takes the wheel");
        let rows = rows_of(&v);
        assert_eq!(rows.len(), 12, "8 in view and 4 of margin");
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.key, row_key(i));
            assert_eq!(row.style.position, taffy::Position::Absolute);
            let (x, y) = at(0.0, i as f32 * theme.row_height);
            assert_eq!((row.style.inset.left, row.style.inset.top), (x, y));
        }
        let inner = inner(&v);
        assert_eq!(
            inner.style.size.height,
            taffy::Dimension::length(30.0 * theme.row_height)
        );
        assert_eq!(inner.style.inset.top, LengthPercentageAuto::length(0.0));
    }

    #[test]
    fn a_scrolled_list_builds_the_rows_around_the_offset_and_moves_the_column_up() {
        let theme = Theme::dark();
        let (_, state) = many(30);
        let offset = 10.0 * theme.row_height;
        let v = card(
            &mut whole,
            &theme,
            &state,
            None,
            &scrolled(&theme, &state, offset, true),
        );
        let keys: Vec<_> = rows_of(&v).iter().map(|r| r.key).collect();
        let want: Vec<_> = (6..22).map(row_key).collect();
        assert_eq!(keys, want);
        let (_, y) = at(0.0, -offset);
        assert_eq!(inner(&v).style.inset.top, y);
        // A row keeps its place in the column, whatever the offset.
        let row = &rows_of(&v)[0];
        assert_eq!(
            row.style.inset.top,
            LengthPercentageAuto::length(6.0 * theme.row_height)
        );
    }

    #[test]
    fn the_calculator_row_keeps_its_own_key_at_the_top_of_the_column() {
        let theme = Theme::dark();
        let index: Vec<_> = (0..30)
            .map(|i| entry(&format!("t{i:02}"), &format!("2+2 Tool {i:02}")))
            .collect();
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.set_query("2+2", &index);
        assert!(matches!(state.rows[0], Row::Calc { .. }));
        let v = card(
            &mut whole,
            &theme,
            &state,
            None,
            &scrolled(&theme, &state, 0.0, false),
        );
        let first = &rows_of(&v)[0];
        assert_eq!(first.key, calc_key());
        assert_eq!(first.style.inset.top, LengthPercentageAuto::length(0.0));
        assert_eq!(rows_of(&v)[1].key, row_key(1));
    }

    #[test]
    fn the_thumb_shows_with_thirty_rows_and_is_absent_with_five() {
        let theme = Theme::dark();
        let (_, state) = many(30);
        let v = card(
            &mut whole,
            &theme,
            &state,
            None,
            &scrolled(&theme, &state, 0.0, true),
        );
        let thumb = area(&v)
            .children
            .iter()
            .find(|c| c.key == thumb_key())
            .expect("30 rows scroll: the bar is built");
        assert!(matches!(
            thumb.kind,
            ViewKind::Box {
                fill: Some(Role::ControlHover),
                radius,
                ..
            } if radius == 2.0
        ));
        assert_eq!(thumb.opacity, 1.0);
        assert!(!thumb.interactive);
        let content_w = LAUNCHER_W - 2.0 * theme.spacing.md;
        assert_eq!(thumb.style.size.width, taffy::Dimension::length(4.0));
        let (_, h) = scrolling::thumb(0.0, 30.0 * theme.row_height, 8.0 * theme.row_height)
            .expect("the content is taller than the area");
        assert_eq!(thumb.style.size.height, taffy::Dimension::length(h));
        assert_eq!(
            thumb.style.inset.left,
            LengthPercentageAuto::length(content_w - 2.0 - 4.0)
        );

        let hidden = card(
            &mut whole,
            &theme,
            &state,
            None,
            &scrolled(&theme, &state, 0.0, false),
        );
        let thumb = area(&hidden)
            .children
            .iter()
            .find(|c| c.key == thumb_key())
            .expect("kept built so that it can fade");
        assert_eq!(thumb.opacity, 0.0);

        let (_, few) = many(5);
        let v = card(
            &mut whole,
            &theme,
            &few,
            None,
            &scrolled(&theme, &few, 0.0, true),
        );
        assert!(area(&v).children.iter().all(|c| c.key != thumb_key()));
        assert!(!area(&v).interactive, "nothing to scroll: no wheel target");
    }

    #[test]
    fn the_selector_builds_at_most_eight_rows_and_no_thumb() {
        use crate::bubbles::{Bubbles, Link};
        use celestina_shell::melibea::Window;
        let theme = Theme::dark();
        let windows: Vec<_> = (0..20)
            .map(|i| Window {
                id: 100 - i,
                app_id: Some("app".to_owned()),
                title: Some(format!("Doc {i}")),
                icon_name: None,
            })
            .collect();
        let state = LauncherState::selector(Bubbles {
            windows,
            link: Link::Live,
        });
        assert_eq!(state.rows.len(), 8);
        let v = card(
            &mut whole,
            &theme,
            &state,
            None,
            &scrolled(&theme, &state, 0.0, true),
        );
        assert_eq!(rows_of(&v).len(), 8);
        assert!(area(&v).children.iter().all(|c| c.key != thumb_key()));
    }

    #[test]
    fn the_area_is_laid_out_inside_the_card_and_clips_its_rows() {
        let theme = Theme::dark();
        let (_, state) = many(30);
        let scroll = scrolled(&theme, &state, 3.0 * theme.row_height, false);
        let surface = (2560.0, 1440.0);
        let mut tree = Tree::new(false, &theme);
        tree.apply(
            launcher(&mut whole, &theme, &state, surface, None, &scroll),
            Instant::now(),
        );
        tree.layout_measured(surface, fake_measure);
        let rect = |key| {
            tree.node_by_key(key)
                .map(|id| tree.rect(id))
                .expect("laid out")
        };
        let (card_r, area_r) = (rect(card_key()), rect(area_key()));
        assert_eq!(area_r.h, 8.0 * theme.row_height);
        assert_eq!(area_r.w, LAUNCHER_W - 2.0 * theme.spacing.md);
        assert!(
            area_r.y + area_r.h <= card_r.y + card_r.h,
            "inside the card"
        );
        let inner_r = rect(inner_key());
        assert_eq!(inner_r.y, area_r.y - 3.0 * theme.row_height);
        let row3 = rect(row_key(3));
        assert_eq!(row3.y, area_r.y, "row 3 is the first in view");
        assert_eq!(row3.h, theme.row_height);
        assert_eq!(row3.w, area_r.w, "a row is as wide as the area");
    }

    #[test]
    fn the_card_holds_the_field_and_one_row_per_answer_and_keeps_the_typed_text() {
        let theme = Theme::dark();
        let index = [entry("aa", "Aa"), entry("ab", "Ab"), entry("ac", "Ac")];
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.set_query("a", &index);
        let mut tree = Tree::new(false, &theme);
        let t0 = Instant::now();
        let size = card_size(&theme, &state.source);
        let surface = (2560.0, 1440.0);
        let at = card_rect(surface, size);
        tree.apply(
            launcher(
                &mut whole,
                &theme,
                &state,
                surface,
                None,
                &ListScroll::default(),
            ),
            t0,
        );
        tree.layout_measured(surface, fake_measure);

        let root = tree.node_by_key(surface_key()).map(|id| tree.rect(id));
        assert_eq!(
            root,
            Some(Rect::whole(surface)),
            "the root fills the surface"
        );
        let laid = tree.node_by_key(card_key()).map(|id| tree.rect(id));
        assert_eq!(
            laid,
            Some(at),
            "the card is laid out where card_rect puts it"
        );
        let field = tree.node_by_key(field_key()).map(|id| tree.rect(id));
        assert!(field.is_some_and(|r| r.w > 0.0
            && r.y >= at.y
            && r.y < at.y + theme.top_bar + theme.spacing.md));
        let rows: Vec<_> = (0..AREA_ROWS)
            .filter_map(|i| tree.node_by_key(row_key(i)))
            .map(|id| tree.rect(id))
            .collect();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.h == theme.row_height));
        assert!(rows.windows(2).all(|p| p[1].y > p[0].y), "top to bottom");
        assert!(
            rows.iter().all(|r| r.y + r.h <= at.y + at.h && r.x >= at.x),
            "inside the card"
        );

        // The person's text lives in the tree's field; a new answer keeps it.
        if let Some(field) = tree.field_mut(FIELD) {
            field.set_text("ab");
        }
        state.set_query("ab", &index);
        tree.apply(
            launcher(
                &mut whole,
                &theme,
                &state,
                surface,
                None,
                &ListScroll::default(),
            ),
            t0,
        );
        tree.layout_measured(surface, fake_measure);
        assert_eq!(
            tree.field(FIELD).map(|f| f.text().to_owned()),
            Some("ab".to_owned())
        );
        // One answer now; the rows that left fade out in the tree.
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        assert_eq!(rows_of(&v).len(), 1);
    }

    #[test]
    fn the_selected_row_has_the_control_fill_and_the_others_none() {
        let theme = Theme::dark();
        let index = [entry("aa", "Aa"), entry("ab", "Ab")];
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.set_query("a", &index);
        state.move_selection(1);
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        let rows = rows_of(&v);
        let fills: Vec<_> = rows
            .iter()
            .map(|r| match r.kind {
                ViewKind::Box { fill, .. } => fill,
                _ => None,
            })
            .collect();
        assert_eq!(fills, [None, Some(Role::ControlFill)]);
        assert!(rows.iter().all(|r| r.interactive && !r.focusable));
        assert_eq!(row_of(row_key(1), 2), Some(1));
        assert_eq!(row_of(row_key(2), 2), None);
    }

    #[test]
    fn the_card_is_centred_on_whole_pixels_and_never_leaves_the_surface() {
        let card = (560.0, 400.0);
        // DP-1, 3840×2160 at scale 1.5.
        assert_eq!(
            card_rect((2560.0, 1440.0), card),
            Rect {
                x: 1000.0,
                y: 520.0,
                w: 560.0,
                h: 400.0
            }
        );
        // An odd logical size floors to a whole pixel.
        let odd = card_rect((1707.0, 961.0), card);
        assert_eq!((odd.x, odd.y), (573.0, 280.0));
        // A surface smaller than the card puts it at the origin.
        let small = card_rect((400.0, 300.0), card);
        assert_eq!((small.x, small.y), (0.0, 0.0));
        // No size yet (before the configure).
        assert_eq!(card_rect((0.0, 0.0), card).x, 0.0);
    }

    #[test]
    fn an_unanswered_query_shows_one_line_and_no_row() {
        let theme = Theme::dark();
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.set_query("/zzz", &[]);
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        let rows = rows_of(&v);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, Key::named("launcher-empty"));
        assert!(!rows[0].interactive);
    }

    fn note_text(v: &View) -> String {
        let rows = rows_of(v);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, Key::named("launcher-empty"));
        assert!(!rows[0].interactive, "the note is not a row to choose");
        match &rows[0].children[0].kind {
            ViewKind::Text { text, .. } => text.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_selector_shows_its_note_when_it_has_no_windows() {
        use crate::bubbles::{Bubbles, Link};
        let theme = Theme::dark();
        let empty = LauncherState::selector(Bubbles {
            windows: Vec::new(),
            link: Link::Live,
        });
        assert_eq!(
            note_text(&card(
                &mut whole,
                &theme,
                &empty,
                None,
                &ListScroll::default()
            )),
            copy::SELECTOR_NO_WINDOWS
        );
        for link in [Link::Absent, Link::Refused, Link::Unavailable] {
            let gone = LauncherState::selector(Bubbles {
                windows: Vec::new(),
                link,
            });
            assert_eq!(
                note_text(&card(
                    &mut whole,
                    &theme,
                    &gone,
                    None,
                    &ListScroll::default()
                )),
                copy::SELECTOR_UNAVAILABLE,
                "{link:?}"
            );
        }
        // Windows that the query does not answer: the launcher's own line.
        let mut some = LauncherState::selector(Bubbles {
            windows: vec![celestina_shell::melibea::Window {
                id: 1,
                app_id: None,
                title: Some("Notes".to_owned()),
                icon_name: None,
            }],
            link: Link::Live,
        });
        some.set_query("zzz", &[]);
        assert_eq!(
            note_text(&card(
                &mut whole,
                &theme,
                &some,
                None,
                &ListScroll::default()
            )),
            copy::LAUNCHER_EMPTY
        );
    }

    #[test]
    fn a_window_row_shows_its_icon_title_and_id_and_names_the_untitled() {
        use crate::bubbles::{Bubbles, Link};
        use celestina_shell::melibea::Window;
        let theme = Theme::dark();
        let state = LauncherState::selector(Bubbles {
            windows: vec![
                Window {
                    id: 1,
                    app_id: Some("kitty".to_owned()),
                    title: Some("\u{1f600}".repeat(10_000)),
                    icon_name: Some("kitty".to_owned()),
                },
                Window {
                    id: 2,
                    app_id: None,
                    title: None,
                    icon_name: None,
                },
            ],
            link: Link::Live,
        });
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        let rows = rows_of(&v);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.interactive));
        let texts = |r: &View| -> Vec<String> {
            r.children
                .iter()
                .filter_map(|c| match &c.kind {
                    ViewKind::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect()
        };
        assert!(matches!(
            &rows[0].children[0].kind,
            ViewKind::Icon { key: IconKey::Theme(n), .. } if n == "kitty"
        ));
        let first = texts(&rows[0]);
        assert!(first[0].chars().count() < 100, "the hostile title is cut");
        assert_eq!(first[1], "kitty");
        assert!(matches!(
            &rows[1].children[0].kind,
            ViewKind::Icon {
                key: IconKey::Semantic(FALLBACK_ICON),
                ..
            }
        ));
        assert_eq!(texts(&rows[1]), [copy::WINDOW_UNTITLED, ""]);
    }

    #[test]
    fn the_clear_button_fades_with_the_query_and_the_selector_has_none() {
        let theme = Theme::dark();
        let index = [entry("aa", "Aa")];
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        let pill = |state: &LauncherState| {
            card(&mut whole, &theme, state, None, &ListScroll::default())
                .children
                .remove(0)
        };
        let empty = pill(&state);
        let button = empty.children.last().expect("the pill has children");
        assert_eq!(button.key, clear_key());
        assert_eq!(button.opacity, 0.0);
        assert!(!button.interactive, "a hidden button takes no click");

        state.set_query("ab", &index);
        let shown = pill(&state);
        let button = shown.children.last().expect("the pill has children");
        assert_eq!(button.key, clear_key());
        assert_eq!(button.opacity, 1.0);
        assert!(button.interactive);
        assert!(matches!(
            button.kind,
            ViewKind::Box { radius, .. } if radius == 14.0
        ));
        assert_eq!(
            button.style.size,
            taffy::Size {
                width: taffy::Dimension::length(28.0),
                height: taffy::Dimension::length(28.0)
            }
        );
        assert!(matches!(
            button.children[0].kind,
            ViewKind::Icon {
                key: IconKey::Semantic("x"),
                tint: Some(Role::TextLo),
                size,
            } if size == 16.0
        ));

        use crate::bubbles::Bubbles;
        let selector = LauncherState::selector(Bubbles::default());
        assert!(pill(&selector)
            .children
            .iter()
            .all(|c| c.key != clear_key()));
    }

    #[test]
    fn the_sweep_is_a_wash_that_grows_from_the_texts_end_inside_the_pill() {
        let theme = Theme::dark();
        let state = LauncherState::new(std::collections::HashMap::new(), None);
        let sweep = Sweep {
            progress: 0.5,
            text_end: 120.0,
        };
        let v = card(
            &mut whole,
            &theme,
            &state,
            Some(sweep),
            &ListScroll::default(),
        );
        let node = v.children[0]
            .children
            .iter()
            .find(|c| c.key == sweep_key())
            .expect("the pill holds the sweep");
        let field_left = theme.spacing.lg + SEARCH_ICON + theme.spacing.sm;
        assert!(matches!(
            node.kind,
            ViewKind::Box {
                fill: Some(Role::AccentSoft),
                ..
            }
        ));
        assert_eq!(node.style.position, taffy::Position::Absolute);
        assert_eq!(node.style.size.width, taffy::Dimension::length(60.0));
        assert_eq!(node.style.size.height, taffy::Dimension::length(28.0));
        assert_eq!(node.style.inset.top, LengthPercentageAuto::length(10.0));
        assert_eq!(
            node.style.inset.left,
            LengthPercentageAuto::length(field_left + 120.0 - 60.0),
            "the right edge stays at the text's end"
        );
        assert!(!node.interactive);
        let first = &v.children[0].children[0];
        assert_eq!(first.key, sweep_key(), "under the text: the first child");

        let none = card(&mut whole, &theme, &state, None, &ListScroll::default());
        assert!(none.children[0]
            .children
            .iter()
            .all(|c| c.key != sweep_key()));
    }

    fn calc_state(copied: bool) -> LauncherState {
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.set_query("2*(3+4)^2", &[]);
        state.copied = copied;
        state
    }

    #[test]
    fn the_calculator_row_shows_the_glyph_the_equals_the_result_and_the_copy_detail() {
        let theme = Theme::dark();
        let v = card(
            &mut whole,
            &theme,
            &calc_state(false),
            None,
            &ListScroll::default(),
        );
        let row = &rows_of(&v)[0];
        assert_eq!(row.key, calc_key());
        assert!(row.interactive);
        assert!(matches!(
            row.kind,
            ViewKind::Box {
                fill: Some(Role::ControlFill),
                ..
            }
        ));
        assert!(matches!(
            row.children[0].kind,
            ViewKind::Icon {
                key: IconKey::Semantic("calculator"),
                tint: Some(Role::TextLo),
                size,
            } if size == 24.0
        ));
        let name = &row.children[1];
        let (eq, result) = (&name.children[0], &name.children[1]);
        assert!(matches!(
            &eq.kind,
            ViewKind::Text { text, style, .. } if text == "=" && style.role == Role::TextLo
        ));
        assert!(matches!(
            &result.kind,
            ViewKind::Text { text, style, .. }
                if text == "98"
                    && style.role == Role::TextHi
                    && style.tabular
                    && style.size == theme.type_scale.row_title
        ));
        let detail = &row.children[2];
        assert!(matches!(
            &detail.children[0].kind,
            ViewKind::Text { text, style, .. }
                if text == copy::LAUNCHER_CALC_COPY && style.role == Role::TextLo
        ));
        let cap = &detail.children[1];
        assert!(matches!(
            cap.kind,
            ViewKind::Box {
                stroke: Some(Role::Divider),
                radius,
                ..
            } if radius == 5.0
        ));
        assert!(matches!(
            &cap.children[0].kind,
            ViewKind::Text { text, .. } if text == copy::LAUNCHER_CALC_KEY
        ));
    }

    #[test]
    fn after_the_copy_the_row_shows_the_check_and_the_copied_word_in_success() {
        let theme = Theme::dark();
        let v = card(
            &mut whole,
            &theme,
            &calc_state(true),
            None,
            &ListScroll::default(),
        );
        let row = &rows_of(&v)[0];
        assert!(matches!(
            row.children[0].kind,
            ViewKind::Icon {
                key: IconKey::Semantic("check"),
                ..
            }
        ));
        let detail = &row.children[2];
        assert_eq!(detail.children.len(), 1, "no key cap once copied");
        assert!(matches!(
            &detail.children[0].kind,
            ViewKind::Text { text, style, .. }
                if text == copy::LAUNCHER_CALC_COPIED && style.role == Role::Success
        ));
    }

    #[test]
    fn a_click_on_the_calculator_row_names_row_zero() {
        assert_eq!(row_of(calc_key(), 3), Some(0));
        assert_eq!(row_of(calc_key(), 0), None);
    }

    fn bubbles_state() -> LauncherState {
        use crate::bubbles::{Bubbles, Link};
        LauncherState::selector(Bubbles {
            windows: Vec::new(),
            link: Link::Live,
        })
    }

    /// A state over `n` applications on the All tab in the grid.
    fn gridded(n: usize) -> LauncherState {
        let (_, mut state) = many(n);
        state.view = ListView::Grid;
        state
    }

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    fn icon_name(v: &View) -> &str {
        match &v.kind {
            ViewKind::Icon {
                key: IconKey::Semantic(name),
                ..
            } => name,
            other => panic!("not a semantic icon: {other:?}"),
        }
    }

    #[test]
    fn the_launcher_card_is_560_by_444_and_the_selectors_560_by_400() {
        let theme = Theme::dark();
        assert_eq!(card_size(&theme, &Source::Apps), (560.0, 444.0));
        assert_eq!(card_size(&theme, &bubbles_state().source), (560.0, 400.0));
        let v = card(
            &mut whole,
            &theme,
            &bubbles_state(),
            None,
            &ListScroll::default(),
        );
        assert!(
            find(&v, tabs_key()).is_none(),
            "the selector has no tab row"
        );
    }

    #[test]
    fn the_tab_row_holds_a_tab_per_visible_tab_then_the_view_button() {
        let theme = Theme::dark();
        let (_, state) = many(3);
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        let row = find(&v, tabs_key()).expect("the launcher has a tab row");
        assert_eq!(row.style.size.height, taffy::Dimension::length(36.0));
        let keys: Vec<_> = row.children.iter().map(|c| c.key).collect();
        assert_eq!(keys, [tabset_key(), view_button_key()]);
        let set = &row.children[0];
        let tabs = state.visible_tabs();
        assert_eq!(set.children.len(), tabs.len());
        for (i, tab) in set.children.iter().enumerate() {
            assert_eq!(tab.key, tab_key(i));
            assert_eq!(tab_of(tab.key, tabs.len()), Some(i));
            assert_eq!(tab.style.size.height, taffy::Dimension::length(32.0));
        }
        assert_eq!(tab_of(tab_key(tabs.len()), tabs.len()), None);
        let button = &row.children[1];
        assert_eq!(button.style.size.width, taffy::Dimension::length(36.0));
        assert!(button.interactive);
        assert_eq!(icon_name(&button.children[0]), "view-grid");
        let grid = gridded(3);
        let v = card(&mut whole, &theme, &grid, None, &ListScroll::default());
        let button = find(&v, view_button_key()).expect("view button");
        assert_eq!(icon_name(&button.children[0]), "view-list");
    }

    #[test]
    fn the_open_tab_is_filled_and_labelled_and_the_others_are_glyphs_that_lift() {
        let theme = Theme::dark();
        let (_, state) = many(3);
        assert_eq!(state.tab, Tab::All);
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        let tabs = &find(&v, tabset_key()).expect("tab set").children;
        let (recent, all) = (&tabs[0], &tabs[1]);
        assert!(matches!(
            all.kind,
            ViewKind::Box {
                fill: Some(Role::ControlFill),
                radius,
                ..
            } if radius == 16.0
        ));
        assert_eq!(all.children.len(), 2, "glyph and label");
        let label = &all.children[1];
        assert_eq!(label.opacity, 1.0);
        assert!(matches!(
            &label.kind,
            ViewKind::Text { text, style, .. }
                if text == copy::LAUNCHER_TAB_ALL
                    && style.role == Role::TextHi
                    && style.size == theme.type_scale.body
        ));
        assert_eq!(icon_name(&all.children[0]), "arrow-down-a-z");
        assert!(matches!(
            all.children[0].kind,
            ViewKind::Icon { tint: Some(Role::TextHi), size, .. } if size == 18.0
        ));
        assert_eq!(recent.children.len(), 1, "a closed tab carries no label");
        assert_eq!(icon_name(&recent.children[0]), "rotate-ccw-clock");
        assert!(matches!(
            recent.children[0].kind,
            ViewKind::Icon {
                tint: Some(Role::TextLo),
                ..
            }
        ));
        assert!(recent.interactive);
        assert!(matches!(recent.kind, ViewKind::Box { fill: None, .. }));
    }

    #[test]
    fn a_typed_query_dims_the_tabs_but_not_the_view_button() {
        let theme = Theme::dark();
        let (index, mut state) = many(3);
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        assert_eq!(find(&v, tabset_key()).expect("set").opacity, 1.0);
        state.set_query("to", &index);
        assert!(state.searching());
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        assert_eq!(find(&v, tabset_key()).expect("set").opacity, 0.45);
        assert_eq!(find(&v, view_button_key()).expect("button").opacity, 1.0);
    }

    #[test]
    fn the_content_node_is_keyed_by_the_tab_and_the_layout_not_by_the_query() {
        let theme = Theme::dark();
        let (index, mut state) = many(3);
        let key = |s: &LauncherState| {
            content(&card(&mut whole, &theme, s, None, &ListScroll::default())).key
        };
        let all = key(&state);
        assert_eq!(all, content_key(Tab::All, false));
        state.set_query("to", &index);
        assert_eq!(key(&state), all, "a query in the tab keeps the node");
        state.set_tab(Tab::Recent, &index);
        assert_ne!(key(&state), all);
        state.set_tab(Tab::All, &index);
        state.view = ListView::Grid;
        assert_eq!(key(&state), content_key(Tab::All, true));
        assert!(find(
            &card(&mut whole, &theme, &state, None, &ListScroll::default()),
            inner_key()
        )
        .is_some());
    }

    #[test]
    fn the_grid_lays_six_cells_a_line_each_80_high_with_a_40_px_icon_and_a_12_px_name() {
        let theme = Theme::dark();
        let state = gridded(14);
        assert!(use_grid(&state));
        let scroll = scrolled(&theme, &state, 0.0, false);
        let v = card(&mut whole, &theme, &state, None, &scroll);
        let cells = rows_of(&v);
        assert_eq!(cells.len(), 14);
        let width = LAUNCHER_W - 2.0 * theme.spacing.md;
        let cell_w = width / 6.0;
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(c.key, row_key(i));
            assert_eq!(c.style.size.height, taffy::Dimension::length(80.0));
            assert_eq!(c.style.size.width, taffy::Dimension::length(cell_w));
            let (x, y) = at((i % 6) as f32 * cell_w, (i / 6) as f32 * 80.0);
            assert_eq!((c.style.inset.left, c.style.inset.top), (x, y));
            assert!(c.interactive);
        }
        let first = &cells[0];
        assert!(matches!(
            first.kind,
            ViewKind::Box { fill: Some(Role::ControlFill), radius, .. } if radius == theme.radius_md
        ));
        assert!(matches!(cells[1].kind, ViewKind::Box { fill: None, .. }));
        assert!(matches!(
            first.children[0].kind,
            ViewKind::Icon { size, .. } if size == 40.0
        ));
        assert!(matches!(
            &first.children[1].kind,
            ViewKind::Text { text, style, .. }
                if text == "Tool 00"
                    && style.size == theme.type_scale.row_secondary
                    && style.role == Role::TextHi
        ));
        assert_eq!(theme.type_scale.row_secondary, 12.0);
        assert_eq!(first.style.gap.height, taffy::LengthPercentage::length(6.0));
        assert_eq!(
            inner(&v).style.size.height,
            taffy::Dimension::length(3.0 * 80.0)
        );
    }

    #[test]
    fn a_long_name_is_cut_with_an_ellipsis_in_a_cell() {
        let theme = Theme::dark();
        let index = vec![entry("long", "An application with a very long name")];
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.update(&index);
        state.set_tab(Tab::All, &index);
        state.view = ListView::Grid;
        // Ten px a character: the cell's 81 px of room keep eight.
        let mut asked = 0.0;
        let mut fit = |s: &str, _: &TextStyle, width: f32| {
            asked = width;
            fit_end(s, width, |t| t.chars().count() as f32 * 10.0)
        };
        let v = card(&mut fit, &theme, &state, None, &ListScroll::default());
        let ViewKind::Text { text, .. } = &rows_of(&v)[0].children[1].kind else {
            panic!("the name is text");
        };
        assert_eq!(text, "An appl\u{2026}");
        assert_eq!(asked, 536.0 / 6.0 - 8.0);
    }

    #[test]
    fn fit_end_keeps_the_longest_start_that_fits() {
        let ten = |t: &str| t.chars().count() as f32 * 10.0;
        assert_eq!(fit_end("Krita", 50.0, ten), "Krita");
        assert_eq!(
            fit_end("AMDGPU TOP (GUI)", 110.0, ten),
            "AMDGPU TOP\u{2026}"
        );
        // A cut before a space drops the space.
        assert_eq!(fit_end("AMDGPU TOP (GUI)", 80.0, ten), "AMDGPU\u{2026}");
        assert_eq!(fit_end("Krita", 5.0, ten), "\u{2026}");
        assert_eq!(fit_end("", 0.0, ten), "");
    }

    #[test]
    fn a_calculator_row_spans_the_width_above_the_cells() {
        let theme = Theme::dark();
        let index: Vec<_> = (0..8)
            .map(|i| entry(&format!("t{i:02}"), &format!("2+2 Tool {i:02}")))
            .collect();
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.update(&index);
        state.view = ListView::Grid;
        state.set_query("2+2", &index);
        assert!(matches!(state.rows[0], Row::Calc { .. }));
        assert!(use_grid(&state));
        let v = card(
            &mut whole,
            &theme,
            &state,
            None,
            &scrolled(&theme, &state, 0.0, false),
        );
        let items = rows_of(&v);
        let width = LAUNCHER_W - 2.0 * theme.spacing.md;
        assert_eq!(items[0].key, calc_key());
        assert_eq!(items[0].style.size.width, taffy::Dimension::length(width));
        assert_eq!(items[0].style.size.height, taffy::Dimension::length(40.0));
        assert_eq!(items[0].style.inset.top, LengthPercentageAuto::length(0.0));
        let cell_w = width / 6.0;
        let (x, y) = at(0.0, 40.0);
        assert_eq!(
            (items[1].style.inset.left, items[1].style.inset.top),
            (x, y),
            "the first cell sits under the row"
        );
        let (x, y) = at(5.0 * cell_w, 40.0);
        assert_eq!(
            (items[6].style.inset.left, items[6].style.inset.top),
            (x, y)
        );
        let (x, y) = at(0.0, 40.0 + 80.0);
        assert_eq!(
            (items[7].style.inset.left, items[7].style.inset.top),
            (x, y)
        );
        assert_eq!(
            content_height(&theme, &state),
            40.0 + 2.0 * 80.0,
            "a row of 40 and two lines"
        );
    }

    #[test]
    fn a_slash_query_lists_switches_as_rows_whatever_the_view() {
        let theme = Theme::dark();
        let mut state = gridded(3);
        state.set_query("/", &[]);
        assert!(state.rows.iter().all(|r| matches!(r, Row::Switch(_))));
        assert!(!use_grid(&state));
        let v = card(&mut whole, &theme, &state, None, &ListScroll::default());
        assert_eq!(
            rows_of(&v)[0].style.size.height,
            taffy::Dimension::length(theme.row_height)
        );
        assert!(!use_grid(&bubbles_state()));
    }

    #[test]
    fn spans_and_heights_follow_the_layout() {
        let theme = Theme::dark();
        let (_, list) = many(30);
        assert_eq!(content_height(&theme, &list), 30.0 * 40.0);
        assert_eq!(item_span(&theme, &list, 3), (120.0, 40.0));
        let grid = gridded(30);
        assert_eq!(content_height(&theme, &grid), 5.0 * 80.0);
        assert_eq!(item_span(&theme, &grid, 0), (0.0, 80.0));
        assert_eq!(item_span(&theme, &grid, 7), (80.0, 80.0));
        assert_eq!(
            ListScroll::new(&theme, &grid, 0.0, true).thumb,
            scrolling::thumb(0.0, 400.0, 320.0)
        );
    }

    #[test]
    fn the_grid_builds_whole_lines_with_two_lines_of_margin() {
        let theme = Theme::dark();
        let state = gridded(120);
        // 20 lines. At the top: 4 in view (320 / 80) and 2 of margin.
        assert_eq!(built_items(&theme, &state, 0.0), 0..36);
        // Scrolled 10 lines down: lines 8..16.
        assert_eq!(built_items(&theme, &state, 800.0), 48..96);
        let v = card(
            &mut whole,
            &theme,
            &state,
            None,
            &scrolled(&theme, &state, 800.0, true),
        );
        let keys: Vec<_> = rows_of(&v).iter().map(|c| c.key).collect();
        assert_eq!(keys, (48..96).map(row_key).collect::<Vec<_>>());
        // The list keeps its own margin.
        let (_, list) = many(30);
        assert_eq!(built_items(&theme, &list, 400.0), 6..22);
    }

    #[test]
    fn the_recent_tab_with_nothing_says_so_and_a_failed_search_says_none() {
        let theme = Theme::dark();
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.update(&[entry("aa", "Aa")]);
        assert_eq!(state.tab, Tab::Recent);
        assert!(state.rows.is_empty());
        let text = |s: &LauncherState| {
            note_text(&card(&mut whole, &theme, s, None, &ListScroll::default()))
        };
        assert_eq!(text(&state), copy::LAUNCHER_RECENT_EMPTY);
        state.set_query("zzz", &[entry("aa", "Aa")]);
        assert_eq!(text(&state), copy::LAUNCHER_EMPTY);
    }
}
