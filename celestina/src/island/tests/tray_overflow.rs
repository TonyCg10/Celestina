//! The tray's hidden-apps chevron and its window (CEL-I-012): a press on the
//! chevron opens a menu window of CEL-D-009 that lists the apps the bar
//! hides; its rows answer like bar icons; the window follows the hidden list
//! and the registered apps.

use celestina_shell::settings::BarLayout;

use celestina_paint::Role;

use super::*;
use crate::{
    pieces::tray::chevron_key,
    sheet::tray_overflow::{card_key, list_size},
    tray::{
        menu::{entry_key, ShellEntry},
        MenuEntry, MenuTree, Status,
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

/// The body of the open window.
fn body(island: &Island) -> &crate::sheet::TrayOverflowBody {
    match island.sheets.overflow_body() {
        Some(b) => b,
        None => panic!("no overflow window open"),
    }
}

/// The window opened, then app `index`'s menu asked for by a right click
/// on its row at `t`.
fn asked(index: usize) -> (Island, Instant) {
    let (mut island, t) = opened();
    let at = centre(row_rect(&island, index, t));
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item, .. }] if *item == item_of(&island, index)),
        "{out:?}"
    );
    (island, t)
}

/// [`asked`] and the app's answer `tree` at once, then `after` ms of frames.
fn unfolded(index: usize, tree: MenuTree, shell: Option<ShellEntry>) -> (Island, Instant) {
    let (mut island, t) = asked(index);
    let item = item_of(&island, index);
    assert!(island.tray_menu_arrived(&item, tree, shell, t));
    step(&mut island, t);
    let settled = t + ms(600);
    run_frames(&mut island, t, settled);
    (island, settled)
}

/// A point of the window that no row covers.
fn blank(island: &Island, t: Instant) -> (f32, f32) {
    let w = shown_rect(island, t);
    (w.x + 2.0, w.y + 2.0)
}

fn app(n: usize, sni: &str) -> TrayRow {
    TrayRow {
        id: ItemId {
            bus_name: format!(":1.{n}"),
            path: "/StatusNotifierItem".to_owned(),
        },
        sni_id: sni.to_owned(),
        title: format!("app {n}"),
        has_menu: true,
        ..TrayRow::default()
    }
}

fn layout(hidden: &[&str]) -> BarLayout {
    BarLayout {
        tray_hidden: hidden.iter().map(|s| (*s).to_owned()).collect(),
        ..BarLayout::default()
    }
}

/// Four registered apps (`a` and `c` hidden, `b` and `d` in the bar) on a
/// 1920 px surface, laid out at rest.
fn overflow_island() -> (Island, Instant) {
    let t0 = Instant::now();
    let mut island = Island::new("DP-1", (1920.0, 560.0));
    island.model = Some(model("DP-1", 1));
    island.rows = rows_for(&model("DP-1", 1), "DP-1");
    island.tray_rows = vec![app(0, "a"), app(1, "b"), app(2, "c"), app(3, "d")];
    island.set_bar(&layout(&["a", "c"]));
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    (island, t0 + Duration::from_secs(1))
}

/// Presses the chevron and lets the window settle.
fn open_window(island: &mut Island, t: Instant) -> Instant {
    let at = centre(bar_rect(island, chevron_key()));
    click_at(island, at, BTN_LEFT, t);
    step(island, t);
    let settled = t + ms(500);
    step(island, settled);
    settled
}

fn opened() -> (Island, Instant) {
    let (mut island, t) = overflow_island();
    let t = open_window(&mut island, t);
    (island, t)
}

/// The rect of the window's row of tray item `index`, in the surface.
fn row_rect(island: &Island, index: usize, t: Instant) -> Rect {
    let window = shown_rect(island, t);
    let key = crate::island::pages::tray::row_key(index);
    match island.sheets.body_node_rect(key) {
        Some(r) => Rect {
            x: window.x + r.x,
            y: window.y + r.y,
            ..r
        },
        None => panic!("no window row for item {index}"),
    }
}

/// The apps the window lists, by their place in the tray's list. A row that
/// left may still be in the tree while it fades out; the list is the truth.
fn listed(island: &Island) -> Vec<usize> {
    island.overflow_rows().into_iter().map(|(i, _)| i).collect()
}

fn window_has_row(island: &Island, index: usize) -> bool {
    listed(island).contains(&index)
        && island
            .sheets
            .body_node_rect(crate::island::pages::tray::row_key(index))
            .is_some()
}

#[test]
fn chevron_opens_the_overflow_window() {
    let (mut island, t) = overflow_island();
    assert!(!island.sheets.is_open());
    let at = centre(bar_rect(&island, chevron_key()));
    click_at(&mut island, at, BTN_LEFT, t);
    assert!(island.sheets.is_open());
    assert_eq!(island.sheets.kind(), Some(SheetKind::TrayOverflow));
    assert_eq!(island.sheets.piece(), Some(chevron_key()));
    // The window grows out of the chevron, which stays in the bar under the
    // open pill.
    let soon = t + ms(100);
    step(&mut island, soon);
    let bar = island.bar.as_ref();
    assert_eq!(bar.and_then(|b| b.open_piece()), Some(chevron_key()));
    let pills = bar.map(|b| b.pill(soon)).unwrap_or_default();
    assert_eq!(pills.len(), 1, "{pills:?}");
    assert_eq!(pills[0].piece, chevron_key());
    assert_eq!(
        bar.and_then(|b| b.pill_of(chevron_key(), soon)),
        Some(pills[0].rect)
    );
    // Pressing the chevron again closes it.
    let later = t + ms(600);
    step(&mut island, later);
    click_at(&mut island, at, BTN_LEFT, later);
    assert!(!island.sheets.is_open());
}

#[test]
fn overflow_window_lists_hidden_apps_only() {
    let (island, t) = opened();
    assert!(window_has_row(&island, 0), "a is hidden");
    assert!(window_has_row(&island, 2), "c is hidden");
    assert!(!window_has_row(&island, 1), "b is in the bar");
    assert!(!window_has_row(&island, 3), "d is in the bar");
    // 280 wide, padding 8, rows of 40 and a gap of 2: no subtitle.
    let window = shown_rect(&island, t);
    assert_eq!((window.w, window.h), (280.0, 8.0 + 40.0 + 2.0 + 40.0 + 8.0));
    let first = row_rect(&island, 0, t);
    assert_eq!((first.x - window.x, first.y - window.y), (8.0, 8.0));
    assert_eq!((first.w, first.h), (264.0, 40.0));
    let second = row_rect(&island, 2, t);
    assert_eq!(second.y - first.y, 42.0);
    assert!(
        island
            .sheets
            .body_node_rect(crate::sheet::subtitle::row_key())
            .is_none(),
        "no subtitle row"
    );
}

#[test]
fn overflow_window_centres_under_the_chevron() {
    let (island, t) = opened();
    let chevron = bar_rect(&island, chevron_key());
    let window = shown_rect(&island, t);
    assert_eq!(window.y, 56.0);
    assert_eq!(window.w, 280.0);
    // Centred under the chevron, unless that would cross the surface's
    // 16 px margin: 280 wide, the window then sits against it.
    let centred = chevron.x + chevron.w / 2.0 - window.w / 2.0;
    let want = centred.min(1920.0 - 16.0 - window.w);
    assert!(
        (window.x - want).abs() < 0.01,
        "under the chevron: {window:?} for {chevron:?}"
    );
    assert!(window.x >= 16.0 && window.x + window.w <= 1920.0 - 16.0);
    // It is the window of CEL-D-009: its glass is in the blur region.
    let centre = (window.x + window.w / 2.0, window.y + window.h / 2.0);
    assert!(island
        .blur_bands(t)
        .iter()
        .any(|b| b.contains(centre.0, centre.1)));
}

#[test]
fn the_chevron_glyph_sits_in_the_centre_of_its_box() {
    let (island, _) = overflow_island();
    let chevron = bar_rect(&island, chevron_key());
    let glyph = bar_rect(&island, Key::named("tray-hidden-icon"));
    assert_eq!((chevron.w, chevron.h), (32.0, 32.0));
    assert!(
        (glyph.x + glyph.w / 2.0 - (chevron.x + chevron.w / 2.0)).abs() < 0.01
            && (glyph.y + glyph.h / 2.0 - (chevron.y + chevron.h / 2.0)).abs() < 0.01,
        "the glyph {glyph:?} turns about the centre of {chevron:?}"
    );
}

#[test]
fn overflow_row_left_click_activates() {
    let (mut island, t) = opened();
    let at = centre(row_rect(&island, 0, t));
    assert_eq!(
        click_at(&mut island, at, BTN_LEFT, t),
        [IslandAction::Tray(TrayCommand::Activate(
            island.tray_rows[0].id.clone()
        ))]
    );
    assert!(!island.sheets.is_open(), "the window closes on the choice");
    // An item that is only a menu opens it from the row.
    let (mut island, t) = opened();
    island.tray_rows[2].item_is_menu = true;
    let at = centre(row_rect(&island, 2, t));
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert!(
        matches!(
            out.as_slice(),
            [IslandAction::AskTrayMenu { item, .. }] if *item == island.tray_rows[2].id
        ),
        "{out:?}"
    );
    // The card hangs from the row, waiting: it holds the menu.
    assert_eq!(island.tray_menus_held(), [island.tray_rows[2].id.clone()]);
    assert!(island.sheets.is_open(), "the window stays under its menu");
}

#[test]
fn overflow_row_right_click_opens_the_app_menu() {
    let (mut island, t) = opened();
    let row = row_rect(&island, 2, t);
    let out = click_at(&mut island, centre(row), BTN_RIGHT, t);
    assert!(
        matches!(
            out.as_slice(),
            [IslandAction::AskTrayMenu { item, .. }] if *item == island.tray_rows[2].id
        ),
        "the row asks for the app's menu: {out:?}"
    );
    assert_eq!(island.tray_menus_held(), [island.tray_rows[2].id.clone()]);
    assert!(island.sheets.is_open());
}

#[test]
fn overflow_row_middle_click_secondary_activates() {
    let (mut island, t) = opened();
    let at = centre(row_rect(&island, 2, t));
    assert_eq!(
        click_at(&mut island, at, BTN_MIDDLE, t),
        [IslandAction::Tray(TrayCommand::SecondaryActivate(
            island.tray_rows[2].id.clone()
        ))]
    );
}

#[test]
fn overflow_row_wheel_scrolls_the_app() {
    let (mut island, t) = opened();
    let (x, y) = centre(row_rect(&island, 0, t));
    feed(&mut island, InputEvent::PointerMotion { x, y }, t);
    // One notch is 15 px; the item's sign is Qt's, the opposite of Wayland's.
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
            island.tray_rows[0].id.clone(),
            -1,
            Orientation::Vertical
        ))]
    );
    assert!(island.sheets.is_open());
}

#[test]
fn attention_marks_the_row_in_the_window() {
    let (mut island, t) = overflow_island();
    island.tray_rows[2].status = Status::NeedsAttention;
    island.invalidate_bar();
    let t = open_window(&mut island, t);
    let _ = t;
    let status = |i| island.sheets.body_node_rect(Key::indexed("tray-status", i));
    assert!(status(2).is_some(), "the row that needs attention says so");
    assert!(status(0).is_none());
}

#[test]
fn last_hidden_app_gone_closes_the_window() {
    let (mut island, t) = opened();
    // One of two unregisters: the row leaves, the window stays.
    island.tray_rows.remove(2);
    island.stale = true;
    island.invalidate_bar();
    step(&mut island, t + ms(10));
    assert!(island.sheets.is_open());
    assert!(window_has_row(&island, 0));
    assert!(!window_has_row(&island, 2));
    // The window shrinks to one row.
    step(&mut island, t + ms(500));
    assert_eq!(shown_rect(&island, t + ms(500)).h, 8.0 + 40.0 + 8.0);
    // The last one: the window closes and the chevron leaves the bar.
    island.tray_rows.remove(0);
    island.stale = true;
    island.invalidate_bar();
    step(&mut island, t + ms(20));
    assert!(!island.sheets.is_open());
    step(&mut island, t + ms(900));
    assert!(
        island.sheets.zone(t + ms(900)).is_none(),
        "the window is gone"
    );
    assert!(island
        .bar
        .as_ref()
        .and_then(|b| b.piece_rect(chevron_key()))
        .is_none());
}

#[test]
fn a_hand_edit_of_the_hidden_list_updates_the_open_window() {
    let (mut island, t) = opened();
    // `shell.json` now hides b as well: the window gains its row.
    island.set_bar(&layout(&["a", "b", "c"]));
    step(&mut island, t + ms(10));
    assert!(window_has_row(&island, 1));
    assert_eq!(
        shown_rect(&island, t + ms(10)).h,
        8.0 + 3.0 * 40.0 + 2.0 * 2.0 + 8.0
    );
    // Showing a and c leaves only b.
    island.set_bar(&layout(&["b"]));
    step(&mut island, t + ms(20));
    assert!(island.sheets.is_open());
    assert!(window_has_row(&island, 1));
    assert!(!window_has_row(&island, 0) && !window_has_row(&island, 2));
}

#[test]
fn showing_the_last_hidden_app_closes_the_window_and_the_chevron_goes() {
    let (mut island, t) = opened();
    // The shell's entry in the window's menu: the layout comes back with the
    // id removed, as the app's relayout hands it.
    island.set_bar(&layout(&["c"]));
    step(&mut island, t + ms(10));
    assert!(island.sheets.is_open(), "c is still hidden");
    assert!(!window_has_row(&island, 0), "a left the window");
    island.set_bar(&layout(&[]));
    step(&mut island, t + ms(20));
    assert!(!island.sheets.is_open());
    step(&mut island, t + ms(900));
    assert!(island
        .bar
        .as_ref()
        .and_then(|b| b.piece_rect(chevron_key()))
        .is_none());
}

#[test]
fn the_window_lives_on_the_output_whose_chevron_was_pressed() {
    let (mut a, t) = overflow_island();
    let mut b = Island::new("DP-2", (1920.0, 560.0));
    b.model = Some(model("DP-2", 1));
    b.rows = rows_for(&model("DP-2", 1), "DP-2");
    b.tray_rows = a.tray_rows.clone();
    b.set_bar(&layout(&["a", "c"]));
    b.apply_views(t);
    lay_fake(&mut b, t);
    let t = open_window(&mut a, t);
    assert!(a.sheets.is_open());
    assert!(!b.sheets.is_open(), "the other output has no window");
    assert!(b
        .bar
        .as_ref()
        .and_then(|x| x.node_rect(chevron_key()))
        .is_some());
    // Hiding on one output reaches the other through the shared layout.
    for island in [&mut a, &mut b] {
        island.set_bar(&layout(&["a", "b", "c"]));
        step(island, t + ms(10));
    }
    assert!(window_has_row(&a, 1));
    assert!(!b.sheets.is_open());
}

#[test]
fn the_glyph_turns_a_half_turn_on_the_open_spring_and_flips_under_reduced_motion() {
    let glyph = Key::named("tray-hidden-icon");
    let (mut island, t) = overflow_island();
    let at = centre(bar_rect(&island, chevron_key()));
    click_at(&mut island, at, BTN_LEFT, t);
    step(&mut island, t);
    let turn =
        |island: &Island, at: Instant| island.bar.as_ref().and_then(|b| b.rotation_of(glyph, at));
    assert_eq!(turn(&island, t), Some(0.0), "it starts upright");
    step(&mut island, t + ms(100));
    let mid = turn(&island, t + ms(100)).unwrap_or(-1.0);
    assert!(mid > 0.0 && mid < 180.0, "turning: {mid}");
    step(&mut island, t + ms(400));
    assert_eq!(turn(&island, t + ms(400)), Some(180.0));
    // Closing turns it back.
    click_at(&mut island, at, BTN_LEFT, t + ms(500));
    step(&mut island, t + ms(520));
    step(&mut island, t + ms(600));
    assert!(turn(&island, t + ms(600)).is_some_and(|a| a < 180.0));
    step(&mut island, t + ms(1000));
    assert_eq!(turn(&island, t + ms(1000)), Some(0.0));
    // Reduced motion: the glyph flips at once.
    let (mut island, t) = overflow_island();
    island.set_reduced_motion(true);
    let at = centre(bar_rect(&island, chevron_key()));
    click_at(&mut island, at, BTN_LEFT, t);
    step(&mut island, t);
    assert_eq!(turn(&island, t), Some(180.0));
}

#[test]
fn the_chevron_leaving_and_entering_slides_the_bar_on_its_spring() {
    let (mut island, t) = overflow_island();
    // The last hidden app is shown again: the chevron leaves its slot.
    island.set_bar(&layout(&[]));
    step(&mut island, t + ms(10));
    let sliding = |island: &Island, at| island.bar.as_ref().is_some_and(|b| b.is_sliding(3, at));
    assert!(
        sliding(&island, t + ms(60)),
        "the capsule glides on the spring"
    );
    step(&mut island, t + ms(1000));
    assert!(!sliding(&island, t + ms(1000)));
    // And hidden again: it comes back the same way.
    island.set_bar(&layout(&["a", "c"]));
    step(&mut island, t + ms(1010));
    assert!(sliding(&island, t + ms(1060)));
}

#[test]
fn the_turn_asks_for_no_frames_at_rest() {
    let (mut island, t) = opened();
    // The turn ran for 350 ms; long after it nothing moves.
    let later = t + ms(2000);
    step(&mut island, later);
    let bar_moving = island.bar.as_ref().is_some_and(|b| b.is_animating(later));
    assert!(!bar_moving);
    assert!(!island.sheets.wants_frame(later));
    // While it turns, the bar asks for frames.
    let (mut island, t) = overflow_island();
    let at = centre(bar_rect(&island, chevron_key()));
    click_at(&mut island, at, BTN_LEFT, t);
    step(&mut island, t + ms(50));
    assert!(island
        .bar
        .as_ref()
        .is_some_and(|b| b.is_animating(t + ms(50))));
}

#[test]
fn a_right_click_on_a_row_presses_it_and_asks_for_the_menu() {
    let (mut island, t) = opened();
    let before = shown_rect(&island, t);
    let item = item_of(&island, 2);
    let at = centre(row_rect(&island, 2, t));
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert_eq!(
        out,
        [IslandAction::AskTrayMenu {
            item: item.clone(),
            at: output_point(at),
        }]
    );
    // Away from the row, it still shows its press fill.
    let away = blank(&island, t);
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: away.0,
            y: away.1,
        },
        t,
    );
    step(&mut island, t + ms(10));
    assert_eq!(body(&island).row_fill(2), Some(Role::ControlHover));
    assert_eq!(body(&island).row_fill(0), None);
    // Nothing else changes until the app answers.
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    assert!(island.sheets.tray_menu_waiting(&item));
    step(&mut island, t + ms(500));
    assert_eq!(shown_rect(&island, t + ms(500)), before);
    assert!(body(&island).card_box().is_none());
    // The rows blur once the card is open, not while it waits.
    assert_eq!(body(&island).blur_of(0, t + ms(300)), (0.0, 1.0));
    assert_eq!(body(&island).blur_of(1, t + ms(300)), (0.0, 1.0));
}

#[test]
fn the_layout_unfolds_the_card_under_the_row_and_blurs_the_others() {
    let (mut island, t) = asked(2);
    let item = item_of(&island, 2);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t));
    assert!(island.sheets.tray_menu_shown(&item));
    run_frames(&mut island, t, t + ms(700));
    let done = t + ms(700);
    let card = island
        .sheets
        .body_node_rect(card_key())
        .unwrap_or_else(|| panic!("no card"));
    let row = island
        .sheets
        .body_node_rect(crate::island::pages::tray::row_key(2))
        .unwrap_or_else(|| panic!("no row"));
    // 264 wide, 2 px under the open row, 3 rows and 4 px of padding over and
    // under them.
    assert_eq!((card.x, card.w, card.h), (8.0, 264.0, 3.0 * 40.0 + 8.0));
    assert_eq!(card.y, row.y + row.h + 2.0);
    // The window is the list, the gap, the card and 4 px below the last row.
    let want = 98.0 + 2.0 + card.h + 4.0;
    assert_eq!(shown_rect(&island, done).h, want);
    // The other row is one place away: 1.5 px and 85 % opacity.
    let (sigma, opacity) = body(&island).blur_of(0, done);
    assert!((sigma - 1.5).abs() < 0.01 && (opacity - 0.85).abs() < 0.01);
    assert_eq!(
        body(&island).blur_of(1, done).0,
        0.0,
        "the open row is sharp"
    );
    // Nothing is asked for at rest.
    assert!(!island.sheets.wants_frame(done + ms(10)));
}

#[test]
fn the_window_glass_follows_the_card_every_frame() {
    let (mut island, t) = asked(2);
    let item = item_of(&island, 2);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t));
    let mut grew = false;
    let mut last = shown_rect(&island, t).h;
    for n in [0u64, 16, 50, 100, 150, 200, 275, 350] {
        let now = t + ms(n);
        step(&mut island, now);
        let h = shown_rect(&island, now).h;
        let body_h = island.sheets.body_size().map_or(-1.0, |s| s.1);
        assert!(
            (h - body_h).abs() < 0.01,
            "{n} ms: window {h} body {body_h}"
        );
        grew |= h > last + 0.5;
        last = h;
    }
    assert!(grew, "the window moved with the card");
    // Folding goes down the same way.
    let at = centre(row_rect(&island, 2, t + ms(400)));
    step(&mut island, t + ms(400));
    click_at(&mut island, at, BTN_RIGHT, t + ms(400));
    for n in [416u64, 450, 500, 600, 700, 750] {
        let now = t + ms(n);
        step(&mut island, now);
        let h = shown_rect(&island, now).h;
        let body_h = island.sheets.body_size().map_or(-1.0, |s| s.1);
        assert!(
            (h - body_h).abs() < 0.01,
            "{n} ms: window {h} body {body_h}"
        );
    }
    assert_eq!(shown_rect(&island, t + ms(900)).h, 98.0);
}

#[test]
fn the_open_row_is_grey_and_semibold() {
    use celestina_paint::Weight;
    let (mut island, t) = asked(2);
    // Waiting: grey, still regular.
    assert_eq!(body(&island).title_weight_of(2), Some(Weight::Regular));
    let item = item_of(&island, 2);
    assert!(island.tray_menu_arrived(&item, menu(2), None, t));
    step(&mut island, t + ms(20));
    let away = blank(&island, t);
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: away.0,
            y: away.1,
        },
        t + ms(20),
    );
    assert_eq!(body(&island).row_fill(2), Some(Role::ControlHover));
    assert_eq!(body(&island).title_weight_of(2), Some(Weight::Semibold));
    assert_eq!(body(&island).title_weight_of(0), Some(Weight::Regular));
}

#[test]
fn the_same_row_again_folds_the_card() {
    let (mut island, t) = unfolded(2, menu(3), None);
    let item = item_of(&island, 2);
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    let at = centre(row_rect(&island, 2, t));
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(out.is_empty(), "no second ask: {out:?}");
    assert!(island.tray_menus_held().is_empty(), "the item is let go");
    assert!(island.sheets.is_open(), "the window stays");
    // It folds over 350 ms; the rows sharpen from the start.
    run_frames(&mut island, t, t + ms(120));
    let mid = body(&island).card_box().map_or(0.0, |r| r.h);
    assert!(mid > 1.0 && mid < 128.0, "folding: {mid}");
    assert!(body(&island).blur_of(0, t + ms(120)).0 < 1.5);
    run_frames(&mut island, t + ms(120), t + ms(800));
    assert!(!body(&island).has_card(), "the card is gone");
    assert_eq!(body(&island).blur_of(0, t + ms(800)).0, 0.0);
    assert_eq!(shown_rect(&island, t + ms(800)).h, 98.0);
    assert!(!island.sheets.wants_frame(t + ms(900)));
}

#[test]
fn another_rows_menu_removes_the_first_card_at_once() {
    let (mut island, t) = unfolded(2, menu(3), None);
    let first = item_of(&island, 2);
    let second = item_of(&island, 0);
    let at = centre(row_rect(&island, 0, t));
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item, .. }] if *item == second),
        "{out:?}"
    );
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&second));
    assert_ne!(first, second);
    step(&mut island, t + ms(10));
    assert!(
        body(&island).card_box().is_none(),
        "the first card left at once, with no fold"
    );
    // Also while the first is still waiting.
    let (mut island, t) = asked(2);
    let first = item_of(&island, 2);
    let second = item_of(&island, 0);
    let at = centre(row_rect(&island, 0, t));
    click_at(&mut island, at, BTN_RIGHT, t);
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&second));
    // The first one's late answer is nobody's.
    assert!(!island.tray_menu_arrived(&first, menu(1), None, t + ms(50)));
    assert!(island.tray_menu_arrived(&second, menu(1), None, t + ms(60)));
}

#[test]
fn escape_goes_back_then_folds_then_closes_the_window() {
    let (mut island, t) = unfolded(2, with_submenu(), None);
    let item = item_of(&island, 2);
    assert!(island.sheets.body_takes_escape());
    // Down to "More" and Right: the submenu opens (the app is asked).
    feed(&mut island, key(0xff54), t);
    let out = feed(&mut island, key(0xff53), t);
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::AboutToShow(
            item.clone(),
            1
        ))]
    );
    assert_eq!(body(&island).card_depth(), 1);
    // Escape: back one level.
    feed(&mut island, escape(), t);
    assert_eq!(body(&island).card_depth(), 0);
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    // Escape: the card folds, the window stays.
    feed(&mut island, escape(), t);
    assert!(island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
    assert!(!island.sheets.body_takes_escape());
    // Escape: the window closes.
    feed(&mut island, escape(), t);
    assert!(!island.sheets.is_open());
    // A card still waiting folds on Escape too.
    let (mut island, t) = asked(2);
    feed(&mut island, escape(), t);
    assert!(island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
    feed(&mut island, escape(), t);
    assert!(!island.sheets.is_open());
}

#[test]
fn choosing_in_a_card_closes_the_whole_window() {
    let (mut island, t) = unfolded(2, menu(3), None);
    let item = item_of(&island, 2);
    let window = shown_rect(&island, t);
    let rect = body(&island)
        .card_row_rect(entry_key(1))
        .unwrap_or_else(|| panic!("no entry"));
    let at = centre(Rect {
        x: window.x + rect.x,
        y: window.y + rect.y,
        ..rect
    });
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert_eq!(out, [IslandAction::Tray(TrayCommand::MenuEvent(item, 2))]);
    assert!(!island.sheets.is_open());
    assert!(island.tray_menus_held().is_empty());
}

#[test]
fn the_shell_entry_in_a_card_shows_the_app_and_closes_the_window() {
    // c is hidden: its card offers to show it.
    let (mut island, t) = unfolded(2, menu(1), Some(ShellEntry::Show));
    let item = item_of(&island, 2);
    let window = shown_rect(&island, t);
    // An entry, the line and the shell's entry.
    let rect = body(&island)
        .card_row_rect(entry_key(2))
        .unwrap_or_else(|| panic!("no shell entry"));
    let at = centre(Rect {
        x: window.x + rect.x,
        y: window.y + rect.y,
        ..rect
    });
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert_eq!(out, [IslandAction::TrayShellEntry(item, ShellEntry::Show)]);
    assert!(!island.sheets.is_open(), "the whole window closes");
    assert!(island.tray_menus_held().is_empty());
}

#[test]
fn a_card_whose_app_leaves_goes_at_once_and_the_window_stays() {
    let (mut island, t) = unfolded(2, menu(3), None);
    assert_eq!(island.tray_menus_held().len(), 1);
    island.tray_rows.remove(2);
    island.stale = true;
    island.invalidate_bar();
    step(&mut island, t + ms(10));
    assert!(island.sheets.is_open(), "the window stays");
    assert!(island.tray_menus_held().is_empty());
    assert!(!body(&island).has_card());
    // The other row sharpens over 250 ms.
    assert_eq!(body(&island).blur_of(0, t + ms(300)).0, 0.0);
    step(&mut island, t + ms(600));
    assert_eq!(shown_rect(&island, t + ms(600)).h, 8.0 + 40.0 + 8.0);
}

#[test]
fn the_wheel_over_the_card_scrolls_the_card_and_over_a_row_scrolls_the_app() {
    // A card of 30 entries scrolls under the wheel.
    let (mut island, t) = unfolded(2, menu(30), None);
    assert!(body(&island).card_can_scroll());
    let window = shown_rect(&island, t);
    let card = body(&island)
        .card_box()
        .unwrap_or_else(|| panic!("no card"));
    let (x, y) = (window.x + card.x + 100.0, window.y + card.y + 60.0);
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
    assert!(island.sheets.wants_frame(t), "the card's scroll bar shows");
    assert_eq!(island.sheets.scroll().offset(), 0.0, "nor the sheet");
    // Over a row the app scrolls as ever.
    let (x, y) = centre(row_rect(&island, 0, t));
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
fn the_menu_key_toggles_the_card_of_the_focused_row_and_focuses_its_first_entry() {
    let (mut island, t) = opened();
    // Tab focuses the first row; the Menu key asks for its menu.
    feed(&mut island, key(0xff09), t);
    let out = feed(&mut island, key(0xff67), t);
    let item = item_of(&island, 0);
    assert!(
        matches!(out.as_slice(), [IslandAction::AskTrayMenu { item: i, .. }] if *i == item),
        "{out:?}"
    );
    assert!(island.tray_menu_arrived(&item, menu(3), None, t + ms(10)));
    run_frames(&mut island, t + ms(10), t + ms(600));
    assert!(
        body(&island).card_focus().is_some(),
        "the first entry has the focus"
    );
    // Enter chooses it.
    let out = feed(&mut island, key(0xff0d), t + ms(620));
    assert_eq!(
        out,
        [IslandAction::Tray(TrayCommand::MenuEvent(item.clone(), 1))]
    );
    // The Menu key again on an open card folds it.
    let (mut island, t) = opened();
    feed(&mut island, key(0xff09), t);
    feed(&mut island, key(0xff67), t);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t + ms(10)));
    run_frames(&mut island, t + ms(10), t + ms(600));
    // The pane has the keys: the list's focus is still on the row, so the
    // window's Menu key reads it.
    let out = feed(&mut island, key(0xff67), t + ms(620));
    assert!(out.is_empty(), "{out:?}");
    assert!(island.tray_menus_held().is_empty());
}

#[test]
fn a_late_answer_for_a_folded_card_opens_nothing() {
    let (mut island, t) = asked(2);
    let item = item_of(&island, 2);
    let at = centre(row_rect(&island, 2, t));
    click_at(&mut island, at, BTN_RIGHT, t);
    assert!(island.tray_menus_held().is_empty());
    assert!(!island.tray_menu_arrived(&item, menu(2), None, t + ms(30)));
    step(&mut island, t + ms(40));
    assert!(!body(&island).has_card());
    // The list's size is what it was.
    assert_eq!(island.sheets.body_size(), Some(list_size(&island.theme, 2)));
}

#[test]
fn reduced_motion_unfolds_and_folds_the_card_and_the_rows_at_once() {
    let (mut island, t) = overflow_island();
    island.set_reduced_motion(true);
    let t = open_window(&mut island, t);
    let at = centre(row_rect(&island, 2, t));
    click_at(&mut island, at, BTN_RIGHT, t);
    let item = item_of(&island, 2);
    assert!(island.tray_menu_arrived(&item, menu(3), None, t));
    step(&mut island, t);
    let h = shown_rect(&island, t).h;
    assert_eq!(h, 98.0 + 2.0 + 128.0 + 4.0, "the card is there at once");
    assert_eq!(body(&island).blur_of(0, t).0, 1.5, "and the rows blurred");
    assert!(!island.sheets.wants_frame(t + ms(1)));
    let at = centre(row_rect(&island, 2, t));
    click_at(&mut island, at, BTN_RIGHT, t + ms(5));
    step(&mut island, t + ms(5));
    assert!(!body(&island).has_card(), "folded at once");
    assert_eq!(body(&island).blur_of(0, t + ms(5)).0, 0.0);
}

/// Five registered apps, `a`, `b` and `c` hidden (places 0, 1 and 2 of the
/// tray's list) and `d` and `e` in the bar, the window open.
fn crowded() -> (Island, Instant) {
    let t0 = Instant::now();
    let mut island = Island::new("DP-1", (1920.0, 560.0));
    island.model = Some(model("DP-1", 1));
    island.rows = rows_for(&model("DP-1", 1), "DP-1");
    island.tray_rows = vec![
        app(0, "a"),
        app(1, "b"),
        app(2, "c"),
        app(3, "d"),
        app(4, "e"),
    ];
    island.set_bar(&layout(&["a", "b", "c"]));
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = open_window(&mut island, t0 + Duration::from_secs(1));
    (island, t)
}

/// App `index`'s menu asked for in `island` by a right click at `t`, then
/// answered with `tree` and unfolded for 700 ms.
fn unfolded_in(island: &mut Island, t: Instant, index: usize, tree: MenuTree) -> Instant {
    let at = centre(row_rect(island, index, t));
    click_at(island, at, BTN_RIGHT, t);
    let item = item_of(island, index);
    assert!(island.tray_menu_arrived(&item, tree, None, t));
    run_frames(island, t, t + ms(700));
    t + ms(700)
}

#[test]
fn an_app_before_the_open_one_leaving_keeps_the_card_under_its_app() {
    let (mut island, t) = crowded();
    let t = unfolded_in(&mut island, t, 2, menu(3));
    let item = item_of(&island, 2);
    // `a` quits: `b` and `c` are places 0 and 1 now.
    island.tray_rows.remove(0);
    island.stale = true;
    island.invalidate_bar();
    step(&mut island, t + ms(10));
    assert_eq!(island.tray_menus_held(), std::slice::from_ref(&item));
    assert!(body(&island).has_card(), "the card stays");
    let row = island
        .sheets
        .body_node_rect(crate::island::pages::tray::row_key(1))
        .unwrap_or_else(|| panic!("no row"));
    let card = island
        .sheets
        .body_node_rect(card_key())
        .unwrap_or_else(|| panic!("no card"));
    assert_eq!(card.y, row.y + row.h + 2.0, "under its app's row");
    // `b` and `c` are blurred by their distance from the open row.
    assert_eq!(body(&island).blur_of(1, t + ms(500)).0, 0.0);
    assert!((body(&island).blur_of(0, t + ms(500)).0 - 1.5).abs() < 0.01);
}

#[test]
fn the_open_app_leaving_while_the_next_one_is_hidden_too_takes_the_card_at_once() {
    let (mut island, t) = crowded();
    let t = unfolded_in(&mut island, t, 0, menu(3));
    assert_eq!(island.tray_menus_held().len(), 1);
    // `a` quits: `b` is place 0 now, under the index the card hung from.
    island.tray_rows.remove(0);
    island.stale = true;
    island.invalidate_bar();
    step(&mut island, t + ms(10));
    assert!(island.sheets.is_open(), "the window stays");
    assert!(island.tray_menus_held().is_empty(), "one held item fewer");
    assert!(!body(&island).has_card());
    assert!(body(&island).card_box().is_none());
}

#[test]
fn rows_added_under_an_open_card_shrink_it_to_the_room_left() {
    let (mut island, t) = overflow_island();
    let t = open_window(&mut island, t);
    // A menu too tall for the room: the card takes all that is left.
    let t = unfolded_in(&mut island, t, 0, menu(40));
    let before = body(&island)
        .card_box()
        .unwrap_or_else(|| panic!("no card"))
        .h;
    assert!(body(&island).card_can_scroll());
    // `b` is hidden too: its row takes 42 px of the room.
    island.set_bar(&layout(&["a", "b", "c"]));
    run_frames(&mut island, t, t + ms(400));
    let after = body(&island)
        .card_box()
        .unwrap_or_else(|| panic!("no card"))
        .h;
    assert!((before - after - 42.0).abs() < 0.01, "{before} -> {after}");
    let window = shown_rect(&island, t + ms(400));
    assert!(window.h <= 496.0 + 0.01, "{}", window.h);
}
