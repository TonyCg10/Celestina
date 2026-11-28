//! The Tray page's app menu card (CEL-D-016, design §5.6): a menu asked from
//! a page row unfolds as a card under the row, the other rows blur by
//! distance, a tall menu scrolls inside the card, and the one `CloseMenu` of
//! the app is the app's, when the card leaves `tray_menus_held`.

use celestina_paint::Role;

use super::*;
use crate::{
    island::view::chrome_height,
    sheet::tray_overflow::card_key,
    tray::{
        menu::{entry_key, ShellEntry},
        MenuEntry, MenuTree,
    },
};

const XKB_DOWN: u32 = 0xff54;
const XKB_RIGHT: u32 = 0xff53;
const XKB_ENTER: u32 = 0xff0d;
const XKB_MENU: u32 = 0xff67;

fn entry(id: i32, label: &str) -> MenuEntry {
    MenuEntry {
        id,
        label: label.to_owned(),
        enabled: true,
        ..MenuEntry::default()
    }
}

/// `n` plain entries, ids 1 to `n`.
fn menu(n: i32) -> MenuTree {
    MenuTree {
        id: 0,
        entries: (1..=n).map(|i| entry(i, &format!("Entry {i}"))).collect(),
    }
}

/// "More" opens a submenu of three entries; "Plain" is a leaf.
fn with_submenu() -> MenuTree {
    MenuTree {
        id: 0,
        entries: vec![
            MenuEntry {
                submenu: Some(MenuTree {
                    id: 1,
                    entries: (11..=13).map(|i| entry(i, &format!("Inner {i}"))).collect(),
                }),
                ..entry(1, "More")
            },
            entry(2, "Plain"),
        ],
    }
}

fn key(keysym: u32) -> InputEvent {
    InputEvent::Key {
        keysym,
        utf8: None,
        modifiers: celestina_wayland::Modifiers::default(),
        pressed: true,
        serial: 0,
    }
}

fn item_of(island: &Island, index: usize) -> ItemId {
    island.tray_rows[index].id.clone()
}

/// An open Tray page of three apps that all serve a menu, settled.
fn page_island() -> (Island, Instant) {
    let (mut island, t0) = tray_island(3);
    for row in &mut island.tray_rows {
        row.has_menu = true;
    }
    island.stale = true;
    let t = t0 + ms(800);
    step(&mut island, t);
    (island, t)
}

fn row_centre(island: &Island, index: usize) -> (f32, f32) {
    centre(page_rect(island, tray_page::row_key(index)))
}

/// The page with the card of app `index` asked for by a right click.
fn asked(index: usize) -> (Island, Instant) {
    let (mut island, t) = page_island();
    let at = row_centre(&island, index);
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item, .. }] if *item == item_of(&island, index)),
        "{out:?}"
    );
    (island, t)
}

/// [`asked`] and the app's answer `tree` at once, then 700 ms of frames.
fn unfolded(index: usize, tree: MenuTree) -> (Island, Instant) {
    let (mut island, t) = asked(index);
    let item = item_of(&island, index);
    assert!(island.tray_menu_arrived(&item, tree, None, t));
    run_frames(&mut island, t, t + ms(700));
    (island, t + ms(700))
}

/// The rect of a row of the card's pane, in the surface.
fn card_row(island: &Island, k: Key) -> Rect {
    let origin = page_rect(island, card_key());
    let pane = island
        .tray_card
        .card()
        .and_then(|c| c.pane_ref())
        .unwrap_or_else(|| panic!("no open pane"));
    let tree = pane.tree();
    let id = tree
        .node_by_key(k)
        .unwrap_or_else(|| panic!("no pane row {k:?}"));
    let r = tree.rect(id);
    Rect {
        x: origin.x + r.x,
        y: origin.y + r.y,
        ..r
    }
}

fn pane_depth(island: &Island) -> usize {
    island
        .tray_card
        .card()
        .and_then(|c| c.pane_ref())
        .map_or(0, crate::tray::pane::MenuPane::depth)
}

#[test]
fn a_page_rows_menu_unfolds_a_card_under_the_row_and_blurs_the_others() {
    let (mut island, t) = asked(1);
    let item = item_of(&island, 1);
    // Waiting: the row is grey, nothing has unfolded, the item is held.
    step(&mut island, t + ms(10));
    let row = tray_page::row_key(1);
    let id = island.page.node_by_key(row).expect("row");
    assert_eq!(island.page.box_roles(id).0, Some(Role::ControlHover));
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    assert!(!island.tray_menu_shown(&item));
    let size = island.state.page_size();
    // The answer unfolds it.
    assert!(island.tray_menu_arrived(&item, menu(3), None, t + ms(20)));
    assert!(island.tray_menu_shown(&item));
    run_frames(&mut island, t + ms(20), t + ms(720));
    let done = t + ms(720);
    let row_r = page_rect(&island, row);
    let card = page_rect(&island, card_key());
    // As wide as the row, 2 px under it, three entries and 4 px over and
    // under them.
    assert_eq!((card.x, card.w), (row_r.x, row_r.w));
    assert_eq!(card.y, row_r.y + row_r.h + 2.0);
    assert_eq!(card.h, 3.0 * 40.0 + 8.0);
    // The row under it moved down by the box and its 6 px.
    let below = page_rect(&island, tray_page::row_key(2));
    assert_eq!(below.y, row_r.y + row_r.h + 6.0 + card.h);
    // The page grew to hold it.
    let want = (chrome_height(&island.theme) + 3.0 * 40.0 + card.h + 6.0).max(size.1);
    assert_eq!(island.state.page_size().1, want);
    // The rows one place away are blurred 1.5 px at 85 %, the open one not.
    for (i, sigma) in [(0, 1.5), (1, 0.0), (2, 1.5)] {
        let (s, o) = island.tray_card.look(i, done);
        assert!((s - sigma).abs() < 0.01, "row {i}: {s}");
        assert!((o - (1.0 - 0.6 * sigma / 6.0)).abs() < 0.01, "row {i}: {o}");
    }
    let id = island.page.node_by_key(row).expect("row");
    assert_eq!(island.page.box_roles(id).0, Some(Role::ControlHover));
    // Nothing is asked for at rest.
    assert!(!island.tray_card_animating(done + ms(10)));
    assert!(!island.state.wants_frame(done + ms(10)));
}

#[test]
fn middle_click_activates_secondarily_and_right_click_unfolds_the_card() {
    let (mut island, t) = page_island();
    island.tray_rows[2].has_menu = false;
    let (x0, y0) = row_centre(&island, 0);
    let (x1, y1) = row_centre(&island, 1);
    let (x2, y2) = row_centre(&island, 2);
    assert_eq!(
        island.other_button(x1, y1, BTN_MIDDLE, t),
        vec![IslandAction::Tray(TrayCommand::SecondaryActivate(item_of(
            &island, 1
        )))]
    );
    assert!(
        island.tray_card.card().is_none(),
        "a middle click opens no card"
    );
    assert_eq!(
        island.other_button(x0, y0, BTN_RIGHT, t),
        vec![IslandAction::AskTrayMenu {
            item: item_of(&island, 0),
            at: (x0.round() as i32, y0.round() as i32),
        }]
    );
    assert_eq!(island.tray_menus_held(), vec![item_of(&island, 0)]);
    // The same row again folds it and asks for nothing.
    assert!(island.other_button(x0, y0, BTN_RIGHT, t).is_empty());
    assert!(island.tray_menus_held().is_empty());
    // An item without a dbusmenu asks the item for its own, at the pointer,
    // and hangs no card; a click between rows and any other button do
    // nothing.
    assert_eq!(
        island.other_button(x2, y2, BTN_RIGHT, t),
        vec![IslandAction::Tray(TrayCommand::ContextMenu(
            item_of(&island, 2),
            x2.round() as i32,
            y2.round() as i32
        ))]
    );
    assert!(island.tray_menus_held().is_empty());
    assert!(island.other_button(1.0, 1.0, BTN_MIDDLE, t).is_empty());
    assert!(island.other_button(x0, y0, 0x113, t).is_empty());
}

#[test]
fn the_menu_key_unfolds_the_card_of_the_focused_row() {
    let (mut island, t) = page_island();
    assert!(island.menu_key(t).is_empty(), "nothing focused");
    // Tab reaches the first row; the Menu key asks for its menu at its
    // centre.
    for _ in 0..30 {
        feed(&mut island, key(0xff09), t);
        let on = island.page.highlight().focused;
        if on.is_some_and(|id| island.page.key_of(id) == tray_page::row_key(0)) {
            break;
        }
    }
    let out = feed(&mut island, key(XKB_MENU), t);
    let item = item_of(&island, 0);
    let (cx, cy) = row_centre(&island, 0);
    assert_eq!(
        out,
        [IslandAction::AskTrayMenu {
            item: item.clone(),
            at: (cx.round() as i32, cy.round() as i32),
        }]
    );
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    // The answer focuses the first entry; Enter chooses it and folds the
    // card, the page staying.
    assert!(island.tray_menu_arrived(&item, menu(3), None, t + ms(10)));
    run_frames(&mut island, t + ms(10), t + ms(600));
    let focus = island
        .tray_card
        .card()
        .and_then(|c| c.pane_ref())
        .and_then(crate::tray::pane::MenuPane::focused_rect);
    assert!(focus.is_some(), "the first entry has the focus");
    let out = feed(&mut island, key(XKB_ENTER), t + ms(620));
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::MenuEvent(item.clone(), 1))]
    );
    assert!(island.tray_menus_held().is_empty());
    assert!(island.state.is_open());
}

#[test]
fn item_is_menu_unfolds_the_card_on_left_click() {
    let (mut island, t) = page_island();
    island.tray_rows[0].item_is_menu = true;
    island.tray_rows[1].item_is_menu = true;
    island.tray_rows[1].has_menu = false;
    island.pointer = (30.4, 40.6);
    // With a dbusmenu: its card unfolds, not an activation.
    let first = island.tray_actions(&UiEvent::Click(tray_page::row_key(0)), t);
    assert!(
        matches!(
            first.as_slice(),
            [IslandAction::AskTrayMenu { at: (30, 41), .. }]
        ),
        "{first:?}"
    );
    assert_eq!(island.tray_menus_held(), vec![item_of(&island, 0)]);
    // Without one: the item's own menu, at the pointer.
    assert_eq!(
        island.tray_actions(&UiEvent::Click(tray_page::row_key(1)), t),
        vec![IslandAction::Tray(TrayCommand::ContextMenu(
            item_of(&island, 1),
            30,
            41
        ))]
    );
    // An item that is not only a menu still activates.
    assert_eq!(
        island.tray_actions(&UiEvent::Click(tray_page::row_key(2)), t),
        vec![IslandAction::Tray(TrayCommand::Activate(item_of(
            &island, 2
        )))]
    );
}

#[test]
fn choosing_in_the_page_card_folds_it_at_once_and_the_page_stays() {
    let (mut island, t) = unfolded(1, menu(3));
    let item = item_of(&island, 1);
    let at = centre(card_row(&island, entry_key(1)));
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::MenuEvent(item.clone(), 2))]
    );
    assert!(island.state.is_open(), "the page stays");
    assert!(island.tray_menus_held().is_empty(), "the item is let go");
    // It folds over 350 ms and the rows sharpen from the start.
    run_frames(&mut island, t, t + ms(120));
    let mid = page_rect(&island, card_key()).h;
    assert!(mid > 1.0 && mid < 128.0, "folding: {mid}");
    assert!(island.tray_card.look(0, t + ms(120)).0 < 1.5);
    run_frames(&mut island, t + ms(120), t + ms(800));
    assert!(island.tray_card.card().is_none(), "the card is gone");
    assert_eq!(island.tray_card.look(0, t + ms(800)).0, 0.0);
    assert!(!island.tray_card_animating(t + ms(900)));
    // The shell's entry folds it too.
    let (mut island, t) = unfolded(2, menu(1));
    let item = item_of(&island, 2);
    assert!(island.tray_menu_arrived(&item, menu(1), Some(ShellEntry::Hide), t));
    run_frames(&mut island, t, t + ms(600));
    let at = centre(card_row(&island, entry_key(2)));
    let out = click_at(&mut island, at, BTN_LEFT, t + ms(600));
    assert_eq!(out, [IslandAction::TrayShellEntry(item, ShellEntry::Hide)]);
    assert!(island.state.is_open() && island.tray_menus_held().is_empty());
}

#[test]
fn escape_goes_back_then_folds_then_closes_the_page() {
    let (mut island, t) = unfolded(1, with_submenu());
    let item = item_of(&island, 1);
    // Down to "More" and Right: the submenu opens (the app is asked).
    feed(&mut island, key(XKB_DOWN), t);
    let out = feed(&mut island, key(XKB_RIGHT), t);
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::AboutToShow(
            item.clone(),
            1
        ))]
    );
    assert_eq!(pane_depth(&island), 1);
    // Escape: back one level.
    feed(&mut island, escape(), t);
    assert_eq!(pane_depth(&island), 0);
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    // Escape: the card folds, the page stays.
    feed(&mut island, escape(), t);
    assert!(island.state.is_open());
    assert!(island.tray_menus_held().is_empty());
    // Escape: the page closes.
    feed(&mut island, escape(), t);
    assert!(!island.state.is_open());
    // A card still waiting folds on Escape too.
    let (mut island, t) = asked(1);
    feed(&mut island, escape(), t);
    assert!(island.state.is_open());
    assert!(island.tray_menus_held().is_empty());
    feed(&mut island, escape(), t);
    assert!(!island.state.is_open());
}

#[test]
fn closing_the_page_drops_its_card_and_nothing_stays_held() {
    for how in 0..3 {
        let (mut island, t) = unfolded(1, menu(3));
        assert_eq!(island.tray_menus_held().len(), 1);
        match how {
            0 => island.close(t),
            // A press outside the page.
            1 => {
                feed(&mut island, press(2.0, 540.0, BTN_LEFT), t);
            }
            // Another page.
            _ => island.open(PageId::ACTIVITY, t),
        }
        assert!(island.tray_menus_held().is_empty(), "case {how}");
        step(&mut island, t + ms(16));
        assert!(island.tray_card.card().is_none(), "case {how}");
        assert_eq!(island.tray_card_view(t + ms(16)), None, "case {how}");
        assert!(!island.tray_card_animating(t + ms(900)), "case {how}");
    }
    // A late answer for a card the page dropped opens nothing.
    let (mut island, t) = asked(1);
    let item = item_of(&island, 1);
    island.close(t);
    assert!(!island.tray_menu_arrived(&item, menu(1), None, t + ms(30)));
}

#[test]
fn a_tall_menu_scrolls_inside_the_page_card() {
    let (mut island, t) = unfolded(1, menu(30));
    let card = page_rect(&island, card_key());
    let root = page_rect(&island, Key::named("page-root"));
    let cap = {
        let room = island.state.max_page_size().1 - chrome_height(&island.theme);
        // The room under its own row (the second).
        room - 2.0 * 40.0 - 6.0
    };
    assert_eq!(card.h, cap, "the card takes the room under its row");
    assert_eq!(
        island.state.page_size().1,
        island.state.max_page_size().1,
        "and the page is as tall as the surface lets it"
    );
    assert!(card.y + card.h < root.y + root.h, "inside the page");
    assert!(island
        .tray_card
        .card()
        .and_then(|c| c.pane_ref())
        .is_some_and(crate::tray::pane::MenuPane::can_scroll));
    // The wheel over the card scrolls it, not the app.
    let (x, y) = (card.x + 100.0, card.y + 60.0);
    feed(&mut island, InputEvent::PointerMotion { x, y }, t);
    let out = feed(
        &mut island,
        InputEvent::Axis {
            x,
            y,
            horizontal: 0.0,
            vertical: 90.0,
        },
        t,
    );
    assert!(out.is_empty(), "the app is not scrolled: {out:?}");
    assert!(island.tray_card_animating(t), "the card's scroll bar shows");
    // Over a row the app scrolls as ever.
    let (x, y) = row_centre(&island, 0);
    feed(&mut island, InputEvent::PointerMotion { x, y }, t);
    let out = feed(
        &mut island,
        InputEvent::Axis {
            x,
            y,
            horizontal: 0.0,
            vertical: 15.0,
        },
        t,
    );
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::Scroll(
            item_of(&island, 0),
            -1,
            Orientation::Vertical
        ))]
    );
}

#[test]
fn a_card_whose_app_leaves_or_that_fails_goes_at_once_and_the_page_stays() {
    let (mut island, t) = unfolded(1, menu(3));
    island.tray_rows.remove(1);
    island.stale = true;
    step(&mut island, t + ms(10));
    assert!(island.state.is_open(), "the page stays");
    assert!(island.tray_menus_held().is_empty());
    assert!(island.tray_card.card().is_none());
    // A menu that failed while the card waited.
    let (mut island, t) = asked(0);
    let item = item_of(&island, 0);
    island.tray_menu_failed(&item, t + ms(20));
    assert!(island.tray_menus_held().is_empty());
    assert!(island.state.is_open());
    step(&mut island, t + ms(40));
    assert!(island.tray_card.card().is_none());
}

#[test]
fn another_rows_menu_removes_the_first_card_at_once() {
    let (mut island, t) = unfolded(2, menu(3));
    let first = item_of(&island, 2);
    let second = item_of(&island, 0);
    let at = row_centre(&island, 0);
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item, .. }] if *item == second),
        "{out:?}"
    );
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&second));
    // The first one's late answer is nobody's.
    assert!(!island.tray_menu_arrived(&first, menu(1), None, t + ms(50)));
    assert!(island.tray_menu_arrived(&second, menu(1), None, t + ms(60)));
}

#[test]
fn reduced_motion_unfolds_and_folds_the_page_card_and_the_rows_at_once() {
    let (mut island, t) = page_island();
    island.set_reduced_motion(true);
    let at = row_centre(&island, 1);
    click_at(&mut island, at, BTN_RIGHT, t);
    let item = item_of(&island, 1);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t));
    step(&mut island, t);
    assert_eq!(page_rect(&island, card_key()).h, 128.0, "there at once");
    assert_eq!(island.tray_card.look(0, t).0, 1.5, "rows blurred");
    assert!(!island.tray_card_animating(t + ms(1)));
    let at = row_centre(&island, 1);
    click_at(&mut island, at, BTN_RIGHT, t + ms(5));
    step(&mut island, t + ms(5));
    assert!(island.tray_card.card().is_none(), "folded at once");
    assert_eq!(island.tray_card.look(0, t + ms(5)).0, 0.0);
}

/// An open Tray page of `n` apps that all serve a menu, settled.
fn page_island_of(n: usize) -> (Island, Instant) {
    let (mut island, t0) = tray_island(n);
    for row in &mut island.tray_rows {
        row.has_menu = true;
    }
    island.stale = true;
    let t = t0 + ms(800);
    step(&mut island, t);
    (island, t)
}

/// The room the rows and a card share on the page, under its lead.
fn page_room(island: &Island) -> f32 {
    island.state.max_page_size().1 - chrome_height(&island.theme) - island.lead
}

fn notice(id: u32) -> celestina_shell::notifications::Notification {
    celestina_shell::notifications::Notification {
        id,
        app_name: "app".to_owned(),
        summary: format!("summary {id}"),
        body: "body".to_owned(),
        image: None,
        actions: Vec::new(),
        urgency: celestina_shell::notifications::Urgency::Normal,
        posted_ms: 0,
        expires_at_ms: None,
        read: false,
    }
}

#[test]
fn an_app_before_the_open_one_leaving_keeps_the_page_card_under_its_app() {
    let (mut island, t) = unfolded(2, menu(3));
    let item = item_of(&island, 2);
    island.tray_rows.remove(0);
    island.stale = true;
    step(&mut island, t + ms(10));
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    assert_eq!(island.tray_card.card().map(|c| c.row()), Some(1));
    let row = page_rect(&island, tray_page::row_key(1));
    let card = page_rect(&island, card_key());
    assert_eq!(card.y, row.y + row.h + 2.0, "under its app's row");
}

#[test]
fn rows_blur_only_once_the_page_card_is_open() {
    let (mut island, t) = asked(1);
    step(&mut island, t + ms(10));
    for i in 0..3 {
        assert_eq!(island.tray_card.look(i, t + ms(300)), (0.0, 1.0), "row {i}");
    }
    let id = island
        .page
        .node_by_key(tray_page::row_key(1))
        .expect("the row");
    assert_eq!(
        island.page.box_roles(id).0,
        Some(Role::ControlHover),
        "the press fill stays while it waits"
    );
}

#[test]
fn the_page_card_has_the_room_under_its_own_row_and_the_rows_that_do_not_fit_leave() {
    let (mut island, t) = page_island_of(8);
    let drawn = island.tray_drawn();
    assert!(drawn >= 8, "premise: {drawn}");
    let at = row_centre(&island, 0);
    click_at(&mut island, at, BTN_RIGHT, t);
    let item = item_of(&island, 0);
    assert!(island.tray_menu_arrived(&item, menu(30), None, t));
    run_frames(&mut island, t, t + ms(700));
    let card = page_rect(&island, card_key());
    assert_eq!(card.h, page_room(&island) - 40.0 - 6.0);
    assert!(card.h > 300.0, "{}", card.h);
    assert_eq!(island.tray_drawn(), 1, "the rows past it do not fit");
    assert!(island
        .tray_card
        .card()
        .and_then(|c| c.pane_ref())
        .is_some_and(crate::tray::pane::MenuPane::can_scroll));
    // A short menu leaves the rows that still fit under it.
    let (mut island, t) = page_island_of(8);
    let at = row_centre(&island, 0);
    click_at(&mut island, at, BTN_RIGHT, t);
    let item = item_of(&island, 0);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t));
    run_frames(&mut island, t, t + ms(700));
    let want = 1 + ((page_room(&island) - 40.0 - 128.0 - 6.0) / 40.0).floor() as usize;
    assert_eq!(island.tray_drawn(), want.min(8));
    // The card's own rows stay in the page.
    let root = page_rect(&island, Key::named("page-root"));
    let card = page_rect(&island, card_key());
    assert!(card.y + card.h <= root.y + root.h, "inside the page");
}

#[test]
fn a_page_row_with_no_room_for_a_card_sends_the_apps_own_menu_instead() {
    let (mut island, t) = page_island_of(30);
    let last = island.tray_drawn() - 1;
    let (before, held) = (island.tray_drawn(), island.tray_menus_held());
    let at = row_centre(&island, last);
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    let item = item_of(&island, last);
    assert!(
        matches!(out.as_slice(), [IslandAction::Tray(TrayCommand::ContextMenu(i, ..))] if *i == item),
        "the app's own menu at the pointer: {out:?}"
    );
    assert!(island.tray_card.card().is_none());
    assert_eq!(island.tray_menus_held(), held);
    assert_eq!(island.tray_drawn(), before, "the rows above stay");
    // The Menu key does the same.
    for _ in 0..40 {
        feed(&mut island, key(0xff09), t);
        let on = island.page.highlight().focused;
        if on.is_some_and(|id| island.page.key_of(id) == tray_page::row_key(last)) {
            break;
        }
    }
    let out = feed(&mut island, key(XKB_MENU), t);
    assert!(
        matches!(out.as_slice(), [IslandAction::Tray(TrayCommand::ContextMenu(i, ..))] if *i == item),
        "{out:?}"
    );
}

#[test]
fn the_page_cards_room_is_less_the_lead_of_a_notice_above_the_page() {
    let (mut island, t) = page_island_of(3);
    island.set_notice(Some(notice(7)), t);
    step(&mut island, t + ms(20));
    assert!(island.lead > 0.0, "premise");
    let at = row_centre(&island, 0);
    click_at(&mut island, at, BTN_RIGHT, t + ms(20));
    let item = item_of(&island, 0);
    assert!(island.tray_menu_arrived(&item, menu(40), None, t + ms(20)));
    run_frames(&mut island, t + ms(20), t + ms(720));
    let card = page_rect(&island, card_key());
    assert_eq!(card.h, page_room(&island) - 40.0 - 6.0);
    let root = page_rect(&island, Key::named("page-root"));
    assert!(card.y + card.h <= root.y + root.h, "inside the page");
}

#[test]
fn the_page_keeps_its_card_slot_so_a_box_never_fades_with_the_tree() {
    let (mut island, t) = asked(1);
    step(&mut island, t + ms(10));
    // Before the first unfold the slot is already there, out of the layout.
    let slot = island.page.node_by_key(card_key());
    assert!(slot.is_some(), "the slot exists while the card waits");
    let item = item_of(&island, 1);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t + ms(20)));
    run_frames(&mut island, t + ms(20), t + ms(720));
    let t = t + ms(720);
    assert_eq!(page_rect(&island, card_key()).h, 128.0);
    // Another row's menu: no box of the old card is painted, in its old
    // place or fading out.
    let boxes = |island: &Island| {
        island
            .page
            .paint_order()
            .into_iter()
            .filter(|id| island.page.key_of(*id) == card_key())
            .count()
    };
    assert_eq!(boxes(&island), 1, "premise: the open box is painted");
    let at = row_centre(&island, 2);
    click_at(&mut island, at, BTN_RIGHT, t);
    for n in [10u64, 40, 120] {
        step(&mut island, t + ms(n));
        assert_eq!(boxes(&island), 0, "{n} ms: a box of the old card shows");
    }
    assert!(
        island.page.node_by_key(card_key()).is_some(),
        "the slot stays"
    );
}

#[test]
fn a_notice_arriving_over_an_open_page_card_shrinks_it_to_the_room_left() {
    let (mut island, t) = unfolded(0, menu(40));
    let before = page_rect(&island, card_key()).h;
    assert_eq!(before, page_room(&island) - 40.0 - 6.0);
    island.set_notice(Some(notice(7)), t);
    run_frames(&mut island, t, t + ms(600));
    let card = page_rect(&island, card_key());
    assert!(island.lead > 0.0, "premise");
    assert_eq!(card.h, page_room(&island) - 40.0 - 6.0);
    assert!(card.h < before);
    let root = page_rect(&island, Key::named("page-root"));
    assert!(card.y + card.h <= root.y + root.h, "inside the page");
}

#[test]
fn the_page_keeps_its_height_while_a_card_unfolds_and_rows_leave_as_the_box_reaches_them() {
    for entries in [30, 12, 6, 3] {
        let (mut island, t) = page_island_of(8);
        let start = island.state.page_size().1;
        let at = row_centre(&island, 0);
        click_at(&mut island, at, BTN_RIGHT, t);
        let item = item_of(&island, 0);
        assert!(island.tray_menu_arrived(&item, menu(entries), None, t));
        let mut last = start;
        let mut ms_in = 0;
        while ms_in + 16 <= 350 {
            ms_in += 16;
            step(&mut island, t + ms(ms_in));
            let h = island.state.page_size().1;
            assert!(
                h + 0.01 >= last,
                "{entries} entries, +{ms_in} ms: the page fell from {last} to {h}"
            );
            last = h;
            // No row under the box is drawn or focusable.
            let root = page_rect(&island, Key::named("page-root"));
            for i in 0..island.tray_drawn() {
                let row = page_rect(&island, tray_page::row_key(i));
                assert!(
                    row.y + row.h <= root.y + root.h + 0.01,
                    "{entries} entries, +{ms_in} ms: row {i} is out of the page"
                );
            }
        }
        assert!(last > start, "the page grew to the room");
        // The unfold over, the page takes the height its rows and box want.
        run_frames(&mut island, t + ms(350), t + ms(1200));
        let wants = page_view(island.state.shown_page()).wants(&island.page_cx());
        assert_eq!(
            island.state.page_size().1,
            wants.1.min(island.state.max_page_size().1)
        );
    }
}

fn row_with(n: usize) -> TrayRow {
    TrayRow {
        id: ItemId {
            bus_name: format!(":9.{n}"),
            path: "/StatusNotifierItem".to_owned(),
        },
        title: format!("new {n}"),
        has_menu: true,
        ..TrayRow::default()
    }
}

#[test]
fn an_app_inserted_before_a_tall_open_card_keeps_the_card_under_its_app() {
    let (mut island, t) = page_island_of(8);
    let at = row_centre(&island, 2);
    click_at(&mut island, at, BTN_RIGHT, t);
    let item = item_of(&island, 2);
    assert!(island.tray_menu_arrived(&item, menu(30), None, t));
    run_frames(&mut island, t, t + ms(700));
    let t = t + ms(700);
    assert_eq!(
        island.tray_drawn(),
        3,
        "premise: nothing fits under the card"
    );
    island.tray_rows.insert(0, row_with(0));
    island.stale = true;
    step(&mut island, t + ms(16));
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    assert_eq!(island.tray_card.card().map(|c| c.row()), Some(3));
    run_frames(&mut island, t + ms(16), t + ms(700));
    let row = page_rect(&island, tray_page::row_key(3));
    let card = page_rect(&island, card_key());
    assert_eq!(card.y, row.y + row.h + 2.0, "under its app's row");
}

#[test]
fn an_app_inserted_before_the_open_card_drops_it_only_when_its_new_row_has_no_room() {
    // 30 apps: the card on the last row that has room for one; one more app
    // in front pushes its row under the cap rule.
    let (mut island, t) = page_island_of(30);
    let row = (0..island.tray_drawn())
        .rev()
        .find(|&i| island.tray_cap_at(i) >= island.tray_card_min())
        .expect("a row with room");
    let at = row_centre(&island, row);
    click_at(&mut island, at, BTN_RIGHT, t);
    let item = item_of(&island, row);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t));
    run_frames(&mut island, t, t + ms(700));
    let t = t + ms(700);
    island.tray_rows.insert(0, row_with(0));
    island.stale = true;
    step(&mut island, t + ms(16));
    assert!(
        island.tray_cap_at(row + 1) < island.tray_card_min(),
        "premise"
    );
    assert!(island.tray_card.card().is_none(), "no room: the card goes");
    assert!(island.tray_menus_held().is_empty());
}

/// The most any node of the page is painted away from its laid-out place at
/// `now`.
fn greatest_slide(island: &Island, now: Instant) -> f32 {
    island
        .page
        .paint_order()
        .into_iter()
        .map(|id| {
            let (r, p) = (island.page.rect(id), island.page.painted_rect(id, now));
            (r.x - p.x).abs().max((r.y - p.y).abs())
        })
        .fold(0.0, f32::max)
}

/// How many page rows from `tray_drawn()` on still show at `now`.
fn rows_still_painted(island: &Island, now: Instant) -> usize {
    (island.tray_drawn()..8)
        .filter_map(|i| island.page.node_by_key(tray_page::row_key(i)))
        .filter(|id| island.page.effective_opacity(*id, now) > 0.0)
        .count()
}

#[test]
fn rows_a_card_leaves_no_room_for_go_at_once_and_never_fade_over_its_box() {
    for reduced in [false, true] {
        let (mut island, t) = page_island_of(8);
        island.set_reduced_motion(reduced);
        let at = row_centre(&island, 0);
        click_at(&mut island, at, BTN_RIGHT, t);
        let item = item_of(&island, 0);
        assert!(island.tray_menu_arrived(&item, menu(30), None, t));
        let mut left = false;
        for n in 1..=30u64 {
            let now = t + ms(n * 16);
            step(&mut island, now);
            left |= island.tray_drawn() < 8;
            assert_eq!(
                rows_still_painted(&island, now),
                0,
                "reduced {reduced}, +{} ms: a row that left still shows",
                n * 16
            );
        }
        assert!(left, "premise: rows left");
    }
}

#[test]
fn a_row_switch_puts_the_rows_and_the_footer_in_place_at_once_with_no_old_box() {
    for (from, to) in [(4, 1), (1, 4), (0, 6)] {
        let (mut island, t) = page_island_of(8);
        let at = row_centre(&island, from);
        click_at(&mut island, at, BTN_RIGHT, t);
        let item = item_of(&island, from);
        assert!(island.tray_menu_arrived(&item, menu(3), None, t));
        run_frames(&mut island, t, t + ms(700));
        let t = t + ms(700);
        let at = row_centre(&island, to);
        click_at(&mut island, at, BTN_RIGHT, t);
        step(&mut island, t + ms(16));
        assert!(
            greatest_slide(&island, t + ms(16)) < 1.0,
            "{from} -> {to}: nodes slide from the old card's layout"
        );
        // The old box is nowhere: the slot is out of the layout.
        let boxes = island
            .page
            .paint_order()
            .into_iter()
            .filter(|id| island.page.key_of(*id) == card_key())
            .count();
        assert_eq!(boxes, 0, "{from} -> {to}");
    }
}

#[test]
fn rows_a_card_leaves_no_room_for_go_at_once_when_it_was_waiting_before_it_opened() {
    for reduced in [false, true] {
        let (mut island, t) = page_island_of(8);
        island.set_reduced_motion(reduced);
        let at = row_centre(&island, 0);
        click_at(&mut island, at, BTN_RIGHT, t);
        // The app takes its time: the waiting card hangs from the row first.
        for n in [1, 3, 6, 9] {
            step(&mut island, t + ms(n * 16));
        }
        let item = item_of(&island, 0);
        assert!(island.tray_menu_arrived(&item, menu(30), None, t + ms(170)));
        for n in 0..14u64 {
            let now = t + ms(170 + n * 16);
            step(&mut island, now);
            assert_eq!(
                rows_still_painted(&island, now),
                0,
                "reduced {reduced}, +{} ms: a row that left still shows",
                170 + n * 16
            );
        }
        assert!(island.tray_drawn() < 8, "premise: rows left");
    }
}
