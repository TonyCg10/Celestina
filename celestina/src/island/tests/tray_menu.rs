//! A tray icon's menu as a capsule menu window (CEL-D-016): the press opens a
//! held drop and asks the app for the menu; the layout opens the window to
//! its size; a choice, Escape or any other way out closes it, and no window
//! is held after. The Island sends every action; the app closes the item's
//! menu once nothing holds it.

use super::*;
use crate::{
    sheet::geometry,
    tray::{
        menu::{entry_key, ShellEntry},
        MenuEntry, MenuTree,
    },
};

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

/// Two entries: "More" opens a submenu of three, "Plain" is a leaf.
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

fn item_of(island: &Island) -> ItemId {
    island.tray_rows[0].id.clone()
}

fn icon(island: &Island) -> (f32, f32) {
    centre(bar_rect(island, tray_page::row_key(0)))
}

/// The bar's one tray icon right-clicked at `t0`: the window is a held drop
/// and the app is asked.
fn asked() -> (Island, Instant, ItemId) {
    let (mut island, t0) = bar_island(1920.0);
    let item = item_of(&island);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_RIGHT, t0);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item: i, .. }] if *i == item),
        "{out:?}"
    );
    (island, t0, item)
}

/// [`asked`] and the app's answer `tree` 300 ms in, then the open run out.
fn shown(tree: MenuTree, shell: Option<ShellEntry>) -> (Island, Instant, ItemId) {
    let (mut island, t0, item) = asked();
    let t = t0 + ms(300);
    assert!(island.tray_menu_arrived(&item, tree, shell, t));
    step(&mut island, t);
    let settled = t + ms(600);
    step(&mut island, settled);
    (island, settled, item)
}

/// The surface rect of the window's row `key`.
fn row(island: &Island, key: Key, t: Instant) -> Rect {
    let anchor = island.sheet_anchor(t).expect("an anchor");
    island.sheets.body_rect(key, &anchor).expect("a row")
}

#[test]
fn a_press_on_a_tray_icon_opens_a_held_drop_and_asks_for_the_menu() {
    let (mut island, t0) = bar_island(1920.0);
    let item = item_of(&island);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_RIGHT, t0);
    assert_eq!(
        out,
        [IslandAction::AskTrayMenu {
            item: item.clone(),
            at: output_point(at),
        }]
    );
    assert_eq!(island.sheets.kind(), Some(SheetKind::TrayMenu));
    assert_eq!(island.sheets.piece(), Some(tray_page::row_key(0)));
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    assert!(island.sheets.tray_menu_waiting(&item));
    // The drop swells and falls for 239 ms (52 % of 460) and then holds as
    // the bulb, asking for no frame, while the app has not answered.
    step(&mut island, t0 + ms(100));
    assert!(island.sheets.wants_frame(t0 + ms(100)));
    for n in [240, 900, 5000] {
        let now = t0 + ms(n);
        step(&mut island, now);
        let anchor = island.sheet_anchor(now).expect("an anchor");
        let frame = island.sheets.frame(&anchor, now).expect("the drop");
        assert!(!frame.drawn, "only the bulb at {n} ms");
        assert!(frame.liquid.is_some_and(|l| l.bulb.is_some()), "{n} ms");
        assert!(!island.sheets.wants_frame(now), "{n} ms");
    }
    // Nothing shows a window yet: the press on its place is outside.
    assert!(island.sheets.tray_menu_waiting(&item));
    assert!(!island.sheets.tray_menu_shown(&item));
}

#[test]
fn the_layout_opens_the_drop_to_the_menus_size() {
    let (mut island, t0, item) = asked();
    let t = t0 + ms(900);
    step(&mut island, t);
    let tree = menu(3);
    assert!(island.tray_menu_arrived(&item, tree, None, t));
    assert!(island.sheets.wants_frame(t));
    assert!(island.sheets.tray_menu_shown(&item));
    // The open phase runs on from the hold: 221 ms later it is the window.
    let done = t + ms(260);
    step(&mut island, done);
    assert!(!island.sheets.wants_frame(done));
    let anchor = island.sheet_anchor(done).expect("an anchor");
    let size = (300.0, 16.0 + 3.0 * island.theme.row_height);
    assert_eq!(island.sheets.body_size(), Some(size));
    let want = geometry::window(
        anchor.capsule,
        anchor.edge,
        anchor.pill,
        size,
        anchor.surface,
    );
    assert_eq!(shown_rect(&island, done), want);
    assert_eq!((want.y, want.w, want.h), (56.0, 300.0, size.1));
    // A republish with one more entry morphs the window in place.
    assert!(island.tray_menu_arrived(&item, menu(4), None, done));
    let later = done + ms(400);
    step(&mut island, later);
    assert_eq!(
        shown_rect(&island, later).h,
        16.0 + 4.0 * island.theme.row_height
    );
    // The answer for an item nothing holds opens nothing.
    let other = ItemId {
        bus_name: ":1.99".to_owned(),
        path: "/StatusNotifierItem".to_owned(),
    };
    assert!(!island.tray_menu_arrived(&other, menu(1), None, later));
}

#[test]
fn a_layout_before_the_drop_has_fallen_does_not_stop_it() {
    let (mut island, t0, item) = asked();
    assert!(island.tray_menu_arrived(&item, menu(2), None, t0 + ms(100)));
    for n in [150, 239, 300, 459] {
        step(&mut island, t0 + ms(n));
        assert!(island.sheets.wants_frame(t0 + ms(n)), "{n} ms");
    }
    let done = t0 + ms(470);
    step(&mut island, done);
    assert!(!island.sheets.wants_frame(done));
    assert!(shown_rect(&island, done).h > 0.0);
}

#[test]
fn a_primary_press_on_the_open_icon_only_closes_its_menu() {
    // Shown: no `Activate`.
    let (mut island, t, item) = shown(menu(2), None);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert!(out.is_empty(), "{out:?}");
    assert!(!island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
    // Still waiting for the app: the same.
    let (mut island, t0, _) = asked();
    let t = t0 + ms(400);
    step(&mut island, t);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert!(out.is_empty(), "{out:?}");
    assert!(island.tray_menus_held().is_empty());
    // Without a menu open, the primary press activates the app as ever.
    let (mut island, t0) = bar_island(1920.0);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_LEFT, t0);
    assert_eq!(out, [IslandAction::Tray(TrayCommand::Activate(item))]);
    // Middle and the wheel on the open icon act on the app as usual.
    let (mut island, t, item) = shown(menu(2), None);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_MIDDLE, t);
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::SecondaryActivate(
            item.clone()
        ))]
    );
    let (mut island, t, item) = shown(menu(2), None);
    let at = icon(&island);
    let wheel = InputEvent::Axis {
        x: at.0,
        y: at.1,
        horizontal: 0.0,
        vertical: 30.0,
    };
    assert_eq!(
        feed(&mut island, wheel, t),
        [IslandAction::Tray(TrayCommand::Scroll(
            item,
            -2,
            Orientation::Vertical
        ))]
    );
    assert!(island.sheets.is_open(), "the wheel keeps the menu");
}

#[test]
fn the_opening_gesture_again_closes_the_menu() {
    let (mut island, t, _) = shown(menu(2), None);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(out.is_empty(), "no second ask: {out:?}");
    assert!(!island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
}

#[test]
fn choosing_sends_the_event_and_closes_the_window() {
    let (mut island, t, item) = shown(menu(3), None);
    let at = centre(row(&island, entry_key(1), t));
    let out = click_at(&mut island, at, BTN_LEFT, t);
    // Only the choice: the window is gone from the Island's holding, so the
    // app's sync sends `CloseMenu` after it and not before.
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::MenuEvent(item.clone(), 2))]
    );
    assert!(!island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
    // The menu key chooses too.
    let (mut island, t, item) = shown(menu(3), None);
    let tab = key(0xff09);
    feed(&mut island, tab, t);
    let out = feed(&mut island, key(0xff0d), t);
    assert_eq!(out, [IslandAction::Tray(TrayCommand::MenuEvent(item, 1))]);
}

#[test]
fn the_shell_entry_sends_its_action_and_closes_the_window() {
    let (mut island, t, item) = shown(menu(1), Some(ShellEntry::Hide));
    // A row, the line before the shell's entry and the entry itself.
    let at = centre(row(&island, entry_key(2), t));
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert_eq!(out, [IslandAction::TrayShellEntry(item, ShellEntry::Hide)]);
    assert!(!island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
}

#[test]
fn a_submenu_asks_about_to_show_and_the_window_follows_its_height() {
    let (mut island, t, item) = shown(with_submenu(), None);
    assert_eq!(
        shown_rect(&island, t).h,
        16.0 + 2.0 * island.theme.row_height
    );
    let at = centre(row(&island, entry_key(0), t));
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::AboutToShow(
            item.clone(),
            1
        ))]
    );
    assert!(island.sheets.is_open(), "the window stays");
    // The back row and three entries: the window morphs to them, on the
    // 200 ms ONE_UI curve, not in a step.
    let (root, sub) = (
        16.0 + 2.0 * island.theme.row_height,
        16.0 + 4.0 * island.theme.row_height,
    );
    step(&mut island, t + ms(50));
    let mid = shown_rect(&island, t + ms(50)).h;
    assert!(mid > root && mid < sub, "{mid} between {root} and {sub}");
    let want = root + (sub - root) * celestina_ui::ONE_UI.eval(0.25);
    assert!((mid - want).abs() < 0.01, "{mid} vs {want}");
    let later = t + ms(400);
    step(&mut island, later);
    assert_eq!(
        shown_rect(&island, later).h,
        16.0 + 4.0 * island.theme.row_height
    );
    assert_eq!(island.tray_menus_held(), [item]);
}

#[test]
fn escape_goes_back_then_closes() {
    let (mut island, t, _) = shown(with_submenu(), None);
    let at = centre(row(&island, entry_key(0), t));
    click_at(&mut island, at, BTN_LEFT, t);
    let t = t + ms(400);
    step(&mut island, t);
    assert!(island.sheets.body_takes_escape(), "one level in");
    feed(&mut island, escape(), t);
    assert!(island.sheets.is_open(), "Escape went back a level");
    assert!(!island.sheets.body_takes_escape());
    let t = t + ms(400);
    step(&mut island, t);
    assert_eq!(
        shown_rect(&island, t).h,
        16.0 + 2.0 * island.theme.row_height,
        "the root again"
    );
    feed(&mut island, escape(), t);
    assert!(
        !island.sheets.is_open(),
        "the next Escape closes the window"
    );
    assert!(island.tray_menus_held().is_empty());
}

/// A way out the window can take, applied to an Island at an instant.
type Close = fn(&mut Island, Instant);

/// Every way the window can go while the app has not answered, or after.
#[test]
fn every_close_path_leaves_nothing_held() {
    let paths: [(&str, Close); 8] = [
        ("Escape", |island, t| {
            feed(island, escape(), t);
        }),
        ("a press outside", |island, t| {
            feed(island, press(5.0, 500.0, BTN_LEFT), t);
        }),
        ("the keyboard leaving", |island, t| {
            feed(island, InputEvent::FocusLost, t);
        }),
        ("another capsule's menu", |island, t| {
            let dot = centre(bar_rect(island, Key::indexed("dot", 3)));
            click_at(island, dot, BTN_LEFT, t);
        }),
        ("the Island's page", |island, t| {
            island.open(PageId::ACTIVITY, t);
        }),
        ("close everything", |island, t| island.close_everything(t)),
        ("the icon leaving", |island, t| {
            island.tray_rows.clear();
            island.stale = true;
            island.invalidate_bar();
            step(island, t + ms(10));
        }),
        ("the app not answering", |island, t| {
            let item = island.tray_rows.first().map(|r| r.id.clone());
            if let Some(item) = item {
                island.tray_menu_failed(&item, t);
            }
        }),
    ];
    for (name, close) in paths {
        // Still waiting for the app.
        let (mut island, t0, _) = asked();
        let t = t0 + ms(400);
        step(&mut island, t);
        assert_eq!(island.tray_menus_held().len(), 1, "{name}: premise");
        close(&mut island, t);
        assert!(island.tray_menus_held().is_empty(), "{name}, waiting");
        // Shown. (The app not answering does not close a menu it showed.)
        let (mut island, t, _) = shown(menu(2), None);
        assert_eq!(island.tray_menus_held().len(), 1, "{name}: premise");
        close(&mut island, t);
        if name == "the app not answering" {
            assert_eq!(island.tray_menus_held().len(), 1, "{name}, shown");
        } else {
            assert!(island.tray_menus_held().is_empty(), "{name}, shown");
        }
    }
    // Choosing leaves nothing held either (see the choice tests).
}

#[test]
fn the_icon_leaving_closes_its_menu_window_and_sends_close_menu_once() {
    let (mut island, t, item) = shown(menu(2), None);
    let before = island.tray_menus_held();
    assert_eq!(before, [item]);
    // The app quits: its row leaves the tray's list.
    island.tray_rows.clear();
    island.stale = true;
    island.invalidate_bar();
    step(&mut island, t + ms(10));
    assert!(!island.sheets.is_open(), "the window closes on its exit");
    // Nothing holds the item from then on, however the close goes on: the
    // app's sync names it closed on the first look and never again.
    for n in [10, 100, 400, 900] {
        assert!(island.tray_menus_held().is_empty(), "{n} ms");
        step(&mut island, t + ms(n));
    }
}

#[test]
fn without_a_shell_entry_a_failed_menu_closes_the_drop() {
    let (mut island, t0, item) = asked();
    let t = t0 + ms(600);
    step(&mut island, t);
    island.tray_menu_failed(&item, t);
    assert!(!island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
    // The drop rises back into the capsule on its own close.
    step(&mut island, t + ms(100));
    assert!(island.sheets.wants_frame(t + ms(100)));
    // With a shell entry the app shows it alone instead: an empty menu.
    let (mut island, t0, item) = asked();
    let t = t0 + ms(600);
    assert!(island.tray_menu_arrived(&item, MenuTree::default(), Some(ShellEntry::Hide), t));
    step(&mut island, t + ms(500));
    assert!(island.sheets.is_open());
    assert_eq!(
        shown_rect(&island, t + ms(500)).h,
        16.0 + island.theme.row_height
    );
}

#[test]
fn a_menu_asked_while_a_page_is_open_waits_for_it_and_is_held() {
    let (mut island, t0) = bar_island(1920.0);
    island.set_reduced_motion(true);
    island.open(PageId::ACTIVITY, t0);
    let item = item_of(&island);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_RIGHT, t0);
    assert!(matches!(out.as_slice(), [IslandAction::AskTrayMenu { .. }]));
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    // The app answers before the page is gone: the queued window has it.
    assert!(island.tray_menu_arrived(&item, menu(2), None, t0));
    assert!(island.sheets.has_queued());
    // The page going gives the menu up, and nothing is held.
    feed(&mut island, escape(), t0);
    assert!(island.tray_menus_held().is_empty());
}

#[test]
fn a_menu_asked_while_another_capsules_window_is_open_replaces_it() {
    let (mut island, t0) = bar_island(1920.0);
    let item = item_of(&island);
    let dot = centre(bar_rect(&island, Key::indexed("dot", 3)));
    click_at(&mut island, dot, BTN_LEFT, t0);
    assert_eq!(island.sheets.kind(), Some(SheetKind::Workspaces));
    let t = t0 + ms(600);
    step(&mut island, t);
    let at = icon(&island);
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { .. }]),
        "{out:?}"
    );
    // The workspaces window closes towards its own piece; the menu's drop
    // is held and is what the Island holds.
    assert_eq!(island.sheets.kind(), Some(SheetKind::TrayMenu));
    assert_eq!(island.tray_menus_held(), [item]);
    assert!(island.sheets.is_leaving());
}

#[test]
fn a_second_icons_menu_takes_the_window_and_the_first_item_is_no_longer_held() {
    let (mut island, t0) = bar_island(1920.0);
    island.tray_rows.push(TrayRow {
        id: ItemId {
            bus_name: ":1.8".to_owned(),
            path: "/StatusNotifierItem".to_owned(),
        },
        title: "second".to_owned(),
        has_menu: true,
        ..TrayRow::default()
    });
    island.stale = true;
    island.invalidate_bar();
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let (first, second) = (
        island.tray_rows[0].id.clone(),
        island.tray_rows[1].id.clone(),
    );
    let at = centre(bar_rect(&island, tray_page::row_key(0)));
    click_at(&mut island, at, BTN_RIGHT, t0);
    let t = t0 + ms(300);
    assert!(island.tray_menu_arrived(&first, menu(2), None, t));
    step(&mut island, t + ms(600));
    let t = t0 + ms(900);
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&first));
    // Another icon of the same capsule: its menu takes the window.
    let at = centre(bar_rect(&island, tray_page::row_key(1)));
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item, .. }] if *item == second),
        "{out:?}"
    );
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&second));
    // The first item's answer, were it to come again, opens nothing.
    assert!(!island.tray_menu_arrived(&first, menu(1), None, t));
    assert!(island.tray_menu_arrived(&second, menu(1), None, t));
}

/// Two tray icons in the bar; the first one's menu of `n` entries is shown,
/// settled. The second icon's item is returned too.
fn two_icons_first_shown(n: i32) -> (Island, Instant, ItemId) {
    let (mut island, t0) = bar_island(1920.0);
    island.tray_rows.push(TrayRow {
        id: ItemId {
            bus_name: ":1.8".to_owned(),
            path: "/StatusNotifierItem".to_owned(),
        },
        title: "second".to_owned(),
        has_menu: true,
        ..TrayRow::default()
    });
    island.stale = true;
    island.invalidate_bar();
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let (first, second) = (
        island.tray_rows[0].id.clone(),
        island.tray_rows[1].id.clone(),
    );
    let at = centre(bar_rect(&island, tray_page::row_key(0)));
    click_at(&mut island, at, BTN_RIGHT, t0);
    assert!(island.tray_menu_arrived(&first, menu(n), None, t0 + ms(300)));
    step(&mut island, t0 + ms(900));
    (island, t0 + ms(900), second)
}

fn window_frame(island: &Island, now: Instant) -> crate::sheet::drop::DropFrame {
    let anchor = island.sheet_anchor(now).expect("an anchor");
    island.sheets.frame(&anchor, now).expect("a window")
}

#[test]
fn a_second_icons_waiting_menu_keeps_the_window_and_morphs_to_the_menu_on_arrival() {
    let (mut island, t, second) = two_icons_first_shown(4);
    let tall = shown_rect(&island, t).h;
    let at = centre(bar_rect(&island, tray_page::row_key(1)));
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item, .. }] if *item == second),
        "{out:?}"
    );
    // The window stays at the height it had while the app is asked.
    for n in [16u64, 150, 300, 380] {
        step(&mut island, t + ms(n));
        let now = shown_rect(&island, t + ms(n));
        assert!((now.h - tall).abs() < 0.01, "{n} ms: {} of {tall}", now.h);
    }
    // The answer after the travel has finished: the window morphs to the
    // menu's height and the new content fades in on the travel's clock.
    let short = 2.0 * 40.0 + 16.0;
    assert!(island.tray_menu_arrived(&second, menu(2), None, t + ms(400)));
    step(&mut island, t + ms(400));
    step(&mut island, t + ms(450));
    let mid = shown_rect(&island, t + ms(450)).h;
    assert!(
        mid < tall && mid > short,
        "{mid} between {short} and {tall}"
    );
    assert!(window_frame(&island, t + ms(450)).content < 1.0);
    run_frames(&mut island, t + ms(450), t + ms(1000));
    assert!((shown_rect(&island, t + ms(1000)).h - short).abs() < 0.01);
    assert_eq!(window_frame(&island, t + ms(1000)).content, 1.0);
    assert!(!island.sheets.wants_frame(t + ms(1100)));
}

#[test]
fn an_answer_while_the_switch_still_flies_bends_the_travel() {
    let (mut island, t, second) = two_icons_first_shown(4);
    let tall = shown_rect(&island, t).h;
    let at = centre(bar_rect(&island, tray_page::row_key(1)));
    click_at(&mut island, at, BTN_RIGHT, t);
    step(&mut island, t + ms(100));
    assert!(island.tray_menu_arrived(&second, menu(2), None, t + ms(120)));
    step(&mut island, t + ms(120));
    run_frames(&mut island, t + ms(120), t + ms(700));
    let short = 2.0 * 40.0 + 16.0;
    assert!((shown_rect(&island, t + ms(700)).h - short).abs() < 0.01);
    assert!(tall > short);
}
