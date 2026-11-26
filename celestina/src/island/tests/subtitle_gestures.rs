//! The subtitle of an open sheet answers the gestures of its piece (CEL-D-008)
//! and the window and bar clocks around it end at rest.

use super::*;
use crate::sheet::subtitle;

#[test]
fn the_only_piece_of_a_capsule_keeps_a_row_and_opens() {
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
    island.set_bar(&celestina_shell::settings::BarLayout {
        start: Vec::new(),
        centre: Vec::new(),
        end: vec!["volume".to_owned()],
        tray_hidden: Vec::new(),
    });
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = t0 + Duration::from_secs(1);
    lay_fake(&mut island, t);
    assert!(open_on(&mut island, volume(), t));
    for ms_on in [0, 60, 200, 400] {
        let at = t + ms(ms_on);
        step(&mut island, at);
        let rect = shown_rect(&island, at);
        assert!(rect.w > 0.0 && rect.h > 0.0, "{rect:?}");
        assert!(rect.w.is_finite() && rect.x.is_finite());
    }
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
fn the_slide_is_followed_only_for_a_moment() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let mut t = t0;
    while t < t0 + ms(340) {
        t += ms(16);
        step(&mut island, t);
    }
    assert!(island.sheets.follows_slide(t));
    // At rest, long after the last frame, a resize is a resize to morph to.
    assert!(!island.sheets.follows_slide(t + ms(400)));
}

#[test]
fn a_right_click_on_the_volume_subtitle_mutes_and_a_click_closes() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    let at = subtitle_point(&island, t);
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    let actions = click_at(&mut island, at, BTN_RIGHT, t);
    assert_eq!(
        actions,
        [IslandAction::Audio(crate::pieces::AudioAction::ToggleMute)]
    );
    // A plain click closes the sheet, as pressing the open piece did.
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    let at = subtitle_point(&island, t);
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(!island.sheets.is_open());
}

/// The middle of the open sheet's subtitle row, in surface coordinates.
pub(in crate::island) fn subtitle_point(island: &Island, now: Instant) -> (f32, f32) {
    let anchor = island.sheet_anchor(now).expect("an anchor");
    let size = island.sheets.body_size().expect("a body");
    let at = geometry::window(anchor.capsule, anchor.edge, size, anchor.surface);
    let row = island
        .sheets
        .body_node_rect(subtitle::row_key())
        .expect("a subtitle row");
    (at.x + row.x + row.w / 2.0, at.y + row.y + row.h / 2.0)
}

/// The session sheet open in the level island's bar, settled.
fn session_open() -> (Island, Instant) {
    let (mut island, t0) = level_island();
    let session = Key::named("session");
    assert_eq!(
        island.sheet_request(
            session,
            crate::pieces::SheetKind::Session,
            t0,
            &mut fake_measure
        ),
        None
    );
    let t = t0 + ms(400);
    step(&mut island, t);
    assert_eq!(island.sheets.piece(), Some(session));
    (island, t)
}

#[test]
fn a_gesture_the_piece_has_no_action_for_keeps_the_sheet_open() {
    let (mut island, t) = session_open();
    let at = subtitle_point(&island, t);
    for button in [BTN_RIGHT, BTN_MIDDLE] {
        assert!(click_at(&mut island, at, button, t).is_empty());
        assert!(island.sheets.is_open(), "{button:#x}");
    }
}

#[test]
fn a_release_on_the_subtitle_after_a_press_elsewhere_is_no_click() {
    let (mut island, t) = session_open();
    let at = subtitle_point(&island, t);
    let below = (at.0, at.1 + subtitle::SUBTITLE_H);
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: below.0,
            y: below.1,
        },
        t,
    );
    feed(&mut island, press(below.0, below.1, BTN_LEFT), t);
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    feed(&mut island, release(at.0, at.1, BTN_LEFT), t);
    assert!(island.sheets.is_open());
    // A click that begins and ends there still closes it.
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(!island.sheets.is_open());
}

#[test]
fn only_a_piece_with_a_wheel_action_takes_the_wheel_over_its_subtitle() {
    let (mut island, _) = level_island();
    let takes = |island: &mut Island, key| {
        island
            .with_bar(|bar, cx| bar.takes_wheel(key, cx))
            .unwrap_or(false)
    };
    assert!(takes(&mut island, volume()));
    assert!(!takes(&mut island, Key::named("session")));
    assert!(!takes(&mut island, Key::named("bell")));
}
