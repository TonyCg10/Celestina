//! The Island growing into a notification, the burst sheets under it, the
//! level display and the gestures. Pure: every method takes the instant,
//! nothing reads a clock and nothing paints. The surface that borrows the
//! Island asks [`IslandNotice::look`] for what to draw each frame.

use std::time::{Duration, Instant};

use celestina_paint::{Rect, Theme};
use celestina_shell::{notifications::Notification, osd::Kind};
use celestina_ui::{Animated, Easing, Key, Motion, Reduced, LINEAR, ONE_UI};

use super::banners::{self, BannerAction};
use crate::providers::notifications::NotificationsCommand;
use crate::{island::piece_fade, sheet::motion::lerp};

/// The notice's width.
const NOTICE_W: f32 = 420.0;
/// The notice's height, and with a row of buttons.
pub(crate) const NOTICE_H: f32 = 88.0;
pub(crate) const NOTICE_H_BUTTONS: f32 = 120.0;
/// The level display's size.
pub(crate) const LEVEL_W: f32 = 274.0;
pub(crate) const LEVEL_H: f32 = 40.0;
/// A collapsed sheet: how much narrower per slot on each side, its height,
/// its radius and how far under the Island's bottom per slot.
const SHEET_INSET: f32 = 12.0;
const SHEET_H: f32 = 24.0;
const SHEET_STEP: f32 = 7.0;
/// A fanned card's distance from the Island and from the card above.
const FAN_GAP: f32 = 8.0;
/// The glass of a sheet.
const SHEET_GLASS: f32 = 0.92;
/// How far above its place a sheet enters from.
const SHEET_RISE: f32 = 10.0;
/// The face blur's peak, the offset's reach and the fade times: the
/// crossfade of the open piece (CEL-D-010), which the design reuses.
const FACE_BLUR: f32 = piece_fade::BLUR_PX;
const FACE_DY: f32 = piece_fade::DROP_PX;
/// Durations, in ms.
const FACE_OUT_MS: u64 = piece_fade::FADE_OUT.as_millis() as u64;
const FACE_IN_MS: u64 = piece_fade::FADE_IN.as_millis() as u64;
const FACE_DELAY_MS: u64 = 120;
const CLOCK_DELAY_MS: u64 = 140;
const REDUCED_FADE_MS: u64 = 150;
const SHEET_IN_MS: u64 = 200;
const SHEET_OUT_MS: u64 = 150;
const FOLD_GRACE_MS: u64 = 250;
/// The most notifications shown (the Island's and two sheets).
const MAX_SHEETS: usize = 2;

/// What the level display shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub kind: Kind,
    pub percent: u8,
    pub muted: bool,
}

/// What sits on the Island's face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceKind {
    /// The time, the date and the weather.
    Clock,
    /// The notification with this id.
    Notice(u32),
    /// The level display.
    Level,
}

/// One face's look: opacity, blur and offset towards the capsule's bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub kind: FaceKind,
    pub opacity: f32,
    pub blur: f32,
    pub dy: f32,
}

/// One sheet under the Island.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetLook {
    pub id: u32,
    pub rect: Rect,
    pub radius: f32,
    /// The glass's opacity.
    pub glass: f32,
    /// The content's opacity: 0 collapsed, 1 fanned.
    pub content: f32,
}

/// What to draw at an instant.
#[derive(Clone, Debug, PartialEq)]
pub struct NoticeLook {
    /// The Island's rect and corner radius.
    pub island: (Rect, f32),
    /// The faces still visible, the wanted one first of any that is shown.
    pub faces: Vec<Face>,
    /// The sheets, nearest the Island first.
    pub sheets: Vec<SheetLook>,
}

/// A person's gesture on a notice or a sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// A left click on this node.
    Left(Key),
    Right,
    Middle,
    /// The wheel, towards the top.
    WheelUp,
}

/// How long the wheel must be quiet before another scroll gesture counts.
pub(crate) const WHEEL_QUIET: Duration = Duration::from_millis(300);

/// Makes a scroll gesture close one notice: a touchpad swipe or a hi-res
/// wheel sends many axis events, and the notice under the pointer after the
/// first close would take the next ones. The toolkit says nothing of where a
/// gesture ends, so a gesture ends when no axis event came for
/// [`WHEEL_QUIET`] (or, on the page card, the pointer left the card).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct WheelLatch {
    last: Option<Instant>,
    closed: bool,
}

impl WheelLatch {
    /// Whether this vertical axis event is the one that closes: the first
    /// one towards the top of its gesture.
    pub(crate) fn up(&mut self, vertical: f32, now: Instant) -> bool {
        let quiet = self
            .last
            .is_none_or(|last| now.saturating_duration_since(last) >= WHEEL_QUIET);
        self.last = Some(now);
        if quiet {
            self.closed = false;
        }
        let fires = vertical < 0.0 && !self.closed;
        self.closed |= fires;
        fires
    }

    /// The gesture is over (the pointer left the page card).
    pub(crate) fn reset(&mut self) {
        *self = WheelLatch::default();
    }
}

/// Opacity, blur, offset.
type Look3 = (f32, f32, f32);

fn lerp3(a: Look3, b: Look3, k: f32) -> Look3 {
    (lerp(a.0, b.0, k), lerp(a.1, b.1, k), lerp(a.2, b.2, k))
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// The fraction of `dur` that `elapsed` covers, 0..=1.
fn fraction(elapsed: Duration, dur: Duration) -> f32 {
    if dur.is_zero() || elapsed >= dur {
        1.0
    } else {
        elapsed.as_secs_f32() / dur.as_secs_f32()
    }
}

/// One face's clock: a flight from a look to another after a delay.
#[derive(Clone, Copy, Debug)]
struct FaceClock {
    from: Look3,
    to: Look3,
    start: Instant,
    delay: Duration,
    dur: Duration,
    curve: Easing,
}

impl FaceClock {
    const SHOWN: Look3 = (1.0, 0.0, 0.0);

    fn settled_shown(now: Instant) -> FaceClock {
        FaceClock {
            from: FaceClock::SHOWN,
            to: FaceClock::SHOWN,
            start: now,
            delay: Duration::ZERO,
            dur: Duration::ZERO,
            curve: Easing::Bezier(ONE_UI),
        }
    }

    fn value(&self, now: Instant) -> Look3 {
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed < self.delay {
            return self.from;
        }
        let x = elapsed - self.delay;
        if x >= self.dur {
            return self.to;
        }
        lerp3(self.from, self.to, self.curve.eval(fraction(x, self.dur)))
    }

    fn settled(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) >= self.delay + self.dur
    }

    /// Flies to shown after `delay` (the reduced fade ignores it).
    fn enter(&mut self, now: Instant, delay_ms: u64, reduced: bool) {
        let cur = self.value(now);
        if self.to == FaceClock::SHOWN && self.start <= now {
            return;
        }
        let hidden = if reduced {
            (0.0, 0.0, 0.0)
        } else {
            (0.0, FACE_BLUR, -FACE_DY)
        };
        *self = FaceClock {
            from: if cur.0 <= 0.0 { hidden } else { cur },
            to: FaceClock::SHOWN,
            start: now,
            delay: if reduced {
                Duration::ZERO
            } else {
                ms(delay_ms)
            },
            dur: ms(if reduced { REDUCED_FADE_MS } else { FACE_IN_MS }),
            curve: if reduced {
                Easing::Bezier(LINEAR)
            } else {
                Easing::Bezier(ONE_UI)
            },
        };
    }

    /// Flies to hidden.
    fn leave(&mut self, now: Instant, reduced: bool) {
        if self.to.0 == 0.0 {
            return;
        }
        let gone = if reduced {
            (0.0, 0.0, 0.0)
        } else {
            (0.0, FACE_BLUR, FACE_DY)
        };
        *self = FaceClock {
            from: self.value(now),
            to: gone,
            start: now,
            delay: Duration::ZERO,
            dur: ms(if reduced {
                REDUCED_FADE_MS
            } else {
                FACE_OUT_MS
            }),
            curve: if reduced {
                Easing::Bezier(LINEAR)
            } else {
                Easing::Bezier(ONE_UI)
            },
        };
    }
}

/// The fan's clock: a flight to fanned on the spring, and a fold that
/// starts only once the grace after the pointer left has passed.
#[derive(Clone, Copy, Debug)]
struct Fan {
    from: f32,
    to: f32,
    start: Instant,
    fold_at: Option<Instant>,
    dur: Duration,
    curve: Easing,
}

impl Fan {
    fn seg(from: f32, to: f32, start: Instant, now: Instant, dur: Duration, curve: Easing) -> f32 {
        let elapsed = now.saturating_duration_since(start);
        if from == to || elapsed >= dur {
            return to;
        }
        lerp(from, to, curve.eval(fraction(elapsed, dur)))
    }

    fn value(&self, now: Instant) -> f32 {
        let first =
            |at: Instant| Fan::seg(self.from, self.to, self.start, at, self.dur, self.curve);
        match self.fold_at {
            Some(f) if now >= f => Fan::seg(first(f), 0.0, f, now, self.dur, self.curve),
            _ => first(now),
        }
    }

    fn animating(&self, now: Instant) -> bool {
        match self.fold_at {
            Some(f) => now < f + self.dur,
            None => self.from != self.to && now < self.start + self.dur,
        }
    }
}

/// A sheet under the Island.
#[derive(Clone, Debug)]
struct SheetSlot {
    id: u32,
    /// Its place under the Island, 1 or 2, flying when it changes.
    pos: Animated,
    slot: usize,
    /// Its height when fanned.
    h: f32,
    born: Instant,
    gone: Option<Instant>,
}

/// What the Island shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Rest,
    Level,
    Notice,
}

/// The Island that grows into notifications and level displays.
#[derive(Debug)]
pub struct IslandNotice {
    theme: Theme,
    reduced: bool,
    rest: Rect,
    /// The latest instant a call carried.
    now: Instant,
    toasts: Vec<Notification>,
    level: Option<Level>,
    last_level: Option<Level>,
    hovered: bool,
    fan: Fan,
    /// x, y, w, h and radius of the Island.
    geo: [Animated; 5],
    faces: Vec<(FaceKind, FaceClock)>,
    sheets: Vec<SheetSlot>,
}

/// What `gesture` on notification `target` among `toasts` asks the app to
/// do: the Island's notice, a sheet, or the card inside a page.
#[must_use]
pub fn gesture_on(toasts: &[Notification], target: u32, gesture: Gesture) -> Vec<BannerAction> {
    if !toasts.iter().any(|n| n.id == target) {
        return Vec::new();
    }
    match gesture {
        Gesture::Left(key) => banners::actions_for(toasts, key),
        Gesture::Right | Gesture::WheelUp => {
            vec![BannerAction::Send(NotificationsCommand::Close(target))]
        }
        Gesture::Middle => vec![BannerAction::Send(NotificationsCommand::DismissAll)],
    }
}

impl IslandNotice {
    /// An Island at rest. `rest` is set with [`IslandNotice::set_rest`].
    #[must_use]
    pub fn new(theme: &Theme, reduced: bool) -> IslandNotice {
        let now = Instant::now();
        let rest = Rect {
            x: 0.0,
            y: 8.0,
            w: 0.0,
            h: 40.0,
        };
        let mut m = IslandNotice {
            theme: theme.clone(),
            reduced,
            rest,
            now,
            toasts: Vec::new(),
            level: None,
            last_level: None,
            hovered: false,
            fan: Fan {
                from: 0.0,
                to: 0.0,
                start: now,
                fold_at: None,
                dur: Duration::ZERO,
                curve: Easing::Bezier(ONE_UI),
            },
            geo: [Animated::new(0.0); 5],
            faces: vec![(FaceKind::Clock, FaceClock::settled_shown(now))],
            sheets: Vec::new(),
        };
        m.jump_geometry();
        m
    }

    /// The panel Island's rest rect on this output. At rest the Island
    /// follows it at once; while something shows, the flight carries on to
    /// the notice centred on the new rect.
    pub fn set_rest(&mut self, rest: Rect, now: Instant) {
        self.now = now;
        self.rest = rest;
        if self.shape() == Shape::Rest && !self.moving(now) {
            self.jump_geometry();
        } else {
            self.retarget(now);
        }
    }

    /// The notifications on screen, newest first; at most three are kept.
    /// Whether anything about them changed.
    pub fn set_toasts(&mut self, toasts: &[Notification], now: Instant) -> bool {
        self.now = now;
        let new = banners::shown(toasts).to_vec();
        if new == self.toasts {
            return false;
        }
        self.toasts = new;
        self.sync(now);
        true
    }

    /// The level display on (`Some`) or off.
    pub fn set_level(&mut self, level: Option<Level>, now: Instant) {
        self.now = now;
        if level == self.level {
            return;
        }
        if level.is_some() {
            self.last_level = level;
        }
        self.level = level;
        self.sync(now);
    }

    /// The pointer is over a notice or a sheet, or left them. Leaving
    /// folds the sheets after a grace.
    pub fn set_hovered(&mut self, over: bool, now: Instant) {
        self.now = now;
        if over == self.hovered {
            return;
        }
        self.hovered = over;
        if over {
            let v = self.fan.value(now);
            self.fan = Fan {
                from: v,
                to: 1.0,
                start: now,
                fold_at: None,
                dur: self.spring(),
                curve: self.spring_curve(),
            };
        } else if self.fan.value(now) > 0.0 || self.fan.to > 0.0 {
            self.fan.fold_at = Some(now + ms(FOLD_GRACE_MS));
            self.fan.dur = self.spring();
        }
    }

    /// What to draw at `now`.
    #[must_use]
    pub fn look(&self, now: Instant) -> NoticeLook {
        let island = self.island(now);
        let wanted = self.wanted_face();
        let mut faces: Vec<Face> = self
            .faces
            .iter()
            .filter_map(|(kind, clock)| {
                let (opacity, blur, dy) = clock.value(now);
                (*kind == wanted || opacity > 0.0).then_some(Face {
                    kind: *kind,
                    opacity,
                    blur,
                    dy,
                })
            })
            .collect();
        faces.sort_by_key(|f| f.kind != wanted);
        NoticeLook {
            island: (island, self.geo[4].value(now)),
            faces,
            sheets: self.sheet_looks(island, now),
        }
    }

    /// Nothing shows and every motion has settled: the surface may go.
    #[must_use]
    pub fn at_rest(&self, now: Instant) -> bool {
        self.toasts.is_empty() && self.level.is_none() && !self.hovered && !self.is_animating(now)
    }

    /// Whether a frame is still owed: a motion runs or the fold's grace is
    /// pending.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        self.moving(now)
            || self.fan.animating(now)
            || self.faces.iter().any(|(_, c)| !c.settled(now))
            || self.sheets.iter().any(|s| {
                s.pos.is_animating(now)
                    || match s.gone {
                        Some(g) => now < g + self.sheet_out(),
                        None => now < s.born + self.sheet_in(),
                    }
            })
    }

    /// Whether the server's clock stays held: the pointer is over a notice
    /// or a level display shows.
    #[must_use]
    pub fn holds_timeouts(&self) -> bool {
        self.hovered || self.level.is_some()
    }

    /// Whether the Island's face is the level display, which takes no input.
    #[must_use]
    pub fn shows_level(&self) -> bool {
        self.level.is_some()
    }

    /// The level shown, or the last one while it fades.
    #[must_use]
    pub fn level(&self) -> Option<Level> {
        self.level.or(self.last_level)
    }

    /// The notifications on screen, newest first.
    #[must_use]
    pub fn toasts(&self) -> &[Notification] {
        &self.toasts
    }

    /// What `gesture` on notification `target` asks the app to do.
    #[must_use]
    pub fn gesture(&self, target: u32, gesture: Gesture) -> Vec<BannerAction> {
        gesture_on(&self.toasts, target, gesture)
    }

    fn spring(&self) -> Duration {
        if self.reduced {
            Duration::ZERO
        } else {
            Motion::Slow.duration(&self.theme.motion)
        }
    }

    fn spring_curve(&self) -> Easing {
        Easing::OutBack(self.theme.overshoot)
    }

    fn sheet_in(&self) -> Duration {
        ms(if self.reduced {
            REDUCED_FADE_MS
        } else {
            SHEET_IN_MS
        })
    }

    fn sheet_out(&self) -> Duration {
        ms(if self.reduced {
            REDUCED_FADE_MS
        } else {
            SHEET_OUT_MS
        })
    }

    fn shape(&self) -> Shape {
        if self.level.is_some() {
            Shape::Level
        } else if self.toasts.is_empty() {
            Shape::Rest
        } else {
            Shape::Notice
        }
    }

    fn wanted_face(&self) -> FaceKind {
        match (self.level, self.toasts.first()) {
            (Some(_), _) => FaceKind::Level,
            (None, Some(n)) => FaceKind::Notice(n.id),
            (None, None) => FaceKind::Clock,
        }
    }

    fn notice_h(n: &Notification) -> f32 {
        if banners::buttons(n).next().is_some() {
            NOTICE_H_BUTTONS
        } else {
            NOTICE_H
        }
    }

    /// The rect and radius the Island flies to.
    fn target(&self) -> (Rect, f32) {
        let cx = self.rest.x + self.rest.w / 2.0;
        let radius = self.theme.radius_lg;
        let (w, h) = match self.shape() {
            Shape::Rest => return (self.rest, radius),
            Shape::Level => (LEVEL_W, LEVEL_H),
            Shape::Notice => (
                NOTICE_W,
                self.toasts.first().map_or(NOTICE_H, IslandNotice::notice_h),
            ),
        };
        (
            Rect {
                x: cx - w / 2.0,
                y: self.rest.y,
                w,
                h,
            },
            radius,
        )
    }

    fn island(&self, now: Instant) -> Rect {
        Rect {
            x: self.geo[0].value(now),
            y: self.geo[1].value(now),
            w: self.geo[2].value(now),
            h: self.geo[3].value(now),
        }
    }

    fn moving(&self, now: Instant) -> bool {
        self.geo.iter().any(|a| a.is_animating(now))
    }

    fn jump_geometry(&mut self) {
        let (r, radius) = self.target();
        for (a, v) in self.geo.iter_mut().zip([r.x, r.y, r.w, r.h, radius]) {
            a.set(v);
        }
    }

    /// Flies the Island to its target: the spring out, ONE_UI back to rest.
    fn retarget(&mut self, now: Instant) {
        let (r, radius) = self.target();
        let (dur, curve) = if self.shape() == Shape::Rest {
            (
                Motion::Exit.duration(&self.theme.motion),
                Easing::Bezier(ONE_UI),
            )
        } else {
            (
                Motion::Slow.duration(&self.theme.motion),
                self.spring_curve(),
            )
        };
        let reduced = Reduced::spatial(self.reduced);
        for (a, v) in self.geo.iter_mut().zip([r.x, r.y, r.w, r.h, radius]) {
            a.animate_to(v, dur, curve, now, reduced);
        }
    }

    /// Brings geometry, faces and sheets to what the toasts and the level
    /// ask.
    fn sync(&mut self, now: Instant) {
        self.retarget(now);
        self.sync_faces(now);
        self.sync_sheets(now);
    }

    fn sync_faces(&mut self, now: Instant) {
        let wanted = self.wanted_face();
        let reduced = self.reduced;
        if !self.faces.iter().any(|(k, _)| *k == wanted) {
            let mut clock = FaceClock::settled_shown(now);
            clock.from = (0.0, 0.0, 0.0);
            clock.to = (0.0, 0.0, 0.0);
            self.faces.push((wanted, clock));
        }
        let delay = if wanted == FaceKind::Clock {
            CLOCK_DELAY_MS
        } else {
            FACE_DELAY_MS
        };
        for (kind, clock) in &mut self.faces {
            if *kind == wanted {
                clock.enter(now, delay, reduced);
            } else {
                clock.leave(now, reduced);
            }
        }
        self.faces
            .retain(|(k, c)| *k == wanted || !(c.settled(now) && c.value(now).0 <= 0.0));
    }

    fn sync_sheets(&mut self, now: Instant) {
        let wanted: Vec<(u32, f32)> = self
            .toasts
            .iter()
            .skip(1)
            .take(if self.level.is_some() { 0 } else { MAX_SHEETS })
            .map(|n| (n.id, IslandNotice::notice_h(n)))
            .collect();
        let out = self.sheet_out();
        self.sheets.retain(|s| s.gone.is_none_or(|g| now < g + out));
        for s in &mut self.sheets {
            if s.gone.is_none() && !wanted.iter().any(|(id, _)| *id == s.id) {
                s.gone = Some(now);
            }
        }
        let dur = self.spring();
        let curve = self.spring_curve();
        let reduced = Reduced::spatial(self.reduced);
        for (i, (id, h)) in wanted.iter().enumerate() {
            let slot = i + 1;
            match self
                .sheets
                .iter_mut()
                .find(|s| s.id == *id && s.gone.is_none())
            {
                Some(s) => {
                    s.slot = slot;
                    s.h = *h;
                    s.pos.animate_to(slot as f32, dur, curve, now, reduced);
                }
                None => self.sheets.push(SheetSlot {
                    id: *id,
                    pos: Animated::new(slot as f32),
                    slot,
                    h: *h,
                    born: now,
                    gone: None,
                }),
            }
        }
        self.sheets.sort_by_key(|s| s.slot);
    }

    fn sheet_looks(&self, island: Rect, now: Instant) -> Vec<SheetLook> {
        // No fan under a level display: the sheets are gone.
        let fan = if self.level.is_some() {
            0.0
        } else {
            self.fan.value(now)
        };
        let spread = fan.clamp(0.0, 1.0);
        let (t_in, t_out) = (self.sheet_in(), self.sheet_out());
        let bottom = island.y + island.h;
        let cx = island.x + island.w / 2.0;
        let mut sheets = Vec::new();
        for s in &self.sheets {
            if s.gone.is_some_and(|g| now >= g + t_out) {
                continue;
            }
            let enter = ONE_UI.eval(fraction(now.saturating_duration_since(s.born), t_in));
            let leave = s.gone.map_or(1.0, |g| {
                1.0 - fraction(now.saturating_duration_since(g), t_out)
            });
            let fade = enter * leave;
            let pos = s.pos.value(now);
            let w = NOTICE_W - 2.0 * SHEET_INSET * pos;
            let collapsed = Rect {
                x: cx - w / 2.0,
                y: bottom + SHEET_STEP * pos - SHEET_H,
                w,
                h: SHEET_H,
            };
            let above: f32 = self
                .sheets
                .iter()
                .filter(|o| o.gone.is_none() && o.slot < s.slot)
                .map(|o| o.h + FAN_GAP)
                .sum();
            let fanned = Rect {
                x: cx - NOTICE_W / 2.0,
                y: bottom + FAN_GAP + above,
                w: NOTICE_W,
                h: s.h,
            };
            let rise = if self.reduced {
                0.0
            } else {
                SHEET_RISE * (1.0 - enter)
            };
            let mut rect = Rect {
                x: lerp(collapsed.x, fanned.x, fan),
                y: lerp(collapsed.y, fanned.y, fan),
                w: lerp(collapsed.w, fanned.w, fan),
                h: lerp(collapsed.h, fanned.h, fan),
            };
            rect.y -= rise;
            sheets.push(SheetLook {
                id: s.id,
                rect,
                radius: lerp(self.theme.radius_md, self.theme.radius_lg, spread),
                glass: SHEET_GLASS * fade,
                content: spread * fade,
            });
        }
        sheets
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use celestina_paint::{Rect, Theme};
    use celestina_shell::{
        notifications::{Incoming, Notification, Notifications, Urgency},
        osd::Kind,
    };

    use super::*;
    use crate::{
        notifications::banners::{banner_key, dismiss_key, BannerAction},
        providers::notifications::NotificationsCommand,
    };

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn rest() -> Rect {
        r(870.0, 8.0, 180.0, 40.0)
    }

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn near_rect(a: Rect, b: Rect) -> bool {
        near(a.x, b.x) && near(a.y, b.y) && near(a.w, b.w) && near(a.h, b.h)
    }

    /// `n` toasts, newest first; the first has buttons when `buttons`.
    fn toasts(n: usize, buttons: bool) -> Vec<Notification> {
        let mut machine = Notifications::new();
        for i in 0..n {
            let actions: Vec<String> = if buttons && i + 1 == n {
                vec!["reply".to_owned(), "Reply".to_owned()]
            } else {
                Vec::new()
            };
            machine.post(
                &Incoming {
                    app_name: "app".to_owned(),
                    replaces_id: 0,
                    app_icon: String::new(),
                    summary: format!("s{i}"),
                    body: "body".to_owned(),
                    actions,
                    urgency: Urgency::Normal,
                    image: None,
                    expire_timeout: -1,
                },
                i as u64,
            );
        }
        let mut live = machine.live().to_vec();
        live.reverse();
        live
    }

    fn model() -> IslandNotice {
        let mut m = IslandNotice::new(&Theme::dark(), false);
        m.set_rest(rest(), Instant::now());
        m
    }

    fn face(look: &NoticeLook, kind: FaceKind) -> Option<Face> {
        look.faces.iter().find(|f| f.kind == kind).copied()
    }

    #[test]
    fn a_toast_grows_the_island_to_the_notice() {
        let t0 = Instant::now();
        let mut m = model();
        assert!(m.at_rest(t0));
        let list = toasts(1, false);
        assert!(m.set_toasts(&list, t0));
        assert!(!m.at_rest(t0));
        assert!(near_rect(m.look(t0).island.0, rest()));
        let end = m.look(ms(t0, 350));
        assert!(near_rect(end.island.0, r(750.0, 8.0, 420.0, 88.0)));
        assert_eq!(end.island.1, 20.0);
        // The spring overshoots on the way.
        assert!(m.look(ms(t0, 200)).island.0.w > 420.0);
        let id = list[0].id;
        let at160 = m.look(ms(t0, 160));
        assert_eq!(
            face(&at160, FaceKind::Clock).map_or(0.0, |f| f.opacity),
            0.0
        );
        let notice = |t| face(&m.look(ms(t0, t)), FaceKind::Notice(id)).unwrap();
        assert_eq!(notice(100).opacity, 0.0);
        assert_eq!(notice(120).opacity, 0.0);
        assert!(notice(200).opacity > 0.0 && notice(200).opacity < 1.0);
        assert_eq!(notice(320).opacity, 1.0);
        assert_eq!((notice(320).blur, notice(320).dy), (0.0, 0.0));
        assert_eq!(notice(0).dy, -10.0);
        assert_eq!(notice(0).blur, 4.0);
        let clock = face(&m.look(ms(t0, 0)), FaceKind::Clock).unwrap();
        assert_eq!((clock.opacity, clock.blur, clock.dy), (1.0, 0.0, 0.0));
        let gone = face(&m.look(ms(t0, 100)), FaceKind::Clock).unwrap();
        assert!(gone.dy > 0.0 && gone.blur > 0.0 && gone.opacity < 1.0);
        assert!(!m.is_animating(ms(t0, 351)));
        assert!(!m.at_rest(ms(t0, 400)));
    }

    #[test]
    fn buttons_make_the_notice_120_high() {
        let t0 = Instant::now();
        let mut m = model();
        m.set_toasts(&toasts(1, true), t0);
        assert!(near_rect(
            m.look(ms(t0, 400)).island.0,
            r(750.0, 8.0, 420.0, 120.0)
        ));
    }

    #[test]
    fn the_last_toast_leaving_shrinks_to_rest_in_300_ms_without_overshoot() {
        let t0 = Instant::now();
        let mut m = model();
        m.set_toasts(&toasts(1, false), t0);
        let t1 = ms(t0, 1000);
        assert!(m.set_toasts(&[], t1));
        for t in (0..=300).step_by(10) {
            let island = m.look(ms(t1, t)).island.0;
            assert!(island.h >= 40.0 - 0.01 && island.w >= 180.0 - 0.01, "{t}");
        }
        assert!(near_rect(m.look(ms(t1, 300)).island.0, rest()));
        // The clock comes back 140 ms after the start.
        assert_eq!(
            face(&m.look(ms(t1, 140)), FaceKind::Clock).unwrap().opacity,
            0.0
        );
        assert_eq!(
            face(&m.look(ms(t1, 340)), FaceKind::Clock).unwrap().opacity,
            1.0
        );
        assert!(!m.at_rest(ms(t1, 200)));
        assert!(m.at_rest(ms(t1, 340)));
    }

    #[test]
    fn the_next_toast_takes_the_island_without_going_to_rest() {
        let t0 = Instant::now();
        let mut m = model();
        let two = toasts(2, false);
        m.set_toasts(&two, t0);
        let t1 = ms(t0, 1000);
        m.set_toasts(&two[1..], t1);
        for t in (0..=700).step_by(10) {
            let island = m.look(ms(t1, t)).island.0;
            assert!(island.h > 80.0 && island.w > 400.0, "{t}");
        }
        // The old notice leaves, the next one comes in.
        let look = m.look(ms(t1, 320));
        assert_eq!(face(&look, FaceKind::Notice(two[0].id)), None);
        assert_eq!(
            face(&look, FaceKind::Notice(two[1].id)).unwrap().opacity,
            1.0
        );
    }

    #[test]
    fn a_burst_shows_two_sheets_under_the_island() {
        let t0 = Instant::now();
        let mut m = model();
        let list = toasts(4, false);
        assert!(m.set_toasts(&list, t0));
        let look = m.look(ms(t0, 1000));
        assert_eq!(look.sheets.len(), 2);
        assert_eq!(look.sheets[0].id, list[1].id);
        assert_eq!(look.sheets[1].id, list[2].id);
        assert!(near_rect(look.sheets[0].rect, r(762.0, 79.0, 396.0, 24.0)));
        assert!(near_rect(look.sheets[1].rect, r(774.0, 86.0, 372.0, 24.0)));
        assert!(look
            .sheets
            .iter()
            .all(|s| near(s.glass, 0.92) && s.content == 0.0 && s.radius == 12.0));
        // Three at most: the fourth is not shown.
        assert!(face(&look, FaceKind::Notice(list[3].id)).is_none());
        // A sheet comes in from 10 px higher.
        let first = m.look(t0).sheets;
        // The Island is still at rest: 48 + 7 - 24, 10 px higher.
        assert!(near(first[0].rect.y, 21.0) && first[0].glass == 0.0);
        let mid = m.look(ms(t0, 100)).sheets;
        assert!(mid[0].glass > 0.0 && mid[0].glass < 0.92);
    }

    #[test]
    fn hover_fans_the_sheets_and_folding_waits_250_ms() {
        let t0 = Instant::now();
        let mut m = model();
        let list = toasts(3, false);
        m.set_toasts(&list, t0);
        let t1 = ms(t0, 1000);
        m.set_hovered(true, t1);
        assert!(m.holds_timeouts());
        let fanned = m.look(ms(t1, 400)).sheets;
        assert!(near_rect(fanned[0].rect, r(750.0, 104.0, 420.0, 88.0)));
        assert!(near_rect(fanned[1].rect, r(750.0, 200.0, 420.0, 88.0)));
        assert!(fanned.iter().all(|s| s.radius == 20.0 && s.content == 1.0));
        // Leave: nothing moves for 250 ms.
        let t2 = ms(t1, 1000);
        m.set_hovered(false, t2);
        assert!(!m.holds_timeouts());
        assert!(m.is_animating(t2));
        let held = m.look(ms(t2, 249)).sheets;
        assert!(near_rect(held[0].rect, fanned[0].rect));
        let folding = m.look(ms(t2, 400)).sheets;
        assert!(folding[0].rect.w < 420.0);
        let folded = m.look(ms(t2, 250 + 350)).sheets;
        assert!(near_rect(folded[0].rect, r(762.0, 79.0, 396.0, 24.0)));
        assert!(folded[0].content == 0.0);
        // Coming back during the grace keeps it fanned.
        let t3 = ms(t2, 2000);
        m.set_hovered(true, t3);
        m.set_hovered(false, ms(t3, 500));
        m.set_hovered(true, ms(t3, 600));
        assert!(near_rect(
            m.look(ms(t3, 5000)).sheets[0].rect,
            fanned[0].rect
        ));
    }

    #[test]
    fn a_level_widens_the_island_in_place_and_holds_timeouts() {
        let t0 = Instant::now();
        let mut m = model();
        let level = Level {
            kind: Kind::Volume,
            percent: 40,
            muted: false,
        };
        m.set_level(Some(level), t0);
        assert!(m.holds_timeouts());
        let look = m.look(ms(t0, 400));
        assert!(near_rect(look.island.0, r(823.0, 8.0, 274.0, 40.0)));
        assert_eq!(look.island.1, 20.0);
        assert_eq!(face(&look, FaceKind::Level).unwrap().opacity, 1.0);
        assert_eq!(face(&look, FaceKind::Clock), None);
        assert_eq!(m.level(), Some(level));
        let t1 = ms(t0, 3000);
        m.set_level(None, t1);
        assert!(!m.holds_timeouts());
        assert!(near_rect(m.look(ms(t1, 300)).island.0, rest()));
        assert!(m.at_rest(ms(t1, 340)));
    }

    #[test]
    fn the_notice_comes_back_after_the_level() {
        let t0 = Instant::now();
        let mut m = model();
        let list = toasts(1, false);
        m.set_toasts(&list, t0);
        let t1 = ms(t0, 1000);
        let level = Level {
            kind: Kind::Brightness,
            percent: 70,
            muted: false,
        };
        m.set_level(Some(level), t1);
        let looked = m.look(ms(t1, 400));
        assert!(near_rect(looked.island.0, r(823.0, 8.0, 274.0, 40.0)));
        assert_eq!(face(&looked, FaceKind::Notice(list[0].id)), None);
        let t2 = ms(t1, 3000);
        m.set_level(None, t2);
        assert!(!m.holds_timeouts());
        let back = m.look(ms(t2, 400));
        assert!(near_rect(back.island.0, r(750.0, 8.0, 420.0, 88.0)));
        assert_eq!(
            face(&back, FaceKind::Notice(list[0].id)).unwrap().opacity,
            1.0
        );
        assert_eq!(face(&back, FaceKind::Level), None);
        assert_eq!(face(&back, FaceKind::Clock), None);
    }

    #[test]
    fn right_click_and_wheel_up_close_middle_dismisses_all() {
        let t0 = Instant::now();
        let mut m = model();
        let list = toasts(2, false);
        m.set_toasts(&list, t0);
        let sheet = list[1].id;
        let close = |id| vec![BannerAction::Send(NotificationsCommand::Close(id))];
        assert_eq!(m.gesture(list[0].id, Gesture::Right), close(list[0].id));
        assert_eq!(m.gesture(sheet, Gesture::WheelUp), close(sheet));
        assert_eq!(
            m.gesture(sheet, Gesture::Middle),
            vec![BannerAction::Send(NotificationsCommand::DismissAll)]
        );
        assert_eq!(
            m.gesture(list[0].id, Gesture::Left(dismiss_key(list[0].id))),
            close(list[0].id)
        );
        assert_eq!(
            m.gesture(list[0].id, Gesture::Left(banner_key(list[0].id))),
            vec![
                BannerAction::OpenPage,
                BannerAction::Send(NotificationsCommand::Close(list[0].id))
            ]
        );
        assert!(m.gesture(999, Gesture::Right).is_empty());
    }

    #[test]
    fn reduced_motion_jumps_geometry_and_fades_150_ms() {
        let t0 = Instant::now();
        let mut m = IslandNotice::new(&Theme::dark(), true);
        m.set_rest(rest(), t0);
        let list = toasts(2, false);
        m.set_toasts(&list, t0);
        assert!(near_rect(m.look(t0).island.0, r(750.0, 8.0, 420.0, 88.0)));
        let id = list[0].id;
        let mid = m.look(ms(t0, 75));
        for f in &mid.faces {
            assert_eq!((f.blur, f.dy), (0.0, 0.0));
        }
        assert!(face(&mid, FaceKind::Clock).unwrap().opacity < 1.0);
        assert!(face(&mid, FaceKind::Notice(id)).unwrap().opacity > 0.0);
        let end = m.look(ms(t0, 150));
        assert_eq!(face(&end, FaceKind::Clock), None);
        assert_eq!(face(&end, FaceKind::Notice(id)).unwrap().opacity, 1.0);
        assert!(!m.is_animating(ms(t0, 150)));
        // Sheets fade, no offset.
        let s = m.look(ms(t0, 75)).sheets;
        assert!(near(s[0].rect.y, 79.0) && s[0].glass > 0.0 && s[0].glass < 0.92);
        m.set_toasts(&[], ms(t0, 1000));
        assert!(near_rect(m.look(ms(t0, 1000)).island.0, rest()));
    }

    #[test]
    fn at_rest_only_after_every_motion_settled() {
        let t0 = Instant::now();
        let mut m = model();
        assert!(m.at_rest(t0) && !m.is_animating(t0));
        let list = toasts(3, false);
        m.set_toasts(&list, t0);
        assert!(!m.at_rest(ms(t0, 5000)));
        let t1 = ms(t0, 5000);
        m.set_toasts(&[], t1);
        // Sheets fade over 150 ms, the island shrinks 300, the clock is back at 140 + 200.
        assert!(!m.at_rest(ms(t1, 339)));
        assert!(m.is_animating(ms(t1, 339)));
        assert!(m.at_rest(ms(t1, 340)));
        assert!(m.look(ms(t1, 340)).sheets.is_empty());
        assert_eq!(m.look(ms(t1, 340)).faces.len(), 1);
        // A toast arriving mid-shrink turns it into the grow.
        m.set_toasts(&toasts(1, false), ms(t1, 1000));
        m.set_toasts(&[], ms(t1, 1050));
        m.set_toasts(&toasts(1, false), ms(t1, 1150));
        let h = m.look(ms(t1, 1150)).island.0.h;
        assert!(h > 40.0);
        assert!(near_rect(
            m.look(ms(t1, 1600)).island.0,
            r(750.0, 8.0, 420.0, 88.0)
        ));
    }

    #[test]
    fn the_rest_rect_can_move_while_at_rest() {
        let t0 = Instant::now();
        let mut m = model();
        m.set_rest(r(100.0, 8.0, 200.0, 40.0), t0);
        assert!(near_rect(m.look(t0).island.0, r(100.0, 8.0, 200.0, 40.0)));
        m.set_toasts(&toasts(1, false), t0);
        assert!(near_rect(
            m.look(ms(t0, 400)).island.0,
            r(-10.0, 8.0, 420.0, 88.0)
        ));
    }

    #[test]
    fn a_rest_rect_that_changes_late_flies_from_where_the_island_is() {
        let t0 = Instant::now();
        let mut m = model();
        m.set_toasts(&toasts(1, false), t0);
        let settled = r(750.0, 8.0, 420.0, 88.0);
        assert!(near_rect(m.look(ms(t0, 1000)).island.0, settled));
        // The capsule's width changes a second after the last call: the
        // Island starts to move now, it does not snap.
        let late = ms(t0, 1000);
        m.set_rest(r(900.0, 8.0, 220.0, 40.0), late);
        assert!(near_rect(m.look(late).island.0, settled));
        assert!(m.is_animating(ms(late, 50)));
        assert!(near_rect(
            m.look(ms(late, 1000)).island.0,
            r(800.0, 8.0, 420.0, 88.0)
        ));
        // A motion that finished long ago is not reported as running.
        let mut m = model();
        m.set_toasts(&toasts(1, false), t0);
        m.set_toasts(&[], ms(t0, 3000));
        m.set_rest(r(850.0, 8.0, 220.0, 40.0), ms(t0, 9000));
        assert!(!m.is_animating(ms(t0, 9000)));
        assert!(near_rect(
            m.look(ms(t0, 9000)).island.0,
            r(850.0, 8.0, 220.0, 40.0)
        ));
    }

    #[test]
    fn sheets_leave_while_a_level_shows_and_return_after() {
        let t0 = Instant::now();
        let mut m = model();
        let list = toasts(3, false);
        m.set_toasts(&list, t0);
        let t1 = ms(t0, 1000);
        m.set_hovered(true, t1);
        let level = Level {
            kind: Kind::Volume,
            percent: 40,
            muted: false,
        };
        m.set_level(Some(level), t1);
        assert!(m.look(ms(t1, 75)).sheets.iter().all(|s| s.glass < 0.92));
        assert!(m.look(ms(t1, 150)).sheets.is_empty());
        assert!(m.look(ms(t1, 2000)).sheets.is_empty());
        m.set_hovered(false, ms(t1, 2000));
        let t2 = ms(t1, 3000);
        m.set_level(None, t2);
        let back = m.look(t2).sheets;
        assert_eq!(back.len(), 2);
        assert!(back.iter().all(|s| s.glass == 0.0 && s.content == 0.0));
        let done = m.look(ms(t2, 1000)).sheets;
        assert!(near_rect(done[0].rect, r(762.0, 79.0, 396.0, 24.0)));
        assert!(near(done[0].glass, 0.92));
    }
}
