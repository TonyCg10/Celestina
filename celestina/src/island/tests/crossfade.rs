//! The open piece crossfades in the bar (CEL-D-010): it fades out, blurs and
//! sinks while its slot closes when its sheet opens, and fades back in at its
//! place from the first frame of the close. The subtitle is never hidden.

use super::*;
use crate::sheet::subtitle;

/// The opacity, blur and drop `piece` is painted with at `now`.
fn fade(island: &Island, piece: Key, now: Instant) -> Option<(f32, f32, f32)> {
    island.bar.as_ref().and_then(|b| b.fade_of(piece, now))
}

#[test]
fn opening_fades_the_piece_down_and_blurred_in_160_ms() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    // The first frame: still whole, about to go.
    assert_eq!(fade(&island, volume(), t0), Some((1.0, 0.0, 0.0)));
    let mid = t0 + ms(50);
    step(&mut island, mid);
    let (k, blur, drop) = fade(&island, volume(), mid).expect("fading");
    assert!(k > 0.0 && k < 1.0, "{k}");
    assert!((blur - 4.0 * (1.0 - k)).abs() < 1e-4, "blur {blur}");
    assert!((drop - 10.0 * (1.0 - k)).abs() < 1e-4, "drop {drop}");
    // It leaves faster than its slot closes (160 ms against the spring).
    let end = t0 + ms(159);
    step(&mut island, end);
    assert!(fade(&island, volume(), end).is_some_and(|(k, ..)| k < 0.01));
    let gone = t0 + ms(160);
    step(&mut island, gone);
    assert_eq!(fade(&island, volume(), gone), None, "the clock ends");
    assert_eq!(
        island.bar.as_ref().and_then(|b| b.lifted()),
        Some(volume()),
        "its slot stays closed"
    );
}

#[test]
fn closing_fades_the_piece_up_and_sharp_in_200_ms_from_the_first_frame() {
    let (mut island, t) = settled();
    assert_eq!(fade(&island, volume(), t), None, "at rest nothing fades");
    island.close_sheet(t);
    step(&mut island, t);
    assert_eq!(
        island.bar.as_ref().and_then(|b| b.lifted()),
        None,
        "the slot reopens on the first frame"
    );
    // From the first frame: out of the bar, blurred and sunk.
    assert_eq!(fade(&island, volume(), t), Some((0.0, 4.0, 10.0)));
    let mid = t + ms(60);
    step(&mut island, mid);
    let (k, blur, drop) = fade(&island, volume(), mid).expect("fading");
    assert!(k > 0.0 && k < 1.0, "{k}");
    assert!((blur - 4.0 * (1.0 - k)).abs() < 1e-4);
    assert!((drop - 10.0 * (1.0 - k)).abs() < 1e-4);
    // The window is still there while the piece is back (260 ms).
    let end = t + ms(199);
    step(&mut island, end);
    assert!(fade(&island, volume(), end).is_some_and(|(k, ..)| k > 0.99));
    assert!(island.sheets.is_shown(end));
    let done = t + ms(200);
    step(&mut island, done);
    assert_eq!(fade(&island, volume(), done), None);
}

#[test]
fn the_subtitle_is_never_hidden() {
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
    // And through the whole close, until the window has gone.
    let t = t0 + ms(400);
    island.close_sheet(t);
    for dt in [0, 30, 70, 150, 259] {
        step(&mut island, t + ms(dt));
        assert!(shown(&island), "close +{dt} ms");
    }
}

#[test]
fn reopening_during_a_close_hands_over() {
    // Asked again while its window closes, the piece's sheet waits for the
    // close; the piece is back in the bar by then and leaves once, whole.
    let (mut island, t) = settled();
    island.close_sheet(t);
    step(&mut island, t);
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(volume(), core, t + ms(60)));
    assert!(island.sheets.has_queued());
    let mid = t + ms(120);
    step(&mut island, mid);
    assert!(fade(&island, volume(), mid).is_some_and(|(k, ..)| k > 0.0 && k < 1.0));
    let back = t + ms(230);
    step(&mut island, back);
    assert_eq!(fade(&island, volume(), back), None, "shown, not fading");
    assert_eq!(island.bar.as_ref().and_then(|b| b.lifted()), None);
    let again = t + ms(262);
    step(&mut island, again);
    step(&mut island, again + ms(1));
    assert_eq!(island.bar.as_ref().and_then(|b| b.lifted()), Some(volume()));
    assert!(fade(&island, volume(), again + ms(1)).is_some_and(|(k, ..)| k > 0.9));
}

#[test]
fn a_switch_back_hands_the_fade_over_from_where_the_piece_is() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    // Switch to the brightness: the volume comes back, the brightness goes.
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    step(&mut island, t);
    assert!(fade(&island, volume(), t).is_some_and(|(k, ..)| k == 0.0));
    assert!(fade(&island, brightness_key(), t).is_some_and(|(k, ..)| k == 1.0));
    // And back to the volume while it is still coming in.
    let back = t + ms(60);
    step(&mut island, back);
    let (shown, ..) = fade(&island, volume(), back).expect("coming in");
    assert!(shown > 0.0 && shown < 1.0, "{shown}");
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(volume(), core, back));
    step(&mut island, back);
    // It leaves again from the opacity it had: no jump, no second copy.
    let (now, ..) = fade(&island, volume(), back).expect("leaving");
    assert!((now - shown).abs() < 1e-4, "{now} against {shown}");
    let later = back + ms(40);
    step(&mut island, later);
    assert!(fade(&island, volume(), later).is_some_and(|(k, ..)| k < shown));
    // The one the switch left comes in again from where it was.
    assert!(fade(&island, brightness_key(), later).is_some_and(|(k, ..)| k > 0.0));
    step(&mut island, back + ms(400));
    assert_eq!(fade(&island, volume(), back + ms(400)), None);
    assert_eq!(fade(&island, brightness_key(), back + ms(400)), None);
}

#[test]
fn a_vanished_piece_drops_its_fade() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    // The piece leaves the bar while it fades out: the sheet closes on its
    // last anchor and nothing stays hidden or fading.
    let t = t0 + ms(60);
    island.levels.audio.output = None;
    island.invalidate_bar();
    step(&mut island, t);
    assert!(!island.sheets.is_open(), "the sheet closes");
    assert_eq!(fade(&island, volume(), t), None, "the clock went with it");
    // The slot is given back on the next frame; no fade starts for it.
    step(&mut island, t + ms(1));
    assert_eq!(fade(&island, volume(), t + ms(1)), None);
    assert_eq!(island.bar.as_ref().and_then(|b| b.lifted()), None);
    let rest = t + ms(900);
    step(&mut island, rest);
    assert!(!island.bar.as_ref().is_some_and(|b| b.is_animating(rest)));
}

#[test]
fn reduced_motion_fades_the_piece_without_blur_or_offset() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    assert!(open_on(&mut island, volume(), t0));
    let mid = t0 + ms(50);
    step(&mut island, mid);
    let (k, blur, drop) = fade(&island, volume(), mid).expect("fading");
    assert!(k > 0.0 && k < 1.0, "{k}");
    assert_eq!((blur, drop), (0.0, 0.0));
    let t = t0 + ms(400);
    step(&mut island, t);
    island.close_sheet(t);
    step(&mut island, t);
    // The window only fades, and the piece fades back in beside it: it does
    // not pop in when the window has gone.
    assert_eq!(fade(&island, volume(), t), Some((0.0, 0.0, 0.0)));
    let mid = t + ms(60);
    step(&mut island, mid);
    let (k, blur, drop) = fade(&island, volume(), mid).expect("fading");
    assert!(k > 0.0 && k < 1.0, "{k}");
    assert_eq!((blur, drop), (0.0, 0.0));
    let done = t + ms(260);
    step(&mut island, done);
    assert_eq!(fade(&island, volume(), done), None);
    assert!(!island.sheets.is_shown(done));
}

/// The fading `piece` stays inside its capsule as painted and clear of the
/// pieces beside it, wherever its slot is at `now`.
fn assert_in_slot(island: &Island, piece: Key, now: Instant, what: &str) {
    let bar = island.bar.as_ref().expect("a bar");
    let Some((clip, _, beside)) = bar.fade_slot(piece, now) else {
        return;
    };
    let zone = bar.zone_of(piece).expect("a zone");
    let capsule = bar.capsule_extent(zone, now).expect("a capsule");
    assert!(
        clip.x >= capsule.x - 0.01 && clip.x + clip.w <= capsule.x + capsule.w + 0.01,
        "{what}: {clip:?} outside {capsule:?}"
    );
    for n in beside {
        let clear = clip.w <= 0.0 || clip.x + clip.w <= n.x + 0.01 || clip.x >= n.x + n.w - 0.01;
        assert!(clear, "{what}: {clip:?} over {n:?}");
    }
}

fn open_session(island: &mut Island, t: Instant) {
    let session = Key::named("session");
    assert_eq!(
        island.sheet_request(
            session,
            crate::pieces::SheetKind::Session,
            t,
            &mut fake_measure
        ),
        None
    );
    step(island, t);
}

#[test]
fn the_fading_piece_stays_in_its_slot_and_its_capsule() {
    for reduced in [false, true] {
        for session in [false, true] {
            let (mut island, t0) = level_island();
            island.set_reduced_motion(reduced);
            let piece = if session {
                Key::named("session")
            } else {
                volume()
            };
            if session {
                open_session(&mut island, t0);
            } else {
                assert!(open_on(&mut island, volume(), t0));
            }
            for dt in [0, 10, 30, 60, 100, 159] {
                step(&mut island, t0 + ms(dt));
                assert_in_slot(
                    &island,
                    piece,
                    t0 + ms(dt),
                    &format!("open +{dt} {reduced} {session}"),
                );
            }
            let t = t0 + ms(400);
            step(&mut island, t);
            island.close_sheet(t);
            for dt in [0, 10, 30, 60, 100, 150, 199] {
                step(&mut island, t + ms(dt));
                assert_in_slot(
                    &island,
                    piece,
                    t + ms(dt),
                    &format!("close +{dt} {reduced} {session}"),
                );
            }
        }
    }
}

#[test]
fn the_fading_piece_is_cut_to_a_slot_that_is_still_shut() {
    // The first frame of a close: the slot is shut, so nothing of the piece
    // shows yet; the room grows with the neighbours' slide.
    let (mut island, t) = settled();
    island.close_sheet(t);
    step(&mut island, t);
    let bar = island.bar.as_ref().expect("a bar");
    let (first, ..) = bar.fade_slot(volume(), t).expect("fading");
    let own = bar.node_rect(volume()).expect("its rect");
    assert!(first.w < own.w, "{first:?} against {own:?}");
    step(&mut island, t + ms(120));
    let bar = island.bar.as_ref().expect("a bar");
    let (later, ..) = bar.fade_slot(volume(), t + ms(120)).expect("fading");
    assert!(later.w > first.w, "the slot has opened: {later:?}");
}

#[test]
fn neighbours_that_switch_share_the_slot_room() {
    // Volume and brightness sit side by side: a switch from one to the other
    // fades one out and the other in, and they share the room between their
    // steady neighbours, the leaving one giving way.
    let (mut island, t) = settled();
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t));
    let mut last_leaving = f32::INFINITY;
    for dt in [0, 20, 50, 90, 130, 150] {
        let now = t + ms(dt);
        step(&mut island, now);
        let bar = island.bar.as_ref().expect("a bar");
        let (leaving, ..) = bar
            .fade_slot(brightness_key(), now)
            .expect("brightness fades out");
        let (landing, ..) = bar.fade_slot(volume(), now).expect("volume fades in");
        if dt < 90 {
            assert!(
                leaving.w > 0.0,
                "+{dt}: the leaving piece still shows: {leaving:?}"
            );
        }
        assert!(leaving.w <= last_leaving + 0.01, "+{dt}: it only gives way");
        last_leaving = leaving.w;
        let overlap = (leaving.x + leaving.w).min(landing.x + landing.w) - leaving.x.max(landing.x);
        assert!(overlap <= 0.5, "+{dt}: {leaving:?} and {landing:?} overlap");
    }
}
