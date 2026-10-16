//! The Island's views: what the capsule and the page show, as pure functions
//! of what the Island knows. The Island hands them to its two trees whenever
//! that changes; the capsule's growth itself is painted from
//! [`super::state::IslandState`], not laid out here (the tree slides moved
//! nodes and never animates a size).

use celestina_paint::{Role, TextStyle, Theme, Weight};
use celestina_ui::{view, Key, View};
use taffy::{AlignItems, JustifyContent};

use super::{
    pages::tabs,
    state::{CAPSULE_H, TOP_GAP},
    workspaces::Row,
};
use crate::{command::Page, copy};

/// Space between the capsule's edge and its content.
pub const CAPSULE_PADDING: f32 = 16.0;
/// A workspace dot.
pub const DOT: f32 = 6.0;
/// The active workspace's pill: as tall as a dot, this wide.
pub const PILL_W: f32 = 14.0;
/// Space between two dots.
pub const DOT_GAP: f32 = 4.0;
/// The page's title row.
pub const TITLE_ROW_H: f32 = 32.0;
/// The title row's buttons.
pub const BUTTON_H: f32 = 28.0;

/// The capsule: its click opens the page.
#[must_use]
pub fn capsule_key() -> Key {
    Key::named("capsule")
}

/// The page's close button.
#[must_use]
pub fn close_key() -> Key {
    Key::named("close")
}

/// The page's overflow menu button.
#[must_use]
pub fn more_key() -> Key {
    Key::named("more")
}

/// The page's title.
#[must_use]
pub fn page_title(page: Page) -> &'static str {
    match page {
        Page::Activity => copy::PAGE_ACTIVITY,
        Page::Notifications => copy::PAGE_NOTIFICATIONS,
        Page::Tray => copy::PAGE_TRAY,
    }
}

pub(crate) fn centred(mut v: View) -> View {
    v.style.align_items = AlignItems::CENTER;
    v
}

pub(crate) fn text_style(size: f32, weight: Weight, tabular: bool, role: Role) -> TextStyle {
    TextStyle {
        size,
        weight,
        tabular,
        role,
    }
}

/// The capsule on a surface `surface_w` wide: the clock in tabular figures
/// at the row-title size, then the output's workspaces as dots with the
/// active one a wider accent pill, or the fallback label while there are
/// none. Centred on the top edge, [`TOP_GAP`] below it.
#[must_use]
pub fn capsule(theme: &Theme, clock: &str, rows: &[Row], surface_w: f32) -> View {
    let clock_style = text_style(
        theme.type_scale.row_title,
        Weight::Semibold,
        true,
        Role::TextHi,
    );
    let workspaces = if rows.is_empty() {
        view::text(
            Key::named("fallback"),
            copy::WORKSPACES_FALLBACK,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
        )
    } else {
        centred(view::row(Key::named("dots")).gap(DOT_GAP)).children(rows.iter().map(|r| {
            let (w, role) = if r.active {
                (PILL_W, Role::Accent)
            } else {
                (DOT, Role::TextLo)
            };
            view::row(Key::indexed("dot", r.id))
                .fill(role)
                .radius(DOT / 2.0)
                .size(w, DOT)
        }))
    };
    let body = centred(view::row(capsule_key()))
        .height(CAPSULE_H)
        .padding_xy(CAPSULE_PADDING, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_lg)
        .interactive()
        .child(view::text(Key::named("clock"), clock, clock_style))
        .child(workspaces);
    let mut root = centred(view::row(Key::named("capsule-root")))
        .absolute(0.0, TOP_GAP)
        .size(surface_w, CAPSULE_H);
    root.style.justify_content = JustifyContent::CENTER;
    root.child(body)
}

/// A switch's track, logical px.
pub const SWITCH_TRACK: (f32, f32) = (36.0, 20.0);
/// A switch's knob diameter, logical px.
pub const SWITCH_KNOB: f32 = 16.0;

/// A switch keyed `key`: the knob at the right on an accent track when
/// `on`, at the left on the control fill when off; a switch that is not
/// `enabled` keeps the control fill and a faint knob. The switch only
/// shows the state: the row holding it is the control.
pub(in crate::island) fn switch(key: Key, on: bool, enabled: bool) -> View {
    let knob_role = match (enabled, on) {
        (false, _) => Role::TextFaint,
        (true, true) => Role::AccentInk,
        (true, false) => Role::TextHi,
    };
    let track_role = if enabled && on {
        Role::Accent
    } else {
        Role::ControlFill
    };
    let knob = view::row(Key::named("switch-knob"))
        .fill(knob_role)
        .radius(SWITCH_KNOB / 2.0)
        .size(SWITCH_KNOB, SWITCH_KNOB);
    let mut track = centred(view::row(key))
        .fill(track_role)
        .radius(SWITCH_TRACK.1 / 2.0)
        .size(SWITCH_TRACK.0, SWITCH_TRACK.1)
        .padding_xy((SWITCH_TRACK.1 - SWITCH_KNOB) / 2.0, 0.0);
    if on {
        track.style.justify_content = JustifyContent::FLEX_END;
    }
    track.child(knob)
}

fn button(theme: &Theme, key: Key, label_key: &str, label: &str) -> View {
    centred(view::row(key))
        .height(BUTTON_H)
        .padding_xy(theme.spacing.md, 0.0)
        .radius(theme.radius_button)
        .fill(Role::ControlFill)
        .interactive()
        .focusable()
        .child(view::text(
            Key::named(label_key),
            label,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
        ))
}

/// What the page spends on everything but its content: the padding, the
/// title row, the tabs and the gaps between them. A page of height `h` has
/// `h - chrome_height` for its content.
#[must_use]
pub fn chrome_height(theme: &Theme) -> f32 {
    2.0 * theme.spacing.lg + TITLE_ROW_H + tabs::TABS_H + 2.0 * theme.spacing.sm
}

/// The page at `size`, centred where the grown Island ends: the title row
/// (title, the overflow button, close), the page's `content` in a clipped
/// body, and the row of page tabs under it with `open` selected. `slide`
/// (−1..1, in body widths) offsets the content horizontally; the title row
/// and the tabs stay where they are.
#[must_use]
pub fn page(
    theme: &Theme,
    open: Page,
    title: &str,
    content: View,
    surface_w: f32,
    size: (f32, f32),
    slide: f32,
) -> View {
    let title = view::text(
        Key::named("title"),
        title,
        text_style(
            theme.type_scale.title,
            Weight::Semibold,
            false,
            Role::TextHi,
        ),
    )
    .grow();
    let title_row = centred(view::row(Key::named("title-row")))
        .height(TITLE_ROW_H)
        .gap(theme.spacing.sm)
        .child(title)
        .child(button(theme, more_key(), "more-label", copy::MORE))
        .child(button(theme, close_key(), "close-label", copy::CLOSE));
    let body_w = (size.0 - 2.0 * theme.spacing.lg).max(0.0);
    let body_h =
        (size.1 - 2.0 * theme.spacing.lg - TITLE_ROW_H - tabs::TABS_H - 2.0 * theme.spacing.sm)
            .max(0.0);
    let body = view::col(Key::named("body"))
        .size(body_w, body_h)
        .clip()
        .child(
            view::col(Key::named("slide"))
                .absolute(slide * body_w, 0.0)
                .size(body_w, body_h)
                .child(content),
        );
    view::col(Key::named("page-root"))
        .absolute((surface_w - size.0) / 2.0, TOP_GAP)
        .size(size.0, size.1)
        .padding(theme.spacing.lg)
        .gap(theme.spacing.sm)
        .child(title_row)
        .child(body)
        .child(tabs::tabs(theme, open, body_w))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_ui::Tree;

    use super::*;

    const SURFACE_W: f32 = 760.0;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn laid_out(v: View) -> Tree {
        let mut tree = Tree::new(false, &Theme::dark());
        tree.apply(v, Instant::now());
        tree.layout_measured((SURFACE_W, 560.0), fake_measure);
        tree
    }

    fn rect_of(tree: &Tree, key: Key) -> celestina_paint::Rect {
        match tree.node_by_key(key) {
            Some(id) => tree.rect(id),
            None => panic!("no node for {key:?}"),
        }
    }

    fn row(id: u64, active: bool) -> Row {
        Row {
            id,
            text: copy::workspace_number(u8::try_from(id).unwrap_or(0)),
            active,
            columns: 0,
        }
    }

    #[test]
    fn the_capsule_is_centred_below_the_top_edge_with_an_accent_pill() {
        let theme = Theme::dark();
        let rows = [row(1, false), row(2, true), row(3, false)];
        let tree = laid_out(capsule(&theme, "12:34", &rows, SURFACE_W));
        let body = rect_of(&tree, capsule_key());
        assert_eq!((body.y, body.h), (TOP_GAP, CAPSULE_H));
        // Centred to the pixel (taffy rounds an odd width's origin).
        assert!((body.x + body.w / 2.0 - SURFACE_W / 2.0).abs() <= 0.5);
        // Padding, the clock (5 chars), the gap, three dots and the pill.
        let content = 5.0 * 7.0 + theme.spacing.md + 2.0 * DOT + PILL_W + 2.0 * DOT_GAP;
        assert_eq!(body.w, content + 2.0 * CAPSULE_PADDING);
        let pill = rect_of(&tree, Key::indexed("dot", 2));
        assert_eq!((pill.w, pill.h), (PILL_W, DOT));
        assert_eq!(pill.y + DOT / 2.0, body.y + CAPSULE_H / 2.0, "centred");
    }

    #[test]
    fn without_workspaces_the_capsule_shows_the_fallback_label() {
        let tree = laid_out(capsule(&Theme::dark(), "12:34", &[], SURFACE_W));
        assert!(tree.node_by_key(Key::named("fallback")).is_some());
        assert!(tree.node_by_key(Key::named("dots")).is_none());
    }

    fn content() -> View {
        view::col(Key::named("content")).child(view::row(Key::named("probe")).height(40.0))
    }

    fn page_at(slide: f32) -> Tree {
        laid_out(page(
            &Theme::dark(),
            Page::Activity,
            copy::PAGE_ACTIVITY,
            content(),
            SURFACE_W,
            (420.0, 300.0),
            slide,
        ))
    }

    #[test]
    fn the_page_fills_the_grown_island_with_buttons_at_the_right_and_tabs_below() {
        let theme = Theme::dark();
        let tree = page_at(0.0);
        let root = rect_of(&tree, Key::named("page-root"));
        assert_eq!((root.x, root.y, root.w, root.h), (170.0, 8.0, 420.0, 300.0));
        let close = rect_of(&tree, close_key());
        assert_eq!(close.x + close.w, root.x + root.w - theme.spacing.lg);
        let more = rect_of(&tree, more_key());
        assert!(more.x + more.w <= close.x);
        let body = rect_of(&tree, Key::named("body"));
        let probe = rect_of(&tree, Key::named("probe"));
        assert_eq!((probe.x, probe.y), (body.x, body.y), "content at rest");
        let tab = rect_of(&tree, tabs::tab_key(Page::Activity));
        assert_eq!(tab.y + tab.h, root.y + root.h - theme.spacing.lg);
        assert!(body.y + body.h <= tab.y, "the body ends above the tabs");
    }

    #[test]
    fn the_slide_offsets_the_content_in_body_widths_and_leaves_the_chrome() {
        let rest = page_at(0.0);
        let left = page_at(-1.0);
        let right = page_at(0.5);
        let body = rect_of(&rest, Key::named("body"));
        let x = |tree: &Tree, key| rect_of(tree, key).x;
        assert_eq!(x(&left, Key::named("probe")), body.x - body.w);
        assert_eq!(x(&right, Key::named("probe")), body.x + body.w / 2.0);
        for key in [close_key(), tabs::tab_key(Page::Tray), Key::named("body")] {
            assert_eq!(x(&rest, key), x(&left, key));
        }
    }
}
