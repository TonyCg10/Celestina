//! The piece a menu is open on stays in its slot (CEL-D-014): its capsule
//! keeps its extent, the grey pill lights behind it, its content goes to 80 %
//! and gives the pointer's lift to the pill, and its gestures are its own.

use celestina_paint::Role;
use celestina_shell::settings::BarLayout;

use super::*;
use crate::{
    pieces::tray::chevron_key,
    sheet::{motion::PIECE_RESTORE_AT, subtitle},
};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// The plate of `piece`'s zone and the held state of `piece`, as the bar
/// hands them to the painted tree at `now`.
fn look(
    island: &mut Island,
    piece: Key,
    now: Instant,
) -> (Option<celestina_ui::Plate>, Option<celestina_ui::Held>) {
    let bar = island.bar.as_mut().expect("a bar");
    let zone = bar.zone_of(piece).expect("a zone");
    bar.synced_look(zone, piece, now)
}

fn bar(island: &Island) -> &crate::island::bar_tree::BarTree {
    island.bar.as_ref().expect("a bar")
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

/// The bar's layout with the volume and then the tray in the end capsule, so
/// that a tray app hidden slides the volume.
fn tray_layout(hidden: &[&str]) -> BarLayout {
    BarLayout {
        end: vec!["volume".to_owned(), "tray".to_owned()],
        tray_hidden: hidden.iter().map(|s| (*s).to_owned()).collect(),
        ..BarLayout::default()
    }
}

/// The level island with three tray apps (`a`, `b`, `c`) after the volume,
/// none hidden.
fn tray_level_island() -> (Island, Instant) {
    let (mut island, t0) = level_island();
    island.tray_rows = vec![app(0, "a"), app(1, "b"), app(2, "c")];
    island.set_bar(&tray_layout(&[]));
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    (island, t0 + Duration::from_secs(1))
}

/// Hides the tray apps `ids`: the chevron comes in and the volume slides.
fn hide(island: &mut Island, ids: &[&str], now: Instant) {
    island.set_bar(&tray_layout(ids));
    island.apply_views(now);
}

/// The middle of the open sheet's header row, in surface coordinates.
fn header_point(island: &Island, now: Instant) -> (f32, f32) {
    let anchor = island.sheet_anchor(now).expect("an anchor");
    let size = island.sheets.body_size().expect("a body");
    let at = geometry::window(anchor.capsule, anchor.edge, size, anchor.surface);
    let row = island
        .sheets
        .body_node_rect(subtitle::row_key())
        .expect("a header row");
    (at.x + row.x + row.w / 2.0, at.y + row.y + row.h / 2.0)
}

#[test]
fn the_open_piece_keeps_its_slot_and_its_capsule_keeps_its_extent() {
    let (mut island, t0) = level_island();
    let keys = [
        volume(),
        brightness_key(),
        Key::named("bell"),
        Key::named("session"),
    ];
    let extent = bar(&island).capsule_extent(3, t0);
    let pills = keys.map(|k| bar(&island).pill_of(k, t0));
    assert!(
        pills.iter().all(Option::is_some),
        "premise: all are in the bar"
    );
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    let mut frames: Vec<Instant> = [0, 60, 175, 350, 400].map(|n| t0 + ms(n)).to_vec();
    island.close_sheet(t);
    frames.extend([0, 100, 259, 300].map(|n| t + ms(n)));
    for now in frames {
        step(&mut island, now);
        let b = bar(&island);
        assert_eq!(b.capsule_extent(3, now), extent, "{now:?}");
        assert_eq!(keys.map(|k| b.pill_of(k, now)), pills, "{now:?}");
        assert!(!b.is_sliding(3, now), "{now:?}");
    }
}

#[test]
fn the_pill_lights_under_the_piece_and_its_content_goes_to_80_percent() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(150);
    step(&mut island, t);
    let (plate, _) = look(&mut island, volume(), t);
    let plate = plate.expect("a plate");
    assert_eq!(Some(plate.rect), bar(&island).pill_of(volume(), t));
    assert_eq!(plate.radius, island.theme.radius_md);
    assert_eq!(plate.radius, 12.0);
    assert_eq!(plate.role, Role::ControlHover);
    assert!(near(plate.alpha, 1.0), "{}", plate.alpha);
    let t = t0 + ms(200);
    step(&mut island, t);
    let (_, held) = look(&mut island, volume(), t);
    let held = held.expect("held");
    assert!(held.quiet);
    assert!(near(held.inside, 0.8), "{}", held.inside);
    // The neighbours keep full content and no plate of theirs.
    assert_eq!(look(&mut island, brightness_key(), t).1, None);
}

#[test]
fn the_open_piece_shows_no_hover_or_press_fill() {
    let (mut island, t0) = level_island();
    let at = centre(bar_rect(&island, volume()));
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t0,
    );
    // Premise: before the open the pointer lifts the piece.
    let _ = look(&mut island, volume(), t0);
    assert_eq!(bar(&island).box_roles(volume()).0, Some(Role::ControlHover));
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    feed(&mut island, press(at.0, at.1, BTN_LEFT), t);
    let _ = look(&mut island, volume(), t);
    assert_eq!(bar(&island).hovered_key(), Some(volume()));
    assert_eq!(bar(&island).box_roles(volume()), (None, None));
    // A neighbour keeps its lift.
    feed(&mut island, release(at.0, at.1, BTN_LEFT), t);
    let other = centre(bar_rect(&island, brightness_key()));
    let t = t + ms(500);
    step(&mut island, t);
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: other.0,
            y: other.1,
        },
        t,
    );
    let _ = look(&mut island, brightness_key(), t);
    assert_eq!(
        bar(&island).box_roles(brightness_key()),
        (Some(Role::ControlHover), None)
    );
}

#[test]
fn a_primary_press_on_the_open_piece_closes_its_menu() {
    let (mut island, t) = settled();
    let at = centre(bar_rect(&island, volume()));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(!island.sheets.is_open());
    assert_eq!(island.sheets.held_piece(t), None);
}

#[test]
fn the_wheel_on_the_open_piece_steps_its_level_and_keeps_the_menu() {
    let (mut island, t) = settled();
    let at = centre(bar_rect(&island, volume()));
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    let out = feed(
        &mut island,
        InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical: 15.0,
        },
        t,
    );
    assert!(
        matches!(out.as_slice(), [IslandAction::Audio(_)]),
        "{out:?}"
    );
    assert!(island.sheets.is_open());
    assert_eq!(island.sheets.piece(), Some(volume()));
}

#[test]
fn a_right_click_on_the_open_volume_piece_acts_on_it() {
    let (mut island, t) = settled();
    let at = centre(bar_rect(&island, volume()));
    let out = click_at(&mut island, at, BTN_RIGHT, t);
    assert_eq!(
        out,
        [IslandAction::Audio(crate::pieces::AudioAction::ToggleMute)]
    );
    // The bar's rule, unchanged: a plain gesture on a piece closes the menu.
    assert!(!island.sheets.is_open());
}

#[test]
fn the_header_row_is_shown_through_open_and_close() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let shown = |island: &Island| {
        island.sheets.body_node_rect(subtitle::icon_key()).is_some()
            && island.sheets.body_node_rect(subtitle::text_key()).is_some()
    };
    // Through the whole open, the icon and the text are the body's own.
    for t in [0, 60, 175, 349, 400] {
        step(&mut island, t0 + ms(t));
        assert!(shown(&island), "+{t} ms");
    }
    // A press on the row is the body's: no piece action, the menu stays.
    let t = t0 + ms(400);
    let at = header_point(&island, t);
    for button in [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE] {
        assert!(
            click_at(&mut island, at, button, t).is_empty(),
            "{button:#x}"
        );
        assert!(island.sheets.is_open(), "{button:#x}");
    }
    // And through the whole close, until the window has gone.
    island.close_sheet(t);
    for dt in [0, 30, 70, 150, 259] {
        step(&mut island, t + ms(dt));
        assert!(shown(&island), "close +{dt} ms");
    }
}

#[test]
fn closing_gives_back_the_pill_and_opacity_at_the_restore_moment() {
    let (mut island, t) = settled();
    island.close_sheet(t);
    let restore = t + ms(260).mul_f32(PIECE_RESTORE_AT);
    let at = |n: u64| restore + ms(n);
    step(&mut island, restore);
    let alpha = |island: &Island, now| bar(island).pill(now).first().map_or(0.0, |p| p.alpha);
    assert!(near(alpha(&island, at(0)), 1.0));
    step(&mut island, at(75));
    let mid = alpha(&island, at(75));
    assert!(near(mid, 1.0 - celestina_ui::EXIT.eval(0.5)), "{mid}");
    step(&mut island, at(150));
    assert!(bar(&island).pill(at(150)).is_empty(), "the pill is gone");
    assert_eq!(bar(&island).open_piece(), None);
    // The content is back at full opacity 200 ms later, and nothing runs.
    step(&mut island, at(100));
    assert!(bar(&island).content_opacity(volume(), at(100)) < 1.0);
    step(&mut island, at(200));
    assert_eq!(bar(&island).content_opacity(volume(), at(200)), 1.0);
    step(&mut island, at(300));
    assert!(!bar(&island).is_animating(at(300)));
}

#[test]
fn the_pill_follows_the_piece_when_it_changes_width() {
    let (mut island, t) = settled();
    let before = bar(&island).pill_of(volume(), t).expect("a pill");
    island.levels.audio.output = Some(Level {
        percent: 100,
        muted: false,
    });
    island.invalidate_bar();
    for dt in [16, 60, 400] {
        let now = t + ms(dt);
        step(&mut island, now);
        let (plate, _) = look(&mut island, volume(), now);
        let live = bar(&island).pill_of(volume(), now).expect("a pill");
        assert_eq!(plate.map(|p| p.rect), Some(live), "+{dt} ms");
    }
    let after = bar(&island).pill_of(volume(), t + ms(400)).expect("a pill");
    assert!(after.w > before.w, "the piece grew: {before:?} {after:?}");
}

#[test]
fn a_vanished_open_piece_leaves_no_plate() {
    let (mut island, t) = settled();
    let (plate, held) = look(&mut island, volume(), t);
    assert!(plate.is_some() && held.is_some(), "premise");
    island.levels.audio.output = None;
    island.invalidate_bar();
    let now = t + ms(16);
    step(&mut island, now);
    let zone = bar(&island).zone_of(brightness_key()).expect("a zone");
    let b = island.bar.as_mut().expect("a bar");
    let (plate, held) = b.synced_look(zone, volume(), now);
    assert_eq!(plate, None, "no plate on the capsule");
    assert_eq!(held, None);
    assert!(!island.sheets.is_open(), "the sheet closes");
    step(&mut island, now + ms(1));
    let (plate, _) = look(&mut island, brightness_key(), now + ms(1));
    assert_eq!(plate, None);
}

#[test]
fn a_tray_slide_while_open_carries_the_pill_with_its_piece() {
    let (mut island, t0) = tray_level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    hide(&mut island, &["b", "c"], t);
    step(&mut island, t);
    let mut moved = false;
    for dt in [0, 60, 175, 260, 400] {
        let now = t + ms(dt);
        step(&mut island, now);
        let (plate, _) = look(&mut island, volume(), now);
        let rect = plate.expect("a plate").rect;
        let live = bar(&island).pill_of(volume(), now).expect("a pill");
        assert_eq!(rect, live, "+{dt} ms");
        let extent = bar(&island).capsule_extent(3, now).expect("a capsule");
        assert!(
            rect.x >= extent.x - 0.01 && rect.x + rect.w <= extent.x + extent.w + 0.01,
            "+{dt} ms: {rect:?} outside {extent:?}"
        );
        moved |= (rect.x - bar(&island).node_rect(volume()).map_or(rect.x, |r| r.x)).abs() > 0.5;
    }
    assert!(
        moved,
        "premise: the piece was sliding while the pill rode it"
    );
}

#[test]
fn the_slide_is_followed_only_for_a_moment() {
    let (mut island, t0) = tray_level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    hide(&mut island, &["b", "c"], t);
    let mut now = t;
    while now < t + ms(340) {
        now += ms(16);
        step(&mut island, now);
    }
    assert!(island.sheets.follows_slide(now));
    // At rest, long after the last frame, a resize is a resize to morph to.
    assert!(!island.sheets.follows_slide(now + ms(400)));
}

/// Four registered apps (`a` and `c` hidden), the chevron in the end
/// capsule, at rest.
fn overflow_island() -> (Island, Instant) {
    let (mut island, t0) = level_island();
    island.tray_rows = vec![app(0, "a"), app(1, "b"), app(2, "c"), app(3, "d")];
    island.set_bar(&BarLayout {
        tray_hidden: vec!["a".to_owned(), "c".to_owned()],
        ..BarLayout::default()
    });
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    (island, t0 + Duration::from_secs(1))
}

#[test]
fn the_chevron_gets_the_pill_and_keeps_its_turn() {
    let (mut island, t0) = overflow_island();
    let at = centre(bar_rect(&island, chevron_key()));
    click_at(&mut island, at, BTN_LEFT, t0);
    step(&mut island, t0);
    let t = t0 + ms(500);
    step(&mut island, t);
    assert_eq!(island.sheets.piece(), Some(chevron_key()));
    let (plate, held) = look(&mut island, chevron_key(), t);
    let plate = plate.expect("a plate");
    assert_eq!(Some(plate.rect), bar(&island).pill_of(chevron_key(), t));
    assert!(held.is_some_and(|h| h.quiet && near(h.inside, 0.8)));
    assert_eq!(
        bar(&island).rotation_of(Key::named("tray-hidden-icon"), t),
        Some(180.0)
    );
    assert_eq!(bar(&island).box_roles(chevron_key()), (None, None));
}

#[test]
fn reduced_motion_lights_and_restores_the_pill_at_once() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    assert!(open_on(&mut island, volume(), t0));
    let (plate, held) = look(&mut island, volume(), t0);
    assert!(plate.is_some_and(|p| near(p.alpha, 1.0)));
    assert!(held.is_some_and(|h| near(h.inside, 0.8)));
    let t = t0 + ms(400);
    step(&mut island, t);
    island.close_sheet(t);
    step(&mut island, t);
    assert!(bar(&island).pill(t).is_empty(), "gone on the first frame");
    assert_eq!(bar(&island).content_opacity(volume(), t), 1.0);
    assert_eq!(look(&mut island, volume(), t), (None, None));
}

#[test]
fn the_only_piece_of_a_capsule_keeps_its_capsule_and_opens() {
    let (mut island, t0) = bar_island(1920.0);
    island.levels = Levels {
        audio: AudioLevels {
            output: Some(Level {
                percent: 40,
                muted: false,
            }),
            input: None,
        },
        brightness: None,
    };
    island.set_bar(&BarLayout {
        start: Vec::new(),
        centre: Vec::new(),
        end: vec!["volume".to_owned()],
        tray_hidden: Vec::new(),
    });
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = t0 + Duration::from_secs(1);
    lay_fake(&mut island, t);
    let extent = bar(&island).capsule_extent(3, t);
    let pill = bar(&island).pill_of(volume(), t);
    assert!(open_on(&mut island, volume(), t));
    for ms_on in [0, 60, 200, 400] {
        let at = t + ms(ms_on);
        step(&mut island, at);
        let rect = shown_rect(&island, at);
        assert!(rect.w > 0.0 && rect.h > 0.0, "{rect:?}");
        assert!(rect.w.is_finite() && rect.x.is_finite());
        // The capsule's rect is the same before and during the open, and the
        // pill sits on the piece.
        assert_eq!(bar(&island).capsule_extent(3, at), extent, "+{ms_on} ms");
        assert_eq!(bar(&island).pill_of(volume(), at), pill, "+{ms_on} ms");
    }
    let (plate, _) = look(&mut island, volume(), t + ms(400));
    assert_eq!(plate.map(|p| p.rect), pill);
    let anchor = island.sheet_anchor(t);
    assert!(anchor.is_some_and(|a| a.capsule.w >= 0.0));
}

#[test]
fn no_frame_is_asked_at_rest() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    assert!(island.sheets.wants_frame(t0 + ms(10)), "the window runs");
    let rest = t0 + ms(800);
    step(&mut island, rest);
    assert!(
        !island.sheets.wants_frame(rest),
        "an open sheet at rest asks none"
    );
    assert!(!island.bar.as_ref().is_some_and(|b| b.is_animating(rest)));
    island.close_sheet(rest);
    step(&mut island, rest + ms(1));
    let after = rest + ms(900);
    step(&mut island, after);
    assert!(!island.sheets.wants_frame(after));
    let later = after + ms(900);
    assert!(!island.bar.as_ref().is_some_and(|b| b.is_animating(later)));
}

#[test]
fn the_leaving_window_keeps_its_own_pill() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    let before = island.sheet_anchor(t).map(|a| a.pill);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    let leaving = island.sheets.leaving_pill();
    assert_eq!(leaving, before, "the old window folds into the pill it had");
    let now = island.sheet_anchor(t).map(|a| a.pill);
    assert_ne!(leaving, now, "the new window grows from the new piece's");
}

#[test]
fn closing_with_the_pointer_on_the_piece_shows_one_fill_only() {
    let (mut island, t) = settled();
    let at = centre(bar_rect(&island, volume()));
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    island.close_sheet(t);
    for dt in [1, 10, 75, 149] {
        let now = t + ms(dt);
        step(&mut island, now);
        let (plate, _) = look(&mut island, volume(), now);
        let plate_on = plate.is_some_and(|p| p.alpha > 0.0);
        let hover = bar(&island).box_roles(volume()).0 == Some(Role::ControlHover);
        assert!(!(plate_on && hover), "+{dt} ms: plate and hover stack");
    }
    // Without the pointer on it the pill still fades over 150 ms.
    let (mut island, t) = settled();
    island.close_sheet(t);
    step(&mut island, t + ms(1));
    step(&mut island, t + ms(75));
    assert!(!bar(&island).pill(t + ms(75)).is_empty());
}
