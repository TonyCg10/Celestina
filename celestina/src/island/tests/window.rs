//! A capsule menu is a window of its own under the bar (CEL-D-009): the bar
//! paints every capsule, the window is a shape apart, a press on the bar goes
//! to the bar, a switch on the same capsule keeps one window that travels to the
//! new menu's shape (CEL-D-015), a menu on another capsule opens at once while
//! the old one closes, and a piece that vanishes takes its window into its last
//! pill.

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

/// Whether any band covers `at`.
fn covers(bands: &[Rect], at: (f32, f32)) -> bool {
    bands.iter().any(|b| b.contains(at.0, at.1))
}

fn centre_of(r: Rect) -> (f32, f32) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// The drawn windows at `now` with their radii, from the glass the sheets
/// hand the Island's paths.
fn windows(island: &Island, now: Instant) -> Vec<(Rect, f32)> {
    let anchor = island.sheet_anchor(now);
    island
        .sheets
        .glass(anchor.as_ref(), island.size, now)
        .iter()
        .filter(|g| g.frame.drawn)
        .map(|g| (g.frame.rect, g.frame.radius))
        .collect()
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
    assert!(!shapes.contains(&(window, radius)), "its glass is a path");
    assert!(
        covers(&island.blur_bands(t), centre_of(window)),
        "the window"
    );
    assert_ne!(end, window);
    assert_eq!(window.y, 56.0, "the capsule's 8 + 40 + a gap of 8");
    assert_eq!(window.x + window.w, end.x + end.w, "right edges agree");
    // Capsules and the Island's capsule: nothing else.
    assert_eq!(shapes.len(), capsules(&island, t).len() + 1);
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
fn a_press_on_another_capsule_opens_its_menu_at_once_while_the_old_closes() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let at = centre(bar_rect(&island, bell));
    assert!(!island
        .bar
        .as_ref()
        .is_some_and(|b| b.zone_of(bell) == Some(3)));
    let old_pill = island.sheet_anchor(t).map(|a| a.pill);
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(island.sheets.is_open(), "the other menu is open at once");
    assert_eq!(island.sheets.piece(), Some(bell));
    assert_eq!(
        island.sheets.page(),
        Some(PageId::NOTIFICATIONS),
        "the bell's page, from the bell's capsule"
    );
    assert!(!island.sheets.has_queued(), "nothing waits");
    assert!(island.sheets.is_leaving(), "the old one closes meanwhile");
    assert_eq!(
        island.sheets.leaving_pill(),
        old_pill,
        "towards the pill its piece had"
    );
    // The old one gathers as a window until 42 % of its close (168 ms); the
    // new one has none until 52 % of its open (239 ms): each drop's liquid
    // alone is no window.
    for (n, want) in [(50, 1), (150, 1), (200, 0), (250, 1)] {
        let at = t + ms(n);
        step(&mut island, at);
        let got = windows(&island, at);
        assert_eq!(got.len(), want, "+{n} ms");
        assert!(island.sheets.is_leaving(), "+{n} ms: the old one closes");
    }
    let after = t + ms(400);
    step(&mut island, after);
    assert!(!island.sheets.is_leaving());
}

#[test]
fn a_switch_on_the_same_capsule_keeps_one_window_that_travels() {
    let (mut island, t) = settled();
    let radius = island.theme.radius_lg;
    let bars = capsules(&island, t).len();
    let old = shown_rect(&island, t);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    assert!(!island.sheets.is_leaving(), "no second window");
    for n in [0, 50, 150, 299] {
        let now = t + ms(n);
        step(&mut island, now);
        let windows = windows(&island, now);
        assert_eq!(windows.len(), 1, "+{n} ms");
        assert_eq!(island.shapes(now).len(), bars + 1, "+{n} ms");
        if n == 0 {
            assert!(near(windows[0].0, old), "starts where it was shown");
        }
    }
    let end = t + ms(300);
    step(&mut island, end);
    let at_end = shown_rect(&island, end);
    let rest = t + ms(900);
    step(&mut island, rest);
    assert!(near(at_end, shown_rect(&island, rest)), "the target");
    let windows = windows(&island, end);
    assert_eq!(windows[0].1, radius);
}

#[test]
fn the_old_content_is_released_at_90_ms_and_nothing_runs_at_rest() {
    let (mut island, t) = settled();
    let bars = capsules(&island, t).len();
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    step(&mut island, t);
    assert!(island.sheets.is_swapping());
    let before = t + ms(89);
    step(&mut island, before);
    assert!(island.sheets.is_swapping(), "+89 ms");
    assert_eq!(island.shapes(before).len(), bars + 1);
    let gone = t + ms(90);
    step(&mut island, gone);
    assert!(!island.sheets.is_swapping(), "+90 ms");
    assert_eq!(island.shapes(gone).len(), bars + 1);
    let last = t + ms(299);
    step(&mut island, last);
    assert!(island.sheets.wants_frame(last));
    assert_eq!(island.shapes(last).len(), bars + 1);
    let rest = t + ms(300);
    step(&mut island, rest);
    assert!(!island.sheets.wants_frame(rest));
    assert_eq!(island.shapes(rest).len(), bars + 1);
}

#[test]
fn a_second_switch_during_a_travel_travels_from_the_shape_shown() {
    let (mut island, t) = settled();
    let volume_rect = shown_rect(&island, t);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    let t1 = t + ms(60);
    step(&mut island, t1);
    let shown = shown_rect(&island, t1);
    let first_old = 1.0 - 60.0 / 90.0;
    let first = island.sheets.swap_alpha(t1).unwrap_or_default();
    assert!((first - first_old).abs() < 1e-3, "{first}");
    // Back to the volume while the travel runs.
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(volume(), core, t1));
    assert!(near(shown_rect(&island, t1), shown), "no jump");
    assert!(!island.sheets.is_leaving());
    let windows = windows(&island, t1);
    assert_eq!(windows.len(), 1, "one window");
    // The content kept is the one that showed more at the switch frame: the
    // first old content (1/3), not the new body (0).
    let kept = island.sheets.swap_alpha(t1).unwrap_or_default();
    assert!((kept - first_old).abs() < 1e-3, "{kept}");
    let end = t1 + ms(300);
    step(&mut island, end);
    assert!(near(shown_rect(&island, end), volume_rect), "its target");
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
        &island.sheet_anchor(t + ms(400)).expect("the last anchor"),
        t + ms(399),
    );
    if let (Some(end), Some(pill)) = (at_end, last) {
        // A close goes 70 % of the way to the piece's pill, never past it:
        // the rect has left the window and is nearer the pill than it was.
        let from = shown_rect(&island, t + ms(1));
        assert!((end.w - pill.w).abs() < (from.w - pill.w).abs());
    }
    // Then nothing stays hidden or painted.
    let later = t + ms(402);
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
    let t = t0 + ms(500);
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
    let end = t + ms(400);
    step(&mut island, end);
    assert!(island.sheet_rect(end).is_none());
}

#[test]
fn going_back_to_a_capsule_whose_window_still_closes_travels_from_it() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let at = centre(bar_rect(&island, bell));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    let t1 = t + ms(100);
    step(&mut island, t1);
    let closing = windows(&island, t1)[0].0;
    // Back to the volume while its window closes.
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(volume(), core, t1));
    assert_eq!(island.sheets.leaving_pieces(t1), [bell]);
    assert_eq!(island.sheets.piece(), Some(volume()));
    assert!(
        near(shown_rect(&island, t1), closing),
        "it travels from the shape the closing window had"
    );
    let windows = windows(&island, t1);
    assert_eq!(windows.len(), 2, "the bell's closing and the volume's one");
    assert!(island.sheets.is_swapping());
}

#[test]
fn reopening_during_a_close_travels_from_the_shape_shown() {
    let (mut island, t) = settled();
    let bars = capsules(&island, t).len();
    let rest_rect = shown_rect(&island, t);
    island.close_sheet(t);
    let t1 = t + ms(130);
    step(&mut island, t1);
    let shown = shown_rect(&island, t1);
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(volume(), core, t1));
    assert!(island.sheets.is_open());
    assert!(!island.sheets.has_queued());
    let now = shown_rect(&island, t1);
    assert!(
        (now.x - shown.x).abs() < 0.5
            && (now.y - shown.y).abs() < 0.5
            && (now.w - shown.w).abs() < 0.5
            && (now.h - shown.h).abs() < 0.5,
        "{now:?} vs {shown:?}"
    );
    assert_eq!(island.shapes(t1).len(), bars + 1, "back in the blur region");
    let end = t1 + ms(300);
    step(&mut island, end);
    assert!(near(shown_rect(&island, end), rest_rect));
}

#[test]
fn the_level_the_old_content_held_goes_out_when_the_travel_starts() {
    let (mut island, t0) = level_island();
    let sent = Rc::new(RefCell::new(Vec::new()));
    let core = sent_core(&island.theme, &sent, AudioGraph::default());
    assert!(island.open_level_sheet_at(volume(), core, t0));
    let t = t0 + ms(500);
    step(&mut island, t);
    let sheet = shown_rect(&island, t);
    let slider = match island.sheets.body_node_rect(crate::osd::popup::slider_key(
        crate::osd::popup::Control::Output,
    )) {
        Some(r) => r,
        None => panic!("no output slider"),
    };
    let at = (
        sheet.x + slider.x + slider.w / 2.0,
        sheet.y + slider.y + slider.h / 2.0,
    );
    let wheel = InputEvent::Axis {
        x: at.0,
        y: at.1,
        horizontal: 0.0,
        vertical: -15.0,
    };
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    // The first notch goes out; the second waits in the throttle.
    feed(&mut island, wheel.clone(), t);
    feed(&mut island, wheel, t);
    assert_eq!(sent.borrow().len(), 1, "the second notch is held");
    // The switch sends it at once, not 90 ms later.
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    assert!(island.sheets.is_swapping());
    assert_eq!(sent.borrow().len(), 2, "the held level went out");
}

#[test]
fn the_open_pill_goes_to_the_other_capsules_piece_at_once() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let at = centre(bar_rect(&island, bell));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    step(&mut island, t + ms(1));
    let bar = island.bar.as_ref();
    assert_eq!(bar.and_then(|b| b.open_piece()), Some(bell));
}

#[test]
fn reduced_motion_swaps_at_once() {
    let (mut island, t) = settled();
    island.set_reduced_motion(true);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    step(&mut island, t);
    let now = shown_rect(&island, t);
    let later = t + ms(900);
    step(&mut island, later);
    assert!(near(now, shown_rect(&island, later)), "at its target");
    assert!(!island.sheets.is_swapping());
    assert!(!island.sheets.wants_frame(t));
    assert!(!island.sheets.is_leaving());
}

#[test]
fn no_frame_is_asked_once_a_window_has_closed() {
    let (mut island, t) = settled();
    island.close_sheet(t);
    step(&mut island, t + ms(399));
    assert!(island.sheets.wants_frame(t + ms(399)));
    let done = t + ms(400);
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

/// The level island with the end capsule `volume`, `brightness`, then eight
/// tray apps: the capsule is wider than a 320 px body, so a window under its
/// first piece is not pushed by the capsule's right edge.
fn wide_end_island() -> (Island, Instant) {
    let (mut island, t0) = level_island();
    island.tray_rows = (0..8)
        .map(|n| TrayRow {
            id: ItemId {
                bus_name: format!(":1.{n}"),
                path: "/StatusNotifierItem".to_owned(),
            },
            sni_id: format!("app{n}"),
            title: format!("app {n}"),
            has_menu: true,
            ..TrayRow::default()
        })
        .collect();
    island.set_bar(&BarLayout {
        end: vec![
            "volume".to_owned(),
            "brightness".to_owned(),
            "tray".to_owned(),
        ],
        ..BarLayout::default()
    });
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    (island, t0 + ms(1000))
}

fn pill_of(island: &Island, key: Key, now: Instant) -> Rect {
    match island.bar.as_ref().and_then(|b| b.pill_of(key, now)) {
        Some(r) => r,
        None => panic!("no pill for {key:?}"),
    }
}

#[test]
fn a_menu_opens_under_its_piece() {
    let (mut island, t0) = wide_end_island();
    let capsule = end_capsule(&island);
    assert!(capsule.w > 320.0 + 40.0, "premise: {capsule:?}");
    let pill = pill_of(&island, volume(), t0);
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(500);
    step(&mut island, t);
    let window = shown_rect(&island, t);
    // 16 px before its piece, which is the capsule's padding: the window sits
    // on the capsule's left edge and ends short of its right edge, where the
    // old rule put it.
    assert!((window.x - (pill.x - 16.0)).abs() < 0.01, "{window:?}");
    assert!((window.x - capsule.x).abs() < 0.01, "{window:?}");
    assert!(
        window.x + window.w < capsule.x + capsule.w - 1.0,
        "{window:?}"
    );
    // The last piece (the tray's chevron-less row) is not under test; the
    // brightness piece sits further right and its window follows it.
    let mut other = wide_end_island().0;
    let (b_pill, t1) = (pill_of(&other, brightness_key(), t0), t0);
    let core = brightness_core(&other.theme);
    assert!(other.open_level_sheet_at(brightness_key(), core, t1));
    let t2 = t1 + ms(500);
    step(&mut other, t2);
    let window = shown_rect(&other, t2);
    assert!((window.x - (b_pill.x - 16.0)).abs() < 0.01, "{window:?}");
}

#[test]
fn a_switch_travels_between_the_two_pieces_rects() {
    let (mut island, t0) = wide_end_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(500);
    step(&mut island, t);
    let old = shown_rect(&island, t);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    step(&mut island, t);
    assert!(
        near(shown_rect(&island, t), old),
        "starts at the first rect"
    );
    let rest = t + ms(900);
    step(&mut island, rest);
    let end = shown_rect(&island, rest);
    let body = island.sheets.body_size().unwrap_or_default();
    let capsule = end_capsule(&island);
    let pill = pill_of(&island, brightness_key(), rest);
    let want = geometry::window(capsule, geometry::Edge::End, pill, body, island.size);
    assert!(near(end, want), "{end:?} vs {want:?}");
    assert!(end.x > old.x + 1.0, "the second piece is further right");
    // Mid-travel the window is between the two rects.
    let mut seen = false;
    let (mut island, t0) = wide_end_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(500);
    step(&mut island, t);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    for n in [60, 120, 180] {
        let now = t + ms(n);
        step(&mut island, now);
        let x = shown_rect(&island, now).x;
        seen |= x > old.x + 0.5 && x < end.x - 0.5;
    }
    assert!(seen, "a mid-travel frame lies between the two rects");
}

#[test]
fn an_open_window_holds_its_x_while_its_pieces_text_changes() {
    let (mut island, t0) = wide_end_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(500);
    step(&mut island, t);
    let before = shown_rect(&island, t);
    let capsule = end_capsule(&island);
    // "40 %" widens to "100 %": the capsule grows to the left and the piece
    // moves with it.
    island.levels.audio.output = Some(Level {
        percent: 100,
        muted: false,
    });
    island.invalidate_bar();
    let t1 = t + ms(20);
    step(&mut island, t1);
    assert!(end_capsule(&island).w > capsule.w, "premise: it widened");
    for n in [0, 16, 100, 300, 600] {
        let now = t1 + ms(n);
        step(&mut island, now);
        let r = shown_rect(&island, now);
        assert_eq!(r.x, before.x, "+{n} ms");
        assert_eq!(r.w, before.w, "+{n} ms");
    }
    // At rest nothing is asked.
    let rest = t1 + ms(900);
    step(&mut island, rest);
    assert_eq!(shown_rect(&island, rest).x, before.x);
    assert!(!island.sheets.wants_frame(rest), "no frame at rest");
}

#[test]
fn a_tray_slide_carries_an_open_window_with_its_piece() {
    let (mut island, t0) = wide_end_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(500);
    step(&mut island, t);
    let before = shown_rect(&island, t);
    // Four tray apps hidden: the end capsule narrows from the left and its
    // first piece slides right with it.
    island.set_bar(&BarLayout {
        end: vec![
            "volume".to_owned(),
            "brightness".to_owned(),
            "tray".to_owned(),
        ],
        tray_hidden: (0..4).map(|n| format!("app{n}")).collect(),
        ..BarLayout::default()
    });
    island.apply_views(t);
    step(&mut island, t);
    let mut now = t;
    for n in [60, 175, 350, 600, 900, 1200, 1500] {
        now = t + ms(n);
        step(&mut island, now);
    }
    let capsule = end_capsule(&island);
    let pill = pill_of(&island, volume(), now);
    let window = shown_rect(&island, now);
    assert!(
        pill.x > before.x + 30.0,
        "premise: the piece slid right: {pill:?}"
    );
    assert!(
        (window.x - (pill.x - 16.0)).abs() < 0.5,
        "{window:?} {pill:?}"
    );
    assert!(
        window.x >= capsule.x - 0.5 && window.x + window.w <= capsule.x + capsule.w + 0.5,
        "{window:?} outside {capsule:?}"
    );
}
