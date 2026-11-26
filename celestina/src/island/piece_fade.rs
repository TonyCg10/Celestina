//! The crossfade of the open piece (CEL-D-010): the piece a sheet opens on
//! fades out of the bar, blurs and sinks towards its bottom edge while its
//! slot closes, and fades back in from there when the sheet closes. The
//! clocks live here, one per piece that is fading; [`super::bar_tree`]
//! paints with them.

use std::time::{Duration, Instant};

use celestina_ui::{Key, ONE_UI};

/// How long the piece takes to fade out when its sheet opens.
pub const FADE_OUT: Duration = Duration::from_millis(160);
/// How long it takes to fade back in, from the first frame of the close.
pub const FADE_IN: Duration = Duration::from_millis(200);
/// The blur of a piece at opacity 0, in logical px.
pub const BLUR_PX: f32 = 4.0;
/// How far below its place a piece at opacity 0 is, in logical px.
pub const DROP_PX: f32 = 10.0;

/// One piece's fade: its opacity goes from `from` to `to` on the One UI
/// curve over `duration`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    key: Key,
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl Fade {
    fn progress(&self, now: Instant) -> f32 {
        let span = self.duration.as_secs_f32();
        if span <= 0.0 {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f32() / span).clamp(0.0, 1.0)
    }

    fn value(&self, now: Instant) -> f32 {
        let k = ONE_UI.eval(self.progress(now));
        self.from + (self.to - self.from) * k
    }

    fn done(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }
}

/// The fades that run, one per piece.
#[derive(Clone, Debug, Default)]
pub struct Fades {
    running: Vec<Fade>,
}

impl Fades {
    /// Starts the fade of `piece` at `now`: back into the bar (`show`) or
    /// out of it. A piece already fading carries on from the opacity it has,
    /// so a reopen during a close fades it out again from where it is; one
    /// that is not starts from out of the bar when it comes back, from fully
    /// shown when it leaves.
    pub fn start(&mut self, piece: Key, show: bool, now: Instant) {
        let shown = self
            .value(piece, now)
            .unwrap_or(if show { 0.0 } else { 1.0 });
        self.running.retain(|f| f.key != piece);
        self.running.push(Fade {
            key: piece,
            from: shown,
            to: if show { 1.0 } else { 0.0 },
            start: now,
            duration: if show { FADE_IN } else { FADE_OUT },
        });
    }

    /// The opacity of `piece` at `now`, while it is fading.
    #[must_use]
    pub fn value(&self, piece: Key, now: Instant) -> Option<f32> {
        self.running
            .iter()
            .find(|f| f.key == piece && !f.done(now))
            .map(|f| f.value(now))
    }

    /// Every piece fading at `now` with its opacity.
    #[must_use]
    pub fn active(&self, now: Instant) -> Vec<(Key, f32)> {
        self.running
            .iter()
            .filter(|f| !f.done(now))
            .map(|f| (f.key, f.value(now)))
            .collect()
    }

    /// Drops the fades that have ended, and those of the pieces `held`
    /// says the bar no longer has.
    pub fn settle(&mut self, now: Instant, held: impl Fn(Key) -> bool) {
        self.running.retain(|f| !f.done(now) && held(f.key));
    }

    /// Whether a fade still runs at `now`.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        self.running.iter().any(|f| !f.done(now))
    }
}

/// The blur and the drop of a piece at opacity `k`; none under reduced
/// motion, which only fades.
#[must_use]
pub fn look(k: f32, reduced: bool) -> (f32, f32) {
    if reduced {
        return (0.0, 0.0);
    }
    let gone = (1.0 - k).clamp(0.0, 1.0);
    (BLUR_PX * gone, DROP_PX * gone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn piece() -> Key {
        Key::named("volume")
    }

    #[test]
    fn a_fade_out_takes_160_ms_and_a_fade_in_200() {
        let t0 = Instant::now();
        let mut out = Fades::default();
        out.start(piece(), false, t0);
        assert_eq!(out.value(piece(), t0), Some(1.0));
        assert!(out.value(piece(), t0 + ms(80)).is_some_and(|v| v < 0.3));
        assert!(out.is_animating(t0 + ms(159)));
        assert!(!out.is_animating(t0 + ms(160)));
        assert_eq!(out.value(piece(), t0 + ms(160)), None, "it has ended");
        let mut back = Fades::default();
        back.start(piece(), true, t0);
        assert_eq!(back.value(piece(), t0), Some(0.0));
        assert!(back.is_animating(t0 + ms(199)));
        assert!(!back.is_animating(t0 + ms(200)));
        assert_eq!(back.value(piece(), t0 + ms(200)), None, "it has ended");
    }

    #[test]
    fn a_fade_that_starts_on_a_running_one_carries_on_from_its_opacity() {
        let t0 = Instant::now();
        let mut fades = Fades::default();
        fades.start(piece(), true, t0);
        let t = t0 + ms(60);
        let shown = fades.value(piece(), t).unwrap_or(f32::NAN);
        assert!(shown > 0.0 && shown < 1.0, "{shown}");
        fades.start(piece(), false, t);
        assert_eq!(fades.value(piece(), t), Some(shown));
        assert!(fades.value(piece(), t + ms(159)).is_some_and(|v| v < 0.01));
        assert_eq!(fades.value(piece(), t + ms(160)), None);
    }

    #[test]
    fn settling_drops_what_ended_and_what_the_bar_lost() {
        let t0 = Instant::now();
        let other = Key::named("brightness");
        let mut fades = Fades::default();
        fades.start(piece(), false, t0);
        fades.start(other, true, t0);
        fades.settle(t0 + ms(10), |k| k == other);
        assert_eq!(fades.active(t0 + ms(10)).len(), 1);
        fades.settle(t0 + ms(200), |_| true);
        assert!(fades.active(t0 + ms(200)).is_empty());
        assert!(!fades.is_animating(t0 + ms(200)));
    }

    #[test]
    fn the_look_is_blurred_and_sunk_until_shown_and_only_faded_when_reduced() {
        assert_eq!(look(1.0, false), (0.0, 0.0));
        assert_eq!(look(0.0, false), (BLUR_PX, DROP_PX));
        let (blur, drop) = look(0.5, false);
        assert!(blur > 0.0 && blur < BLUR_PX && drop > 0.0 && drop < DROP_PX);
        assert_eq!(look(0.0, true), (0.0, 0.0));
    }
}
