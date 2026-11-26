//! The workspaces sheet's list (CEL-D-005): one row per workspace of the
//! Island's output, each with its windows' icons. The Island builds it from
//! [`crate::island::workspaces::rows_for`] and the model's windows.

use celestina_paint::{IconKey, Role, Theme, Weight};
use celestina_ui::{view, Key, View, ViewKind};

use crate::{
    copy,
    island::{
        view::{centred, text_style},
        workspaces::Row,
    },
    providers::niri::Window,
};

/// The list's width, logical px.
pub const LIST_W: f32 = 240.0;
/// A row's height.
pub const ROW_H: f32 = 40.0;
/// The padding around the rows (the session menu's `sm`).
const PAD: f32 = 8.0;
/// The gap between rows.
const ROW_GAP: f32 = 2.0;
/// A row's horizontal padding and the gap between its parts.
const ROW_PAD_X: f32 = 10.0;
const ROW_INNER_GAP: f32 = 10.0;
/// The width of a numbered label.
const NUMBER_W: f32 = 18.0;
/// A window icon's size and the gap between two.
const ICON_SIZE: f32 = 20.0;
const ICON_GAP: f32 = 6.0;
/// The most icons a row shows before `+N`: six icons and their gaps are 150
/// px, which with `+N` fits the 176 px a row leaves after its label.
pub const MAX_ICONS: usize = 6;
/// The longest name a row shows, in grapheme clusters, the ellipsis included,
/// and the widest its label gets: a long name never pushes the icons out.
const MAX_NAME_GRAPHEMES: usize = 12;
const MAX_NAME_W: f32 = 96.0;
const FALLBACK_ICON: &str = "app-window";

/// The list's root.
#[must_use]
pub fn list_key() -> Key {
    Key::named("ws-list")
}

/// The row of workspace `id`.
#[must_use]
pub fn row_key(id: u64) -> Key {
    Key::indexed("ws-row", id)
}

/// The windows of `workspace` in the order their icons show: tiled ones by
/// column then row, floating ones last, then by id.
#[must_use]
pub fn window_order(windows: &[Window], workspace: u64) -> Vec<&Window> {
    let mut on: Vec<&Window> = windows
        .iter()
        .filter(|w| w.workspace_id == Some(workspace))
        .collect();
    on.sort_by_key(|w| {
        (
            w.column.is_none(),
            w.column.unwrap_or(0),
            w.row.unwrap_or(0),
            w.id,
        )
    });
    on
}

/// The list's size for `rows` workspaces, logical px.
#[must_use]
pub fn list_size(rows: usize) -> (f32, f32) {
    let n = rows as f32;
    let gaps = (n - 1.0).max(0.0) * ROW_GAP;
    (LIST_W, 2.0 * PAD + n * ROW_H + gaps)
}

/// The list: one row per entry of `rows`, in the order given.
#[must_use]
pub fn workspaces_view(theme: &Theme, rows: &[Row], windows: &[Window]) -> View {
    let mut list = view::col(list_key())
        .width(LIST_W)
        .padding(PAD)
        .gap(ROW_GAP);
    for row in rows {
        list = list.child(row_view(theme, row, windows));
    }
    list
}

fn row_view(theme: &Theme, row: &Row, windows: &[Window]) -> View {
    let label =
        crate::island::pages::activity::truncate_graphemes(&label_of(row), MAX_NAME_GRAPHEMES);
    let label_role = if row.active {
        Role::Accent
    } else {
        Role::TextLo
    };
    let mut label_view = view::text(
        Key::indexed("ws-row-label", row.id),
        &label,
        text_style(theme.type_scale.body, Weight::Semibold, false, label_role),
    );
    // A short label (a number, named or not) takes the fixed column, so the
    // rows' icons line up; a longer name keeps its own width.
    if label.chars().count() <= 2 {
        label_view = label_view.width(NUMBER_W);
    } else {
        label_view.style.max_size.width = taffy::LengthPercentageAuto::length(MAX_NAME_W);
    }
    let mut v = centred(view::row(row_key(row.id)))
        .height(ROW_H)
        .padding_xy(ROW_PAD_X, 0.0)
        .gap(ROW_INNER_GAP)
        .radius(theme.radius_button)
        .interactive()
        .focusable();
    if row.active {
        v = v.fill(Role::AccentSoft).stroke(Role::AccentSoftBorder);
    }
    v = v.child(label_view);
    let on = window_order(windows, row.id);
    if on.is_empty() {
        return v.child(view::text(
            Key::indexed("ws-row-empty", row.id),
            copy::WORKSPACE_EMPTY,
            text_style(
                theme.type_scale.body,
                Weight::Regular,
                false,
                Role::TextFaint,
            ),
        ));
    }
    let mut icons = centred(view::row(Key::indexed("ws-row-icons", row.id))).gap(ICON_GAP);
    for window in on.iter().take(MAX_ICONS) {
        icons = icons.child(icon_view(window));
    }
    if on.len() > MAX_ICONS {
        icons = icons.child(view::text(
            Key::indexed("ws-row-more", row.id),
            &copy::more_windows(on.len() - MAX_ICONS),
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
        ));
    }
    v.child(icons)
}

/// The name, or the index alone when the workspace has no name.
fn label_of(row: &Row) -> String {
    if row.named {
        row.text.clone()
    } else {
        row.idx.to_string()
    }
}

/// A window's icon: its `.desktop` icon, or the fallback tinted TextLo, as
/// the Activity rows draw it.
fn icon_view(window: &Window) -> View {
    let (key, tint) = match window.icon.as_deref().and_then(IconKey::desktop_icon) {
        Some(key) => (key, None),
        None => (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo)),
    };
    View::new(
        Key::indexed("ws-icon", window.id),
        ViewKind::Icon {
            key,
            tint,
            size: ICON_SIZE,
        },
    )
    .size(ICON_SIZE, ICON_SIZE)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_paint::TextStyle;
    use celestina_ui::Tree;

    use super::*;

    fn row(id: u64, idx: u8, name: Option<&str>, active: bool) -> Row {
        Row {
            id,
            text: name.map_or_else(|| copy::workspace_number(idx), str::to_owned),
            idx,
            named: name.is_some(),
            active,
            columns: 0,
        }
    }

    fn win(id: u64, ws: u64, pos: Option<(u16, u16)>, icon: Option<&str>) -> Window {
        Window {
            id,
            workspace_id: Some(ws),
            icon: icon.map(str::to_owned),
            column: pos.map(|p| p.0),
            row: pos.map(|p| p.1),
            ..Default::default()
        }
    }

    fn find(v: &View, key: &Key) -> Option<View> {
        if v.key == *key {
            return Some(v.clone());
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    fn icon_ids(v: &View, ws: u64) -> Vec<u64> {
        let Some(icons) = find(v, &Key::indexed("ws-row-icons", ws)) else {
            return Vec::new();
        };
        icons
            .children
            .iter()
            .filter(|c| matches!(c.kind, ViewKind::Icon { .. }))
            .map(|c| {
                (0..100)
                    .find(|id| c.key == Key::indexed("ws-icon", *id))
                    .unwrap_or(0)
            })
            .collect()
    }

    fn text_of(v: &View) -> Option<(String, TextStyle)> {
        match &v.kind {
            ViewKind::Text { text, style, .. } => Some((text.clone(), *style)),
            _ => None,
        }
    }

    #[test]
    fn short_labels_named_or_not_share_the_number_column() {
        let rows = [
            row(3, 3, Some("3"), false),
            row(7, 7, None, false),
            row(9, 8, Some("Code"), false),
        ];
        let v = workspaces_view(&Theme::dark(), &rows, &[]);
        let width = |id: u64| {
            find(&v, &Key::indexed("ws-row-label", id))
                .map(|l| l.style.size.width)
                .expect("label")
        };
        let column = taffy::Dimension::length(NUMBER_W);
        assert_eq!(width(3), column, "a named number takes the column");
        assert_eq!(width(7), column, "an unnamed index takes the column");
        assert_ne!(width(9), column, "a longer name keeps its own width");
    }

    #[test]
    fn one_row_per_workspace_in_niris_order_40_tall() {
        let theme = Theme::dark();
        let rows = [
            row(7, 1, None, false),
            row(3, 2, Some("web"), true),
            row(9, 3, None, false),
        ];
        let v = workspaces_view(&theme, &rows, &[]);
        let keys: Vec<Key> = v.children.iter().map(|c| c.key).collect();
        assert_eq!(keys, [row_key(7), row_key(3), row_key(9)]);
        let mut tree = Tree::new(false, &theme);
        tree.apply(v, Instant::now());
        tree.layout_measured((LIST_W, 560.0), |s: &str, _: &TextStyle, _| {
            (7.0 * s.chars().count() as f32, 16.0)
        });
        let rect = |k| tree.rect(tree.node_by_key(k).expect("node"));
        let root = rect(list_key());
        assert_eq!(root.w, 240.0);
        assert_eq!(root.h, 3.0 * 40.0 + 2.0 * 2.0 + 2.0 * 8.0);
        for id in [7, 3, 9] {
            let r = rect(row_key(id));
            assert_eq!((r.w, r.h), (224.0, 40.0), "row {id}");
        }
        // A number takes 18 px, the name its own width.
        assert_eq!(rect(Key::indexed("ws-row-label", 7)).w, 18.0);
        assert_eq!(rect(Key::indexed("ws-row-label", 3)).w, 21.0);
        let row_view = find(&workspaces_view(&theme, &rows, &[]), &row_key(7)).expect("row");
        assert!(row_view.interactive && row_view.focusable);
        assert_eq!(
            text_of(&row_view.children[0]).map(|t| t.0),
            Some("1".into())
        );
    }

    #[test]
    fn icons_follow_the_column_order_and_floating_windows_come_last() {
        let windows = [
            win(5, 1, None, None),
            win(4, 1, Some((2, 1)), Some("kitty")),
            win(3, 1, Some((1, 2)), None),
            win(2, 1, Some((1, 1)), None),
            win(1, 1, None, None),
            win(9, 2, Some((1, 1)), None),
        ];
        let order: Vec<u64> = window_order(&windows, 1).iter().map(|w| w.id).collect();
        assert_eq!(order, [2, 3, 4, 1, 5]);
        let theme = Theme::dark();
        let v = workspaces_view(&theme, &[row(1, 1, None, false)], &windows);
        assert_eq!(icon_ids(&v, 1), [2, 3, 4, 1, 5]);
        let icon = |id| find(&v, &Key::indexed("ws-icon", id)).expect("icon").kind;
        assert!(matches!(
            icon(4),
            ViewKind::Icon { key: IconKey::Theme(_), tint: None, size } if size == 20.0
        ));
        assert!(matches!(
            icon(2),
            ViewKind::Icon {
                key: IconKey::Semantic("app-window"),
                tint: Some(Role::TextLo),
                ..
            }
        ));
    }

    #[test]
    fn more_than_six_windows_show_six_icons_and_plus_n() {
        let windows: Vec<Window> = (1..=11)
            .map(|i| win(i, 1, Some((i as u16, 1)), None))
            .collect();
        let theme = Theme::dark();
        let v = workspaces_view(&theme, &[row(1, 1, None, false)], &windows);
        assert_eq!(icon_ids(&v, 1), (1..=6).collect::<Vec<u64>>());
        let more = find(&v, &Key::indexed("ws-row-more", 1)).expect("+N");
        let (text, style) = text_of(&more).expect("text");
        assert_eq!((text.as_str(), style.role), ("+5", Role::TextLo));
        // Exactly six windows: no `+N`.
        let v = workspaces_view(&theme, &[row(1, 1, None, false)], &windows[..6]);
        assert!(find(&v, &Key::indexed("ws-row-more", 1)).is_none());
    }

    #[test]
    fn a_long_name_is_cut_and_never_pushes_the_icons_out() {
        let theme = Theme::dark();
        let mut long = row(1, 1, None, false);
        long.named = true;
        long.text = "a very long workspace name indeed".to_owned();
        let windows = [win(1, 1, Some((1, 1)), None)];
        let v = workspaces_view(&theme, &[long], &windows);
        let label = find(&v, &Key::indexed("ws-row-label", 1)).expect("label");
        let (text, _) = text_of(&label).expect("text");
        assert_eq!(text.chars().count(), MAX_NAME_GRAPHEMES);
        assert!(text.ends_with('\u{2026}'));
        assert_eq!(
            label.style.max_size.width,
            taffy::LengthPercentageAuto::length(MAX_NAME_W)
        );
        assert!(find(&v, &Key::indexed("ws-row-icons", 1)).is_some());
    }

    #[test]
    fn an_empty_workspace_says_vacio_in_the_faint_role() {
        let theme = Theme::dark();
        let windows = [win(1, 2, Some((1, 1)), None)];
        let v = workspaces_view(
            &theme,
            &[row(1, 1, None, false), row(2, 2, None, false)],
            &windows,
        );
        let (text, style) =
            text_of(&find(&v, &Key::indexed("ws-row-empty", 1)).expect("empty")).expect("text");
        assert_eq!(
            (text.as_str(), style.role),
            (copy::WORKSPACE_EMPTY, Role::TextFaint)
        );
        assert!(find(&v, &Key::indexed("ws-row-empty", 2)).is_none());
        assert!(find(&v, &Key::indexed("ws-row-icons", 1)).is_none());
    }

    #[test]
    fn the_current_row_has_the_accent_soft_fill_border_and_label() {
        let theme = Theme::dark();
        let rows = [row(1, 1, None, false), row(2, 2, None, true)];
        let v = workspaces_view(&theme, &rows, &[]);
        let current = find(&v, &row_key(2)).expect("row");
        assert!(matches!(
            current.kind,
            ViewKind::Box {
                fill: Some(Role::AccentSoft),
                stroke: Some(Role::AccentSoftBorder),
                radius,
                ..
            } if radius == theme.radius_button
        ));
        let label = text_of(&current.children[0]).expect("label");
        assert_eq!(
            (label.0.as_str(), label.1.role, label.1.weight),
            ("2", Role::Accent, Weight::Semibold)
        );
        let other = find(&v, &row_key(1)).expect("row");
        assert!(matches!(
            other.kind,
            ViewKind::Box {
                fill: None,
                stroke: None,
                ..
            }
        ));
        let label = text_of(&other.children[0]).expect("label");
        assert_eq!(label.1.role, Role::TextLo);
    }
}
