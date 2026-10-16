//! The launcher's view: a Haze card with the search field on top and up to
//! eight rows under it, centred in a transparent surface that fills the
//! output (design §5.5, Launcher; `celestina-style/DESIGN.md`:
//! search fields are pills, rows of the row height). An application's row
//! shows its icon, its name and its generic name; a switch's row its
//! semantic icon, its command and what it does. The selected row has the
//! control fill the Island's pages give their current row.

use celestina_paint::{IconKey, Rect, Role, TextStyle, Theme, Weight};
use celestina_ui::{view, FieldId, Key, View, ViewKind};
use taffy::AlignItems;

use super::{
    commands::Switch,
    state::{LauncherState, Row, MAX_ROWS},
};
use crate::{copy, island::pages::activity::truncate_title};

/// The card's width, logical px.
pub const LAUNCHER_W: f32 = 560.0;
/// A row's icon.
pub const ICON_SIZE: f32 = 24.0;
/// The search glyph in the field.
const SEARCH_ICON: f32 = 18.0;
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

/// The row at `index`.
#[must_use]
pub fn row_key(index: usize) -> Key {
    Key::indexed("launcher-row", index as u64)
}

/// The row `key` names, if it is one of `count` rows.
#[must_use]
pub fn row_of(key: Key, count: usize) -> Option<usize> {
    (0..count).find(|&i| row_key(i) == key)
}

/// The card's logical size with `theme`: the card's width; the field, the
/// small gap and eight rows, padded by the medium spacing.
#[must_use]
pub fn card_size(theme: &Theme) -> (f32, f32) {
    let h = 2.0 * theme.spacing.md
        + theme.top_bar
        + theme.spacing.sm
        + MAX_ROWS as f32 * theme.row_height;
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
pub fn launcher(theme: &Theme, state: &LauncherState, surface: (f32, f32)) -> View {
    let at = card_rect(surface, card_size(theme));
    view::col(surface_key())
        .size(surface.0, surface.1)
        .child(card(theme, state).absolute(at.x, at.y))
}

/// The card for `state` in `theme`.
#[must_use]
pub fn card(theme: &Theme, state: &LauncherState) -> View {
    let (w, h) = card_size(theme);
    let pill = centred(view::row(Key::named("launcher-pill")))
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
                copy::LAUNCHER_PLACEHOLDER,
                style(theme.type_scale.row_title, Role::TextHi),
            )
            .grow(),
        );
    let rows = view::col(Key::named("launcher-rows"));
    let rows = if state.rows.is_empty() {
        rows.child(
            centred(view::row(Key::named("launcher-empty")))
                .height(theme.row_height)
                .padding_xy(theme.spacing.md, 0.0)
                .child(view::text(
                    Key::named("launcher-empty-text"),
                    copy::LAUNCHER_EMPTY,
                    style(theme.type_scale.body, Role::TextLo),
                )),
        )
    } else {
        rows.children(
            state
                .rows
                .iter()
                .enumerate()
                .map(|(i, r)| row(theme, i, r, i == state.selected)),
        )
    };
    view::col(card_key())
        .haze(theme.radius_lg)
        .size(w, h)
        .padding(theme.spacing.md)
        .gap(theme.spacing.sm)
        .child(pill)
        .child(rows)
}

/// One row: the icon, the name growing, the secondary text on the right.
fn row(theme: &Theme, index: usize, row: &Row, selected: bool) -> View {
    let i = index as u64;
    let (glyph, tint, name, detail) = match row {
        Row::App(entry) => {
            let (glyph, tint) = if entry.icon.is_empty() {
                (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo))
            } else {
                (IconKey::Theme(entry.icon.clone()), None)
            };
            (
                glyph,
                tint,
                truncate_title(&entry.name),
                truncate_title(&entry.generic_name),
            )
        }
        Row::Switch(switch) => switch_parts(*switch),
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

    #[test]
    fn the_card_holds_the_field_and_one_row_per_answer_and_keeps_the_typed_text() {
        let theme = Theme::dark();
        let index = [entry("aa", "Aa"), entry("ab", "Ab"), entry("ac", "Ac")];
        let mut state = LauncherState::new(Vec::new());
        state.set_query("a", &index);
        let mut tree = Tree::new(false, &theme);
        let t0 = Instant::now();
        let size = card_size(&theme);
        let surface = (2560.0, 1440.0);
        let at = card_rect(surface, size);
        tree.apply(launcher(&theme, &state, surface), t0);
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
        let rows: Vec<_> = (0..MAX_ROWS)
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
        tree.apply(launcher(&theme, &state, surface), t0);
        tree.layout_measured(surface, fake_measure);
        assert_eq!(
            tree.field(FIELD).map(|f| f.text().to_owned()),
            Some("ab".to_owned())
        );
        // One answer now; the rows that left fade out in the tree.
        assert_eq!(card(&theme, &state).children[1].children.len(), 1);
    }

    #[test]
    fn the_selected_row_has_the_control_fill_and_the_others_none() {
        let theme = Theme::dark();
        let index = [entry("aa", "Aa"), entry("ab", "Ab")];
        let mut state = LauncherState::new(Vec::new());
        state.set_query("a", &index);
        state.move_selection(1);
        let v = card(&theme, &state);
        let rows = &v.children[1].children;
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
        let mut state = LauncherState::new(Vec::new());
        state.set_query("/zzz", &[]);
        let v = card(&theme, &state);
        let rows = &v.children[1].children;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, Key::named("launcher-empty"));
        assert!(!rows[0].interactive);
    }
}
