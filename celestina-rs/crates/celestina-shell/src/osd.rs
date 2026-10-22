//! When the on-screen display shows, what it shows and when it goes away.
//!
//! The display reacts to a *change* the person did not see happen: a key
//! binding moved the volume, a monitor button moved the brightness. It does
//! not announce the level the shell happened to start at, and it stays out of
//! the way of the kinds the slider popup is itself driving, while it is open
//! and while the levels it asked for are still arriving after it closed
//! (bounded by [`AUDIO_SETTLE`] and [`BRIGHTNESS_SETTLE`]). This module is
//! that policy as a pure state machine; the audio and brightness workers feed
//! it readings, and the surface draws what it returns. It does no IO and reads
//! no clock: time is the caller's `Duration` on one monotonic clock.

use std::time::Duration;

use crate::settings::{OsdKinds, OsdSettings, MAX_OSD_TIMEOUT_MS, MIN_OSD_TIMEOUT_MS};

/// What the display can be showing the level of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The default output device.
    Volume,
    /// The default input device.
    Microphone,
    /// A monitor's backlight.
    Brightness,
}

impl Kind {
    const COUNT: usize = 3;

    fn index(self) -> usize {
        match self {
            Self::Volume => 0,
            Self::Microphone => 1,
            Self::Brightness => 2,
        }
    }

    /// How long after the popup closed the level it asked for may still be
    /// on its way: an audio read-back follows its write within a few tens of
    /// milliseconds; a DDC write and its read-back take one to two seconds
    /// warm, and the popup may leave one in flight and one owed.
    fn settle(self) -> Duration {
        match self {
            Self::Volume | Self::Microphone => AUDIO_SETTLE,
            Self::Brightness => BRIGHTNESS_SETTLE,
        }
    }
}

/// The longest a closed popup's audio request keeps its kind quiet.
pub const AUDIO_SETTLE: Duration = Duration::from_secs(1);
/// The longest a closed popup's brightness request keeps its kind quiet.
pub const BRIGHTNESS_SETTLE: Duration = Duration::from_secs(6);

/// A kind the closed popup asked a level of, and which is still arriving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Settle {
    /// Past this the kind's changes show again, whatever arrived.
    until: Duration,
    /// The level asked for last; its arrival ends the settle. `None` for a
    /// mute toggle, which only the bound ends.
    expected: Option<u8>,
}

/// One reading of a level. Muted is not zero: the level is the one the device
/// remembers. Brightness is never muted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    pub percent: u8,
    pub muted: bool,
}

/// What the display shows right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub kind: Kind,
    pub reading: Reading,
}

/// A timer the caller must arm: when `deadline` passes it calls
/// [`Osd::expire`] with `generation`. A later change re-arms by issuing a new
/// generation, so an old timer firing is recognised and ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timer {
    pub generation: u64,
    pub deadline: Duration,
}

/// What a reading or a setting change asks of the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Nothing visible changes.
    None,
    /// Show (or update in place) this content and arm this timer.
    Show(Shown, Timer),
    /// Take the display away now.
    Hide,
}

/// The display's state.
#[derive(Clone, Debug)]
pub struct Osd {
    kinds: OsdKinds,
    timeout: Duration,
    /// The last reading seen per kind; `None` until the baseline is taken.
    seen: [Option<Reading>; Kind::COUNT],
    shown: Option<Shown>,
    generation: u64,
    /// The kinds an open popup drives.
    suppressed: [bool; Kind::COUNT],
    /// The kinds a closed popup asked a level of that may still arrive.
    settling: [Option<Settle>; Kind::COUNT],
}

impl Osd {
    /// A display with nothing seen yet, so the first reading of each kind is
    /// its baseline.
    #[must_use]
    pub fn new(settings: &OsdSettings) -> Self {
        Self {
            kinds: settings.kinds,
            timeout: timeout_of(settings.timeout_ms),
            seen: [None; Kind::COUNT],
            shown: None,
            generation: 0,
            suppressed: [false; Kind::COUNT],
            settling: [None; Kind::COUNT],
        }
    }

    /// What is on screen, if anything.
    #[must_use]
    pub fn shown(&self) -> Option<Shown> {
        self.shown
    }

    /// Takes in a reading at `now`.
    ///
    /// The first reading of a kind is its baseline and shows nothing; so does
    /// a reading equal to the last one, a kind the person switched off, and
    /// a reading of a kind the slider popup drives or just drove. A change of
    /// the same kind as the one on screen updates the value, another kind
    /// replaces it, and either re-arms the timer.
    pub fn observe(&mut self, kind: Kind, reading: Reading, now: Duration) -> Effect {
        let slot = &mut self.seen[kind.index()];
        let previous = slot.replace(reading);
        if previous.is_none() || previous == Some(reading) {
            self.unchanged(kind, reading);
            return Effect::None;
        }
        self.changed(kind, reading, now)
    }

    /// Takes in a reading the caller established as no change (a baseline,
    /// or the level it already had): it shows nothing, but it is the
    /// kind's last reading, and if it is the level a closed popup asked for,
    /// that popup's request has arrived.
    pub fn unchanged(&mut self, kind: Kind, reading: Reading) {
        self.seen[kind.index()] = Some(reading);
        let slot = &mut self.settling[kind.index()];
        if slot.is_some_and(|settle| settle.expected == Some(reading.percent)) {
            *slot = None;
        }
    }

    /// Takes in a reading the caller already established as a change against
    /// a baseline of its own: brightness is kept per monitor, while this
    /// machine keeps one slot per kind, so two monitors at the same level
    /// would otherwise hide each other's change. It still shows nothing for a
    /// kind the person switched off or one the slider popup drives or just
    /// drove, and it records the reading as the kind's last one.
    pub fn changed(&mut self, kind: Kind, reading: Reading, now: Duration) -> Effect {
        self.seen[kind.index()] = Some(reading);
        if self.quiet(kind, reading, now) || !self.enabled(kind) {
            return Effect::None;
        }
        let shown = Shown { kind, reading };
        self.shown = Some(shown);
        self.generation = self.generation.wrapping_add(1);
        Effect::Show(
            shown,
            Timer {
                generation: self.generation,
                deadline: now.saturating_add(self.timeout),
            },
        )
    }

    /// A timer fired. Returns whether the display must now go away: only the
    /// latest generation does, so a re-armed display outlives its old timer.
    pub fn expire(&mut self, generation: u64) -> bool {
        if self.shown.is_none() || generation != self.generation {
            return false;
        }
        self.shown = None;
        true
    }

    /// A slider popup driving `kinds` opened. While it is open those kinds
    /// show nothing (the popup shows its own value) and one of them on screen
    /// goes away; other kinds still show. The readings it drives still update
    /// the baselines.
    pub fn suppress(&mut self, kinds: &[Kind]) -> Effect {
        for kind in kinds {
            self.suppressed[kind.index()] = true;
            self.settling[kind.index()] = None;
        }
        match self.shown {
            Some(shown) if kinds.contains(&shown.kind) => self.hide(),
            _ => Effect::None,
        }
    }

    /// The popup driving `kinds` closed at `now`; `asked` is the last level
    /// it asked of each kind (`None` for a mute toggle). A kind it asked a
    /// level of stays quiet until that level arrives (an intermediate
    /// read-back of a coalesced write is quiet too), at most its settle
    /// bound; a kind whose asked level was already read needs no wait.
    pub fn release(&mut self, kinds: &[Kind], asked: &[(Kind, Option<u8>)], now: Duration) {
        for kind in kinds {
            let index = kind.index();
            self.suppressed[index] = false;
            self.settling[index] = None;
            let Some((_, expected)) = asked.iter().rev().find(|(of, _)| of == kind) else {
                continue;
            };
            let arrived =
                expected.is_some() && self.seen[index].map(|seen| seen.percent) == *expected;
            if !arrived {
                self.settling[index] = Some(Settle {
                    until: now.saturating_add(kind.settle()),
                    expected: *expected,
                });
            }
        }
    }

    /// Whether `kind` shows nothing now because a popup drives it or its
    /// request is still arriving; ends the settle the reading completes.
    fn quiet(&mut self, kind: Kind, reading: Reading, now: Duration) -> bool {
        let index = kind.index();
        if self.suppressed[index] {
            return true;
        }
        let Some(settle) = self.settling[index] else {
            return false;
        };
        if now >= settle.until {
            self.settling[index] = None;
            return false;
        }
        if settle.expected == Some(reading.percent) {
            self.settling[index] = None;
        }
        true
    }

    /// Forgets the baseline of a kind whose device went away, so the next
    /// reading (a different device) is a baseline too, not a change.
    pub fn forget(&mut self, kind: Kind) {
        self.seen[kind.index()] = None;
        if self.shown.is_some_and(|shown| shown.kind == kind) {
            self.hide();
        }
    }

    /// Applies new settings. A kind switched off while on screen goes away.
    pub fn configure(&mut self, settings: &OsdSettings) -> Effect {
        self.kinds = settings.kinds;
        self.timeout = timeout_of(settings.timeout_ms);
        match self.shown {
            Some(shown) if !self.enabled(shown.kind) => self.hide(),
            _ => Effect::None,
        }
    }

    fn enabled(&self, kind: Kind) -> bool {
        match kind {
            Kind::Volume => self.kinds.volume,
            Kind::Microphone => self.kinds.microphone,
            Kind::Brightness => self.kinds.brightness,
        }
    }

    fn hide(&mut self) -> Effect {
        if self.shown.take().is_some() {
            // Invalidate the pending timer.
            self.generation = self.generation.wrapping_add(1);
            Effect::Hide
        } else {
            Effect::None
        }
    }
}

fn timeout_of(milliseconds: u32) -> Duration {
    Duration::from_millis(u64::from(
        milliseconds.clamp(MIN_OSD_TIMEOUT_MS, MAX_OSD_TIMEOUT_MS),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: Duration = Duration::from_secs(10);

    fn level(percent: u8) -> Reading {
        Reading {
            percent,
            muted: false,
        }
    }

    fn osd() -> Osd {
        Osd::new(&OsdSettings::default())
    }

    fn shown(effect: Effect) -> (Shown, Timer) {
        match effect {
            Effect::Show(shown, timer) => (shown, timer),
            other => panic!("expected Show, got {other:?}"),
        }
    }

    #[test]
    fn the_first_reading_is_a_baseline_and_shows_nothing() {
        let mut osd = osd();
        for kind in [Kind::Volume, Kind::Microphone, Kind::Brightness] {
            assert_eq!(osd.observe(kind, level(40), T0), Effect::None);
        }
        assert_eq!(osd.shown(), None);
        // The same value again is not a change either.
        assert_eq!(osd.observe(Kind::Volume, level(40), T0), Effect::None);
    }

    #[test]
    fn a_later_change_shows_with_a_timer_of_the_configured_timeout() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        let (content, timer) = shown(osd.observe(Kind::Volume, level(45), T0));
        assert_eq!(content.kind, Kind::Volume);
        assert_eq!(content.reading, level(45));
        assert_eq!(timer.deadline, T0 + Duration::from_millis(1500));
        assert_eq!(osd.shown(), Some(content));
    }

    #[test]
    fn muting_is_a_change_that_keeps_the_remembered_level() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        let muted = Reading {
            percent: 40,
            muted: true,
        };
        let (content, _) = shown(osd.observe(Kind::Volume, muted, T0));
        assert_eq!(content.reading, muted);
    }

    #[test]
    fn the_same_kind_updates_and_another_kind_replaces() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        osd.observe(Kind::Brightness, level(70), T0);
        let (_, first) = shown(osd.observe(Kind::Volume, level(45), T0));
        let (content, second) =
            shown(osd.observe(Kind::Volume, level(50), T0 + Duration::from_millis(300)));
        assert_eq!(content.reading.percent, 50);
        assert!(second.generation > first.generation);
        assert_eq!(second.deadline, T0 + Duration::from_millis(1800));

        let (content, third) = shown(osd.observe(Kind::Brightness, level(60), T0));
        assert_eq!(content.kind, Kind::Brightness);
        assert!(third.generation > second.generation);
        assert_eq!(osd.shown().map(|shown| shown.kind), Some(Kind::Brightness));
    }

    #[test]
    fn an_old_timer_does_not_close_a_rearmed_display() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        let (_, old) = shown(osd.observe(Kind::Volume, level(45), T0));
        let (_, new) = shown(osd.observe(Kind::Volume, level(50), T0));
        assert!(!osd.expire(old.generation));
        assert!(osd.shown().is_some());
        assert!(osd.expire(new.generation));
        assert_eq!(osd.shown(), None);
        // A timer for a display already gone is nothing.
        assert!(!osd.expire(new.generation));
    }

    #[test]
    fn a_kind_switched_off_is_tracked_but_never_shown() {
        let mut settings = OsdSettings::default();
        settings.kinds.microphone = false;
        let mut osd = Osd::new(&settings);
        osd.observe(Kind::Microphone, level(30), T0);
        assert_eq!(osd.observe(Kind::Microphone, level(35), T0), Effect::None);
        // Switching it on does not announce what it already moved to.
        settings.kinds.microphone = true;
        osd.configure(&settings);
        assert_eq!(osd.observe(Kind::Microphone, level(35), T0), Effect::None);
        assert!(matches!(
            osd.observe(Kind::Microphone, level(40), T0),
            Effect::Show(..)
        ));
    }

    #[test]
    fn switching_off_the_kind_on_screen_hides_it() {
        let mut settings = OsdSettings::default();
        let mut osd = Osd::new(&settings);
        osd.observe(Kind::Volume, level(40), T0);
        let (_, timer) = shown(osd.observe(Kind::Volume, level(45), T0));
        settings.kinds.volume = false;
        assert_eq!(osd.configure(&settings), Effect::Hide);
        assert!(!osd.expire(timer.generation));
        assert_eq!(osd.configure(&settings), Effect::None);
    }

    #[test]
    fn the_slider_popup_suppresses_the_display_and_its_changes_are_not_announced() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        let (_, timer) = shown(osd.observe(Kind::Volume, level(45), T0));

        assert_eq!(osd.suppress(VOLUME_POPUP), Effect::Hide);
        assert!(!osd.expire(timer.generation));
        assert_eq!(osd.observe(Kind::Volume, level(60), T0), Effect::None);
        assert_eq!(osd.suppress(VOLUME_POPUP), Effect::None);

        osd.release(VOLUME_POPUP, &[(Kind::Volume, Some(60))], T0);
        // 60 was seen while suppressed: not a change now.
        assert_eq!(osd.observe(Kind::Volume, level(60), T0), Effect::None);
        assert!(matches!(
            osd.observe(Kind::Volume, level(65), T0),
            Effect::Show(..)
        ));
    }

    const VOLUME_POPUP: &[Kind] = &[Kind::Volume, Kind::Microphone];
    const BRIGHTNESS_POPUP: &[Kind] = &[Kind::Brightness];

    #[test]
    fn a_popup_suppresses_only_its_own_kinds() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        osd.observe(Kind::Brightness, level(70), T0);
        let (_, timer) = shown(osd.observe(Kind::Brightness, level(60), T0));
        // The volume popup opens: the brightness on screen stays.
        assert_eq!(osd.suppress(VOLUME_POPUP), Effect::None);
        assert_eq!(osd.shown().map(|s| s.kind), Some(Kind::Brightness));
        assert!(osd.expire(timer.generation));
        assert_eq!(osd.observe(Kind::Volume, level(60), T0), Effect::None);
        // A brightness key while it is open is shown.
        assert!(matches!(
            osd.observe(Kind::Brightness, level(55), T0),
            Effect::Show(..)
        ));
        // The brightness popup hides brightness only.
        assert_eq!(osd.suppress(BRIGHTNESS_POPUP), Effect::Hide);
        osd.release(VOLUME_POPUP, &[], T0);
        assert!(matches!(
            osd.observe(Kind::Volume, level(65), T0),
            Effect::Show(..)
        ));
    }

    #[test]
    fn the_level_the_popup_asked_for_arriving_after_it_closed_is_not_announced() {
        let mut osd = osd();
        osd.observe(Kind::Brightness, level(50), T0);
        osd.suppress(BRIGHTNESS_POPUP);
        // The popup asked for 30; the monitor read back 40 so far.
        osd.changed(Kind::Brightness, level(40), T0);
        osd.release(BRIGHTNESS_POPUP, &[(Kind::Brightness, Some(30))], T0);
        // Read-backs of what it asked arrive after it closed: an intermediate
        // one and the one asked for.
        let later = T0 + Duration::from_millis(1500);
        assert_eq!(
            osd.changed(Kind::Brightness, level(35), later),
            Effect::None
        );
        assert_eq!(
            osd.changed(Kind::Brightness, level(30), later),
            Effect::None
        );
        // The settle ends with it: a key press right after is shown.
        assert!(matches!(
            osd.changed(Kind::Brightness, level(25), later),
            Effect::Show(..)
        ));
    }

    #[test]
    fn the_settle_after_the_popup_is_bounded_and_needless_when_already_read() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        osd.suppress(VOLUME_POPUP);
        osd.release(VOLUME_POPUP, &[(Kind::Volume, Some(70))], T0);
        // The asked level never comes: past the grace, changes show again.
        assert_eq!(osd.observe(Kind::Volume, level(45), T0), Effect::None);
        assert!(matches!(
            osd.observe(Kind::Volume, level(50), T0 + AUDIO_SETTLE),
            Effect::Show(..)
        ));

        // Its read-back already arrived while the popup was open: nothing to
        // wait for, the next change shows at once.
        let mut osd = self::osd();
        osd.observe(Kind::Volume, level(40), T0);
        osd.suppress(VOLUME_POPUP);
        osd.observe(Kind::Volume, level(70), T0);
        osd.release(VOLUME_POPUP, &[(Kind::Volume, Some(70))], T0);
        assert!(matches!(
            osd.observe(Kind::Volume, level(75), T0),
            Effect::Show(..)
        ));

        // A repeated read-back equal to the last level still ends the settle.
        let mut osd = self::osd();
        osd.observe(Kind::Microphone, level(40), T0);
        osd.suppress(VOLUME_POPUP);
        osd.release(VOLUME_POPUP, &[(Kind::Microphone, Some(20))], T0);
        osd.observe(Kind::Microphone, level(20), T0);
        assert_eq!(osd.observe(Kind::Microphone, level(20), T0), Effect::None);
        assert!(matches!(
            osd.observe(Kind::Microphone, level(25), T0),
            Effect::Show(..)
        ));
    }

    #[test]
    fn a_change_established_by_the_caller_shows_even_at_the_level_last_seen() {
        let mut osd = osd();
        // Monitor A moved to 60; monitor B, keyed by the caller, moves from
        // 40 to 60 too: the kind's slot already says 60, the change is real.
        osd.observe(Kind::Brightness, level(50), T0);
        let (_, first) = shown(osd.observe(Kind::Brightness, level(60), T0));
        let (content, second) = shown(osd.changed(Kind::Brightness, level(60), T0));
        assert_eq!(content.reading, level(60));
        assert!(second.generation > first.generation);
        // The kind's switch and the popup still hold.
        osd.suppress(BRIGHTNESS_POPUP);
        assert_eq!(osd.changed(Kind::Brightness, level(70), T0), Effect::None);
        osd.release(BRIGHTNESS_POPUP, &[], T0);
        let mut settings = OsdSettings::default();
        settings.kinds.brightness = false;
        osd.configure(&settings);
        assert_eq!(osd.changed(Kind::Brightness, level(80), T0), Effect::None);
    }

    #[test]
    fn a_vanished_device_makes_the_next_reading_a_baseline() {
        let mut osd = osd();
        osd.observe(Kind::Volume, level(40), T0);
        osd.observe(Kind::Volume, level(45), T0);
        osd.forget(Kind::Volume);
        assert_eq!(osd.shown(), None);
        assert_eq!(osd.observe(Kind::Volume, level(90), T0), Effect::None);
    }

    #[test]
    fn the_timeout_is_bounded_even_when_the_caller_passes_nonsense() {
        for (milliseconds, wanted) in [(0, 500), (100, 500), (1500, 1500), (60_000, 5000)] {
            let settings = OsdSettings {
                timeout_ms: milliseconds,
                ..OsdSettings::default()
            };
            let mut osd = Osd::new(&settings);
            osd.observe(Kind::Volume, level(1), T0);
            let (_, timer) = shown(osd.observe(Kind::Volume, level(2), T0));
            assert_eq!(timer.deadline - T0, Duration::from_millis(wanted));
        }
    }
}
