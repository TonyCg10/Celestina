//! What the shell does when the person stops using the session, and when it
//! must not.
//!
//! The chain has two steps. After [`IdleConfig::screen_off`] of inactivity the
//! monitors are powered off; after [`IdleConfig::suspend`] the session is
//! locked and the machine suspended. The compositor measures the inactivity
//! (one idle notification per step, both counted from the last input); this
//! module only decides what each notification means, so it does no IO, reads
//! no clock and owns no thread.
//!
//! Three rules shape every answer.
//!
//! **An idle period acts at most once per step.** A period starts with the
//! first [`IdleChain::idled`] and ends with any [`IdleChain::resumed`]: input
//! is input, whichever notification reports it. Inside one period a duplicate
//! notification, a switch flipped back and forth or a locker appearing never
//! repeats a step that was already carried out or refused.
//!
//! **Idle state is tracked even while nothing may act.** With the duty off or
//! caffeine on, notifications still move the state, so the moment both allow
//! acting again the steps already due fire — once, and in order.
//!
//! **Locking fails closed.** A session that suspends unlocked wakes unlocked,
//! so while no locker is available the second step is reported as
//! [`Refusal::NoLocker`] and nothing is suspended (decision D1 of
//! `celestina/docs/evidence/2026-10-16-energy-plan.md`).

use std::error::Error;
use std::fmt;
use std::time::Duration;

/// The shortest screen-off delay. Less than this turns the monitors off while
/// someone is reading.
pub const MIN_SCREEN_OFF: Duration = Duration::from_secs(5);
/// The longest delay either step accepts. A day of inactivity is a forgotten
/// session, not a preference.
pub const MAX_DELAY: Duration = Duration::from_secs(24 * 60 * 60);

const DEFAULT_SCREEN_OFF: Duration = Duration::from_secs(20);
const DEFAULT_SUSPEND: Duration = Duration::from_secs(240);

/// The two delays of the chain, both measured from the last input.
///
/// The fields are private so that every value in use passed
/// [`IdleConfig::new`]'s bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdleConfig {
    screen_off: Duration,
    suspend: Duration,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            screen_off: DEFAULT_SCREEN_OFF,
            suspend: DEFAULT_SUSPEND,
        }
    }
}

impl IdleConfig {
    /// A chain's delays, if they make sense: the screen goes off no sooner than
    /// [`MIN_SCREEN_OFF`], the session suspends strictly after the screen went
    /// off, and neither waits longer than [`MAX_DELAY`].
    ///
    /// # Errors
    ///
    /// The first bound the pair breaks, carrying the offending values.
    pub fn new(screen_off: Duration, suspend: Duration) -> Result<Self, IdleConfigError> {
        if screen_off < MIN_SCREEN_OFF {
            return Err(IdleConfigError::ScreenOffTooShort { screen_off });
        }
        if screen_off > MAX_DELAY {
            return Err(IdleConfigError::TooLong {
                step: Step::ScreenOff,
                delay: screen_off,
            });
        }
        if suspend > MAX_DELAY {
            return Err(IdleConfigError::TooLong {
                step: Step::LockAndSuspend,
                delay: suspend,
            });
        }
        if suspend <= screen_off {
            return Err(IdleConfigError::SuspendNotAfterScreenOff {
                screen_off,
                suspend,
            });
        }
        Ok(Self {
            screen_off,
            suspend,
        })
    }

    /// How long without input before the monitors are powered off.
    #[must_use]
    pub fn screen_off(&self) -> Duration {
        self.screen_off
    }

    /// How long without input before the session is locked and suspended,
    /// counted from the last input, not from the screen going off.
    #[must_use]
    pub fn suspend(&self) -> Duration {
        self.suspend
    }
}

/// Why a pair of delays was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleConfigError {
    /// The screen would go off sooner than [`MIN_SCREEN_OFF`].
    ScreenOffTooShort { screen_off: Duration },
    /// Suspending would not come strictly after the screen going off.
    SuspendNotAfterScreenOff {
        screen_off: Duration,
        suspend: Duration,
    },
    /// A step would wait longer than [`MAX_DELAY`].
    TooLong { step: Step, delay: Duration },
}

impl fmt::Display for IdleConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScreenOffTooShort { screen_off } => write!(
                formatter,
                "screen-off delay {screen_off:?} is shorter than the minimum {MIN_SCREEN_OFF:?}"
            ),
            Self::SuspendNotAfterScreenOff {
                screen_off,
                suspend,
            } => write!(
                formatter,
                "suspend delay {suspend:?} does not come after the screen-off delay {screen_off:?}"
            ),
            Self::TooLong { step, delay } => write!(
                formatter,
                "{step:?} delay {delay:?} is longer than the maximum {MAX_DELAY:?}"
            ),
        }
    }
}

impl Error for IdleConfigError {}

/// One step of the chain, in the order the steps come due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    ScreenOff,
    LockAndSuspend,
}

impl Step {
    /// Every step, earliest first.
    pub const ALL: [Self; 2] = [Self::ScreenOff, Self::LockAndSuspend];

    fn index(self) -> usize {
        match self {
            Self::ScreenOff => 0,
            Self::LockAndSuspend => 1,
        }
    }
}

/// Why a due step was not carried out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Suspending needs a locker first, and none is available.
    NoLocker,
}

/// What the worker around the chain must do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleAction {
    /// Ask the compositor to power off every monitor.
    PowerOffMonitors,
    /// Lock the session, then suspend the machine.
    LockAndSuspend,
    /// A step came due and was deliberately not carried out.
    Refused(Refusal),
}

/// The idle chain: which steps are idle, whether anything may act, and what
/// this idle period has already done.
#[derive(Clone, Debug)]
pub struct IdleChain {
    config: IdleConfig,
    idle: [bool; 2],
    handled: [bool; 2],
    caffeine: bool,
    enabled: bool,
    locker_available: bool,
}

impl IdleChain {
    /// A chain with nothing idle, the duty off, caffeine off and no locker:
    /// nothing acts until the worker says the duty is on.
    #[must_use]
    pub fn new(config: IdleConfig) -> Self {
        Self {
            config,
            idle: [false; 2],
            handled: [false; 2],
            caffeine: false,
            enabled: false,
            locker_available: false,
        }
    }

    #[must_use]
    pub fn config(&self) -> IdleConfig {
        self.config
    }

    /// The idle notifications the worker must arm, earliest first, each with
    /// its delay from the last input.
    #[must_use]
    pub fn steps(&self) -> [(Step, Duration); 2] {
        [
            (Step::ScreenOff, self.config.screen_off),
            (Step::LockAndSuspend, self.config.suspend),
        ]
    }

    /// Whether the compositor last reported this step as idle in the current
    /// period.
    #[must_use]
    pub fn is_idle(&self, step: Step) -> bool {
        self.idle[step.index()]
    }

    #[must_use]
    pub fn caffeine(&self) -> bool {
        self.caffeine
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn locker_available(&self) -> bool {
        self.locker_available
    }

    /// The compositor reports `step`'s delay has passed without input.
    ///
    /// Being idle long enough to suspend implies being idle long enough to
    /// turn the screen off, so a lone `LockAndSuspend` also marks `ScreenOff`
    /// idle and the screen still goes off first.
    pub fn idled(&mut self, step: Step) -> Vec<IdleAction> {
        for earlier in Step::ALL {
            self.idle[earlier.index()] = true;
            if earlier == step {
                break;
            }
        }
        self.due()
    }

    /// The compositor reports input after `step` was idle. Input ends the idle
    /// period for every step, whichever notification reported it, and owes no
    /// action: turning the monitors back on is the compositor's job.
    pub fn resumed(&mut self, _step: Step) -> Vec<IdleAction> {
        self.idle = [false; 2];
        self.handled = [false; 2];
        Vec::new()
    }

    /// Caffeine on keeps every step from acting; turning it off acts on the
    /// steps already due.
    pub fn set_caffeine(&mut self, caffeine: bool) -> Vec<IdleAction> {
        self.caffeine = caffeine;
        self.due()
    }

    /// The duty switch. Off keeps every step from acting; on acts on the steps
    /// already due.
    pub fn set_enabled(&mut self, enabled: bool) -> Vec<IdleAction> {
        self.enabled = enabled;
        self.due()
    }

    /// Whether a locker can lock the session before it suspends. A step this
    /// period already refused stays refused until the next period.
    pub fn set_locker_available(&mut self, available: bool) -> Vec<IdleAction> {
        self.locker_available = available;
        self.due()
    }

    /// Every step that is idle, may act and has not acted this period, in
    /// order.
    fn due(&mut self) -> Vec<IdleAction> {
        if !self.enabled || self.caffeine {
            return Vec::new();
        }
        let mut actions = Vec::new();
        for step in Step::ALL {
            let index = step.index();
            if !self.idle[index] || self.handled[index] {
                continue;
            }
            self.handled[index] = true;
            actions.push(match step {
                Step::ScreenOff => IdleAction::PowerOffMonitors,
                Step::LockAndSuspend if self.locker_available => IdleAction::LockAndSuspend,
                Step::LockAndSuspend => IdleAction::Refused(Refusal::NoLocker),
            });
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFUSED: IdleAction = IdleAction::Refused(Refusal::NoLocker);

    fn chain() -> IdleChain {
        let mut chain = IdleChain::new(IdleConfig::default());
        assert!(chain.set_enabled(true).is_empty());
        chain
    }

    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn the_default_config_is_twenty_seconds_then_four_minutes() {
        let config = IdleConfig::default();
        assert_eq!(config.screen_off(), secs(20));
        assert_eq!(config.suspend(), secs(240));
        assert_eq!(IdleConfig::new(secs(20), secs(240)), Ok(config));
    }

    #[test]
    fn configs_outside_the_bounds_are_refused_with_their_values() {
        assert_eq!(
            IdleConfig::new(secs(4), secs(240)),
            Err(IdleConfigError::ScreenOffTooShort {
                screen_off: secs(4)
            })
        );
        assert_eq!(
            IdleConfig::new(secs(60), secs(60)),
            Err(IdleConfigError::SuspendNotAfterScreenOff {
                screen_off: secs(60),
                suspend: secs(60)
            })
        );
        assert_eq!(
            IdleConfig::new(secs(60), secs(30)),
            Err(IdleConfigError::SuspendNotAfterScreenOff {
                screen_off: secs(60),
                suspend: secs(30)
            })
        );
        assert_eq!(
            IdleConfig::new(secs(60), MAX_DELAY + secs(1)),
            Err(IdleConfigError::TooLong {
                step: Step::LockAndSuspend,
                delay: MAX_DELAY + secs(1)
            })
        );
        assert_eq!(
            IdleConfig::new(MAX_DELAY + secs(1), MAX_DELAY + secs(2)),
            Err(IdleConfigError::TooLong {
                step: Step::ScreenOff,
                delay: MAX_DELAY + secs(1)
            })
        );
        assert!(IdleConfig::new(MIN_SCREEN_OFF, MAX_DELAY).is_ok());
        assert!(!IdleConfigError::ScreenOffTooShort {
            screen_off: secs(1)
        }
        .to_string()
        .is_empty());
    }

    #[test]
    fn steps_name_both_delays_earliest_first() {
        let config = IdleConfig::new(secs(30), secs(600)).unwrap_or_default();
        let chain = IdleChain::new(config);
        assert_eq!(
            chain.steps(),
            [
                (Step::ScreenOff, secs(30)),
                (Step::LockAndSuspend, secs(600))
            ]
        );
    }

    #[test]
    fn a_new_chain_does_nothing_until_the_duty_is_on() {
        let mut chain = IdleChain::new(IdleConfig::default());
        assert!(chain.idled(Step::ScreenOff).is_empty());
        assert!(chain.is_idle(Step::ScreenOff));
        assert_eq!(chain.set_enabled(true), [IdleAction::PowerOffMonitors]);
    }

    #[test]
    fn the_screen_goes_off_before_the_session_is_locked_and_suspended() {
        let mut chain = chain();
        assert!(chain.set_locker_available(true).is_empty());
        assert_eq!(chain.idled(Step::ScreenOff), [IdleAction::PowerOffMonitors]);
        assert_eq!(
            chain.idled(Step::LockAndSuspend),
            [IdleAction::LockAndSuspend]
        );
    }

    #[test]
    fn a_lone_suspend_notification_still_turns_the_screen_off_first() {
        let mut chain = chain();
        chain.set_locker_available(true);
        assert_eq!(
            chain.idled(Step::LockAndSuspend),
            [IdleAction::PowerOffMonitors, IdleAction::LockAndSuspend]
        );
        assert!(chain.is_idle(Step::ScreenOff));
    }

    #[test]
    fn caffeine_keeps_every_step_from_acting_but_idle_is_still_tracked() {
        let mut chain = chain();
        chain.set_locker_available(true);
        assert!(chain.set_caffeine(true).is_empty());
        assert!(chain.idled(Step::ScreenOff).is_empty());
        assert!(chain.idled(Step::LockAndSuspend).is_empty());
        assert!(chain.is_idle(Step::ScreenOff));
        assert!(chain.is_idle(Step::LockAndSuspend));
    }

    #[test]
    fn a_disabled_duty_keeps_every_step_from_acting_but_idle_is_still_tracked() {
        let mut chain = IdleChain::new(IdleConfig::default());
        assert!(chain.idled(Step::ScreenOff).is_empty());
        assert!(chain.idled(Step::LockAndSuspend).is_empty());
        assert!(chain.is_idle(Step::LockAndSuspend));
        assert!(!chain.enabled());
    }

    #[test]
    fn caffeine_off_while_idle_fires_the_due_step_once() {
        let mut chain = chain();
        chain.set_caffeine(true);
        chain.idled(Step::ScreenOff);
        assert_eq!(chain.set_caffeine(false), [IdleAction::PowerOffMonitors]);
        assert!(chain.set_caffeine(true).is_empty());
        assert!(chain.set_caffeine(false).is_empty());
        assert!(chain.idled(Step::ScreenOff).is_empty());
    }

    #[test]
    fn caffeine_off_after_both_steps_came_due_fires_both_in_order() {
        let mut chain = chain();
        chain.set_locker_available(true);
        chain.set_caffeine(true);
        chain.idled(Step::ScreenOff);
        chain.idled(Step::LockAndSuspend);
        assert_eq!(
            chain.set_caffeine(false),
            [IdleAction::PowerOffMonitors, IdleAction::LockAndSuspend]
        );
    }

    #[test]
    fn a_resume_before_the_second_timeout_means_suspend_never_fires() {
        let mut chain = chain();
        chain.set_locker_available(true);
        assert_eq!(chain.idled(Step::ScreenOff), [IdleAction::PowerOffMonitors]);
        assert!(chain.resumed(Step::ScreenOff).is_empty());
        assert!(!chain.is_idle(Step::ScreenOff));
        assert!(!chain.is_idle(Step::LockAndSuspend));
        assert!(chain.set_caffeine(false).is_empty());
        assert!(chain.set_enabled(true).is_empty());
    }

    #[test]
    fn a_resume_clears_the_period_for_every_step() {
        let mut chain = chain();
        chain.set_locker_available(true);
        chain.idled(Step::LockAndSuspend);
        assert!(chain.resumed(Step::ScreenOff).is_empty());
        assert!(!chain.is_idle(Step::LockAndSuspend));
        assert_eq!(chain.idled(Step::ScreenOff), [IdleAction::PowerOffMonitors]);
        assert_eq!(
            chain.idled(Step::LockAndSuspend),
            [IdleAction::LockAndSuspend]
        );
    }

    #[test]
    fn duplicate_idle_notifications_never_repeat_an_action() {
        let mut chain = chain();
        assert_eq!(chain.idled(Step::ScreenOff), [IdleAction::PowerOffMonitors]);
        assert!(chain.idled(Step::ScreenOff).is_empty());
        assert_eq!(chain.idled(Step::LockAndSuspend), [REFUSED]);
        assert!(chain.idled(Step::LockAndSuspend).is_empty());
        assert!(chain.idled(Step::ScreenOff).is_empty());
    }

    #[test]
    fn without_a_locker_suspend_is_refused_once_per_period_and_never_carried_out() {
        let mut chain = chain();
        assert!(!chain.locker_available());
        chain.idled(Step::ScreenOff);
        assert_eq!(chain.idled(Step::LockAndSuspend), [REFUSED]);
        assert!(chain.idled(Step::LockAndSuspend).is_empty());
        assert!(chain.set_caffeine(true).is_empty());
        assert!(chain.set_caffeine(false).is_empty());
        assert!(chain.set_enabled(false).is_empty());
        assert!(chain.set_enabled(true).is_empty());
        assert!(chain.set_locker_available(true).is_empty());

        chain.resumed(Step::LockAndSuspend);
        chain.set_locker_available(false);
        assert_eq!(
            chain.idled(Step::LockAndSuspend),
            [IdleAction::PowerOffMonitors, REFUSED]
        );
    }

    #[test]
    fn disabling_the_duty_mid_idle_emits_nothing_and_enabling_it_fires_what_is_due() {
        let mut chain = chain();
        chain.set_locker_available(true);
        assert!(chain.set_enabled(false).is_empty());
        assert!(chain.idled(Step::ScreenOff).is_empty());
        assert!(chain.idled(Step::LockAndSuspend).is_empty());
        assert_eq!(
            chain.set_enabled(true),
            [IdleAction::PowerOffMonitors, IdleAction::LockAndSuspend]
        );
        assert!(chain.set_enabled(false).is_empty());
        assert!(chain.set_enabled(true).is_empty());
    }

    #[test]
    fn disabling_after_the_screen_went_off_still_fires_suspend_once_reenabled() {
        let mut chain = chain();
        chain.set_locker_available(true);
        assert_eq!(chain.idled(Step::ScreenOff), [IdleAction::PowerOffMonitors]);
        assert!(chain.set_enabled(false).is_empty());
        assert!(chain.idled(Step::LockAndSuspend).is_empty());
        assert_eq!(chain.set_enabled(true), [IdleAction::LockAndSuspend]);
    }
}
