//! A menu window's glass is one Haze path with its capsule (CEL-D-013): the
//! capsule's glass is painted once on every frame, by the path while a drop
//! hangs from it and by the bar's tree otherwise, and the path lies under
//! the bar's content.

use celestina_shell::settings::BarLayout;

use super::*;
use crate::{
    island::sheets::GlassPath,
    pieces::tray::chevron_key,
    sheet::{
        drop::DropFrame,
        geometry::{self, Hit},
    },
};

/// The bar's zones.
const ZONES: std::ops::Range<usize> = 0..4;

/// One frame as the surface runs it: the sheets move, the layout settles,
/// then the glass is handed over.
fn frame(island: &mut Island, now: Instant) -> Vec<GlassPath> {
    step(island, now);
    island.prepare_glass(now)
}

/// How many times zone `zone`'s capsule glass is painted: by the bar's tree
/// and by a path joined to it.
fn glass_count(island: &Island, paths: &[GlassPath], zone: usize) -> usize {
    let tree = island
        .bar
        .as_ref()
        .is_some_and(|bar| bar.tree_paints_glass(zone));
    usize::from(tree) + paths.iter().filter(|p| p.joined == Some(zone)).count()
}

fn extent(island: &Island, zone: usize, now: Instant) -> Option<Rect> {
    island
        .bar
        .as_ref()
        .and_then(|bar| bar.capsule_extent(zone, now))
}

fn middle(r: Rect) -> (f32, f32) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

#[test]
fn an_open_window_and_its_capsule_are_one_haze_path() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let now = t0 + ms(350);
    let paths = frame(&mut island, now);
    assert_eq!(paths.len(), 1, "one fill for the capsule and its window");
    let joined = &paths[0];
    assert_eq!(joined.joined, Some(3));
    assert_eq!(joined.opacity, 1.0);
    let capsule = extent(&island, 3, now).expect("the capsule");
    let window = shown_rect(&island, now);
    assert!(joined.path.contains(middle(capsule).0, middle(capsule).1));
    assert!(joined.path.contains(middle(window).0, middle(window).1));
    // The tree paints no glass for it, and the other capsules keep theirs.
    assert!(!island.bar.as_ref().is_some_and(|b| b.tree_paints_glass(3)));
    for zone in ZONES.filter(|&z| z != 3 && extent(&island, z, now).is_some()) {
        assert_eq!(glass_count(&island, &paths, zone), 1, "zone {zone}");
        assert!(island
            .bar
            .as_ref()
            .is_some_and(|b| b.tree_paints_glass(zone)));
    }
}

#[test]
fn the_capsule_glass_is_painted_once_on_every_frame_of_an_open_and_a_close() {
    let (mut island, t0) = level_island();
    let zones: Vec<usize> = ZONES
        .filter(|&z| extent(&island, z, t0).is_some())
        .collect();
    assert!(zones.contains(&3), "premise: the end capsule exists");
    let check = |island: &Island, paths: &[GlassPath], n: u64, what: &str| {
        for &zone in &zones {
            assert_eq!(
                glass_count(island, paths, zone),
                1,
                "{what} +{n} ms: zone {zone}"
            );
        }
    };
    // Before the open.
    let paths = frame(&mut island, t0);
    check(&island, &paths, 0, "before");
    // The open, from its first frame to well after it has settled.
    assert!(open_on(&mut island, volume(), t0));
    let mut joined = Vec::new();
    for n in (0..=700).step_by(10) {
        let paths = frame(&mut island, t0 + ms(n));
        check(&island, &paths, n, "open");
        joined.push(paths.iter().any(|p| p.joined == Some(3)));
    }
    assert!(joined.iter().all(|j| *j), "joined from the first frame on");
    // The close, from its first frame to the frame its window is reaped.
    let close = t0 + ms(800);
    island.close_sheet(close);
    let mut after = Vec::new();
    for n in (0..=560).step_by(10) {
        let paths = frame(&mut island, close + ms(n));
        check(&island, &paths, n, "close");
        after.push((n, paths.iter().any(|p| p.joined == Some(3))));
    }
    assert!(after.first().is_some_and(|(_, j)| *j), "joined at first");
    let first_free = after.iter().find(|(_, j)| !*j).map(|(n, _)| *n);
    assert_eq!(
        first_free,
        Some(400),
        "the tree takes over as the close ends"
    );
    assert!(!island.sheets.is_shown(close + ms(560)));
}

#[test]
fn the_hand_over_frames_paint_the_glass_once() {
    // The frame the sheet appears on and the frame its close ends on, one
    // millisecond each side.
    let (mut island, t0) = level_island();
    assert_eq!(frame(&mut island, t0).len(), 0, "nothing joined at rest");
    assert!(open_on(&mut island, volume(), t0));
    let first = frame(&mut island, t0);
    assert_eq!(first.len(), 1);
    assert_eq!(glass_count(&island, &first, 3), 1);
    let close = t0 + ms(700);
    island.close_sheet(close);
    for n in [399, 400, 401] {
        let paths = frame(&mut island, close + ms(n));
        assert_eq!(glass_count(&island, &paths, 3), 1, "+{n} ms");
        assert_eq!(paths.len(), usize::from(n < 400), "+{n} ms");
    }
}

#[test]
fn the_glass_is_handed_over_before_the_bar_paints() {
    // `Island::paint` paints the paths, then the bar. The bar's tree must
    // already be glassless for the zone by then: one call does both.
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let now = t0 + ms(300);
    step(&mut island, now);
    assert!(
        island.bar.as_ref().is_some_and(|b| b.tree_paints_glass(3)),
        "premise: the tree still has its glass before the hand-over"
    );
    let paths = island.prepare_glass(now);
    assert_eq!(paths.len(), 1);
    assert!(!island.bar.as_ref().is_some_and(|b| b.tree_paints_glass(3)));
}

#[test]
fn two_capsules_drop_at_once_each_in_its_own_path() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let bell_zone = island
        .bar
        .as_ref()
        .and_then(|b| b.zone_of(bell))
        .expect("the bell's zone");
    assert_ne!(bell_zone, 3, "premise: another capsule");
    let at = centre(bar_rect(&island, bell));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(island.sheets.is_leaving(), "the old one closes meanwhile");
    for n in [10, 60, 150, 250, 350, 399] {
        let now = t + ms(n);
        let paths = frame(&mut island, now);
        let zones: Vec<_> = paths.iter().filter_map(|p| p.joined).collect();
        assert_eq!(zones.len(), 2, "+{n} ms: {zones:?}");
        assert!(zones.contains(&3) && zones.contains(&bell_zone), "+{n} ms");
        for p in &paths {
            let own = p.joined.and_then(|z| extent(&island, z, now));
            let other = ZONES
                .filter(|z| Some(*z) != p.joined)
                .filter_map(|z| extent(&island, z, now))
                .map(middle);
            let own = own.map(middle).expect("its capsule");
            assert!(p.path.contains(own.0, own.1), "+{n} ms: its own capsule");
            for (x, y) in other {
                assert!(!p.path.contains(x, y), "+{n} ms: no other capsule");
            }
        }
        for zone in ZONES.filter(|&z| extent(&island, z, now).is_some()) {
            assert_eq!(
                glass_count(&island, &paths, zone),
                1,
                "+{n} ms: zone {zone}"
            );
        }
    }
    let after = t + ms(400);
    let paths = frame(&mut island, after);
    assert_eq!(paths.len(), 1, "the old window has closed");
    assert_eq!(paths[0].joined, Some(bell_zone));
}

#[test]
fn a_reduced_motion_window_is_its_own_path_under_its_glass() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    assert!(open_on(&mut island, volume(), t0));
    // Half way through the 150 ms fade.
    let now = t0 + ms(75);
    let paths = frame(&mut island, now);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].joined, None, "not joined to its capsule");
    assert!(
        (paths[0].opacity - 0.5).abs() < 0.02,
        "under its glass: {}",
        paths[0].opacity
    );
    let window = shown_rect(&island, now);
    assert!(paths[0].path.contains(middle(window).0, middle(window).1));
    let capsule = extent(&island, 3, now).expect("the capsule");
    assert!(
        !paths[0].path.contains(middle(capsule).0, middle(capsule).1),
        "the capsule's glass is the tree's"
    );
    assert_eq!(glass_count(&island, &paths, 3), 1);
    // Faded in: it joins the capsule's path.
    let paths = frame(&mut island, t0 + ms(150));
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].joined, Some(3));
    assert_eq!(glass_count(&island, &paths, 3), 1);
    // Fading out on the close: its own path again, at its glass.
    let close = t0 + ms(400);
    island.close_sheet(close);
    let paths = frame(&mut island, close + ms(75));
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].joined, None);
    assert!((paths[0].opacity - 0.5).abs() < 0.02);
    assert_eq!(glass_count(&island, &paths, 3), 1);
    let gone = frame(&mut island, close + ms(150));
    assert!(gone.is_empty());
    assert_eq!(glass_count(&island, &gone, 3), 1);
}

#[test]
fn the_tray_chevron_window_joins_the_capsule_that_holds_the_chevron() {
    let t0 = Instant::now();
    let mut island = Island::new("DP-1", (1920.0, 560.0));
    island.model = Some(model("DP-1", 1));
    island.rows = rows_for(&model("DP-1", 1), "DP-1");
    island.tray_rows = ["a", "b", "c", "d"]
        .iter()
        .enumerate()
        .map(|(n, sni)| TrayRow {
            id: ItemId {
                bus_name: format!(":1.{n}"),
                path: "/StatusNotifierItem".to_owned(),
            },
            sni_id: (*sni).to_owned(),
            title: format!("app {n}"),
            has_menu: true,
            ..TrayRow::default()
        })
        .collect();
    island.set_bar(&BarLayout {
        tray_hidden: vec!["a".to_owned(), "c".to_owned()],
        ..BarLayout::default()
    });
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = t0 + Duration::from_secs(1);
    let zone = island
        .bar
        .as_ref()
        .and_then(|b| b.zone_of(chevron_key()))
        .expect("the zone holding the chevron");
    let at = centre(bar_rect(&island, chevron_key()));
    click_at(&mut island, at, BTN_LEFT, t);
    assert_eq!(island.sheets.piece(), Some(chevron_key()));
    let now = t + ms(350);
    let paths = frame(&mut island, now);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].joined, Some(zone));
    let capsule = extent(&island, zone, now).expect("the capsule");
    let chevron = bar_rect(&island, chevron_key());
    assert!(
        capsule.w > chevron.w,
        "premise: the capsule is more than the chevron"
    );
    // The capsule's own glass, away from the chevron, is in the path.
    assert!(paths[0].path.contains(capsule.x + 2.0, middle(capsule).1));
    let window = shown_rect(&island, now);
    assert!(paths[0].path.contains(middle(window).0, middle(window).1));
    assert_eq!(glass_count(&island, &paths, zone), 1);
}

#[test]
fn a_window_never_fades_with_a_capsule_whose_pieces_are_leaving() {
    let (mut island, t) = settled();
    island.set_bar(&BarLayout {
        start: vec!["notifications".to_owned()],
        centre: Vec::new(),
        end: Vec::new(),
        tray_hidden: Vec::new(),
    });
    island.apply_views(t);
    step(&mut island, t + ms(1));
    // The volume piece is gone: its window closes towards the last pill
    // while the capsule fades out. Whatever the capsule does, the window's
    // glass is whole and the capsule's glass is painted once.
    let mut fading = false;
    for n in (1..=399).step_by(7) {
        let now = t + ms(n);
        let paths = frame(&mut island, now);
        assert_eq!(paths.len(), 1, "+{n} ms");
        assert_eq!(paths[0].opacity, 1.0, "+{n} ms: the window's glass");
        fading |= paths[0].joined.is_none();
        let bounds = paths[0].path.bounds().expect("a shape");
        assert!(
            [bounds.x, bounds.y, bounds.w, bounds.h]
                .iter()
                .all(|v| v.is_finite()),
            "+{n} ms: {bounds:?}"
        );
        assert_eq!(glass_count(&island, &paths, 3), 1, "+{n} ms");
    }
    assert!(fading, "premise: the capsule fades while the window closes");
    let done = frame(&mut island, t + ms(402));
    assert!(done.is_empty());
    assert_eq!(glass_count(&island, &done, 3), 1);
}

fn covers(bands: &[Rect], at: (f32, f32)) -> bool {
    bands.iter().any(|b| b.contains(at.0, at.1))
}

/// Whether the pixel centre (`x` + 0.5, `y` + 0.5) is painted glass at `now`:
/// inside a glass path, or inside a bar capsule the paths do not hold.
fn glass_at(island: &Island, paths: &[GlassPath], now: Instant, x: i32, y: i32) -> bool {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    if paths.iter().any(|p| p.path.contains(px, py)) {
        return true;
    }
    island.shapes(now).iter().any(|&(rect, radius)| {
        let rad = radius.min(rect.w / 2.0).min(rect.h / 2.0);
        let cx = px.clamp(rect.x + rad, rect.x + rect.w - rad);
        let cy = py.clamp(rect.y + rad, rect.y + rect.h - rad);
        rect.contains(px, py) && (px - cx).powi(2) + (py - cy).powi(2) <= rad * rad
    })
}

/// Every sampled pixel painted at `now` has a band: every pixel of the
/// neck's rows, one in `stride` elsewhere.
fn assert_covered(island: &mut Island, now: Instant, label: &str) {
    let paths = frame(island, now);
    let bands = island.blur_bands(now);
    let (w, h) = (island.size.0 as i32, island.size.1 as i32);
    let stride = 11;
    for y in 0..h {
        let rows = (40..72).contains(&y);
        let step = if rows { 3 } else { stride };
        if !rows && y % stride != 0 {
            continue;
        }
        let mut x = 0;
        while x < w {
            if glass_at(island, &paths, now, x, y)
                && !covers(&bands, (x as f32 + 0.5, y as f32 + 0.5))
            {
                panic!("{label}: painted ({x},{y}) has no band");
            }
            x += step;
        }
    }
}

#[test]
fn the_region_never_misses_a_painted_pixel_through_a_whole_open_and_close() {
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    for n in (0..=480).step_by(20) {
        assert_covered(&mut island, t0 + ms(n), &format!("open +{n} ms"));
    }
    let t = t0 + ms(500);
    step(&mut island, t);
    island.close_sheet(t);
    for n in (0..=420).step_by(20) {
        assert_covered(&mut island, t + ms(n), &format!("close +{n} ms"));
    }
}

#[test]
fn the_region_covers_two_drops_at_once() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let at = centre(bar_rect(&island, bell));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(island.sheets.is_leaving(), "premise: two drops");
    for n in (10..=410).step_by(40) {
        assert_covered(&mut island, t + ms(n), &format!("two drops +{n} ms"));
    }
}

#[test]
fn the_region_keeps_the_window_until_the_capsule_absorbs_it() {
    let (mut island, t) = settled();
    island.close_sheet(t);
    // No fraction of the glass's opacity drops the window from the region:
    // the drop's pixels stay covered until the sheet is reaped.
    for n in [1, 100, 200, 300, 399] {
        let now = t + ms(n);
        let paths = frame(&mut island, now);
        let bands = island.blur_bands(now);
        assert!(!paths.is_empty(), "+{n} ms: a path still holds the glass");
        let joined = paths.iter().find(|p| p.joined == Some(3));
        let capsule = extent(&island, 3, now).expect("the capsule");
        assert!(joined.is_some(), "+{n} ms");
        assert!(covers(&bands, middle(capsule)), "+{n} ms: the capsule");
        let below = (middle(capsule).0, capsule.y + capsule.h + 1.0);
        let in_path = joined.is_some_and(|p| p.path.contains(below.0, below.1));
        assert!(
            !in_path || covers(&bands, below),
            "+{n} ms: what the drop paints below the capsule is covered"
        );
    }
}

#[test]
fn after_the_close_no_region_of_the_old_window_is_left() {
    let (mut island, t) = settled();
    let window = shown_rect(&island, t);
    island.close_sheet(t);
    let before = t + ms(399);
    step(&mut island, before);
    assert!(island.sheets.is_shown(before));
    let after = t + ms(400);
    let paths = frame(&mut island, after);
    assert!(paths.is_empty(), "no path once the sheet is reaped");
    let bands = island.blur_bands(after);
    assert!(!covers(&bands, middle(window)), "the window's region");
    assert_eq!(
        bands,
        silhouette::union(&island.shapes(after)),
        "only the capsules and the Island are left"
    );
}

#[test]
fn the_region_has_at_most_64_rects_for_a_capsule_a_window_and_a_neck() {
    let (mut island, t0) = level_island();
    let mut most = 0;
    let mut check = |island: &mut Island, now: Instant, label: &str| {
        let paths = frame(island, now);
        let count: usize = paths
            .iter()
            .map(|p| celestina_paint::slices_of_path(&p.path, state::SILHOUETTE_STEP).len())
            .sum();
        assert!(count <= 64, "{label}: {count} rects");
        most = most.max(count);
        // A capsule joined to a path is in its bands once: the region holds
        // no more than the paths and the capsules left.
        let capsules = island.shapes(now).len();
        let bands = island.blur_bands(now).len();
        assert!(bands <= count + capsules * 24, "{label}: {bands} rects");
    };
    assert!(open_on(&mut island, volume(), t0));
    for n in (0..=480).step_by(10) {
        check(&mut island, t0 + ms(n), &format!("open +{n} ms"));
    }
    let t = t0 + ms(500);
    step(&mut island, t);
    island.close_sheet(t);
    for n in (0..=410).step_by(10) {
        check(&mut island, t + ms(n), &format!("close +{n} ms"));
    }
    assert!(most > 0, "premise: a path had bands");
    eprintln!("most rects in one frame: {most}");
}

/// The bar's open pill and the held piece, as the bar has them at `now`.
fn pill_alpha(island: &Island, now: Instant) -> f32 {
    island
        .bar
        .as_ref()
        .and_then(|b| b.pill(now).first().map(|p| p.alpha))
        .unwrap_or(0.0)
}

fn held(island: &Island) -> Option<Key> {
    island.bar.as_ref().and_then(|b| b.open_piece())
}

fn content_opacity(island: &Island, key: Key, now: Instant) -> f32 {
    island
        .bar
        .as_ref()
        .map_or(1.0, |b| b.content_opacity(key, now))
}

#[test]
fn the_pill_and_the_opacity_return_at_80_percent_of_the_close() {
    let (mut island, t) = settled();
    step(&mut island, t);
    assert!(near_f(pill_alpha(&island, t), 1.0), "premise: the pill");
    island.close_sheet(t);
    assert_eq!(island.sheets.held_piece(t), Some(volume()));
    assert_eq!(island.sheets.held_piece(t + ms(319)), Some(volume()));
    assert_eq!(island.sheets.held_piece(t + ms(320)), None);
    // Until the merge the pill stays whole and the piece stays muted.
    for n in [0, 100, 168, 250, 319] {
        let now = t + ms(n);
        step(&mut island, now);
        assert_eq!(held(&island), Some(volume()), "+{n} ms");
        assert!(near_f(pill_alpha(&island, now), 1.0), "+{n} ms: the pill");
        assert!(
            near_f(content_opacity(&island, volume(), now), 0.8),
            "+{n} ms: the piece"
        );
    }
    // At the merge the pill fades out over 150 ms and the piece goes back
    // to whole over 200 ms.
    let merge = t + ms(320);
    step(&mut island, merge);
    assert_eq!(held(&island), None);
    assert!(near_f(pill_alpha(&island, merge), 1.0), "starts whole");
    let mid = merge + ms(75);
    step(&mut island, mid);
    let want = 1.0 - celestina_ui::EXIT.eval(0.5);
    assert!(near_f(pill_alpha(&island, mid), want));
    step(&mut island, merge + ms(150));
    assert!(island
        .bar
        .as_ref()
        .is_some_and(|b| b.pill(merge + ms(150)).is_empty()));
    step(&mut island, merge + ms(200));
    assert_eq!(content_opacity(&island, volume(), merge + ms(200)), 1.0);
}

#[test]
fn reduced_motion_changes_the_pill_and_the_opacity_at_once() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    assert!(open_on(&mut island, volume(), t0));
    // Open: the pill shows and the piece mutes at the first frame.
    assert!(near_f(pill_alpha(&island, t0), 1.0), "the pill at once");
    assert!(
        near_f(content_opacity(&island, volume(), t0), 0.8),
        "the piece at once"
    );
    let t = t0 + ms(500);
    step(&mut island, t);
    island.close_sheet(t);
    // Close: no drop to wait for, so both go back on the close's first frame
    // with no fade, while the window's own 150 ms fade runs.
    assert_eq!(island.sheets.held_piece(t), None);
    step(&mut island, t);
    assert_eq!(held(&island), None);
    assert!(
        island.bar.as_ref().is_some_and(|b| b.pill(t).is_empty()),
        "no pill left to fade"
    );
    assert_eq!(content_opacity(&island, volume(), t), 1.0);
    assert!(island.sheets.is_shown(t + ms(75)), "the window still fades");
    // An open that is still running when it closes does the same.
    let again = t + ms(600);
    assert!(open_on(&mut island, volume(), again));
    island.close_sheet(again + ms(40));
    assert_eq!(island.sheets.held_piece(again + ms(40)), None);
}

fn near_f(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn near_r(a: Rect, b: Rect) -> bool {
    near_f(a.x, b.x) && near_f(a.y, b.y) && near_f(a.w, b.w) && near_f(a.h, b.h)
}

/// The sheet's frame at `now`.
fn sheet_frame(island: &Island, now: Instant) -> DropFrame {
    let anchor = island.sheet_anchor(now).expect("an anchor");
    island.sheets.frame(&anchor, now).expect("a shown window")
}

/// That the window shown at `now` is a plain travelling window: drawn, with
/// no liquid left and a corner radius on its way to the theme's large one.
fn assert_travelling(island: &Island, frame: DropFrame, label: &str) {
    assert!(frame.drawn, "{label}: drawn");
    assert!(frame.liquid.is_none(), "{label}: no thread or tip");
    assert!(
        frame.radius <= island.theme.radius_lg + 1e-3,
        "{label}: radius {}",
        frame.radius
    );
}

#[test]
fn a_switch_mid_drop_travels_from_the_shown_frame() {
    // Each instant is a phase of the open: the swell, the fall (liquid only),
    // the first frames of the window and its overshoot.
    for n in [60u64, 100, 184, 235, 245, 322, 400] {
        let (mut island, t0) = level_island();
        assert!(open_on(&mut island, volume(), t0));
        let now = t0 + ms(n);
        step(&mut island, now);
        let shown = sheet_frame(&island, now);
        let core = volume_core(&island.theme);
        assert!(island.open_level_sheet_at(brightness_key(), core, now));
        step(&mut island, now);
        // One window, on its way from what was shown; nothing is left of the
        // drop and no second window starts from the piece.
        let first = sheet_frame(&island, now);
        assert!(
            near_r(first.rect, shown.rect),
            "+{n} ms: {first:?} {shown:?}"
        );
        assert_travelling(&island, first, &format!("+{n} ms"));
        assert!(
            near_f(first.radius, shown.radius),
            "+{n} ms: starts from the radius shown, {} against {}",
            first.radius,
            shown.radius
        );
        assert!(!island.sheets.is_leaving(), "+{n} ms: one window");
        let anchor = island.sheet_anchor(now).expect("an anchor");
        let glass = island.sheets.glass(Some(&anchor), island.size, now);
        assert_eq!(glass.len(), 1, "+{n} ms");
        assert!(glass[0].frame.liquid.is_none() && glass[0].frame.drawn);
        // Joined to its capsule: one path, no liquid, the capsule's glass once.
        let paths = frame(&mut island, now);
        assert_eq!(paths.len(), 1, "+{n} ms");
        assert_eq!(glass_count(&island, &paths, 3), 1, "+{n} ms");
        // It travels: from the shown rect towards the target, the old
        // content on its way out and the new one coming in.
        let target = sheet_frame(&island, now + ms(400)).rect;
        let mid = sheet_frame(&island, now + ms(60));
        assert_travelling(&island, mid, &format!("+{n}+60 ms"));
        let (a, b) = (first.rect, mid.rect);
        let between = |lo: f32, hi: f32, v: f32| v >= lo.min(hi) - 0.01 && v <= lo.max(hi) + 0.01;
        assert!(
            between(a.w, target.w, b.w) && between(a.h, target.h, b.h),
            "+{n} ms: {a:?} {b:?} {target:?}"
        );
        let end = sheet_frame(&island, now + ms(300));
        assert!(near_r(end.rect, target), "+{n} ms: it arrives");
        assert!(near_f(end.radius, island.theme.radius_lg), "+{n} ms");
        assert_eq!(end.content, 1.0);
    }
}

#[test]
fn a_switch_from_the_bulb_starts_from_the_bulbs_box_without_its_liquid() {
    // The swell: only the bulb shows. The travel starts from its box at the
    // radius the bulb shows (a circle) and the liquid goes.
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let now = t0 + ms(100);
    step(&mut island, now);
    let bulb = sheet_frame(&island, now);
    assert!(!bulb.drawn && bulb.liquid.is_some(), "premise: only liquid");
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, now));
    step(&mut island, now);
    let first = sheet_frame(&island, now);
    assert!(near_r(first.rect, bulb.rect));
    assert!(first.drawn && first.liquid.is_none());
    assert!(near_f(first.radius, bulb.radius));
    assert!(first.rect.w <= 2.0 * first.radius + 1e-3 && first.rect.h <= 2.0 * first.radius + 1e-3);
    // No content yet: the old one was hidden, the new one starts at 0.
    assert_eq!(first.content, 0.0);
    // The path of that frame holds the capsule and the box.
    let paths = frame(&mut island, now);
    assert_eq!(paths.len(), 1);
    assert!(
        paths[0]
            .path
            .contains(middle(first.rect).0, middle(first.rect).1),
        "the box is painted"
    );
}

#[test]
fn reopening_during_the_close_travels_from_the_shown_frame() {
    // The gather (a drawn window shrinking), the rise (liquid only) and the
    // absorb, with the pill held, and after the merge with it gone.
    for n in [20u64, 100, 168, 220, 300, 330, 380] {
        let (mut island, t) = settled();
        island.close_sheet(t);
        let now = t + ms(n);
        step(&mut island, now);
        let shown = sheet_frame(&island, now);
        // A press on the same piece does not close it again: it asks for the
        // menu once more.
        let asked = island.sheet_request(
            volume(),
            crate::pieces::SheetKind::Volume,
            now,
            &mut fake_measure,
        );
        assert!(
            matches!(asked, Some(IslandAction::OpenLevelSheet { .. })),
            "+{n} ms: {asked:?}"
        );
        let core = volume_core(&island.theme);
        assert!(island.open_level_sheet_at(volume(), core, now));
        step(&mut island, now);
        assert!(island.sheets.is_open(), "+{n} ms: open again");
        assert!(!island.sheets.is_leaving(), "+{n} ms: one window");
        let first = sheet_frame(&island, now);
        assert!(
            near_r(first.rect, shown.rect),
            "+{n} ms: {first:?} {shown:?}"
        );
        assert_travelling(&island, first, &format!("+{n} ms"));
        assert!(
            near_f(first.radius, shown.radius),
            "+{n} ms: starts from the radius shown"
        );
        // Never a new drop from the piece: the window stays out of the
        // liquid-only phase to its end, and the capsule's glass is once.
        for m in (0..=300).step_by(20) {
            let at = now + ms(m);
            let f = sheet_frame(&island, at);
            assert!(f.drawn && f.liquid.is_none(), "+{n}+{m} ms");
            let paths = frame(&mut island, at);
            assert_eq!(glass_count(&island, &paths, 3), 1, "+{n}+{m} ms");
        }
        // The piece is held again, pill and all, once it is open.
        assert_eq!(island.sheets.held_piece(now + ms(10)), Some(volume()));
        step(&mut island, now + ms(400));
        assert_eq!(held(&island), Some(volume()), "+{n} ms");
        assert!(near_f(pill_alpha(&island, now + ms(400)), 1.0), "+{n} ms");
    }
}

#[test]
fn another_capsule_opens_at_once_while_the_first_drops_back() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let bell_zone = island
        .bar
        .as_ref()
        .and_then(|b| b.zone_of(bell))
        .expect("the bell's zone");
    assert_ne!(bell_zone, 3, "premise: another capsule");
    let before = sheet_frame(&island, t);
    let at = centre(bar_rect(&island, bell));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    // At once: nothing waits, the new menu is open and the old window closes
    // on its own clock towards its own piece.
    assert!(!island.sheets.has_queued());
    assert!(island.sheets.is_open());
    assert_eq!(island.sheets.piece(), Some(bell));
    assert_eq!(island.sheets.leaving_pieces(t + ms(10)), [volume()]);
    let anchor = island.sheet_anchor(t).expect("an anchor");
    let first = island.sheets.glass(Some(&anchor), island.size, t);
    assert_eq!(
        first.iter().map(|g| g.zone).collect::<Vec<_>>(),
        [3, bell_zone],
        "two windows, two zones"
    );
    // The old window starts its close from the frame it showed; the new one
    // starts its drop from the bell's piece.
    assert!(near_r(first[0].frame.rect, before.rect));
    assert!(!first[1].frame.drawn && first[1].frame.liquid.is_some());
    for n in [50, 150, 300, 399] {
        let now = t + ms(n);
        let anchor = island.sheet_anchor(now).expect("an anchor");
        let glass = island.sheets.glass(Some(&anchor), island.size, now);
        assert_eq!(glass.len(), 2, "+{n} ms");
        assert_ne!(glass[0].zone, glass[1].zone, "+{n} ms");
        assert!(island.sheets.is_leaving(), "+{n} ms");
    }
    // The old one is reaped at the end of its close; the new one goes on.
    let done = t + ms(400);
    step(&mut island, done);
    let anchor = island.sheet_anchor(done).expect("an anchor");
    let glass = island.sheets.glass(Some(&anchor), island.size, done);
    assert_eq!(
        glass.iter().map(|g| g.zone).collect::<Vec<_>>(),
        [bell_zone]
    );
}

#[test]
fn a_press_on_the_falling_bulb_does_not_reach_the_body() {
    // During the fall only liquid shows: its bulb's box is no window, so a
    // press there is a press outside, as one on a closing window is.
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    let now = t0 + ms(180);
    step(&mut island, now);
    let frame = sheet_frame(&island, now);
    assert!(!frame.drawn, "premise: only liquid at +180 ms");
    let at = middle(frame.rect);
    assert!(
        matches!(geometry::hit(frame.rect, at.0, at.1), Hit::Body),
        "premise: the box would be the body's"
    );
    assert!(island.sheets.is_open());
    assert!(click_at(&mut island, at, BTN_LEFT, now).is_empty());
    assert!(
        !island.sheets.is_open(),
        "outside the window: the press closes it"
    );
    // A drawn window still takes its own presses.
    let (mut island, t) = settled();
    let sheet = shown_rect(&island, t);
    assert!(sheet_frame(&island, t).drawn);
    let body = (sheet.x + sheet.w / 2.0, sheet.y + sheet.h - 4.0);
    assert!(click_at(&mut island, body, BTN_LEFT, t).is_empty());
    assert!(island.sheets.is_open());
}

#[test]
fn under_reduced_motion_a_faint_window_leaves_the_blur_region() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    assert!(open_on(&mut island, volume(), t0));
    let window = |island: &Island, now: Instant| middle(shown_rect(island, now));
    // 20 ms into the 150 ms fade the glass is 0.13: no region, though the
    // window's glass is painted at that opacity.
    let now = t0 + ms(20);
    let paths = frame(&mut island, now);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].joined, None);
    assert!(paths[0].opacity > 0.0 && paths[0].opacity < 0.35);
    let at = window(&island, now);
    assert!(paths[0].path.contains(at.0, at.1), "premise: painted");
    assert!(!covers(&island.blur_bands(now), at));
    // At 0.4 it is back in the region.
    let now = t0 + ms(60);
    let paths = frame(&mut island, now);
    assert!(paths[0].opacity > 0.35 && paths[0].opacity < 1.0);
    let at = window(&island, now);
    assert!(covers(&island.blur_bands(now), at));
    // Fading out on the close: in the region above 0.35, out below it.
    let close = t0 + ms(400);
    island.close_sheet(close);
    for (n, in_region) in [(30u64, true), (120, false)] {
        let now = close + ms(n);
        let paths = frame(&mut island, now);
        assert_eq!(paths[0].joined, None, "+{n} ms");
        let at = window(&island, now);
        assert!(paths[0].path.contains(at.0, at.1), "+{n} ms: painted");
        assert_eq!(covers(&island.blur_bands(now), at), in_region, "+{n} ms");
    }
    // A whole glass is always in the region (outside reduced motion).
    let (mut island, t0) = level_island();
    assert!(open_on(&mut island, volume(), t0));
    for n in (0..=460).step_by(20) {
        let now = t0 + ms(n);
        let paths = frame(&mut island, now);
        assert!(paths.iter().all(|p| p.opacity == 1.0), "+{n} ms");
        assert_covered(&mut island, now, &format!("+{n} ms"));
    }
}

/// The pills the bar paints at `now`, by zone.
fn pill_in_zone(island: &Island, zone: usize, now: Instant) -> Option<f32> {
    island
        .bar
        .as_ref()
        .and_then(|b| b.pill(now).into_iter().find(|p| p.zone == zone))
        .map(|p| p.alpha)
}

#[test]
fn a_window_closing_on_another_capsule_holds_its_piece_until_its_merge() {
    let (mut island, t) = settled();
    let bell = Key::named("bell");
    let bell_zone = island
        .bar
        .as_ref()
        .and_then(|b| b.zone_of(bell))
        .expect("the bell's zone");
    let at = centre(bar_rect(&island, bell));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(island.sheets.is_leaving(), "premise: the old window closes");
    assert_eq!(island.sheets.held_pieces(t + ms(10)), [bell, volume()]);
    // The old window's pill stays whole and its piece muted until its drop
    // merges at 320 ms; the new window's are whole from the first frame.
    for n in [10, 100, 250, 319] {
        let now = t + ms(n);
        step(&mut island, now);
        assert!(
            near_f(pill_in_zone(&island, 3, now).unwrap_or(-1.0), 1.0),
            "+{n} ms: the old pill"
        );
        assert!(
            near_f(content_opacity(&island, volume(), now), 0.8),
            "+{n} ms: the old piece"
        );
        assert!(
            near_f(pill_in_zone(&island, bell_zone, now).unwrap_or(-1.0), 1.0),
            "+{n} ms: the new pill"
        );
    }
    // At its merge it gives them back on the exit fade and the 200 ms return.
    let merge = t + ms(320);
    step(&mut island, merge);
    assert_eq!(island.sheets.held_pieces(merge), [bell]);
    assert!(near_f(pill_in_zone(&island, 3, merge).unwrap_or(-1.0), 1.0));
    step(&mut island, merge + ms(75));
    let want = 1.0 - celestina_ui::EXIT.eval(0.5);
    assert!(near_f(
        pill_in_zone(&island, 3, merge + ms(75)).unwrap_or(-1.0),
        want
    ));
    step(&mut island, merge + ms(150));
    assert_eq!(pill_in_zone(&island, 3, merge + ms(150)), None);
    step(&mut island, merge + ms(200));
    assert_eq!(content_opacity(&island, volume(), merge + ms(200)), 1.0);
    // The new window's piece is held throughout.
    assert!(near_f(content_opacity(&island, bell, merge + ms(200)), 0.8));
}

#[test]
fn under_reduced_motion_the_leaving_windows_piece_goes_back_at_once() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    assert!(open_on(&mut island, volume(), t0));
    let t = t0 + ms(500);
    step(&mut island, t);
    let bell = Key::named("bell");
    let at = centre(bar_rect(&island, bell));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert!(island.sheets.is_leaving());
    assert_eq!(island.sheets.held_pieces(t), [bell]);
    step(&mut island, t);
    // The old pill is gone at once: no fade under reduced motion.
    assert_eq!(pill_in_zone(&island, 3, t), None);
    assert_eq!(content_opacity(&island, volume(), t), 1.0);
}
