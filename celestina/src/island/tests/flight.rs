//! The flight of the open piece into its sheet's subtitle (CEL-D-008): which
//! nodes nobody paints while it flies, when they show again, and what the
//! flyers do when the piece or the sheet changes under them.

use celestina_shell::settings::BarLayout;

use super::*;
use crate::sheet::subtitle;

fn icon_and_text() -> Vec<Key> {
    vec![subtitle::icon_key(), subtitle::text_key()]
}

#[test]
fn the_flying_piece_and_the_subtitle_are_hidden_until_the_flight_lands() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    island.resolve_flights(t0);
    assert_eq!(island.sheets.flights().len(), 1, "one flyer going down");
    let down = &island.sheets.flights()[0];
    assert!(!down.up && down.flies_text());
    assert_eq!(down.from.icon_rect.w, 18.0, "the bar's icon");
    assert_eq!(down.to.icon_rect.w, 22.0, "the subtitle's icon");
    let (from, to) = (down.from.text.as_ref(), down.to.text.as_ref());
    assert_eq!(from.map(|t| t.size), Some(13.0));
    assert_eq!(to.map(|t| t.size), Some(15.0));
    // The piece is out of the row, and the subtitle's own icon and text are
    // not painted, until the flight lands.
    assert_eq!(island.sheets.hidden_pieces(t0), vec![volume()]);
    assert_eq!(island.sheets.subtitle_hidden(volume(), t0), icon_and_text());
    let late = t0 + ms(349);
    assert_eq!(
        island.sheets.subtitle_hidden(volume(), late),
        icon_and_text()
    );
    let landed = t0 + ms(351);
    step(&mut island, landed);
    assert!(island.sheets.flights().is_empty());
    assert!(island.sheets.subtitle_hidden(volume(), landed).is_empty());
    assert!(!island.sheets.is_flying(landed));
}

#[test]
fn closing_keeps_the_piece_hidden_until_its_flight_up_lands() {
    let (mut island, t0) = settled();
    let t = t0;
    island.close_sheet(t);
    island.resolve_flights(t);
    let up = &island.sheets.flights()[0];
    assert!(up.up && up.flies_text());
    // The slot reopens at once.
    step(&mut island, t + ms(1));
    assert_eq!(island.bar.as_ref().and_then(|b| b.lifted()), None);
    // 0 to 60 ms: the subtitle still shows (it fades with the body).
    assert!(island
        .sheets
        .subtitle_hidden(volume(), t + ms(30))
        .is_empty());
    assert_eq!(
        island.sheets.subtitle_hidden(volume(), t + ms(70)),
        icon_and_text()
    );
    // The sheet is gone at 150 ms and the piece is still not painted.
    step(&mut island, t + ms(160));
    assert!(!island.sheets.is_shown(t + ms(160)));
    assert_eq!(island.sheets.hidden_pieces(t + ms(160)), vec![volume()]);
    assert_eq!(island.sheets.hidden_pieces(t + ms(259)), vec![volume()]);
    // It lands at 60 + 200 ms.
    assert!(island.sheets.hidden_pieces(t + ms(261)).is_empty());
    step(&mut island, t + ms(262));
    assert!(island.sheets.flights().is_empty());
}

#[test]
fn switching_flies_one_piece_down_and_one_up_and_paints_none_twice() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    // Click brightness while the volume's flyer is still going down.
    let t = t0 + ms(100);
    step(&mut island, t);
    island.resolve_flights(t); // a frame's paint
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    island.resolve_flights(t);
    let flights = island.sheets.flights();
    assert_eq!(flights.len(), 2, "{flights:?}");
    let up: Vec<_> = flights.iter().filter(|f| f.up).collect();
    let down: Vec<_> = flights.iter().filter(|f| !f.up).collect();
    assert_eq!((up.len(), down.len()), (1, 1));
    assert_eq!(up[0].keys.piece, volume());
    assert_eq!(down[0].keys.piece, brightness_key());
    // The old flyer goes up from where it was, not from the subtitle.
    let half = island.sheets.flights().iter().find(|f| f.up);
    assert!(half.is_some_and(|f| f.from.icon_rect.w > 18.5));
    // Each piece is hidden once: the new one lifted, the old one till landing.
    let hidden = island.sheets.hidden_pieces(t);
    assert_eq!(hidden.len(), 2);
    assert!(hidden.contains(&volume()) && hidden.contains(&brightness_key()));
    // The leaving body's subtitle is not painted either.
    assert_eq!(island.sheets.subtitle_hidden(volume(), t), icon_and_text());
    // 150 ms on, the old piece has landed and shows; the new one still flies.
    let later = t + ms(151);
    assert_eq!(island.sheets.hidden_pieces(later), vec![brightness_key()]);
    step(&mut island, t + ms(400));
    assert!(island.sheets.flights().is_empty());
    let bar = island.bar.as_ref();
    assert_eq!(bar.and_then(|b| b.lifted()), Some(brightness_key()));
    assert!(bar.is_some_and(|b| b.piece_rect(volume()).is_some_and(|r| r.w > 0.0)));
}

#[test]
fn reduced_motion_has_no_flight() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    assert!(open_on(&mut island, volume(), t0));
    assert!(island.sheets.flights().is_empty());
    assert!(!island.sheets.is_flying(t0));
    assert!(island.sheets.subtitle_hidden(volume(), t0).is_empty());
    let t = t0 + ms(400);
    step(&mut island, t);
    island.close_sheet(t);
    assert!(island.sheets.flights().is_empty());
    assert!(island.sheets.hidden_pieces(t + ms(1)).is_empty());
}

#[test]
fn a_vanished_piece_drops_its_flight() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    // The piece leaves the bar while its flyer is still going down: the
    // sheet closes on its last anchor and nothing flies to the missing slot.
    let t = t0 + ms(100);
    island.levels.audio.output = None;
    island.invalidate_bar();
    step(&mut island, t);
    assert!(!island.sheets.is_open(), "the sheet closes");
    island.resolve_flights(t + ms(1));
    assert!(
        island.sheets.flights().is_empty(),
        "{:?}",
        island.sheets.flights()
    );
    assert!(!island.sheets.is_flying(t + ms(1)));
}

#[test]
fn the_flyer_shows_the_live_text() {
    let (mut island, t0) = level_island();
    let theme = island.theme.clone();
    let cell = Rc::new(Cell::new(Levels {
        audio: AudioLevels {
            output: Some(Level {
                percent: 40,
                muted: false,
            }),
            input: None,
        },
        brightness: None,
    }));
    let sink: Sink = Rc::new(|_| {});
    let graph: SharedGraph = Rc::new(RefCell::new(AudioGraph::default()));
    let mut fit = |name: &str, _: f32| name.to_owned();
    let core = LevelCore::new(
        &theme,
        level_popup::Kind::Volume,
        Feeds {
            cell: Rc::clone(&cell),
            graph,
            monitors: MonitorFeed::default(),
        },
        sink,
        false,
        t0,
        &mut fit,
    );
    assert!(island.open_level_sheet_at(volume(), core, t0));
    step(&mut island, t0);
    island.resolve_flights(t0);
    let text = |island: &Island| {
        island
            .sheets
            .flights()
            .first()
            .and_then(|f| f.to.text.as_ref().map(|t| t.text.clone()))
    };
    assert_eq!(text(&island).as_deref(), Some("40 %"));
    // The volume moves while the piece flies.
    let t = t0 + ms(120);
    cell.set(Levels {
        audio: AudioLevels {
            output: Some(Level {
                percent: 55,
                muted: false,
            }),
            input: None,
        },
        brightness: None,
    });
    step(&mut island, t);
    island.resolve_flights(t);
    assert_eq!(text(&island).as_deref(), Some("55 %"), "the flyer is live");
    // At rest the subtitle shows the same string.
    let rest = t0 + ms(500);
    step(&mut island, rest);
    assert_eq!(
        island.sheets.subtitle_text().as_deref(),
        Some("55 %"),
        "no stale string"
    );
}

#[test]
fn a_piece_without_text_flies_its_icon_only() {
    let (mut island, t0) = bar_island(1920.0);
    island.set_connectivity(&crate::connectivity::state::sample::ready());
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = t0 + Duration::from_secs(2);
    lay_fake(&mut island, t);
    let session = Key::named("session");
    assert_eq!(
        island.sheet_request(session, crate::pieces::SheetKind::Session, t),
        None
    );
    step(&mut island, t);
    island.resolve_flights(t);
    let f = island.sheets.flights();
    assert_eq!(f.len(), 1);
    assert!(
        !f[0].flies_text(),
        "the subtitle's fallback text is not flown"
    );
    // Only the icon is hidden in the subtitle: the fallback text shows with
    // the body.
    assert_eq!(
        island.sheets.subtitle_hidden(session, t),
        vec![subtitle::icon_key()]
    );
}

#[test]
fn the_only_piece_of_a_capsule_keeps_a_row_and_flies() {
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
    assert!(open_on(&mut island, volume(), t));
    for ms_on in [0, 60, 200, 400] {
        let at = t + ms(ms_on);
        step(&mut island, at);
        island.resolve_flights(at);
        let rect = shown_rect(&island, at);
        assert!(rect.w > 0.0 && rect.h >= geometry::ROW_H, "{rect:?}");
        assert!(rect.w.is_finite() && rect.x.is_finite());
    }
    // No zero-width flyer end.
    island.resolve_flights(t);
    let anchor = island.sheet_anchor(t);
    assert!(anchor.is_some_and(|a| a.capsule.w >= 0.0));
}

#[test]
fn no_frame_is_asked_at_rest() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    assert!(island.sheets.wants_frame(t0 + ms(10)), "the flight runs");
    let rest = t0 + ms(800);
    step(&mut island, rest);
    assert!(!island.sheets.is_flying(rest));
    assert!(
        !island.sheets.wants_frame(rest),
        "an open sheet at rest asks none"
    );
    island.close_sheet(rest);
    step(&mut island, rest + ms(1));
    let after = rest + ms(900);
    step(&mut island, after);
    assert!(!island.sheets.wants_frame(after));
    let later = after + ms(900);
    assert!(!island.bar.as_ref().is_some_and(|b| b.is_animating(later)));
}

/// An Island with the clock and the bell in the end capsule, the date known.
fn clock_island() -> (Island, Instant) {
    let (mut island, t0) = bar_island(1920.0);
    island.today = Some(celestina_shell::glance::Today {
        year: 2026,
        month: 11,
        day: 26,
        weekday: 3,
    });
    island.set_bar(&BarLayout {
        start: Vec::new(),
        centre: Vec::new(),
        end: vec!["clock".to_owned(), "notifications".to_owned()],
        tray_hidden: Vec::new(),
    });
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = t0 + Duration::from_secs(1);
    lay_fake(&mut island, t);
    (island, t)
}

#[test]
fn a_text_only_piece_flies_its_text() {
    let (mut island, t) = clock_island();
    let clock = Key::named("clock-date");
    assert_eq!(
        island.sheet_request(
            clock,
            crate::pieces::SheetKind::Page(crate::pieces::PageId::CLOCK),
            t
        ),
        None
    );
    step(&mut island, t);
    island.resolve_flights(t);
    let f = island.sheets.flights();
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].flies_text() && !f[0].flies_icon());
    assert!(f[0].keys.icon.is_none());
    // Only what flies is hidden in the subtitle: its icon fades in with the
    // body.
    assert_eq!(
        island.sheets.subtitle_hidden(clock, t),
        vec![subtitle::text_key()]
    );
}

#[test]
fn a_switch_does_not_hide_the_text_of_a_face_without_text() {
    let (mut island, t0) = level_island();
    let session = Key::named("session");
    assert_eq!(
        island.sheet_request(session, crate::pieces::SheetKind::Session, t0),
        None
    );
    step(&mut island, t0);
    let t = t0 + ms(400);
    step(&mut island, t);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    // Before any resolve: the session's bar has no text to fly, so its
    // leaving subtitle keeps the menu's name.
    assert_eq!(
        island.sheets.subtitle_hidden(session, t),
        vec![subtitle::icon_key()]
    );
}

#[test]
fn the_leaving_body_keeps_its_own_slot() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    let before = island.sheet_anchor(t).map(|a| a.piece);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    let leaving = island.sheets.leaving_slot();
    assert_eq!(leaving, before, "the old body keeps the slot it had");
    let now = island.sheet_anchor(t).map(|a| a.piece);
    assert_ne!(leaving, now, "the new body has the new piece's slot");
}

#[test]
fn a_scrolled_subtitle_flies_from_where_it_is() {
    let theme = Theme::dark();
    let mut body = crate::sheet::SheetBody::Level(Box::new(volume_core(&theme)));
    body.layout(&mut |tree: &mut Tree, room: (f32, f32)| {
        tree.layout_measured(room, fake_measure);
    });
    let sheet = Rect {
        x: 100.0,
        y: 8.0,
        w: 320.0,
        h: 300.0,
    };
    let slot = Rect {
        x: 200.0,
        y: 8.0,
        w: 0.0,
        h: 40.0,
    };
    let at =
        |scroll| crate::sheet::subtitle_parts(&body, sheet, slot, scroll).map(|p| p.icon_rect.y);
    assert_eq!(at(0.0).zip(at(30.0)).map(|(a, b)| a - b), Some(30.0));
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
    let rect = shown_rect(island, now);
    let anchor = island.sheet_anchor(now).expect("an anchor");
    let size = island.sheets.body_size().expect("a body");
    let at = geometry::body_box(rect, anchor.piece, size.0);
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
        island.sheet_request(session, crate::pieces::SheetKind::Session, t0),
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

#[test]
fn a_text_node_gone_mid_flight_leaves_the_icon_to_fly() {
    let (island, t) = level_island();
    let bar = island.bar.as_ref().expect("a bar");
    let text = copy::osd_percent(40);
    let (keys, _) = bar
        .flyers_of(volume(), Some(text.as_str()), t)
        .expect("the volume flies");
    assert!(keys.icon.is_some() && keys.text.is_some());
    let gone = crate::sheet::flight::BarKeys {
        text: Some(Key::named("gone")),
        ..keys
    };
    let parts = bar.parts_at(gone, t).expect("the icon still flies");
    assert!(parts.icon.is_some());
    assert!(parts.text.is_none());
}
