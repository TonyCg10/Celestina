//! The Island's views: what the capsule and the page show, as pure functions
//! of what the Island knows. The Island hands them to its two trees whenever
//! that changes; the capsule's growth itself is painted from
//! [`super::state::IslandState`], not laid out here (the tree slides moved
//! nodes and never animates a size).

use celestina_paint::{Role, TextStyle, Theme, Weight};
use celestina_ui::{view, Key, View};
use taffy::{AlignItems, JustifyContent};

use super::{
    state::{CAPSULE_H, PAGE_SIZE, TOP_GAP},
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

/// The Activity page's row of workspace `id`.
#[must_use]
pub fn workspace_key(id: u64) -> Key {
    Key::indexed("workspace", id)
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

fn centred(mut v: View) -> View {
    v.style.align_items = AlignItems::CENTER;
    v
}

fn style(size: f32, weight: Weight, tabular: bool, role: Role) -> TextStyle {
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
    let clock_style = style(
        theme.type_scale.row_title,
        Weight::Semibold,
        true,
        Role::TextHi,
    );
    let workspaces = if rows.is_empty() {
        view::text(
            Key::named("fallback"),
            copy::WORKSPACES_FALLBACK,
            style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
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
            style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
        ))
}

fn workspace_row(theme: &Theme, row: &Row) -> View {
    let mut v = centred(view::row(workspace_key(row.id)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive()
        .focusable();
    if row.active {
        v = v.fill(Role::ControlFill);
    }
    let dot = if row.active {
        Role::Accent
    } else {
        Role::TextLo
    };
    v.child(
        view::row(Key::indexed("workspace-dot", row.id))
            .fill(dot)
            .radius(DOT / 2.0)
            .size(DOT, DOT),
    )
    .child(view::text(
        Key::indexed("workspace-name", row.id),
        &row.text,
        style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
    ))
}

/// The page at its final size, centred where the grown Island ends: the
/// title row (title, the overflow button, close) over the page's content.
/// The Activity page lists the output's workspaces; the other pages are
/// filled by CEL-1-D, F and G.
#[must_use]
pub fn page(theme: &Theme, page: Page, rows: &[Row], surface_w: f32) -> View {
    let title = view::text(
        Key::named("title"),
        page_title(page),
        style(
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
    let body = view::col(Key::named("body")).children(match page {
        Page::Activity => rows.iter().map(|r| workspace_row(theme, r)).collect(),
        Page::Notifications | Page::Tray => Vec::new(),
    });
    view::col(Key::named("page-root"))
        .absolute((surface_w - PAGE_SIZE.0) / 2.0, TOP_GAP)
        .size(PAGE_SIZE.0, PAGE_SIZE.1)
        .padding(theme.spacing.lg)
        .gap(theme.spacing.sm)
        .child(title_row)
        .child(body)
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

    #[test]
    fn the_page_fills_the_grown_island_with_its_buttons_at_the_right() {
        let theme = Theme::dark();
        let rows = [row(1, true), row(2, false)];
        let tree = laid_out(page(&theme, Page::Activity, &rows, SURFACE_W));
        let root = rect_of(&tree, Key::named("page-root"));
        assert_eq!((root.x, root.y, root.w, root.h), (170.0, 8.0, 420.0, 300.0));
        let close = rect_of(&tree, close_key());
        assert_eq!(close.x + close.w, root.x + root.w - theme.spacing.lg);
        let more = rect_of(&tree, more_key());
        assert!(more.x + more.w <= close.x);
        for id in [1, 2] {
            let r = rect_of(&tree, workspace_key(id));
            assert_eq!(r.h, theme.row_height);
            assert_eq!(r.x, root.x + theme.spacing.lg);
            assert!(r.y + r.h <= root.y + root.h - theme.spacing.lg);
        }
        // The other pages have their own title and no rows yet.
        let tray = laid_out(page(&theme, Page::Tray, &rows, SURFACE_W));
        assert!(tray.node_by_key(workspace_key(1)).is_none());
    }
}
