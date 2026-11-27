//! The Tray piece: the status notifier items as inline icons, and the Tray
//! page. Every input sends what the page's rows send: a click activates,
//! a middle click activates secondarily, a right click (or the Menu key)
//! asks for the item's menu (its own, through `ContextMenu`, when it has no
//! dbusmenu), a left click does too on an item that is only a menu, and the
//! wheel scrolls it. Items whose `Id` is
//! in `bar.tray_hidden` are left out of the bar only; the page lists them,
//! and, while at least one registered app is hidden, a chevron at the tray's
//! left end opens the overflow window that lists them (CEL-I-012).

use celestina_paint::Role;
use celestina_ui::{view, Key, View};

use super::{
    notifications::attention_dot, Axis, BarCx, Face, Gesture, PageEntry, PageId, Piece,
    PieceAction, PieceId, PieceInput, SheetKind,
};
use crate::{
    copy,
    island::{
        pages::tray::{icon_view, index_of, row_key, Tray as Page, TrayRow, ICON_SIZE, MAX_ROWS},
        view::centred,
    },
    tray::{Orientation, Status, TrayCommand},
};

/// The space around an icon that still counts as the icon.
const HIT_PAD: f32 = 4.0;
/// The space between two icons.
const GAP: f32 = 2.0;
/// The chevron's box: an icon's 24 px and the space around it.
const CHEVRON_BOX: f32 = ICON_SIZE + 2.0 * HIT_PAD;
/// The chevron glyph's size.
const CHEVRON_GLYPH: f32 = 18.0;
/// Where the attention dot sits in the chevron's box.
const DOT_AT: (f32, f32) = (21.0, 2.0);
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

/// The key of the chevron that opens the overflow window.
#[must_use]
pub fn chevron_key() -> Key {
    Key::named("tray-hidden")
}

/// The registered apps (at most the page's bound) that `hidden` names, with
/// their place in `items`: what the overflow window lists.
pub(crate) fn hidden_rows<'a>(
    items: &'a [TrayRow],
    hidden: &'a [String],
) -> impl Iterator<Item = (usize, &'a TrayRow)> {
    items
        .iter()
        .enumerate()
        .take(MAX_ROWS)
        .filter(|(_, row)| !row.sni_id.is_empty() && hidden.contains(&row.sni_id))
}

/// What `gesture` on the app `row` asks for, whether the node is its bar icon
/// or its row in the overflow window: a click activates, a click on an item
/// that is only a menu opens the menu, a middle click activates
/// secondarily, a right click asks for the menu and the wheel scrolls (Qt's
/// sign, the opposite of Wayland's).
pub(crate) fn row_action(row: &TrayRow, gesture: Gesture) -> PieceAction {
    let id = row.id.clone();
    match gesture {
        // An item that is only a menu opens it on the left click: its
        // dbusmenu, else its own.
        Gesture::Primary if row.item_is_menu => menu_action(row),
        Gesture::Primary => PieceAction::Tray(TrayCommand::Activate(id)),
        Gesture::Middle => PieceAction::Tray(TrayCommand::SecondaryActivate(id)),
        Gesture::Context => menu_action(row),
        Gesture::Scroll(_, 0) => PieceAction::None,
        Gesture::Scroll(axis, steps) => {
            let orientation = match axis {
                Axis::Vertical => Orientation::Vertical,
                Axis::Horizontal => Orientation::Horizontal,
            };
            PieceAction::Tray(TrayCommand::Scroll(id, steps.saturating_neg(), orientation))
        }
    }
}

/// Whether a registered app is hidden: the chevron shows only then.
fn any_hidden(cx: &BarCx<'_>) -> bool {
    hidden_rows(cx.items, cx.tray_hidden).next().is_some()
}

/// Whether a hidden app needs attention.
fn hidden_attention(cx: &BarCx<'_>) -> bool {
    hidden_rows(cx.items, cx.tray_hidden).any(|(_, row)| row.status == Status::NeedsAttention)
}

/// The chevron: the box and states of a tray icon, the Lucide `chevron-up`
/// glyph (turned while the window is open, and the box lit), and the accent
/// dot while a hidden app needs attention.
fn chevron(cx: &BarCx<'_>) -> View {
    let glyph = view::icon(Key::named("tray-hidden-icon"), "chevron-up", CHEVRON_GLYPH)
        .tint(Role::TextHi)
        .rotate(if cx.tray_open { 180.0 } else { 0.0 });
    let dot = hidden_attention(cx)
        .then(|| attention_dot(Key::named("tray-hidden-attention"), DOT_AT.0, DOT_AT.1));
    // Centred on both axes: the glyph sits, and turns, in the middle of
    // the box, as a tray icon sits in its plate.
    let mut row = centred(view::row(chevron_key()));
    row.style.justify_content = taffy::JustifyContent::CENTER;
    let node = row
        .size(CHEVRON_BOX, CHEVRON_BOX)
        .radius(cx.theme.radius_md)
        .interactive()
        .focusable()
        .child(glyph)
        .children(dot);
    if cx.tray_open {
        node.fill(Role::ControlHover)
    } else {
        node
    }
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
        if key == chevron_key() {
            return any_hidden(cx).then(|| {
                if hidden_attention(cx) {
                    copy::TRAY_HIDDEN_ATTENTION.to_owned()
                } else {
                    copy::TRAY_HIDDEN.to_owned()
                }
            });
        }
        let row = item_of(key, cx)?;
        Some(copy::bar_tray_item_name(
            &row.title,
            row.status == Status::NeedsAttention,
            row.status == Status::Passive,
        ))
    }
}

fn icon(index: usize, row: &TrayRow) -> View {
    icon_view(Key::indexed("tray-icon", index as u64), &row.icon)
}

/// The item's menu: its dbusmenu when it serves one, or the shell's own
/// card when the item has an `Id` (the host asks the app for its own menu
/// too); else only the item's own (`ContextMenu`).
fn menu_action(row: &TrayRow) -> PieceAction {
    if row.has_menu || !row.sni_id.is_empty() {
        PieceAction::TrayMenu(row.id.clone())
    } else {
        PieceAction::TrayContext(row.id.clone())
    }
}

impl Piece for Tray {
    fn id(&self) -> PieceId {
        PieceId("tray")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_TRAY
    }

    /// The chevron, while a registered app is hidden, then the icons of the
    /// items that are not hidden; absent when there are neither, so the zone
    /// can vanish.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let chevron = any_hidden(cx).then(|| chevron(cx));
        let icons: Vec<View> = chevron
            .into_iter()
            .chain(shown(cx).map(|(index, row)| {
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
            }))
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

    /// The tray opens no sheet: its items act on their own and their menus
    /// are popups.
    fn face(&self, _cx: &BarCx<'_>) -> Option<Face> {
        None
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::TRAY,
            view: &Page,
        })
    }

    /// The same commands the page's rows send. The wheel's sign is Qt's,
    /// the opposite of Wayland's, as the page's is. The chevron opens the
    /// overflow window and answers nothing else.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.key == chevron_key() {
            return match (any_hidden(input.cx), input.gesture) {
                (true, Gesture::Primary) => PieceAction::Sheet(SheetKind::TrayOverflow),
                _ => PieceAction::None,
            };
        }
        match item_of(input.key, input.cx) {
            Some(row) => row_action(row, input.gesture),
            None => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::{IconKey, PixmapId, Role, Theme};
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        island::pages::{tray::RowIcon, NameOwner},
        tray::ItemId,
    };

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
        with_open(items, hidden, false, f)
    }

    fn with_open<R>(
        items: &[TrayRow],
        hidden: &[&str],
        open: bool,
        f: impl FnOnce(&BarCx<'_>) -> R,
    ) -> R {
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
            tray_open: open,
            sysmon: None,
            weather: None,
            today: None,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles: &crate::bubbles::NONE,
            media: &crate::island::pages::media::NO_MEDIA,
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
        assert_eq!(
            keys,
            [chevron_key(), row_key(1), row_key(2)],
            "the hidden one is skipped and the chevron stands for it"
        );
        assert!(v.children.iter().all(|c| c.interactive && c.focusable));
        assert_eq!(v.children[1].children[0].key, Key::indexed("tray-icon", 1));
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
            // The icons the bar shows; the chevron is counted apart.
            let n = with(&rows, &hidden, |cx| {
                Tray.bar(cx).map_or(0, |v| {
                    v.children.iter().filter(|c| c.key != chevron_key()).count()
                })
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
    fn an_empty_tray_is_absent_and_a_fully_hidden_one_is_the_chevron_alone() {
        assert!(with(&[], &[], |cx| Tray.bar(cx)).is_none());
        let rows = items();
        let v = with(&rows, &["nm-applet", "steam", "blueman"], |cx| Tray.bar(cx))
            .expect("the chevron");
        assert_eq!(v.children.len(), 1);
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

    #[test]
    fn a_file_icon_and_the_bundled_fallback_draw_in_the_bar() {
        let mut rows = items();
        rows[0].icon = RowIcon::File("/usr/share/icons/x.png".into());
        let v = with(&rows, &[], |cx| Tray.bar(cx)).expect("icons");
        assert!(matches!(
            &v.children[0].children[0].kind,
            ViewKind::Icon {
                key: IconKey::File(_),
                tint: None,
                ..
            }
        ));
        assert!(matches!(
            &v.children[1].children[0].kind,
            ViewKind::Icon {
                key: IconKey::Semantic("app-window"),
                tint: Some(Role::TextLo),
                ..
            }
        ));
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
    fn an_item_without_a_dbusmenu_is_asked_for_its_own_menu() {
        let mut rows = items();
        // With an `Id` the shell's card opens too (the host asks the app).
        assert!(!rows[2].has_menu && !rows[2].sni_id.is_empty());
        assert_eq!(
            act(&rows, &[], row_key(2), Gesture::Context),
            PieceAction::TrayMenu(rows[2].id.clone())
        );
        // Without an `Id` it cannot be listed: only the item's own menu.
        rows[2].sni_id.clear();
        assert_eq!(
            act(&rows, &[], row_key(2), Gesture::Context),
            PieceAction::TrayContext(rows[2].id.clone())
        );
    }

    #[test]
    fn item_is_menu_opens_the_menu_on_left_click() {
        let mut rows = items();
        rows[0].item_is_menu = true;
        rows[2].item_is_menu = true;
        rows[2].sni_id.clear();
        assert_eq!(
            act(&rows, &[], row_key(0), Gesture::Primary),
            PieceAction::TrayMenu(rows[0].id.clone())
        );
        assert_eq!(
            act(&rows, &[], row_key(2), Gesture::Primary),
            PieceAction::TrayContext(rows[2].id.clone()),
            "no dbusmenu: the item's own"
        );
        assert_eq!(
            act(&rows, &[], row_key(1), Gesture::Primary),
            PieceAction::Tray(TrayCommand::Activate(rows[1].id.clone())),
            "an item that is not only a menu activates"
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

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn chevron_shows_only_with_a_hidden_app() {
        let rows = items();
        // No hidden app: today's tray, no chevron.
        let v = with(&rows, &[], |cx| Tray.bar(cx)).expect("icons");
        assert!(find(&v, chevron_key()).is_none());
        // A hidden app that is not registered does not count.
        let v = with(&rows, &["ghost"], |cx| Tray.bar(cx)).expect("icons");
        assert!(find(&v, chevron_key()).is_none());
        // An item without an `Id` is never hidden, so it never makes one.
        let bare = vec![row(0, "", true)];
        let v = with(&bare, &[""], |cx| Tray.bar(cx)).expect("icons");
        assert!(find(&v, chevron_key()).is_none());
        // One hidden app: the chevron is the first node, 32 px, radius 12,
        // interactive and focusable, with the 18 px glyph.
        let v = with(&rows, &["nm-applet"], |cx| Tray.bar(cx)).expect("icons");
        assert_eq!(v.children[0].key, chevron_key(), "at the left end");
        let chevron = &v.children[0];
        assert!(chevron.interactive && chevron.focusable);
        assert_eq!(
            (chevron.style.size.width, chevron.style.size.height),
            (
                taffy::Dimension::length(32.0),
                taffy::Dimension::length(32.0)
            )
        );
        assert!(matches!(
            chevron.kind,
            ViewKind::Box { radius, .. } if radius == 12.0
        ));
        let glyph = &chevron.children[0];
        assert!(matches!(
            glyph.kind,
            ViewKind::Icon {
                key: IconKey::Semantic("chevron-up"),
                size,
                ..
            } if size == 18.0
        ));
        assert_eq!(v.children.len(), 3, "the chevron and the two shown apps");
        // Every app hidden: only the chevron is left, so the tray is not
        // absent.
        let all = with(&rows, &["nm-applet", "steam", "blueman"], |cx| Tray.bar(cx))
            .expect("the chevron");
        assert_eq!(all.children.len(), 1);
        assert_eq!(all.children[0].key, chevron_key());
        // No registered app at all: nothing.
        assert!(with(&[], &["nm-applet"], |cx| Tray.bar(cx)).is_none());
    }

    #[test]
    fn chevron_opens_the_overflow_window() {
        let rows = items();
        for (gesture, want) in [
            (
                Gesture::Primary,
                PieceAction::Sheet(crate::pieces::SheetKind::TrayOverflow),
            ),
            (Gesture::Middle, PieceAction::None),
            (Gesture::Context, PieceAction::None),
            (Gesture::Scroll(Axis::Vertical, 1), PieceAction::None),
        ] {
            assert_eq!(
                act(&rows, &["nm-applet"], chevron_key(), gesture),
                want,
                "{gesture:?}"
            );
        }
        // Without a hidden app there is no chevron to press.
        assert_eq!(
            act(&rows, &[], chevron_key(), Gesture::Primary),
            PieceAction::None
        );
    }

    #[test]
    fn attention_dot_on_chevron_for_a_hidden_app() {
        let mut rows = items();
        let dot = |rows: &[TrayRow], hidden: &[&str]| {
            let v = with(rows, hidden, |cx| Tray.bar(cx)).expect("icons");
            find(&v, Key::named("tray-hidden-attention")).cloned()
        };
        // Quiet hidden app: no dot, the plain name.
        assert!(dot(&rows, &["steam"]).is_none());
        with(&rows, &["steam"], |cx| {
            assert_eq!(
                Tray::label(chevron_key(), cx),
                Some(copy::TRAY_HIDDEN.to_owned())
            );
        });
        // A shown app that needs attention does not light the chevron.
        rows[1].status = Status::NeedsAttention;
        assert!(dot(&rows, &["nm-applet"]).is_none());
        // A hidden one does: 7 px, accent, at (21, 2) of the 32 px box.
        let mark = dot(&rows, &["steam"]).expect("the dot");
        assert!(matches!(
            mark.kind,
            ViewKind::Box { fill: Some(Role::Accent), radius, .. } if radius == 3.5
        ));
        assert_eq!(
            (mark.style.size.width, mark.style.size.height),
            (taffy::Dimension::length(7.0), taffy::Dimension::length(7.0))
        );
        assert_eq!(mark.style.position, taffy::Position::Absolute);
        assert_eq!(
            mark.style.inset.left,
            taffy::LengthPercentageAuto::length(21.0)
        );
        assert_eq!(
            mark.style.inset.top,
            taffy::LengthPercentageAuto::length(2.0)
        );
        with(&rows, &["steam"], |cx| {
            assert_eq!(
                Tray::label(chevron_key(), cx),
                Some(copy::TRAY_HIDDEN_ATTENTION.to_owned())
            );
        });
        // A passive hidden app asks for nothing.
        rows[1].status = Status::Passive;
        assert!(dot(&rows, &["steam"]).is_none());
    }

    #[test]
    fn the_chevron_is_lit_and_turned_while_its_window_is_open() {
        let rows = items();
        let at_rest = with(&rows, &["nm-applet"], |cx| Tray.bar(cx)).expect("icons");
        let open = with_open(&rows, &["nm-applet"], true, |cx| Tray.bar(cx)).expect("icons");
        let (rest, lit) = (&at_rest.children[0], &open.children[0]);
        assert_eq!(rest.children[0].rotation, 0.0);
        assert_eq!(lit.children[0].rotation, 180.0, "the glyph turns");
        assert!(matches!(rest.kind, ViewKind::Box { fill: None, .. }));
        assert!(matches!(
            lit.kind,
            ViewKind::Box {
                fill: Some(Role::ControlHover),
                ..
            }
        ));
    }

    #[test]
    fn hidden_rows_are_the_registered_apps_the_list_names() {
        let rows = items();
        let hidden = |ids: &[&str]| -> Vec<usize> {
            let ids: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
            hidden_rows(&rows, &ids).map(|(i, _)| i).collect()
        };
        assert_eq!(hidden(&["steam", "ghost"]), [1]);
        assert_eq!(
            hidden(&["blueman", "nm-applet"]),
            [0, 2],
            "in the host's order"
        );
        assert!(hidden(&[]).is_empty());
    }

    #[test]
    fn a_hidden_row_answers_the_gestures_of_a_bar_icon() {
        let mut rows = items();
        rows[1].item_is_menu = true;
        let id = |i: usize| rows[i].id.clone();
        let on = |i: usize, gesture| row_action(&rows[i], gesture);
        assert_eq!(
            on(0, Gesture::Primary),
            PieceAction::Tray(TrayCommand::Activate(id(0)))
        );
        assert_eq!(on(1, Gesture::Primary), PieceAction::TrayMenu(id(1)));
        assert_eq!(
            on(0, Gesture::Middle),
            PieceAction::Tray(TrayCommand::SecondaryActivate(id(0)))
        );
        assert_eq!(on(0, Gesture::Context), PieceAction::TrayMenu(id(0)));
        assert_eq!(
            on(0, Gesture::Scroll(Axis::Vertical, 2)),
            PieceAction::Tray(TrayCommand::Scroll(id(0), -2, Orientation::Vertical))
        );
    }
}
