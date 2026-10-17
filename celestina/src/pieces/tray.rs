//! The Tray piece: the status notifier items as inline icons, and the Tray
//! page. Every input sends what the page's rows send: a click activates,
//! a middle click activates secondarily, a right click (or the Menu key)
//! asks for the item's menu and the wheel scrolls it. Items whose `Id` is
//! in `bar.tray_hidden` are left out of the bar only; the page lists them.

use celestina_paint::IconKey;
use celestina_ui::{view, Key, View, ViewKind};

use super::{Axis, BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy,
    island::{
        pages::tray::{index_of, row_key, RowIcon, Tray as Page, TrayRow, ICON_SIZE, MAX_ROWS},
        view::centred,
    },
    tray::{Orientation, Status, TrayCommand},
};

/// The space around an icon that still counts as the icon.
const HIT_PAD: f32 = 4.0;
/// The space between two icons.
const GAP: f32 = 2.0;
/// How much of a passive item's icon shows.
const PASSIVE_OPACITY: f32 = 0.5;

/// The Tray piece.
#[derive(Clone, Copy, Debug)]
pub struct Tray;

/// Whether the bar leaves `row` out: its `Id` is listed (an item without an
/// `Id` is never left out).
fn hidden(row: &TrayRow, cx: &BarCx<'_>) -> bool {
    !row.sni_id.is_empty() && cx.tray_hidden.contains(&row.sni_id)
}

/// The items the bar shows with their place in `cx.items`, which is what
/// their node keys carry: at most the page's bound, minus the hidden ones.
fn shown<'a>(cx: &'a BarCx<'_>) -> impl Iterator<Item = (usize, &'a TrayRow)> {
    cx.items
        .iter()
        .enumerate()
        .take(MAX_ROWS)
        .filter(|(_, row)| !hidden(row, cx))
}

/// The item whose node is `key`, if the bar shows it.
fn item_of<'a>(key: Key, cx: &'a BarCx<'_>) -> Option<&'a TrayRow> {
    let index = index_of(key, cx.items.len())?;
    let row = cx.items.get(index)?;
    (!hidden(row, cx)).then_some(row)
}

impl Tray {
    /// The accessible name of the node `key` of the widget built from `cx`:
    /// the item's title and, when it is not simply active, its status.
    #[must_use]
    pub fn label(key: Key, cx: &BarCx<'_>) -> Option<String> {
        let row = item_of(key, cx)?;
        Some(copy::bar_tray_item_name(
            &row.title,
            row.status == Status::NeedsAttention,
            row.status == Status::Passive,
        ))
    }
}

fn icon(index: usize, row: &TrayRow) -> View {
    let key = Key::indexed("tray-icon", index as u64);
    match &row.icon {
        RowIcon::Theme(name) => View::new(
            key,
            ViewKind::Icon {
                key: IconKey::Theme(name.clone()),
                tint: None,
                size: ICON_SIZE,
            },
        ),
        RowIcon::Pixmap(id) => view::image(key, *id, 1.0),
    }
    .size(ICON_SIZE, ICON_SIZE)
}

impl Piece for Tray {
    fn id(&self) -> PieceId {
        PieceId("tray")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_TRAY
    }

    /// The icons of the items that are not hidden; absent when there are
    /// none, so the zone can vanish.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let icons: Vec<View> = shown(cx)
            .map(|(index, row)| {
                let node = centred(view::row(row_key(index)))
                    .padding(HIT_PAD)
                    .radius(cx.theme.radius_md)
                    .interactive()
                    .focusable()
                    .child(icon(index, row));
                if row.status == Status::Passive {
                    node.opacity(PASSIVE_OPACITY)
                } else {
                    node
                }
            })
            .collect();
        if icons.is_empty() {
            return None;
        }
        Some(
            centred(view::row(Key::named("tray")))
                .gap(GAP)
                .children(icons),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::TRAY,
            view: &Page,
        })
    }

    /// The same commands the page's rows send. The wheel's sign is Qt's,
    /// the opposite of Wayland's, as the page's is.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        let Some(row) = item_of(input.key, input.cx) else {
            return PieceAction::None;
        };
        let id = row.id.clone();
        match input.gesture {
            Gesture::Primary => PieceAction::Tray(TrayCommand::Activate(id)),
            Gesture::Middle => PieceAction::Tray(TrayCommand::SecondaryActivate(id)),
            Gesture::Context if row.has_menu => PieceAction::TrayMenu(id),
            Gesture::Context | Gesture::Scroll(_, 0) => PieceAction::None,
            Gesture::Scroll(axis, steps) => {
                let orientation = match axis {
                    Axis::Vertical => Orientation::Vertical,
                    Axis::Horizontal => Orientation::Horizontal,
                };
                PieceAction::Tray(TrayCommand::Scroll(id, steps.saturating_neg(), orientation))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::{PixmapId, Theme};

    use super::*;
    use crate::{island::pages::NameOwner, tray::ItemId};

    fn row(n: usize, sni: &str, has_menu: bool) -> TrayRow {
        TrayRow {
            id: ItemId {
                bus_name: format!(":1.{n}"),
                path: "/StatusNotifierItem".to_owned(),
            },
            sni_id: sni.to_owned(),
            title: format!("item {n}"),
            has_menu,
            ..TrayRow::default()
        }
    }

    fn with<R>(items: &[TrayRow], hidden: &[&str], f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        let hidden: Vec<String> = hidden.iter().map(|s| (*s).to_owned()).collect();
        f(&BarCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &theme,
            output: "DP-1",
            model: None,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items,
            tray_hidden: &hidden,
            sysmon: None,
            weather: None,
            today: None,
        })
    }

    fn items() -> Vec<TrayRow> {
        vec![
            row(0, "nm-applet", true),
            row(1, "steam", true),
            row(2, "blueman", false),
        ]
    }

    #[test]
    fn the_bar_draws_one_focusable_icon_per_shown_item_keyed_by_its_place() {
        let rows = items();
        let v = with(&rows, &["nm-applet"], |cx| Tray.bar(cx)).expect("icons");
        let keys: Vec<Key> = v.children.iter().map(|c| c.key).collect();
        assert_eq!(keys, [row_key(1), row_key(2)], "the hidden one is skipped");
        assert!(v.children.iter().all(|c| c.interactive && c.focusable));
        assert_eq!(v.children[0].children[0].key, Key::indexed("tray-icon", 1));
    }

    #[test]
    fn hidden_matches_the_id_exactly_and_the_page_still_lists_every_item() {
        let rows = items();
        for (hidden, shown) in [
            (vec![], 3),
            (vec!["nm-applet", "blueman"], 1),
            (vec!["NM-APPLET"], 3),
            (vec!["nm-applet "], 3),
            (vec!["item 1"], 3),
            (vec![""], 3),
        ] {
            let n = with(&rows, &hidden, |cx| {
                Tray.bar(cx).map_or(0, |v| v.children.len())
            });
            assert_eq!(n, shown, "{hidden:?}");
        }
        // The page takes no part in the filter.
        let page = Tray.page().expect("page");
        assert_eq!(page.id, PageId::TRAY);
    }

    #[test]
    fn an_item_without_an_id_is_never_hidden() {
        let rows = vec![row(0, "", true)];
        assert!(with(&rows, &[""], |cx| Tray.bar(cx)).is_some());
    }

    #[test]
    fn an_empty_or_fully_hidden_tray_is_absent() {
        assert!(with(&[], &[], |cx| Tray.bar(cx)).is_none());
        let rows = items();
        assert!(with(&rows, &["nm-applet", "steam", "blueman"], |cx| Tray.bar(cx)).is_none());
    }

    #[test]
    fn the_bar_is_bounded_like_the_page() {
        let rows: Vec<TrayRow> = (0..MAX_ROWS + 10).map(|i| row(i, "x", false)).collect();
        let n = with(&rows, &[], |cx| {
            Tray.bar(cx).map_or(0, |v| v.children.len())
        });
        assert_eq!(n, MAX_ROWS);
        // A node past the bound asks for nothing.
        let past = row_key(MAX_ROWS);
        let action = with(&rows, &[], |cx| {
            Tray.activate(&PieceInput {
                key: past,
                gesture: Gesture::Primary,
                cx,
            })
        });
        assert_eq!(action, PieceAction::None);
    }

    #[test]
    fn icons_draw_theme_names_and_pixmaps_and_passive_ones_are_dimmed() {
        let mut rows = items();
        rows[0].icon = RowIcon::Theme("network-wired".to_owned());
        rows[1].icon = RowIcon::Pixmap(PixmapId(7));
        rows[1].status = Status::Passive;
        let v = with(&rows, &[], |cx| Tray.bar(cx)).expect("icons");
        assert!(matches!(
            v.children[0].children[0].kind,
            ViewKind::Icon {
                key: IconKey::Theme(_),
                ..
            }
        ));
        assert!(matches!(
            v.children[1].children[0].kind,
            ViewKind::Image { .. }
        ));
        assert_eq!(v.children[0].opacity, 1.0);
        assert_eq!(v.children[1].opacity, PASSIVE_OPACITY);
    }

    fn act(rows: &[TrayRow], hidden: &[&str], key: Key, gesture: Gesture) -> PieceAction {
        with(rows, hidden, |cx| {
            Tray.activate(&PieceInput { key, gesture, cx })
        })
    }

    #[test]
    fn every_input_sends_what_the_pages_rows_send() {
        let rows = items();
        let id = |i: usize| rows[i].id.clone();
        let on = |gesture| act(&rows, &[], row_key(0), gesture);
        assert_eq!(
            on(Gesture::Primary),
            PieceAction::Tray(TrayCommand::Activate(id(0)))
        );
        assert_eq!(
            on(Gesture::Middle),
            PieceAction::Tray(TrayCommand::SecondaryActivate(id(0)))
        );
        assert_eq!(on(Gesture::Context), PieceAction::TrayMenu(id(0)));
        // Qt's sign, whole steps, one axis each.
        assert_eq!(
            on(Gesture::Scroll(Axis::Vertical, 2)),
            PieceAction::Tray(TrayCommand::Scroll(id(0), -2, Orientation::Vertical))
        );
        assert_eq!(
            on(Gesture::Scroll(Axis::Horizontal, -3)),
            PieceAction::Tray(TrayCommand::Scroll(id(0), 3, Orientation::Horizontal))
        );
        assert_eq!(on(Gesture::Scroll(Axis::Vertical, 0)), PieceAction::None);
        assert_eq!(
            on(Gesture::Scroll(Axis::Vertical, i32::MIN)),
            PieceAction::Tray(TrayCommand::Scroll(id(0), i32::MAX, Orientation::Vertical))
        );
    }

    #[test]
    fn a_menu_is_asked_only_of_an_item_that_serves_one() {
        let rows = items();
        assert_eq!(
            act(&rows, &[], row_key(2), Gesture::Context),
            PieceAction::None
        );
    }

    #[test]
    fn a_hidden_stale_or_foreign_node_asks_for_nothing() {
        let rows = items();
        for key in [row_key(0), row_key(9), Key::named("tray")] {
            assert_eq!(
                act(&rows, &["nm-applet"], key, Gesture::Primary),
                PieceAction::None,
                "{key:?}"
            );
        }
    }

    #[test]
    fn the_labels_name_the_item_and_its_status() {
        let mut rows = items();
        rows[1].status = Status::NeedsAttention;
        rows[2].status = Status::Passive;
        rows[2].title.clear();
        with(&rows, &["nm-applet"], |cx| {
            assert_eq!(Tray::label(row_key(0), cx), None, "hidden");
            assert_eq!(
                Tray::label(row_key(1), cx),
                Some(copy::bar_tray_item_name("item 1", true, false))
            );
            assert_eq!(
                Tray::label(row_key(2), cx),
                Some(copy::bar_tray_item_name("", false, true))
            );
        });
        assert!(copy::bar_tray_item_name("", false, false) == copy::TRAY_ITEM_UNTITLED);
    }
}
