//! The Island's views: what the capsule and the page show, as pure functions
//! of what the Island knows. The Island hands them to its two trees whenever
//! that changes; the capsule's growth itself is painted from
//! [`super::state::IslandState`], not laid out here (the tree slides moved
//! nodes and never animates a size).

use celestina_paint::{Role, TextStyle, Theme, Weight};
use celestina_shell::settings::{BarLayout, MAX_ZONE_IDS};
use celestina_ui::{view, Key, View};
use taffy::{AlignItems, JustifyContent};

use super::{
    pages::{page_view, tabs},
    state::{CAPSULE_H, TOP_GAP},
    workspaces::Row,
};
use crate::{
    copy,
    pieces::{workspaces::Workspaces, PageId, Piece},
};

/// Space between the capsule's edge and its content.
pub const CAPSULE_PADDING: f32 = 16.0;
/// A workspace dot.
pub const DOT: f32 = 6.0;
/// The active workspace's pill: as tall as a dot, this wide.
pub const PILL_W: f32 = 14.0;
/// Space between two dots.
pub const DOT_GAP: f32 = 4.0;
/// The weather's condition icon in the capsule.
pub const WEATHER_ICON: f32 = 16.0;
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

/// The overflow button of a page shown as a capsule sheet.
#[must_use]
pub fn sheet_more_key() -> Key {
    Key::named("sheet-more")
}

/// The page's title: its view's, from the piece table.
#[must_use]
pub fn page_title(page: PageId) -> &'static str {
    page_view(page).title()
}

/// The icon of `page`'s tab.
#[must_use]
pub fn page_icon(page: PageId) -> &'static str {
    page_view(page).icon()
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
/// none. Centred on the top edge, [`TOP_GAP`] below it. This one has no date
/// and no weather.
#[must_use]
pub fn capsule(theme: &Theme, clock: &str, rows: &[Row], surface_w: f32) -> View {
    capsule_with(theme, clock, Glance::default(), Some(rows), surface_w)
}

/// What the capsule reads besides the time: the date and the weather, each
/// absent until the Island knows it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glance<'a> {
    /// The short date ("Wed 25").
    pub date: Option<&'a str>,
    /// The weather: the condition icon when the icon set has one, and the
    /// degrees ("12°").
    pub weather: Option<(Option<&'static str>, &'a str)>,
}

/// Whether the resting capsule shows the workspaces beside its clock: not
/// while the bar (`bar` is the layout in force, `None` only before the app
/// has handed it) places the Workspaces piece in one of its zones, which then
/// shows them; otherwise as in 1.0.0 (parity spec §6.3). One rule, no
/// setting.
#[must_use]
pub fn capsule_shows_workspaces(bar: Option<&BarLayout>) -> bool {
    let id = Workspaces.id().as_str();
    !bar.is_some_and(|layout| {
        [&layout.start, &layout.centre, &layout.end]
            .into_iter()
            .any(|zone| zone.iter().take(MAX_ZONE_IDS).any(|placed| placed == id))
    })
}

/// [`capsule`] with the date and the weather of `glance` after the clock, and
/// `rows` `None` for the capsule that shows no workspaces
/// ([`capsule_shows_workspaces`]).
#[must_use]
pub fn capsule_with(
    theme: &Theme,
    clock: &str,
    glance: Glance<'_>,
    rows: Option<&[Row]>,
    surface_w: f32,
) -> View {
    let clock_style = text_style(
        theme.type_scale.row_title,
        Weight::Semibold,
        true,
        Role::TextHi,
    );
    let date = glance.date.map(|date| {
        view::text(
            Key::named("capsule-date"),
            date,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
        )
    });
    let weather = glance.weather.map(|(icon, degrees)| {
        centred(view::row(Key::named("capsule-weather")))
            .gap(theme.spacing.xs)
            .children(icon.map(|name| {
                view::icon(Key::named("capsule-weather-icon"), name, WEATHER_ICON)
                    .tint(Role::TextLo)
            }))
            .child(view::text(
                Key::named("capsule-weather-text"),
                degrees,
                text_style(theme.type_scale.body, Weight::Regular, true, Role::TextLo),
            ))
    });
    let workspaces = rows.map(|rows| {
        if rows.is_empty() {
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
        }
    });
    let body = centred(view::row(capsule_key()))
        .height(CAPSULE_H)
        .padding_xy(CAPSULE_PADDING, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_lg)
        .interactive()
        .child(view::text(Key::named("clock"), clock, clock_style))
        .children(date)
        .children(weather)
        .children(workspaces);
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
pub(crate) fn switch(key: Key, on: bool, enabled: bool) -> View {
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

pub(in crate::island) fn button(theme: &Theme, key: Key, label_key: &str, label: &str) -> View {
    button_filled(
        theme,
        key,
        label_key,
        label,
        Role::ControlFill,
        Role::TextHi,
    )
}

/// A text button of the page's height, `fill` behind `ink`: the control fill
/// for an ordinary button, the accent for the one that acts (the polkit
/// prompt's authenticate control).
pub(crate) fn button_filled(
    theme: &Theme,
    key: Key,
    label_key: &str,
    label: &str,
    fill: Role,
    ink: Role,
) -> View {
    centred(view::row(key))
        .height(BUTTON_H)
        .padding_xy(theme.spacing.md, 0.0)
        .radius(theme.radius_button)
        .fill(fill)
        .interactive()
        .focusable()
        .child(view::text(
            Key::named(label_key),
            label,
            text_style(theme.type_scale.body, Weight::Regular, false, ink),
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
/// body, and the row of page tabs under it with `open` selected. The title
/// row also shows `clock` (`HH:MM:SS`) in tabular figures, between the title
/// and the buttons. `slide`
/// (−1..1, in body widths) offsets the content horizontally; the title row
/// and the tabs stay where they are.
#[must_use]
pub fn page(
    theme: &Theme,
    open: PageId,
    title: &str,
    clock: &str,
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
    let seconds = view::text(
        Key::named("page-clock"),
        clock,
        text_style(
            theme.type_scale.row_title,
            Weight::Regular,
            true,
            Role::TextLo,
        ),
    );
    let title_row = centred(view::row(Key::named("title-row")))
        .height(TITLE_ROW_H)
        .gap(theme.spacing.sm)
        .child(title)
        .child(seconds)
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

/// A page sheet's width, logical px: the Island page's.
pub const PAGE_SHEET_W: f32 = 420.0;

/// The size of a page sheet's body whose page content is `content_h` tall:
/// the top padding, the header, the gap, the content and the bottom padding.
#[must_use]
pub fn page_sheet_size(theme: &Theme, content_h: f32) -> (f32, f32) {
    (
        PAGE_SHEET_W,
        theme.spacing.xs + TITLE_ROW_H + theme.spacing.sm + content_h + theme.spacing.lg,
    )
}

/// The body of a page sheet (CEL-D-003): a header row with the page's
/// `title` and the overflow button, then the page's `content` in a clipped
/// box `content_h` tall. No clock, no close button and no tabs: the sheet is
/// closed by the way it was opened, and the Island's tabs stay on the Island.
#[must_use]
pub fn page_sheet(theme: &Theme, title: &str, content: View, content_h: f32) -> View {
    let size = page_sheet_size(theme, content_h);
    let title = view::text(
        Key::named("sheet-title"),
        title,
        text_style(
            theme.type_scale.title,
            Weight::Semibold,
            false,
            Role::TextHi,
        ),
    )
    .grow();
    let header = centred(view::row(Key::named("sheet-header")))
        .height(TITLE_ROW_H)
        .gap(theme.spacing.sm)
        .child(title)
        .child(button(
            theme,
            sheet_more_key(),
            "sheet-more-label",
            copy::MORE,
        ));
    let body_w = (size.0 - 2.0 * theme.spacing.lg).max(0.0);
    let body = view::col(Key::named("sheet-content"))
        .size(body_w, content_h)
        .clip()
        .child(content);
    view::col(Key::named("page-sheet"))
        .size(size.0, size.1)
        .padding_sides(theme.spacing.xs, theme.spacing.lg, theme.spacing.lg)
        .gap(theme.spacing.sm)
        .child(header)
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
            idx: u8::try_from(id).unwrap_or(0),
            named: false,
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

    #[test]
    fn the_capsule_drops_its_dots_only_while_the_bars_workspaces_show_them() {
        let theme = Theme::dark();
        let rows = [row(1, true), row(2, false)];
        // No layout handed yet: the dots, as in 1.0.0.
        assert!(capsule_shows_workspaces(None));
        // The default layout: Workspaces is placed, so the
        // capsule is the clock alone.
        let on = BarLayout::default();
        assert!(!capsule_shows_workspaces(Some(&on)));
        // A layout without the piece (in any zone): the dots come back.
        let mut without = on.clone();
        without.start.retain(|id| id != "workspaces");
        assert!(capsule_shows_workspaces(Some(&without)));
        for zone in 0..3 {
            let mut l = without.clone();
            [&mut l.start, &mut l.centre, &mut l.end][zone].push("workspaces".to_owned());
            assert!(!capsule_shows_workspaces(Some(&l)), "zone {zone}");
        }

        let clock_only = laid_out(capsule_with(
            &theme,
            "12:34",
            Glance::default(),
            None,
            SURFACE_W,
        ));
        assert!(clock_only.node_by_key(Key::named("clock")).is_some());
        for gone in ["dots", "fallback"] {
            assert!(clock_only.node_by_key(Key::named(gone)).is_none(), "{gone}");
        }
        assert!(clock_only.node_by_key(Key::indexed("dot", 1)).is_none());
        let body = rect_of(&clock_only, capsule_key());
        assert_eq!(body.w, 5.0 * 7.0 + 2.0 * CAPSULE_PADDING);
        // `capsule` is the capsule with its workspaces.
        assert_eq!(
            capsule(&theme, "12:34", &rows, SURFACE_W),
            capsule_with(&theme, "12:34", Glance::default(), Some(&rows), SURFACE_W)
        );
    }

    fn glanced(date: Option<&str>, weather: Option<(Option<&'static str>, &str)>) -> View {
        capsule_with(
            &Theme::dark(),
            "17:03",
            Glance { date, weather },
            None,
            SURFACE_W,
        )
    }

    fn text_size(v: &View, key: Key) -> f32 {
        fn find(v: &View, key: Key) -> Option<&View> {
            if v.key == key {
                return Some(v);
            }
            v.children.iter().find_map(|c| find(c, key))
        }
        match find(v, key).map(|n| &n.kind) {
            Some(celestina_ui::ViewKind::Text { style, .. }) => style.size,
            other => panic!("no text {key:?}: {other:?}"),
        }
    }

    #[test]
    fn the_capsule_reads_time_date_and_weather_in_that_order() {
        let theme = Theme::dark();
        let v = glanced(Some("Wed 25"), Some((Some("sun"), "12°")));
        assert_eq!(
            text_size(&v, Key::named("clock")),
            theme.type_scale.row_title
        );
        assert_eq!(
            text_size(&v, Key::named("capsule-date")),
            theme.type_scale.body
        );
        assert_eq!(
            text_size(&v, Key::named("capsule-weather-text")),
            theme.type_scale.body
        );
        assert_eq!(theme.type_scale.row_title, 15.0);
        assert_eq!(theme.type_scale.body, 13.0);
        let tree = laid_out(v);
        let clock = rect_of(&tree, Key::named("clock"));
        let date = rect_of(&tree, Key::named("capsule-date"));
        let weather = rect_of(&tree, Key::named("capsule-weather"));
        let icon = rect_of(&tree, Key::named("capsule-weather-icon"));
        let degrees = rect_of(&tree, Key::named("capsule-weather-text"));
        assert!(clock.x < date.x && date.x < weather.x);
        assert_eq!(date.x - (clock.x + clock.w), 12.0, "gap md");
        assert_eq!(weather.x - (date.x + date.w), 12.0, "gap md");
        assert_eq!((icon.w, icon.h), (16.0, 16.0));
        assert_eq!(degrees.x - (icon.x + icon.w), theme.spacing.xs);
        // Padding, "17:03", "Wed 25", the icon and "12°", two gaps and the
        // icon's own.
        let content = 5.0 * 7.0 + 6.0 * 7.0 + 16.0 + theme.spacing.xs + 3.0 * 7.0 + 24.0;
        assert_eq!(
            rect_of(&tree, capsule_key()).w,
            content + 2.0 * CAPSULE_PADDING
        );
    }

    #[test]
    fn without_weather_the_capsule_shows_time_and_date() {
        let tree = laid_out(glanced(Some("Wed 25"), None));
        assert!(tree.node_by_key(Key::named("capsule-date")).is_some());
        for gone in [
            "capsule-weather",
            "capsule-weather-icon",
            "capsule-weather-text",
        ] {
            assert!(tree.node_by_key(Key::named(gone)).is_none(), "{gone}");
        }
        let tree = laid_out(glanced(None, Some((None, "12°"))));
        assert!(tree.node_by_key(Key::named("capsule-date")).is_none());
        assert!(tree.node_by_key(Key::named("capsule-weather")).is_some());
        let tree = laid_out(glanced(None, None));
        assert_eq!(
            rect_of(&tree, capsule_key()).w,
            5.0 * 7.0 + 2.0 * CAPSULE_PADDING
        );
    }

    #[test]
    fn a_clear_night_shows_the_degrees_without_an_icon() {
        let tree = laid_out(glanced(Some("Wed 25"), Some((None, "-3°"))));
        assert!(tree
            .node_by_key(Key::named("capsule-weather-icon"))
            .is_none());
        let weather = rect_of(&tree, Key::named("capsule-weather"));
        let degrees = rect_of(&tree, Key::named("capsule-weather-text"));
        assert_eq!((weather.x, weather.w), (degrees.x, degrees.w), "text only");
    }

    #[test]
    fn the_capsule_keeps_its_dots_only_without_a_workspaces_zone() {
        let rows = [row(1, true), row(2, false)];
        let glance = Glance {
            date: Some("Wed 25"),
            weather: Some((Some("cloud"), "9°")),
        };
        let theme = Theme::dark();
        let with = laid_out(capsule_with(
            &theme,
            "17:03",
            glance,
            Some(&rows),
            SURFACE_W,
        ));
        let without = laid_out(capsule_with(&theme, "17:03", glance, None, SURFACE_W));
        assert!(with.node_by_key(Key::named("dots")).is_some());
        assert!(without.node_by_key(Key::named("dots")).is_none());
        // The dots come last, after the weather.
        let weather = rect_of(&with, Key::named("capsule-weather"));
        let dots = rect_of(&with, Key::named("dots"));
        assert!(weather.x + weather.w < dots.x);
        // The default layout places Workspaces, so no dots; one without it
        // brings them back.
        assert!(!capsule_shows_workspaces(Some(&BarLayout::default())));
        let mut bare = BarLayout::default();
        bare.start.retain(|id| id != "workspaces");
        assert!(capsule_shows_workspaces(Some(&bare)));
    }

    fn content() -> View {
        view::col(Key::named("content")).child(view::row(Key::named("probe")).height(40.0))
    }

    fn page_at(slide: f32) -> Tree {
        laid_out(page(
            &Theme::dark(),
            PageId::ACTIVITY,
            copy::PAGE_ACTIVITY,
            "12:34:56",
            content(),
            SURFACE_W,
            (420.0, 300.0),
            slide,
        ))
    }

    #[test]
    fn the_phone_page_draws_the_tab_row_with_no_tab_of_its_own() {
        let tree = laid_out(page(
            &Theme::dark(),
            PageId::PHONE,
            copy::PAGE_PHONE,
            "12:34:56",
            content(),
            SURFACE_W,
            (420.0, 300.0),
            0.0,
        ));
        assert!(tree.node_by_key(tabs::tab_key(PageId::PHONE)).is_none());
        let root = rect_of(&tree, Key::named("page-root"));
        let row = rect_of(&tree, Key::named("tabs"));
        assert!(row.x >= root.x && row.x + row.w <= root.x + root.w);
        assert!(tree.node_by_key(tabs::tab_key(PageId::ACTIVITY)).is_some());
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
        let tab = rect_of(&tree, tabs::tab_key(PageId::ACTIVITY));
        assert_eq!(tab.y + tab.h, root.y + root.h - theme.spacing.lg);
        assert!(body.y + body.h <= tab.y, "the body ends above the tabs");
    }

    #[test]
    fn the_title_row_shows_the_seconds_clock_between_the_title_and_the_buttons() {
        let tree = page_at(0.0);
        let clock = rect_of(&tree, Key::named("page-clock"));
        let more = rect_of(&tree, more_key());
        let title = rect_of(&tree, Key::named("title"));
        assert!(clock.w >= 7.0 * 8.0, "HH:MM:SS fits");
        assert!(title.x + title.w <= clock.x && clock.x + clock.w <= more.x);
        let row = rect_of(&tree, Key::named("title-row"));
        assert!(clock.y >= row.y && clock.y + clock.h <= row.y + row.h);
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
        for key in [close_key(), tabs::tab_key(PageId::TRAY), Key::named("body")] {
            assert_eq!(x(&rest, key), x(&left, key));
        }
    }

    #[test]
    fn a_page_sheet_is_a_header_and_the_content_in_a_clipped_box() {
        let theme = Theme::dark();
        let content_h = 120.0;
        let tree = laid_out(page_sheet(&theme, "Red", content(), content_h));
        let (w, h) = page_sheet_size(&theme, content_h);
        assert_eq!(w, PAGE_SHEET_W);
        let root = rect_of(&tree, Key::named("page-sheet"));
        assert_eq!((root.x, root.y, root.w, root.h), (0.0, 0.0, w, h));
        let header = rect_of(&tree, Key::named("sheet-header"));
        assert_eq!(header.h, TITLE_ROW_H);
        assert_eq!(header.y, theme.spacing.xs);
        assert_eq!(header.x, theme.spacing.lg);
        let more = rect_of(&tree, sheet_more_key());
        assert_eq!(more.x + more.w, w - theme.spacing.lg, "at the right");
        let title = rect_of(&tree, Key::named("sheet-title"));
        assert!(title.x + title.w <= more.x);
        let body = rect_of(&tree, Key::named("sheet-content"));
        assert_eq!(body.y, header.y + header.h + theme.spacing.sm);
        assert_eq!(
            (body.x, body.w, body.h),
            (theme.spacing.lg, w - 2.0 * theme.spacing.lg, content_h)
        );
        assert_eq!(body.y + body.h + theme.spacing.lg, h);
        let probe = rect_of(&tree, Key::named("probe"));
        assert_eq!((probe.x, probe.y), (body.x, body.y));
        // No clock, no close button, no tabs, no Island overflow button.
        for key in [
            Key::named("page-clock"),
            close_key(),
            more_key(),
            Key::named("tabs"),
            tabs::tab_key(PageId::ACTIVITY),
        ] {
            assert!(tree.node_by_key(key).is_none(), "{key:?}");
        }
        assert_ne!(sheet_more_key(), more_key());
    }

    #[test]
    fn every_page_icon_has_a_file() {
        let want = [
            (PageId::ACTIVITY, "view-grid"),
            (PageId::NOTIFICATIONS, "bell"),
            (PageId::TRAY, "system-tray"),
            (PageId::SYSMON, "gauge"),
            (PageId::CLOCK, "calendar"),
            (PageId::CLIPBOARD, "clipboard-paste"),
            (PageId::NETWORK, "wifi"),
            (PageId::BLUETOOTH, "bluetooth"),
            (PageId::POWER, "zap"),
            (PageId::PHONE, "phone"),
            (PageId::MEDIA, "music"),
        ];
        assert_eq!(want.len(), PageId::all().len(), "every page is named");
        let icons =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../celestina-style/icons");
        for (page, name) in want {
            assert_eq!(page_icon(page), name, "{page:?}");
            assert!(icons.join(format!("{name}.svg")).is_file(), "{name}.svg");
        }
    }

    #[test]
    fn page_titles_are_the_copy_of_each_page() {
        // Characterisation (CEL-2-A, spec 6.7): pinned before `Page` is
        // replaced by the piece table.
        assert_eq!(page_title(PageId::ACTIVITY), copy::PAGE_ACTIVITY);
        assert_eq!(page_title(PageId::NOTIFICATIONS), copy::PAGE_NOTIFICATIONS);
        assert_eq!(page_title(PageId::TRAY), copy::PAGE_TRAY);
        assert_eq!(page_title(PageId::CLIPBOARD), copy::PAGE_CLIPBOARD);
        assert_eq!(page_title(PageId::PHONE), copy::PAGE_PHONE);
        assert_eq!(page_title(PageId::MEDIA), copy::PAGE_MEDIA);
        let titles: Vec<&str> = PageId::all().into_iter().map(page_title).collect();
        assert_eq!(
            titles,
            [
                copy::PAGE_ACTIVITY,
                copy::PAGE_NOTIFICATIONS,
                copy::PAGE_TRAY,
                copy::PAGE_SYSMON,
                copy::PAGE_CLOCK,
                copy::PAGE_CLIPBOARD,
                copy::PAGE_NETWORK,
                copy::PAGE_BLUETOOTH,
                copy::PAGE_POWER,
                copy::PAGE_PHONE,
                copy::PAGE_MEDIA,
            ]
        );
        assert_eq!(page_title(PageId::SYSMON), copy::PAGE_SYSMON);
        assert_eq!(page_title(PageId::CLOCK), copy::PAGE_CLOCK);
        assert_eq!(page_title(PageId::NETWORK), copy::PAGE_NETWORK);
        assert_eq!(page_title(PageId::BLUETOOTH), copy::PAGE_BLUETOOTH);
        assert_eq!(page_title(PageId::POWER), copy::PAGE_POWER);
    }
}
