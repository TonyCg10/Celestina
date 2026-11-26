//! The flight of the open piece (CEL-D-008): its icon and its text fly
//! between their place in the bar and the sheet's subtitle, down with the
//! open spring when the sheet opens on the piece, up when the sheet leaves
//! it. Pure timing and interpolation, plus the paint of one flyer: the icon
//! grows from its bar size to 22 px, the text from `body` to `row_title`,
//! and the text's weight goes from regular to semibold by painting both
//! weights, regular at `1 - k` and semibold at `k` (femtovg cannot
//! interpolate a font's weight axis).

use std::time::{Duration, Instant};

use celestina_paint::{IconKey, MotionMs, Painter, Rect, Role, Weight};
use celestina_ui::{Easing, Key, Motion, EXIT};

use super::motion::{lerp, lerp_rect};
use crate::island::view::text_style;

/// After a close reopens the piece's slot, the flight starts this much later.
pub const CLOSE_DELAY: Duration = Duration::from_millis(60);
/// How long the flight of a close takes.
pub const CLOSE_FLIGHT: Duration = Duration::from_millis(200);

/// The bar's nodes of a flying piece: the piece's own node, its icon (none
/// for a piece drawn without an icon node, like the system load) and the text
/// that becomes the subtitle's (none for a piece whose face has no text in the
/// bar). At least one of the two is there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarKeys {
    pub piece: Key,
    pub icon: Option<Key>,
    pub text: Option<Key>,
}

/// How long a flyer that starts over a fading subtitle takes to reach full
/// opacity.
const FADE_IN: Duration = Duration::from_millis(50);

/// A text at a place: its rect (the text is centred vertically in it and
/// starts at its left edge) and its size.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPart {
    pub text: String,
    pub rect: Rect,
    pub size: f32,
}

/// What flies at one end: the icon and the text, each when there is one.
#[derive(Clone, Debug, PartialEq)]
pub struct Parts {
    pub icon: Option<IconKey>,
    /// Where the icon is (empty without one).
    pub icon_rect: Rect,
    pub text: Option<TextPart>,
    /// The weight cross-fade a flyer had when it handed over (0 regular, 1
    /// semibold); `None` for an end that is not a flyer.
    pub k: Option<f32>,
}

/// One flyer at an instant.
#[derive(Clone, Debug, PartialEq)]
pub struct FlightFrame {
    pub icon: Rect,
    /// The text's rect and size; `None` when only the icon flies.
    pub text: Option<(Rect, f32)>,
    /// The weight cross-fade: 0 regular, 1 semibold.
    pub k: f32,
}

/// The icon and text of one piece on their way between the bar and the
/// subtitle.
#[derive(Clone, Debug)]
pub struct Flight {
    pub keys: BarKeys,
    /// Towards the bar (the piece leaves the subtitle) or down into it.
    pub up: bool,
    pub from: Parts,
    pub to: Parts,
    /// Whether `from` is where the flyer really starts (a close's flight
    /// takes it from the subtitle when it starts, unless a flyer was already
    /// on its way).
    pub armed: bool,
    start: Instant,
    delay: Duration,
    duration: Duration,
    easing: Easing,
    /// Painted at `from` during the delay.
    hold: bool,
    /// The flyer fades in over its first frames (it starts over a subtitle
    /// that is fading out with the body).
    fade_in: bool,
}

impl Flight {
    /// The piece flies down into the subtitle on the open spring.
    #[must_use]
    pub fn down(
        keys: BarKeys,
        from: Parts,
        to: Parts,
        ladder: &MotionMs,
        overshoot: f32,
        now: Instant,
    ) -> Flight {
        Flight {
            keys,
            up: false,
            from,
            to,
            armed: true,
            start: now,
            delay: Duration::ZERO,
            duration: Motion::Slow.duration(ladder),
            easing: Easing::OutBack(overshoot),
            hold: false,
            fade_in: false,
        }
    }

    /// The piece flies up on a switch: the exit step and curve, at once.
    #[must_use]
    pub fn up_on_switch(
        keys: BarKeys,
        from: Parts,
        to: Parts,
        ladder: &MotionMs,
        now: Instant,
    ) -> Flight {
        Flight {
            keys,
            up: true,
            from,
            to,
            armed: true,
            start: now,
            delay: Duration::ZERO,
            duration: Motion::Exit.duration(ladder),
            easing: Easing::Bezier(EXIT),
            hold: false,
            fade_in: false,
        }
    }

    /// The piece flies up on a close: 60 ms after the slot reopens, 200 ms
    /// with the exit curve. `from` is the flyer already on its way (`held`:
    /// painted there during the delay), or a first guess that the flight
    /// replaces with the subtitle's place when it starts.
    #[must_use]
    pub fn up_on_close(keys: BarKeys, from: Parts, to: Parts, held: bool, now: Instant) -> Flight {
        Flight {
            keys,
            up: true,
            from,
            to,
            armed: held,
            start: now,
            delay: CLOSE_DELAY,
            duration: CLOSE_FLIGHT,
            easing: Easing::Bezier(EXIT),
            hold: held,
            fade_in: !held,
        }
    }

    fn begins(&self) -> Instant {
        self.start + self.delay
    }

    /// Whether the flight has begun (a held flyer counts from the start).
    #[must_use]
    pub fn started(&self, now: Instant) -> bool {
        self.hold || now >= self.begins()
    }

    /// Whether the flyer has landed.
    #[must_use]
    pub fn done(&self, now: Instant) -> bool {
        now >= self.begins() + self.duration
    }

    /// Whether the flyer is painted at `now`.
    #[must_use]
    pub fn flying(&self, now: Instant) -> bool {
        self.started(now) && !self.done(now)
    }

    /// Whether the icon flies: both ends have one.
    #[must_use]
    pub fn flies_icon(&self) -> bool {
        self.from.icon.is_some() && self.to.icon.is_some()
    }

    /// Whether the text flies too: both ends have one.
    #[must_use]
    pub fn flies_text(&self) -> bool {
        self.from.text.is_some() && self.to.text.is_some()
    }

    fn progress(&self, now: Instant) -> f32 {
        if now < self.begins() {
            return self.easing.eval(0.0);
        }
        if self.duration.is_zero() {
            return self.easing.eval(1.0);
        }
        let t = now.saturating_duration_since(self.begins()).as_secs_f32()
            / self.duration.as_secs_f32();
        self.easing.eval(t.clamp(0.0, 1.0))
    }

    /// The flyer at `now`: `None` before it starts (unless held) and once it
    /// has landed.
    #[must_use]
    pub fn at(&self, now: Instant) -> Option<FlightFrame> {
        if !self.flying(now) {
            return None;
        }
        let p = self.progress(now);
        let text = self.text_at(p).map(|t| (t.rect, t.size));
        Some(FlightFrame {
            icon: lerp_rect(self.from.icon_rect, self.to.icon_rect, p),
            text,
            k: self.k_at(p),
        })
    }

    /// The weight cross-fade at progress `p`: from where the flyer handed
    /// over (else regular going down, semibold going up) to the other end.
    fn k_at(&self, p: f32) -> f32 {
        let (k0, k1) = if self.up { (1.0, 0.0) } else { (0.0, 1.0) };
        lerp(self.from.k.unwrap_or(k0), k1, p).clamp(0.0, 1.0)
    }

    /// The flyer as parts, where it is at `now`: what a flight that takes
    /// over from this one starts from.
    #[must_use]
    pub fn current(&self, now: Instant) -> Parts {
        let p = self.progress(now);
        Parts {
            icon: self.to.icon.clone().filter(|_| self.flies_icon()),
            icon_rect: lerp_rect(self.from.icon_rect, self.to.icon_rect, p),
            text: self.text_at(p),
            k: Some(self.k_at(p)),
        }
    }

    /// The flying text at progress `p`, with the live string of `to`; `None`
    /// when only the icon flies.
    fn text_at(&self, p: f32) -> Option<TextPart> {
        let (Some(a), Some(b)) = (&self.from.text, &self.to.text) else {
            return None;
        };
        Some(TextPart {
            text: b.text.clone(),
            rect: lerp_rect(a.rect, b.rect, p),
            size: lerp(a.size, b.size, p),
        })
    }

    /// Paints the flyer at `now`, over everything: the icon tinted `TextHi`,
    /// the text twice (regular at `1 - k`, semibold at `k`). The live text is
    /// the one of the end that stays (`to`).
    pub fn paint(&self, painter: &mut Painter, dpr: f32, now: Instant) {
        let Some(frame) = self.at(now) else {
            return;
        };
        let color = painter.theme().color(Role::TextHi);
        let fade = if self.fade_in {
            (now.saturating_duration_since(self.begins()).as_secs_f32() / FADE_IN.as_secs_f32())
                .clamp(0.0, 1.0)
        } else {
            1.0
        };
        painter.push_state();
        painter.opacity(fade);
        self.paint_parts(painter, dpr, &frame, color);
        painter.pop_state();
    }

    fn paint_parts(
        &self,
        painter: &mut Painter,
        dpr: f32,
        frame: &FlightFrame,
        color: celestina_paint::Color,
    ) {
        if let (Some(icon), true) = (&self.to.icon, self.flies_icon()) {
            painter.icon(icon, frame.icon, Some(color));
        }
        let (Some((rect, size)), Some(text)) = (frame.text, &self.to.text) else {
            return;
        };
        for (weight, alpha) in [
            (Weight::Regular, 1.0 - frame.k),
            (Weight::Semibold, frame.k),
        ] {
            if alpha <= 0.0 {
                continue;
            }
            let style = text_style(size, weight, true, Role::TextHi);
            let run = painter.text().layout(&text.text, &style, None, dpr);
            let y = rect.y + ((rect.h - run.size().1) / 2.0).max(0.0);
            painter.push_state();
            painter.opacity(alpha);
            painter.glyphs(&run, rect.x, y, color);
            painter.pop_state();
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;

    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn keys() -> BarKeys {
        BarKeys {
            piece: Key::named("volume"),
            icon: Some(Key::named("volume-icon")),
            text: Some(Key::named("volume-percent")),
        }
    }

    /// The bar's end: icon 18, text 13.
    fn bar() -> Parts {
        Parts {
            icon: Some(IconKey::Semantic("media-volume")),
            icon_rect: r(100.0, 15.0, 18.0, 18.0),
            text: Some(TextPart {
                text: "50 %".to_owned(),
                rect: r(122.0, 15.0, 30.0, 18.0),
                size: 13.0,
            }),
            k: None,
        }
    }

    /// The subtitle's end: icon 22, text 15.
    fn subtitle() -> Parts {
        Parts {
            icon: Some(IconKey::Semantic("media-volume")),
            icon_rect: r(300.0, 60.0, 22.0, 22.0),
            text: Some(TextPart {
                text: "50 %".to_owned(),
                rect: r(332.0, 60.0, 40.0, 22.0),
                size: 15.0,
            }),
            k: None,
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn opening_flies_icon_and_text_down_with_the_open_spring() {
        let theme = Theme::dark();
        let t0 = Instant::now();
        let f = Flight::down(
            keys(),
            bar(),
            subtitle(),
            &theme.motion,
            theme.overshoot,
            t0,
        );
        let spring = Easing::OutBack(theme.overshoot);
        let d = Motion::Slow.duration(&theme.motion);
        assert_eq!(d, ms(350));
        let first = f.at(t0).expect("a frame at 0");
        assert!(near(first.icon.x, 100.0) && near(first.icon.w, 18.0));
        assert!(near(first.text.as_ref().expect("text").1, 13.0));
        assert!(near(first.k, 0.0));
        let half = f.at(t0 + d / 2).expect("a frame at 50 %");
        let p = spring.eval(0.5);
        assert!(near(half.icon.x, 100.0 + (300.0 - 100.0) * p));
        assert!(near(half.icon.y, 15.0 + (60.0 - 15.0) * p));
        assert!(near(half.icon.w, 18.0 + 4.0 * p), "icon 18 to 22");
        let (rect, size) = half.text.expect("text");
        assert!(near(size, 13.0 + 2.0 * p), "text 13 to 15");
        assert!(near(rect.x, 122.0 + (332.0 - 122.0) * p));
        assert!(near(half.k, p.clamp(0.0, 1.0)));
        // At the end the flyer has landed: nothing is painted and the piece
        // in the subtitle takes over.
        assert!(f.at(t0 + d).is_none() && f.done(t0 + d));
        // Just before, it is at the subtitle (the spring's end is 1).
        let last = f.at(t0 + d - ms(1)).expect("a frame near the end");
        assert!(near(last.icon.w, 22.0) || (last.icon.w - 22.0).abs() < 0.5);
        assert!(last.k > 0.99);
    }

    #[test]
    fn closing_reopens_the_slot_then_flies_up_after_60_ms_in_200_ms() {
        let t0 = Instant::now();
        let f = Flight::up_on_close(keys(), subtitle(), bar(), false, t0);
        // Nothing flies during the delay: the subtitle still shows.
        assert!(f.at(t0).is_none() && !f.started(t0 + ms(59)));
        assert!(!f.done(t0 + ms(259)));
        let begin = t0 + ms(60);
        let first = f.at(begin).expect("a frame when it starts");
        assert!(near(first.icon.x, 300.0) && near(first.k, 1.0));
        let exit = Easing::Bezier(EXIT);
        let mid = f.at(begin + ms(100)).expect("a frame at 50 %");
        let p = exit.eval(0.5);
        assert!(near(mid.icon.x, 300.0 + (100.0 - 300.0) * p));
        assert!(near(mid.k, 1.0 - p));
        assert!(f.done(t0 + ms(260)) && f.at(t0 + ms(260)).is_none());
    }

    #[test]
    fn a_held_flyer_is_painted_where_it_was_during_the_delay() {
        let t0 = Instant::now();
        let f = Flight::up_on_close(keys(), subtitle(), bar(), true, t0);
        let frame = f.at(t0 + ms(10)).expect("held");
        assert!(near(frame.icon.x, 300.0));
        assert!(f.started(t0));
    }

    #[test]
    fn switching_flies_the_old_piece_up_in_150_ms_and_the_new_one_down() {
        let theme = Theme::dark();
        let t0 = Instant::now();
        let up = Flight::up_on_switch(keys(), subtitle(), bar(), &theme.motion, t0);
        let down = Flight::down(
            keys(),
            bar(),
            subtitle(),
            &theme.motion,
            theme.overshoot,
            t0,
        );
        assert!(up.at(t0).is_some() && !up.done(t0 + ms(149)) && up.done(t0 + ms(150)));
        assert!(!down.done(t0 + ms(349)) && down.done(t0 + ms(350)));
        let mid = up.at(t0 + ms(75)).expect("a frame");
        let p = Easing::Bezier(EXIT).eval(0.5);
        assert!(near(mid.icon.x, 300.0 + (100.0 - 300.0) * p));
        assert!(near(mid.icon.w, 22.0 + (18.0 - 22.0) * p), "icon 22 to 18");
    }

    #[test]
    fn a_flight_without_text_flies_the_icon_only() {
        let t0 = Instant::now();
        let theme = Theme::dark();
        let mut to = subtitle();
        to.text = None;
        let mut from = bar();
        from.text = None;
        let f = Flight::down(keys(), from, to, &theme.motion, theme.overshoot, t0);
        assert!(!f.flies_text());
        assert!(f.at(t0 + ms(100)).expect("frame").text.is_none());
    }

    #[test]
    fn the_flyer_that_takes_over_starts_where_the_old_one_is() {
        let theme = Theme::dark();
        let t0 = Instant::now();
        let f = Flight::down(
            keys(),
            bar(),
            subtitle(),
            &theme.motion,
            theme.overshoot,
            t0,
        );
        let at = t0 + ms(100);
        let frame = f.at(at).expect("frame");
        let now = f.current(at);
        assert!(near(now.icon_rect.x, frame.icon.x));
        assert!(near(
            now.text.expect("text").size,
            frame.text.expect("text").1
        ));
    }

    #[test]
    fn a_flyer_that_takes_over_keeps_its_weight() {
        let theme = Theme::dark();
        let t0 = Instant::now();
        let down = Flight::down(
            keys(),
            bar(),
            subtitle(),
            &theme.motion,
            theme.overshoot,
            t0,
        );
        let at = t0 + ms(120);
        let k = down.at(at).expect("frame").k;
        assert!(k > 0.2 && k < 1.0, "{k}");
        // The flyer going back up starts at that weight, not at semibold.
        let up = Flight::up_on_switch(keys(), down.current(at), bar(), &theme.motion, at);
        let first = up.at(at).expect("frame").k;
        assert!(near(first, k), "{first} vs {k}");
        assert!(up.at(at + ms(140)).expect("frame").k < k);
    }

    #[test]
    fn a_text_only_flight_has_no_icon() {
        let theme = Theme::dark();
        let t0 = Instant::now();
        let mut from = bar();
        from.icon = None;
        let f = Flight::down(keys(), from, subtitle(), &theme.motion, theme.overshoot, t0);
        assert!(!f.flies_icon() && f.flies_text());
        assert!(f.current(t0 + ms(50)).icon.is_none());
    }
}
