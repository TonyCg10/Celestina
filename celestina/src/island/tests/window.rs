//! A capsule menu is a window of its own under the bar (CEL-D-009): the bar
//! paints every capsule, the window is a shape apart, a press on the bar goes
//! to the bar, a switch shows two windows while the old one closes and a piece that
//! vanishes takes its window into its last pill.

use celestina_shell::settings::BarLayout;

use super::*;

/// The capsules the bar paints at `now`, as the regions have them.
fn capsules(island: &Island, now: Instant) -> Vec<Shape> {
    island
        .bar
        .as_ref()
        .map(|bar| bar.shapes(now, island.theme.radius_lg))
        .unwrap_or_default()
}

#[test]
fn the_bar_paints_every_capsule_while_a_window_is_open() {
    let (mut island, t0) = level_island();
    let rest = capsules(&island, t0);
    assert!(rest.len() >= 2, "premise: more than one capsule");
    let rest_extent = island.bar.as_ref().and_then(|b| b.capsule_extent(3, t0));
    assert!(open_on(&mut island, volume(), t0));
    for t in [0, 60, 175, 349, 400, 900] {
        let now = t0 + ms(t);
        step(&mut island, now);
        // No zone is left out of the bar's paint or of its regions.
        let shapes = island.shapes(now);
        for capsule in capsules(&island, now) {
            assert!(shapes.contains(&capsule), "+{t} ms: {capsule:?}");
        }
        // The open piece stays in its slot: the capsule keeps its extent.
        let bar = island.bar.as_ref();
        assert_eq!(bar.and_then(|b| b.open_piece()), Some(volume()), "+{t} ms");
        let extent = bar.and_then(|b| b.capsule_extent(3, now));
        assert_eq!(extent, rest_extent, "+{t} ms");
    }
}

#[test]
fn the_window_is_its_own_shape() {
    let (island, t) = settled();
    let radius = island.theme.radius_lg;
    let window = shown_rect(&island, t);
    let end = zone(island.bar.as_ref().and_then(|b| b.capsule_extent(3, t)));
    let shapes = island.shapes(t);
    assert!(shapes.contains(&(end, radius)), "the capsule");
    assert!(shapes.contains(&(window, radius)), "the window");
    assert_ne!(end, window);
    assert_eq!(window.y, 56.0, "the capsule's 8 + 40 + a gap of 8");
    assert_eq!(window.x + window.w, end.x + end.w, "right edges agree");
    // Capsules, the window and the Island's capsule: nothing else.
    assert_eq!(shapes.len(), capsules(&island, t).len() + 2);
}

#[test]
fn a_press_on_another_piece_of_the_capsule_switches() {
    let (mut island, t) = settled();
    let at = centre(bar_rect(&island, brightness_key()));
    // The window does not cover the bar: the press reaches the piece, which
    // asks the app for its sheet (the app answers with the body).
    assert!(!shown_rect(&island, t).contains(at.0, at.1));
    assert_eq!(
        click_at(&mut island, at, BTN_LEFT, t),
        [IslandAction::OpenLevelSheet {
            kind: level_popup::Kind::Brightness,
            piece: brightness_key(),
        }]
    );
    assert!(island.sheets.is_open(), "the window waits for the answer");
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    step(&mut island, t);
    assert!(island.sheets.is_open());
    assert!(
        !island.sheets.has_queued(),
        "a switch, not a close and open"
    );
    assert_eq!(island.sheets.piece(), Some(brightness_key()));
}

#[test]
fn a_press_on_the_glass_of_its_own_capsule_closes() {
    let (mut island, t) = settled();
    let end = zone(island.bar.as_ref().and_then(|b| b.capsule_extent(3, t)));
    // The capsule's padding is empty glass, not a slot: a press there is on
    // no piece.
    let at = (end.x + 1.0, end.y + end.h / 2.0);
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(!island.sheets.is_open());
}

#[test]
fn a_press_on_another_capsule_closes_then_opens() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let at = centre(bar_rect(&island, bell));
    assert!(!island
        .bar
        .as_ref()
        .is_some_and(|b| b.zone_of(bell) == Some(3)));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(!island.sheets.is_open(), "the open window closes");
    assert!(island.sheets.is_shown(t + ms(1)), "on its exit motion");
    assert!(island.sheets.has_queued(), "the other waits for it");
    step(&mut island, t + ms(260));
    assert!(island.sheets.is_open());
    assert_eq!(
        island.sheets.page(),
        Some(PageId::NOTIFICATIONS),
        "the bell's page, from the bell's capsule"
    );
    assert_eq!(island.sheets.piece(), Some(bell));
}

#[test]
fn a_switch_closes_the_old_window_into_its_piece_while_the_new_one_grows() {
    let (mut island, t) = settled();
    let radius = island.theme.radius_lg;
    let bars = capsules(&island, t).len();
    let old = shown_rect(&island, t);
    let old_pill = island.sheet_anchor(t).map(|a| a.pill);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    assert!(island.sheets.is_leaving());
    assert_eq!(
        island.sheets.leaving_pill(),
        old_pill,
        "it closes towards the pill its piece had"
    );
    step(&mut island, t);
    // Two windows for the close: the old one first, the new one after it.
    let both = island.shapes(t + ms(50));
    assert_eq!(both.len(), bars + 3, "capsules, two windows, the Island");
    // The old window closes toward its pill and the new one grows from its
    // own piece's: neither has the other's size at the half.
    let mid = t + ms(75);
    step(&mut island, mid);
    let windows = island
        .sheets
        .windows(island.sheet_anchor(mid).as_ref(), island.size, mid);
    assert_eq!(windows.len(), 2);
    let (folding, growing) = (windows[0].0, windows[1].0);
    assert!(folding.h < old.h, "{folding:?} closes from {old:?}");
    assert!(growing.h < shown_rect(&island, t + ms(400)).h * 1.2);
    // After the close only one window is left, and no region is stale.
    let after = t + ms(260);
    step(&mut island, after);
    assert!(!island.sheets.is_leaving());
    let shapes = island.shapes(after);
    assert_eq!(shapes.len(), bars + 2, "capsules, the window, the Island");
    assert!(shapes.iter().filter(|s| s.1 == radius).count() >= bars);
}

#[test]
fn only_the_new_window_takes_input_during_a_switch() {
    let (mut island, t) = settled();
    let old = shown_rect(&island, t);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    let soon = t + ms(10);
    step(&mut island, soon);
    let windows = island
        .sheets
        .windows(island.sheet_anchor(soon).as_ref(), island.size, soon);
    assert_eq!(windows.len(), 2);
    let (folding, growing) = (windows[0].0, windows[1].0);
    // The old window is still large; the new one has barely begun.
    let at = (folding.x + folding.w / 2.0, folding.y + folding.h / 2.0);
    assert!(folding.contains(at.0, at.1) && !growing.contains(at.0, at.1));
    assert!(old.contains(at.0, at.1));
    // A press on the old window, still painted, takes nothing and does not
    // close the new one.
    assert!(click_at(&mut island, at, BTN_LEFT, soon).is_empty());
    assert!(island.sheets.is_open());
    // One outside both windows still closes it.
    assert!(click_at(&mut island, (30.0, 400.0), BTN_LEFT, soon).is_empty());
    assert!(!island.sheets.is_open());
}

#[test]
fn after_the_close_no_region_of_the_old_window_is_left() {
    let (mut island, t) = settled();
    let bars = capsules(&island, t).len();
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    step(&mut island, t);
    let before_fold_ends = t + ms(190);
    step(&mut island, before_fold_ends);
    assert!(island.sheets.is_leaving());
    assert_eq!(island.shapes(before_fold_ends).len(), bars + 3);
    // Its glass is below 35 % from 199 ms: the blur leaves it, though it
    // still closes.
    let faint = t + ms(259);
    step(&mut island, faint);
    assert!(island.sheets.is_leaving());
    assert_eq!(island.shapes(faint).len(), bars + 2);
    let after = t + ms(260);
    step(&mut island, after);
    assert!(!island.sheets.is_leaving());
    assert_eq!(island.shapes(after).len(), bars + 2);
    // The new window settles on its target; nothing runs at rest.
    let rest = t + ms(900);
    step(&mut island, rest);
    assert!(!island.sheets.wants_frame(rest));
}

#[test]
fn a_narrow_output_keeps_a_start_window_inside() {
    let t0 = Instant::now();
    let mut island = Island::new("DP-1", (520.0, 560.0));
    island.set_bar(&BarLayout {
        start: vec!["notifications".to_owned()],
        centre: Vec::new(),
        end: Vec::new(),
        tray_hidden: Vec::new(),
    });
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t0 = t0 + Duration::from_secs(1);
    let bell = Key::named("bell");
    assert_eq!(
        island.sheet_request(
            bell,
            crate::pieces::SheetKind::Page(PageId::NOTIFICATIONS),
            t0,
            &mut fake_measure
        ),
        None
    );
    for t in [0, 60, 175, 349, 400, 900] {
        let now = t0 + ms(t);
        step(&mut island, now);
        let window = shown_rect(&island, now);
        assert!(
            window.x >= 0.0 && window.x + window.w <= 520.0,
            "+{t} ms: {window:?} stays on the surface"
        );
    }
    let at_rest = shown_rect(&island, t0 + ms(900));
    let capsule = start_zone(&island);
    assert_eq!(
        at_rest.x,
        capsule.x.max(crate::sheet::geometry::SIDE_MARGIN),
        "the left edge rule and the margin agree"
    );
    assert!(at_rest.x + at_rest.w <= 520.0 - crate::sheet::geometry::SIDE_MARGIN + 0.01);
}

#[test]
fn a_vanished_piece_closes_towards_its_last_pill() {
    let (mut island, t) = settled();
    let last = island.sheet_anchor(t).map(|a| a.pill);
    // The layout drops the volume piece while its window is open.
    island.set_bar(&BarLayout {
        start: vec!["notifications".to_owned()],
        centre: Vec::new(),
        end: vec!["brightness".to_owned()],
        tray_hidden: Vec::new(),
    });
    island.apply_views(t);
    step(&mut island, t + ms(1));
    assert!(!island.sheets.is_open(), "it closes");
    assert!(island.sheets.is_shown(t + ms(2)), "on its exit motion");
    // It closes toward the last pill the piece had.
    let mid = shown_rect(&island, t + ms(75));
    let from = shown_rect(&island, t + ms(1));
    assert!(mid.h < from.h);
    let at_end = island.sheets.rect(
        &island.sheet_anchor(t + ms(260)).expect("the last anchor"),
        t + ms(259),
    );
    if let (Some(end), Some(pill)) = (at_end, last) {
        // A close goes 70 % of the way to the piece's pill, never past it:
        // the rect has left the window and is nearer the pill than it was.
        let from = shown_rect(&island, t + ms(1));
        assert!((end.w - pill.w).abs() < (from.w - pill.w).abs());
    }
    // Then nothing stays hidden or painted.
    let later = t + ms(300);
    step(&mut island, later);
    assert!(!island.sheets.is_shown(later));
    assert!(island.sheet_rect(later).is_none());
    assert_eq!(
        island.shapes(later).len(),
        capsules(&island, later).len() + 1
    );
}

#[test]
fn closing_a_scrolled_window_closes_towards_the_pill() {
    let (mut island, t0) = level_island();
    let sent = Rc::new(RefCell::new(Vec::new()));
    let core = sent_core_with(&island.theme, &sent, busy_graph(), true);
    assert!(island.open_level_sheet_at(volume(), core, t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    let window = shown_rect(&island, t);
    assert_eq!(window.h, crate::sheet::geometry::max_height(island.size));
    // Over the rows' empty space the wheel scrolls.
    let at = (window.x + 4.0, window.y + 2.0);
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    // The wheel over the body scrolls it.
    for _ in 0..3 {
        feed(
            &mut island,
            InputEvent::Axis {
                x: at.0,
                y: at.1,
                horizontal: 0.0,
                vertical: 10.0,
            },
            t,
        );
    }
    let scroll = island.sheets.scroll().offset();
    assert!(scroll > 0.0, "premise: it scrolled");
    island.close_sheet(t);
    // The close ends on the live pill.
    let end = t + ms(260);
    step(&mut island, end);
    assert!(island.sheet_rect(end).is_none());
}

#[test]
fn a_second_switch_within_260_ms_keeps_the_first_window_closing() {
    let (mut island, t) = settled();
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    let t1 = t + ms(60);
    step(&mut island, t1);
    let first = island
        .sheets
        .windows(island.sheet_anchor(t1).as_ref(), island.size, t1)[0]
        .0;
    // Back to the volume while the first window still closes.
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(volume(), core, t1));
    let t2 = t1 + ms(20);
    step(&mut island, t2);
    let windows = island
        .sheets
        .windows(island.sheet_anchor(t2).as_ref(), island.size, t2);
    assert_eq!(windows.len(), 3, "two closing and one growing: {windows:?}");
    assert!(windows[0].0.h <= first.h, "the first keeps closing");
    // Each is gone once its own close ends, and no region stays.
    let bars = capsules(&island, t2).len();
    assert_eq!(island.shapes(t2).len(), bars + 4);
    let end = t + ms(400);
    step(&mut island, end);
    assert!(!island.sheets.is_leaving());
    assert_eq!(island.shapes(end).len(), bars + 2);
}

#[test]
fn the_blur_region_of_a_closing_window_follows_its_rect_and_leaves_with_its_glass() {
    let (mut island, t) = settled();
    let bars = capsules(&island, t).len();
    island.close_sheet(t);
    step(&mut island, t + ms(100));
    let mid = t + ms(100);
    let shapes = island.shapes(mid);
    assert_eq!(shapes.len(), bars + 2, "capsules, the window, the Island");
    let window = shown_rect(&island, mid);
    assert!(
        shapes.iter().any(|&(r, _)| r == window),
        "the region is the window's rect as painted"
    );
    // Below 35 % glass (from 199 ms) it is gone, the window still painted.
    let faint = t + ms(230);
    step(&mut island, faint);
    assert!(island.sheets.is_shown(faint));
    assert_eq!(island.shapes(faint).len(), bars + 1);
}

#[test]
fn no_frame_is_asked_once_a_window_has_closed() {
    let (mut island, t) = settled();
    island.close_sheet(t);
    step(&mut island, t + ms(259));
    assert!(island.sheets.wants_frame(t + ms(259)));
    let done = t + ms(260);
    step(&mut island, done);
    assert!(!island.sheets.is_shown(done));
    assert!(!island.sheets.wants_frame(done + ms(1)));
    assert!(!island.sheets.is_leaving());
}

#[test]
fn a_press_on_a_window_still_closing_does_nothing() {
    let (mut island, t) = settled();
    let window = shown_rect(&island, t);
    island.close_sheet(t);
    let now = t + ms(50);
    step(&mut island, now);
    let inside = (window.x + window.w / 2.0, window.y + window.h / 2.0);
    assert!(island
        .sheets
        .rect(&island.sheet_anchor(now).expect("anchor"), now)
        .is_some());
    assert!(click_at(&mut island, inside, BTN_LEFT, now).is_empty());
    assert!(!island.sheets.has_queued());
    assert!(!island.sheets.is_open(), "it did not come back either");
}
