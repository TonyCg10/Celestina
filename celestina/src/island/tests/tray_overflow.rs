//! The tray's hidden-apps chevron and its window (CEL-I-012): a press on the
//! chevron opens a menu window of CEL-D-009 that lists the apps the bar
//! hides; its rows answer like bar icons; the window follows the hidden list
//! and the registered apps.

use celestina_shell::settings::BarLayout;

use super::*;
use crate::{pieces::tray::chevron_key, tray::Status};

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
    // The window grows out of the chevron, which does not leave the bar.
    step(&mut island, t + ms(100));
    assert_eq!(island.bar.as_ref().and_then(|b| b.lifted()), None);
    assert!(island
        .bar
        .as_ref()
        .and_then(|b| b.fade_of(chevron_key(), t + ms(100)))
        .is_none());
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
    // 240 wide, padding 8, rows of 40 and a gap of 2: no subtitle.
    let window = shown_rect(&island, t);
    assert_eq!((window.w, window.h), (240.0, 8.0 + 40.0 + 2.0 + 40.0 + 8.0));
    let first = row_rect(&island, 0, t);
    assert_eq!((first.x - window.x, first.y - window.y), (8.0, 8.0));
    assert_eq!((first.w, first.h), (224.0, 40.0));
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
    assert_eq!(window.w, 240.0);
    assert!(
        (window.x + window.w / 2.0 - (chevron.x + chevron.w / 2.0)).abs() < 0.01,
        "centred under the chevron: {window:?} for {chevron:?}"
    );
    assert!(window.x >= 16.0 && window.x + window.w <= 1920.0 - 16.0);
    // It is the window of CEL-D-009: its own shape beside the capsules.
    assert!(island.shapes(t).contains(&(window, island.theme.radius_lg)));
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
    let row = row_rect(&island, 2, t);
    let out = click_at(&mut island, at, BTN_LEFT, t);
    assert!(
        matches!(
            out.as_slice(),
            [IslandAction::OpenTrayMenu { item, row: r, keyboard: false, .. }]
                if *item == island.tray_rows[2].id && *r == row
        ),
        "{out:?}"
    );
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
            [IslandAction::OpenTrayMenu { item, row: r, keyboard: false, .. }]
                if *item == island.tray_rows[2].id && *r == row
        ),
        "the card hangs from the row: {out:?}"
    );
    assert!(island.bar_menu_asked, "the menu may open from the window");
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
fn escape_closes_the_apps_menu_first_then_the_window() {
    let (mut island, t) = opened();
    let row = row_rect(&island, 0, t);
    click_at(&mut island, centre(row), BTN_RIGHT, t);
    // The app's card opened from the row (the app's answer).
    island.tray_menu = Some(PopupId(5));
    island.tray_menu_on_bar = true;
    island.bar_menu_asked = false;
    // The popup has the keyboard: its Escape closes that level only.
    assert!(island.menu_closed(PopupId(5), CloseReason::Shell, t));
    assert!(island.sheets.is_open(), "the window stays");
    // The next Escape is the window's.
    feed(&mut island, escape(), t);
    assert!(!island.sheets.is_open());
    // A click outside the shell (the compositor dismisses the card) closes
    // the window as it closes a page.
    let (mut island, t) = opened();
    island.tray_menu = Some(PopupId(6));
    island.tray_menu_on_bar = true;
    assert!(island.menu_closed(PopupId(6), CloseReason::Dismissed, t));
    assert!(!island.sheets.is_open());
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
fn a_card_hung_from_a_window_row_goes_with_the_window() {
    let (mut island, t) = opened();
    let row = row_rect(&island, 0, t);
    click_at(&mut island, centre(row), BTN_RIGHT, t);
    assert!(
        island.overflow_menu_asked.is_some(),
        "asked from the window"
    );
    // The app's card opens from the row (the app's answer).
    island.tray_menu = Some(PopupId(5));
    island.tray_menu_on_bar = true;
    island.tray_menu_from_window = island.overflow_menu_asked.take();
    assert!(!island.tray_card_orphaned(), "the window is open");
    // The last hidden app unregisters: the window closes and the card is
    // left hanging, so it is due to close.
    island.tray_rows.clear();
    island.stale = true;
    island.invalidate_bar();
    step(&mut island, t + ms(10));
    assert!(!island.sheets.is_open());
    assert!(island.tray_card_orphaned());
    // A card of a bar icon is not tied to any window.
    let (mut island, t) = opened();
    island.tray_menu = Some(PopupId(6));
    island.tray_menu_on_bar = true;
    island.tray_menu_from_window = None;
    island.sheets.drop_open(t);
    assert!(!island.tray_card_orphaned());
}

#[test]
fn a_card_goes_when_its_app_is_shown_while_the_window_stays() {
    let (mut island, t) = opened();
    let row = row_rect(&island, 0, t);
    click_at(&mut island, centre(row), BTN_RIGHT, t);
    island.tray_menu = Some(PopupId(7));
    island.tray_menu_on_bar = true;
    island.tray_menu_from_window = island.overflow_menu_asked.take();
    assert!(!island.tray_card_orphaned());
    // `a` is shown from elsewhere; `c` keeps the window open.
    island.set_bar(&layout(&["c"]));
    step(&mut island, t + ms(10));
    assert!(island.sheets.is_open(), "the window stays for `c`");
    assert!(island.tray_card_orphaned(), "the card of `a` goes");
}
