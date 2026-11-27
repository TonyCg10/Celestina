//! The open piece's look in the bar (CEL-D-014): the grey pill behind the
//! piece a menu is open on, and the opacity of that piece's content. Pure
//! clocks: every method takes the instant and the piece's live rect, nothing
//! reads a clock or the tree.
//!
//! One pill per zone. It shows at once when a menu opens, slides
//! on the open spring to the next piece of the same capsule, fades out when
//! the menu closes, and always follows its piece's live rect. A piece's
//! content goes to the muted opacity while it is open and back to full when
//! it is left.

use std::time::Instant;

use celestina_paint::{MotionMs, Rect, Theme};
use celestina_ui::{Animated, Easing, Key, Motion, Reduced, EXIT, ONE_UI};

use crate::sheet::motion::lerp_rect;

/// A pill to paint: the zone's capsule, the piece it sits behind, the rect
/// it is at and its alpha (0..=1, times the role colour's own).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PillLook {
    pub zone: usize,
    pub piece: Key,
    pub rect: Rect,
    pub alpha: f32,
}

/// One zone's pill.
#[derive(Clone, Copy, Debug)]
struct Pill {
    zone: usize,
    piece: Key,
    alpha: Animated,
    /// A slide to `piece` from the rect the pill was painted at, and when it
    /// started.
    slide: Option<(Rect, Instant)>,
}

/// The clocks of the open piece's pill and content.
#[derive(Clone, Debug)]
pub struct OpenPiece {
    ladder: MotionMs,
    overshoot: f32,
    muted: f32,
    reduced: bool,
    open: Option<Key>,
    pills: Vec<Pill>,
    /// The content opacity of every piece that is not at full.
    contents: Vec<(Key, Animated)>,
}

impl OpenPiece {
    #[must_use]
    pub fn new(theme: &Theme, reduced: bool) -> OpenPiece {
        OpenPiece {
            ladder: theme.motion,
            overshoot: theme.overshoot,
            muted: theme.muted_content_opacity,
            reduced,
            open: None,
            pills: Vec::new(),
            contents: Vec::new(),
        }
    }

    /// Takes `theme`'s ladder, overshoot and muted opacity for the clocks
    /// started from now on.
    pub fn set_theme(&mut self, theme: &Theme) {
        self.ladder = theme.motion;
        self.overshoot = theme.overshoot;
        self.muted = theme.muted_content_opacity;
    }

    /// Switches reduced motion; turning it on ends every running clock at its
    /// target.
    pub fn set_reduced(&mut self, on: bool) {
        self.reduced = on;
        if on {
            for pill in &mut self.pills {
                pill.alpha.set(pill.alpha.target());
                pill.slide = None;
            }
            for (_, a) in &mut self.contents {
                a.set(a.target());
            }
        }
    }

    fn policy(&self) -> Reduced {
        Reduced::spatial(self.reduced)
    }

    fn slide_duration(&self) -> std::time::Duration {
        Motion::Slow.duration(&self.ladder)
    }

    /// The rect `pill` is painted at, or `None` without a live rect for its
    /// piece.
    fn rect_of(
        &self,
        pill: &Pill,
        now: Instant,
        live: &dyn Fn(Key) -> Option<Rect>,
    ) -> Option<Rect> {
        let to = live(pill.piece)?;
        let Some((from, start)) = pill.slide else {
            return Some(to);
        };
        let span = self.slide_duration().as_secs_f32();
        let t = now.saturating_duration_since(start).as_secs_f32() / span.max(f32::EPSILON);
        if t >= 1.0 {
            return Some(to);
        }
        Some(lerp_rect(from, to, Easing::OutBack(self.overshoot).eval(t)))
    }

    fn content_to(&mut self, key: Key, to: f32, now: Instant) {
        let duration = Motion::Normal.duration(&self.ladder);
        let policy = self.policy();
        if let Some((_, a)) = self.contents.iter_mut().find(|(k, _)| *k == key) {
            a.animate_to(to, duration, ONE_UI, now, policy);
        } else if to < 1.0 {
            let mut a = Animated::new(1.0);
            a.animate_to(to, duration, ONE_UI, now, policy);
            self.contents.push((key, a));
        }
    }

    /// The piece and zone a menu is open on from `now`, or `None` when none
    /// is. `live` gives a piece's rect as the bar has it now.
    pub fn set(
        &mut self,
        open: Option<(Key, usize)>,
        now: Instant,
        live: &dyn Fn(Key) -> Option<Rect>,
    ) {
        let policy = self.policy();
        let duration = Motion::Exit.duration(&self.ladder);
        let Some((piece, zone)) = open else {
            for pill in &mut self.pills {
                pill.alpha.animate_to(0.0, duration, EXIT, now, policy);
            }
            if let Some(old) = self.open.take() {
                self.content_to(old, 1.0, now);
            }
            return;
        };
        for pill in self.pills.iter_mut().filter(|p| p.zone != zone) {
            pill.alpha.animate_to(0.0, duration, EXIT, now, policy);
        }
        if let Some(i) = self.pills.iter().position(|p| p.zone == zone) {
            let pill = self.pills[i];
            if pill.piece != piece && pill.alpha.target() <= 0.0 {
                // A pill that is leaving belongs to its old piece: the new
                // one gets a whole pill of its own, with no slide.
                self.pills[i] = Pill {
                    zone,
                    piece,
                    alpha: Animated::new(1.0),
                    slide: None,
                };
                self.finish_set(piece, now);
                return;
            }
            let slide = if pill.piece == piece || self.reduced {
                pill.slide.filter(|_| pill.piece == piece)
            } else {
                self.rect_of(&pill, now, live).map(|from| (from, now))
            };
            let pill = &mut self.pills[i];
            pill.piece = piece;
            pill.slide = slide;
            pill.alpha.animate_to(1.0, duration, EXIT, now, policy);
        } else {
            // A fresh pill shows at once: it takes over the hover the press
            // already showed, so the piece is never left without a fill.
            let alpha = Animated::new(1.0);
            self.pills.push(Pill {
                zone,
                piece,
                alpha,
                slide: None,
            });
        }
        self.finish_set(piece, now);
    }

    /// The content clocks of a menu open on `piece`.
    fn finish_set(&mut self, piece: Key, now: Instant) {
        if let Some(old) = self.open.replace(piece).filter(|k| *k != piece) {
            self.content_to(old, 1.0, now);
        }
        self.content_to(piece, self.muted, now);
    }

    /// Drops every pill at once, with no fade: the piece's own hover fill
    /// takes over.
    pub fn cut_pills(&mut self) {
        for pill in &mut self.pills {
            pill.alpha.set(0.0);
        }
    }

    /// The piece a menu is open on.
    #[must_use]
    pub fn open(&self) -> Option<Key> {
        self.open
    }

    /// The pills to paint at `now`, at most one per zone. A pill that has
    /// faded out, or whose piece has no live rect, is left out.
    #[must_use]
    pub fn pills(&self, now: Instant, live: &dyn Fn(Key) -> Option<Rect>) -> Vec<PillLook> {
        self.pills
            .iter()
            .filter_map(|pill| {
                let alpha = pill.alpha.value(now);
                if alpha <= 0.0 && pill.alpha.target() <= 0.0 {
                    return None;
                }
                Some(PillLook {
                    zone: pill.zone,
                    piece: pill.piece,
                    rect: self.rect_of(pill, now, live)?,
                    alpha,
                })
            })
            .collect()
    }

    /// The opacity of `key`'s content at `now`; `None` means full opacity
    /// and not held.
    #[must_use]
    pub fn content(&self, key: Key, now: Instant) -> Option<f32> {
        self.contents
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, a)| a.value(now))
    }

    /// Drops the clocks that have finished and those of pieces `holds` says
    /// the bar no longer has.
    pub fn settle(&mut self, now: Instant, holds: &dyn Fn(Key) -> bool) {
        let span = self.slide_duration();
        self.pills
            .retain(|p| holds(p.piece) && !(p.alpha.target() <= 0.0 && !p.alpha.is_animating(now)));
        for pill in &mut self.pills {
            if pill
                .slide
                .is_some_and(|(_, start)| now.saturating_duration_since(start) >= span)
            {
                pill.slide = None;
            }
        }
        self.contents
            .retain(|(k, a)| holds(*k) && !(a.target() >= 1.0 && !a.is_animating(now)));
        if self.open.is_some_and(|k| !holds(k)) {
            self.open = None;
        }
    }

    /// Whether a clock is running and the bar needs a frame.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        let span = self.slide_duration();
        self.pills.iter().any(|p| {
            p.alpha.is_animating(now)
                || p.slide
                    .is_some_and(|(_, start)| now.saturating_duration_since(start) < span)
        }) || self.contents.iter().any(|(_, a)| a.is_animating(now))
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, time::Duration};

    use super::*;

    const RA: Rect = Rect {
        x: 100.0,
        y: 10.0,
        w: 60.0,
        h: 28.0,
    };
    const RB: Rect = Rect {
        x: 170.0,
        y: 10.0,
        w: 40.0,
        h: 28.0,
    };
    const RC: Rect = Rect {
        x: 20.0,
        y: 10.0,
        w: 50.0,
        h: 28.0,
    };

    fn a() -> Key {
        Key::named("a")
    }
    fn b() -> Key {
        Key::named("b")
    }
    fn c() -> Key {
        Key::named("c")
    }

    fn live(key: Key) -> Option<Rect> {
        [(a(), RA), (b(), RB), (c(), RC)]
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, r)| r)
    }

    fn model(reduced: bool) -> OpenPiece {
        OpenPiece::new(&Theme::dark(), reduced)
    }

    fn at(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    fn near(x: f32, y: f32) -> bool {
        (x - y).abs() < 1e-4
    }

    fn near_rect(p: Rect, q: Rect) -> bool {
        near(p.x, q.x) && near(p.y, q.y) && near(p.w, q.w) && near(p.h, q.h)
    }

    fn one(op: &OpenPiece, now: Instant) -> PillLook {
        let pills = op.pills(now, &live);
        assert_eq!(pills.len(), 1, "{pills:?}");
        pills[0]
    }

    #[test]
    fn opening_shows_the_pill_at_once_and_mutes_the_content_in_200_ms() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        assert_eq!(op.open(), Some(a()));
        for n in [0, 75, 150] {
            assert!(near(one(&op, at(t0, n)).alpha, 1.0));
        }
        assert_eq!(op.content(a(), t0), Some(1.0));
        let mid = op.content(a(), at(t0, 100)).unwrap_or(-1.0);
        assert!(near(mid, 1.0 - 0.2 * ONE_UI.eval(0.5)), "{mid}");
        assert!(near(op.content(a(), at(t0, 200)).unwrap_or(-1.0), 0.8));
        assert_eq!(op.content(b(), at(t0, 200)), None);
        for n in [0, 75, 150, 400] {
            let look = one(&op, at(t0, n));
            assert_eq!((look.zone, look.piece, look.rect), (3, a(), RA));
        }
        assert!(op.is_animating(at(t0, 199)));
        assert!(!op.is_animating(at(t0, 200)));
    }

    #[test]
    fn a_switch_in_the_capsule_slides_the_pill_on_the_spring() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        let t1 = at(t0, 1000);
        op.settle(t1, &|_| true);
        op.set(Some((b(), 3)), t1, &live);
        let want = lerp_rect(RA, RB, Easing::OutBack(1.15).eval(0.5));
        assert!(near_rect(one(&op, at(t1, 175)).rect, want));
        assert!(near_rect(one(&op, at(t1, 350)).rect, RB));
        assert_eq!(one(&op, t1).rect, RA);
        for n in [0, 100, 175, 350] {
            let look = one(&op, at(t1, n));
            assert!(near(look.alpha, 1.0));
            assert_eq!((look.zone, look.piece), (3, b()));
        }
        assert!(near(op.content(a(), at(t1, 200)).unwrap_or(-1.0), 1.0));
        assert!(near(op.content(b(), at(t1, 200)).unwrap_or(-1.0), 0.8));
        assert!(op.is_animating(at(t1, 349)));
        assert!(!op.is_animating(at(t1, 350)));
        // A request for the piece already open slides nothing.
        op.settle(at(t1, 400), &|_| true);
        op.set(Some((b(), 3)), at(t1, 400), &live);
        assert!(!op.is_animating(at(t1, 400)));
        assert_eq!(one(&op, at(t1, 400)).rect, RB);
    }

    #[test]
    fn a_slide_ends_on_the_piece_where_it_is_now() {
        let t0 = Instant::now();
        let width = Cell::new(40.0);
        let moving = |key: Key| {
            live(key).map(|r| {
                if key == b() {
                    Rect {
                        w: width.get(),
                        ..r
                    }
                } else {
                    r
                }
            })
        };
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &moving);
        let t1 = at(t0, 1000);
        op.settle(t1, &|_| true);
        op.set(Some((b(), 3)), t1, &moving);
        width.set(70.0);
        let end = op.pills(at(t1, 350), &moving);
        assert_eq!(end.len(), 1);
        assert!(
            near_rect(end[0].rect, Rect { w: 70.0, ..RB }),
            "{:?}",
            end[0]
        );
        // In the middle of it, the target is the live one too.
        let mid = op.pills(at(t1, 175), &moving)[0].rect;
        let k = Easing::OutBack(1.15).eval(0.5);
        assert!(near_rect(mid, lerp_rect(RA, Rect { w: 70.0, ..RB }, k)));
    }

    #[test]
    fn another_capsule_fades_the_old_pill_and_shows_a_new_one() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        let t1 = at(t0, 1000);
        op.settle(t1, &|_| true);
        op.set(Some((c(), 0)), t1, &live);
        let pills = op.pills(at(t1, 75), &live);
        assert_eq!(pills.len(), 2, "{pills:?}");
        let old = pills.iter().find(|p| p.zone == 3).copied();
        let new = pills.iter().find(|p| p.zone == 0).copied();
        let (Some(old), Some(new)) = (old, new) else {
            panic!("{pills:?}")
        };
        assert!(near(old.alpha, 1.0 - EXIT.eval(0.5)));
        assert!(near(new.alpha, 1.0));
        assert_eq!((old.piece, old.rect), (a(), RA));
        assert_eq!((new.piece, new.rect), (c(), RC));
        let later = op.pills(at(t1, 150), &live);
        assert_eq!(later.len(), 1);
        assert_eq!((later[0].zone, later[0].piece), (0, c()));
        assert!(near(later[0].alpha, 1.0));
        assert!(near(op.content(a(), at(t1, 200)).unwrap_or(-1.0), 1.0));
        assert!(near(op.content(c(), at(t1, 200)).unwrap_or(-1.0), 0.8));
    }

    #[test]
    fn closing_fades_the_pill_in_150_ms_and_restores_the_content() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        let t1 = at(t0, 1000);
        op.settle(t1, &|_| true);
        op.set(None, t1, &live);
        assert_eq!(op.open(), None);
        assert!(near(one(&op, at(t1, 75)).alpha, 1.0 - EXIT.eval(0.5)));
        assert!(op.pills(at(t1, 150), &live).is_empty());
        let mid = op.content(a(), at(t1, 100)).unwrap_or(-1.0);
        assert!(near(mid, 0.8 + 0.2 * ONE_UI.eval(0.5)), "{mid}");
        assert!(near(op.content(a(), at(t1, 200)).unwrap_or(-1.0), 1.0));
        assert!(op.is_animating(at(t1, 199)));
        assert!(!op.is_animating(at(t1, 200)));
        op.settle(at(t1, 200), &|_| true);
        assert_eq!(op.content(a(), at(t1, 200)), None);
        assert!(op.pills(at(t1, 200), &live).is_empty());
    }

    #[test]
    fn reopening_during_the_fade_out_turns_back_from_where_it_is() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        let t1 = at(t0, 1000);
        op.settle(t1, &|_| true);
        op.set(None, t1, &live);
        let turn = at(t1, 50);
        let before = one(&op, turn).alpha;
        let content_before = op.content(a(), turn).unwrap_or(-1.0);
        assert!(before < 1.0 && before > 0.0);
        op.set(Some((a(), 3)), turn, &live);
        assert!(near(one(&op, turn).alpha, before));
        assert!(near(op.content(a(), turn).unwrap_or(-1.0), content_before));
        assert!(one(&op, at(t1, 100)).alpha > before);
        assert!(near(one(&op, at(t1, 200)).alpha, 1.0));
        assert!(near(op.content(a(), at(t1, 400)).unwrap_or(-1.0), 0.8));
        for n in [50, 100, 200, 400] {
            assert_eq!(one(&op, at(t1, n)).rect, RA);
        }
    }

    #[test]
    fn a_vanished_piece_drops_its_pill_and_opacity() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        let now = at(t0, 50);
        assert!(op.is_animating(now));
        op.settle(now, &|k| k != a());
        assert!(op.pills(now, &live).is_empty());
        assert_eq!(op.content(a(), now), None);
        assert!(!op.is_animating(now));
        assert_eq!(op.open(), None);
        // A pill whose piece has no live rect is not painted, held or not.
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        assert!(op.pills(at(t0, 200), &|_| None).is_empty());
    }

    #[test]
    fn reduced_motion_lights_slides_and_restores_at_once() {
        let t0 = Instant::now();
        let mut op = model(true);
        op.set(Some((a(), 3)), t0, &live);
        assert!(near(one(&op, t0).alpha, 1.0));
        assert_eq!(op.content(a(), t0), Some(0.8));
        assert!(!op.is_animating(t0));
        op.set(Some((b(), 3)), t0, &live);
        let look = one(&op, t0);
        assert_eq!((look.piece, look.rect), (b(), RB));
        assert_eq!(op.content(a(), t0).unwrap_or(1.0), 1.0);
        assert_eq!(op.content(b(), t0), Some(0.8));
        assert!(!op.is_animating(t0));
        op.set(Some((c(), 0)), t0, &live);
        op.settle(t0, &|_| true);
        let look = one(&op, t0);
        assert_eq!((look.zone, look.piece), (0, c()));
        op.set(None, t0, &live);
        op.settle(t0, &|_| true);
        assert!(op.pills(t0, &live).is_empty());
        assert_eq!(op.content(c(), t0), None);
        assert!(!op.is_animating(t0));
    }

    #[test]
    fn turning_reduced_motion_on_ends_the_running_clocks() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        op.set_reduced(true);
        assert!(!op.is_animating(at(t0, 10)));
        assert!(near(one(&op, at(t0, 10)).alpha, 1.0));
        assert_eq!(op.content(a(), at(t0, 10)), Some(0.8));
    }

    #[test]
    fn a_pill_fading_out_is_not_reused_for_another_piece() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        op.set(None, at(t0, 300), &live);
        // Another piece of the same capsule, 50 ms into the fade-out.
        op.set(Some((b(), 3)), at(t0, 350), &live);
        let look = one(&op, at(t0, 350));
        assert_eq!((look.piece, look.rect), (b(), RB), "no slide from a");
        assert!(near(look.alpha, 1.0), "{}", look.alpha);
        // Another capsule: the old pill keeps fading, the new one is whole.
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        op.set(None, at(t0, 300), &live);
        op.set(Some((c(), 0)), at(t0, 350), &live);
        let pills = op.pills(at(t0, 350), &live);
        let new = pills.iter().find(|p| p.zone == 0).map(|p| p.alpha);
        assert!(new.is_some_and(|a| near(a, 1.0)), "{pills:?}");
    }

    #[test]
    fn cutting_the_pills_drops_them_at_once() {
        let t0 = Instant::now();
        let mut op = model(false);
        op.set(Some((a(), 3)), t0, &live);
        op.set(None, at(t0, 300), &live);
        assert!(!op.pills(at(t0, 310), &live).is_empty());
        op.cut_pills();
        assert!(op.pills(at(t0, 310), &live).is_empty());
    }
}
