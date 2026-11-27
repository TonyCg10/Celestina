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
use taffy::{AlignItems, JustifyContent, LengthPercentageAuto};

use super::{
    commands::Switch,
    scrolling, selector,
    state::{LauncherState, Row, Source},
};
use crate::{copy, island::pages::activity::truncate_title};

/// The card's width, logical px.
pub const LAUNCHER_W: f32 = 560.0;
/// How many rows the area shows at once.
pub const AREA_ROWS: usize = 8;
/// Rows built beyond each edge of the area, so a row that enters with the
/// tree's entrance animation enters out of view.
pub const BUILD_MARGIN: usize = 4;
/// The scroll bar's width, its distance from the area's right edge and its
/// corner radius.
const THUMB_W: f32 = 4.0;
const THUMB_INSET: f32 = 2.0;
const THUMB_RADIUS: f32 = 2.0;
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

/// The row at `index`.
#[must_use]
pub fn row_key(index: usize) -> Key {
    Key::indexed("launcher-row", index as u64)
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
    /// The scroll of `rows` rows in `theme`, `offset` px down, with the bar
    /// shown or not.
    #[must_use]
    pub fn new(theme: &Theme, rows: usize, offset: f32, shown: bool) -> ListScroll {
        let content = rows as f32 * theme.row_height;
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

/// The card's logical size with `theme`: the card's width; the field, the
/// small gap and the area of eight rows, padded by the medium spacing.
#[must_use]
pub fn card_size(theme: &Theme) -> (f32, f32) {
    let h = 2.0 * theme.spacing.md
        + theme.top_bar
        + theme.spacing.sm
        + AREA_ROWS as f32 * theme.row_height;
    (LAUNCHER_W, h)
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
    theme: &Theme,
    state: &LauncherState,
    surface: (f32, f32),
    sweep: Option<Sweep>,
    scroll: &ListScroll,
) -> View {
    let at = card_rect(surface, card_size(theme));
    view::col(surface_key())
        .size(surface.0, surface.1)
        .child(card(theme, state, sweep, scroll).absolute(at.x, at.y))
}

/// The card for `state` in `theme`, with the `sweep` over the query while
/// there is one (the launcher of applications only) and the list scrolled
/// as `scroll` says.
#[must_use]
pub fn card(
    theme: &Theme,
    state: &LauncherState,
    sweep: Option<Sweep>,
    scroll: &ListScroll,
) -> View {
    let (w, h) = card_size(theme);
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
    let area = rows_area(theme, state, scroll, area_w, note);
    view::col(card_key())
        .haze(theme.radius_lg)
        .size(w, h)
        .padding(theme.spacing.md)
        .gap(theme.spacing.sm)
        .child(pill)
        .child(area)
}

/// The area: a clipped box of eight rows' height holding the inner column,
/// which is offset up by the scroll and holds the rows near the view, each at
/// its full place; and the scroll bar over it. Only rows in view and
/// [`BUILD_MARGIN`] beyond are built.
fn rows_area(
    theme: &Theme,
    state: &LauncherState,
    scroll: &ListScroll,
    width: f32,
    note: Option<&str>,
) -> View {
    let row_h = theme.row_height;
    let viewport = AREA_ROWS as f32 * row_h;
    let count = state.rows.len();
    let mut area = view::col(area_key()).size(width, viewport).clip();
    if scroll.thumb.is_some() {
        // Rows cover the whole area, so the box itself is never what the
        // pointer lifts: it only needs to be a target when it scrolls.
        area = area.interactive();
    }
    let inner = if count == 0 {
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
                        note.unwrap_or(copy::LAUNCHER_EMPTY),
                        style(theme.type_scale.body, Role::TextLo),
                    )),
            )
    } else {
        let built = scrolling::built(scroll.offset, row_h, viewport, count, BUILD_MARGIN);
        view::col(inner_key())
            .size(width, count as f32 * row_h)
            .absolute(0.0, 0.0 - scroll.offset)
            .children(built.map(|i| {
                row(theme, i, &state.rows[i], i == state.selected, state.copied)
                    .width(width)
                    .absolute(0.0, i as f32 * row_h)
            }))
    };
    area = area.child(inner);
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

/// One row: the icon, the name growing, the secondary text on the right.
fn row(theme: &Theme, index: usize, row: &Row, selected: bool, copied: bool) -> View {
    let i = index as u64;
    if let Row::Calc { result } = row {
        return calc_row(theme, result, selected, copied);
    }
    let (glyph, tint, name, detail) = match row {
        Row::App(app) => {
            let (glyph, tint) = match IconKey::desktop_icon(&app.entry.icon) {
                Some(key) => (key, None),
                None => (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo)),
            };
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

    /// The column inside the area that holds the built rows.
    fn inner(v: &View) -> &View {
        area(v)
            .children
            .iter()
            .find(|c| c.key == inner_key())
            .expect("the area holds the inner column")
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
        (index, state)
    }

    fn scrolled(theme: &Theme, rows: usize, offset: f32, shown: bool) -> ListScroll {
        ListScroll::new(theme, rows, offset, shown)
    }

    #[test]
    fn thirty_answers_build_the_rows_near_the_top_in_a_clipped_area() {
        let theme = Theme::dark();
        let (_, state) = many(30);
        assert_eq!(state.rows.len(), 30);
        let scroll = scrolled(&theme, 30, 0.0, false);
        let v = card(&theme, &state, None, &scroll);
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
        let v = card(&theme, &state, None, &scrolled(&theme, 30, offset, true));
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
            &theme,
            &state,
            None,
            &scrolled(&theme, state.rows.len(), 0.0, false),
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
        let v = card(&theme, &state, None, &scrolled(&theme, 30, 0.0, true));
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

        let hidden = card(&theme, &state, None, &scrolled(&theme, 30, 0.0, false));
        let thumb = area(&hidden)
            .children
            .iter()
            .find(|c| c.key == thumb_key())
            .expect("kept built so that it can fade");
        assert_eq!(thumb.opacity, 0.0);

        let (_, few) = many(5);
        let v = card(&theme, &few, None, &scrolled(&theme, 5, 0.0, true));
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
            &theme,
            &state,
            None,
            &scrolled(&theme, state.rows.len(), 0.0, true),
        );
        assert_eq!(rows_of(&v).len(), 8);
        assert!(area(&v).children.iter().all(|c| c.key != thumb_key()));
    }

    #[test]
    fn the_area_is_laid_out_inside_the_card_and_clips_its_rows() {
        let theme = Theme::dark();
        let (_, state) = many(30);
        let scroll = scrolled(&theme, 30, 3.0 * theme.row_height, false);
        let surface = (2560.0, 1440.0);
        let mut tree = Tree::new(false, &theme);
        tree.apply(
            launcher(&theme, &state, surface, None, &scroll),
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
        let size = card_size(&theme);
        let surface = (2560.0, 1440.0);
        let at = card_rect(surface, size);
        tree.apply(
            launcher(&theme, &state, surface, None, &ListScroll::default()),
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
            launcher(&theme, &state, surface, None, &ListScroll::default()),
            t0,
        );
        tree.layout_measured(surface, fake_measure);
        assert_eq!(
            tree.field(FIELD).map(|f| f.text().to_owned()),
            Some("ab".to_owned())
        );
        // One answer now; the rows that left fade out in the tree.
        let v = card(&theme, &state, None, &ListScroll::default());
        assert_eq!(rows_of(&v).len(), 1);
    }

    #[test]
    fn the_selected_row_has_the_control_fill_and_the_others_none() {
        let theme = Theme::dark();
        let index = [entry("aa", "Aa"), entry("ab", "Ab")];
        let mut state = LauncherState::new(std::collections::HashMap::new(), None);
        state.set_query("a", &index);
        state.move_selection(1);
        let v = card(&theme, &state, None, &ListScroll::default());
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
        let v = card(&theme, &state, None, &ListScroll::default());
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
            note_text(&card(&theme, &empty, None, &ListScroll::default())),
            copy::SELECTOR_NO_WINDOWS
        );
        for link in [Link::Absent, Link::Refused, Link::Unavailable] {
            let gone = LauncherState::selector(Bubbles {
                windows: Vec::new(),
                link,
            });
            assert_eq!(
                note_text(&card(&theme, &gone, None, &ListScroll::default())),
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
            note_text(&card(&theme, &some, None, &ListScroll::default())),
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
        let v = card(&theme, &state, None, &ListScroll::default());
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
            card(&theme, state, None, &ListScroll::default())
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
        let v = card(&theme, &state, Some(sweep), &ListScroll::default());
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

        let none = card(&theme, &state, None, &ListScroll::default());
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
        let v = card(&theme, &calc_state(false), None, &ListScroll::default());
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
        let v = card(&theme, &calc_state(true), None, &ListScroll::default());
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
}
