//! What a key binding may ask the session to become, before anything is done
//! about it.
//!
//! A session verb arrives as text from a keyboard binding and reaches a device,
//! a compositor or a locker. The part worth owning here is the part that has no
//! IO in it: which verbs exist, what their options must be for the request to
//! mean anything, and what a level becomes when a step is applied to it. A
//! provider then carries out a request it can no longer misread, and a refusal
//! is one sentence the panel can show instead of a silence.
//!
//! Nothing here decides whether a provider exists. A verb that names an absent
//! capability parses fine and is refused where the capability would have been —
//! parsing is vocabulary, not availability.

use serde_json::Value;

use crate::snapshot::Payload;

/// The loudest a `set` may ask for. Sessions that allow overdrive report it
/// (see [`crate::audio`]), but nothing in this shell *asks* for more than the
/// device's nominal maximum: a binding that overshoots is a mistake, not an
/// intent.
pub const MAX_LEVEL: u8 = 100;
/// The largest single step a binding may take. A wheel notch is five; anything
/// past a full range is a typo.
pub const MAX_STEP: i16 = 100;

/// Where a level should end up: at a value, or that far from wherever it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelChange {
    Set(u8),
    Step(i16),
}

impl LevelChange {
    /// The level this change leaves, given the one the device reports now.
    ///
    /// Clamped at both ends: a step past either edge stops there rather than
    /// wrapping, and a reading above [`MAX_LEVEL`] is stepped down from where
    /// it really is instead of being snapped to the ceiling first.
    #[must_use]
    pub fn applied_to(self, current: u8) -> u8 {
        match self {
            Self::Set(level) => level.min(MAX_LEVEL),
            Self::Step(step) => {
                let ceiling = i16::from(current).max(i16::from(MAX_LEVEL));
                let target = i16::from(current).saturating_add(step).clamp(0, ceiling);
                u8::try_from(target).unwrap_or(MAX_LEVEL)
            }
        }
    }

    /// The one change that means what this one and then `next` meant.
    ///
    /// A device slow enough to answer once per burst — a monitor over DDC —
    /// applies the newest target rather than every notch on the way to it, and
    /// this is what "newest" means: a `set` replaces whatever was owed, and
    /// steps accumulate instead of overwriting each other.
    #[must_use]
    pub fn followed_by(self, next: Self) -> Self {
        match (self, next) {
            (_, Self::Set(level)) => Self::Set(level.min(MAX_LEVEL)),
            (Self::Set(level), Self::Step(step)) => Self::Set(
                u8::try_from(
                    i16::from(level)
                        .saturating_add(step)
                        .clamp(0, i16::from(MAX_LEVEL)),
                )
                .unwrap_or(MAX_LEVEL),
            ),
            (Self::Step(owed), Self::Step(step)) => Self::Step(owed.saturating_add(step)),
        }
    }
}

/// A state a binding may ask a two-state feature to take. `Toggle` is a
/// separate verb rather than a missing value, so a lost keystroke can never be
/// read as "the other one".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    On,
    Off,
    Toggle,
}

/// Which end of the session's audio a mute verb means. They are separate verbs
/// rather than one verb with a device option, because a binding that silences
/// the wrong end of a call is the failure this vocabulary exists to prevent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuteDevice {
    Output,
    Input,
}

/// One typed request. The variants name capabilities, not tools: which program
/// or protocol serves night light or a lock is the provider's business, and is
/// deliberately absent from the vocabulary a binding writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionRequest {
    Volume(LevelChange),
    Mute(MuteDevice, Switch),
    Brightness(LevelChange),
    NightLight(Switch),
    /// Keeping the session awake on purpose.
    Caffeine(Switch),
    /// Blanking the outputs now. There is no "on": any input wakes them, and a
    /// verb that claimed to turn them back on would be describing the
    /// compositor's behaviour rather than causing it.
    DisplaysOff,
    Lock,
    LockAndSuspend,
}

fn level(options: &Payload) -> Result<u8, String> {
    let value = options
        .get("level")
        .ok_or_else(|| "this verb needs a 'level' between 0 and 100".to_owned())?;
    let level = value
        .as_i64()
        .filter(|level| (0..=i64::from(MAX_LEVEL)).contains(level))
        .ok_or_else(|| format!("'level' must be a whole number from 0 to {MAX_LEVEL}"))?;

    u8::try_from(level).map_err(|_| "'level' is out of range".to_owned())
}

fn step(options: &Payload) -> Result<i16, String> {
    let value = options
        .get("by")
        .ok_or_else(|| "this verb needs a 'by' step in percent".to_owned())?;
    let step = value
        .as_i64()
        .filter(|step| (-i64::from(MAX_STEP)..=i64::from(MAX_STEP)).contains(step))
        .ok_or_else(|| format!("'by' must be a whole number from -{MAX_STEP} to {MAX_STEP}"))?;
    if step == 0 {
        return Err("'by' must move the level".to_owned());
    }

    i16::try_from(step).map_err(|_| "'by' is out of range".to_owned())
}

/// Whether a two-state verb was given a state, defaulting to a toggle only for
/// the verb that is spelled that way.
fn switch(verb_suffix: &str) -> Option<Switch> {
    match verb_suffix {
        "on" => Some(Switch::On),
        "off" => Some(Switch::Off),
        "toggle" => Some(Switch::Toggle),
        _ => None,
    }
}

/// A verb of the form `<feature>-<on|off|toggle>`, if it is one for `feature`.
fn two_state(verb: &str, feature: &str) -> Option<Switch> {
    verb.strip_prefix(feature)
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(switch)
}

/// Reads one session verb and its options into a typed request.
///
/// # Errors
///
/// Returns the sentence the requester should be shown: an unknown verb, or a
/// known verb whose options cannot mean a level or a step.
pub fn parse_request(verb: &str, options: &Payload) -> Result<SessionRequest, String> {
    // Checked before the output's own verb, because `mic-mute-` is not a
    // suffix of `mute-` and must not fall through to it.
    if let Some(state) = two_state(verb, "mic-mute") {
        return Ok(SessionRequest::Mute(MuteDevice::Input, state));
    }
    if let Some(state) = two_state(verb, "mute") {
        return Ok(SessionRequest::Mute(MuteDevice::Output, state));
    }
    if let Some(state) = two_state(verb, "night-light") {
        return Ok(SessionRequest::NightLight(state));
    }
    if let Some(state) = two_state(verb, "caffeine") {
        return Ok(SessionRequest::Caffeine(state));
    }

    match verb {
        "volume-set" => Ok(SessionRequest::Volume(LevelChange::Set(level(options)?))),
        "volume-step" => Ok(SessionRequest::Volume(LevelChange::Step(step(options)?))),
        "brightness-set" => Ok(SessionRequest::Brightness(LevelChange::Set(level(
            options,
        )?))),
        "brightness-step" => Ok(SessionRequest::Brightness(LevelChange::Step(step(
            options,
        )?))),
        "displays-off" => Ok(SessionRequest::DisplaysOff),
        "lock" => Ok(SessionRequest::Lock),
        "lock-and-suspend" => Ok(SessionRequest::LockAndSuspend),
        _ => Err(format!("this shell does not serve the verb '{verb}'")),
    }
}

/// The refusal a provider owes a verb it does not serve, whether the shell has
/// no such verb at all or another provider carries it.
#[must_use]
pub fn unserved_verb(provider: &str, verb: &str) -> String {
    format!("'{provider}' does not serve the verb '{verb}'")
}

/// Reads a verb on behalf of one named provider.
///
/// Same as [`parse_request`], except that an unknown verb is refused in the
/// provider's own words. A known verb whose options cannot mean a level keeps
/// the sentence that says which option, because that is what the requester has
/// to fix.
///
/// # Errors
///
/// Returns the sentence the requester should be shown.
pub fn parse_for(provider: &str, verb: &str, options: &Payload) -> Result<SessionRequest, String> {
    parse_request(verb, options).map_err(|reason| {
        if reason.contains(verb) {
            unserved_verb(provider, verb)
        } else {
            reason
        }
    })
}

/// The refusal a request owes when the capability it names has no provider.
///
/// Locking is fail-closed by contract: a shell that cannot lock says so loudly
/// rather than reporting success and leaving the session open. The same
/// sentence serves every capability, because "nothing is carrying this" is the
/// same fact in each case.
#[must_use]
pub fn no_provider(request: SessionRequest) -> String {
    let capability = match request {
        SessionRequest::Volume(_) | SessionRequest::Mute(MuteDevice::Output, _) => {
            "the session's audio device"
        }
        SessionRequest::Mute(MuteDevice::Input, _) => "the session's microphone",
        SessionRequest::Brightness(_) => "monitor brightness",
        SessionRequest::NightLight(_) => "night light",
        SessionRequest::Caffeine(_) => "the idle inhibitor",
        SessionRequest::DisplaysOff => "the compositor's display power control",
        SessionRequest::Lock | SessionRequest::LockAndSuspend => "a session locker",
    };
    format!("this shell has no provider for {capability}")
}

/// Builds the option map a caller sends with a level verb, so hosts and tests
/// name the key in one place.
#[must_use]
pub fn level_option(value: u8) -> Payload {
    let mut options = Payload::new();
    options.insert("level".to_owned(), Value::from(value));
    options
}

/// Builds the option map a caller sends with a step verb.
#[must_use]
pub fn step_option(value: i16) -> Payload {
    let mut options = Payload::new();
    options.insert("by".to_owned(), Value::from(value));
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_verb_carries_a_whole_percent() {
        assert_eq!(
            parse_request("volume-set", &level_option(40)),
            Ok(SessionRequest::Volume(LevelChange::Set(40)))
        );
        assert_eq!(
            parse_request("brightness-set", &level_option(0)),
            Ok(SessionRequest::Brightness(LevelChange::Set(0)))
        );
    }

    #[test]
    fn a_step_verb_carries_a_signed_step() {
        assert_eq!(
            parse_request("volume-step", &step_option(-5)),
            Ok(SessionRequest::Volume(LevelChange::Step(-5)))
        );
        assert_eq!(
            parse_request("brightness-step", &step_option(5)),
            Ok(SessionRequest::Brightness(LevelChange::Step(5)))
        );
    }

    #[test]
    fn a_level_that_cannot_mean_a_percent_is_refused_by_name() {
        let missing = parse_request("volume-set", &Payload::new()).expect_err("no level");
        assert!(missing.contains("level"));

        let mut wrong = Payload::new();
        wrong.insert("level".to_owned(), Value::from("loud"));
        assert!(parse_request("volume-set", &wrong)
            .expect_err("not a number")
            .contains("level"));

        let mut over = Payload::new();
        over.insert("level".to_owned(), Value::from(101));
        assert!(parse_request("volume-set", &over).is_err());

        let mut under = Payload::new();
        under.insert("level".to_owned(), Value::from(-1));
        assert!(parse_request("volume-set", &under).is_err());

        let mut fractional = Payload::new();
        fractional.insert("level".to_owned(), Value::from(40.5));
        assert!(parse_request("volume-set", &fractional).is_err());
    }

    #[test]
    fn a_step_that_moves_nothing_is_not_a_step() {
        assert!(parse_request("volume-step", &step_option(0)).is_err());
        assert!(parse_request("volume-step", &Payload::new()).is_err());
        assert!(parse_request("brightness-step", &step_option(MAX_STEP + 1)).is_err());
        assert!(parse_request("brightness-step", &step_option(-MAX_STEP - 1)).is_err());
    }

    #[test]
    fn a_two_state_verb_says_which_state_it_means() {
        assert_eq!(
            parse_request("mute-toggle", &Payload::new()),
            Ok(SessionRequest::Mute(MuteDevice::Output, Switch::Toggle))
        );
        // The microphone is its own verb and never reached by falling through
        // the speaker's.
        assert_eq!(
            parse_request("mic-mute-toggle", &Payload::new()),
            Ok(SessionRequest::Mute(MuteDevice::Input, Switch::Toggle))
        );
        assert_eq!(
            parse_request("mic-mute-on", &Payload::new()),
            Ok(SessionRequest::Mute(MuteDevice::Input, Switch::On))
        );
        assert_eq!(
            parse_request("night-light-on", &Payload::new()),
            Ok(SessionRequest::NightLight(Switch::On))
        );
        assert_eq!(
            parse_request("caffeine-off", &Payload::new()),
            Ok(SessionRequest::Caffeine(Switch::Off))
        );
        // A state is never inferred from a bare feature name.
        assert!(parse_request("caffeine", &Payload::new()).is_err());
        assert!(parse_request("night-light-maybe", &Payload::new()).is_err());
    }

    #[test]
    fn the_verbs_with_no_options_are_the_ones_that_take_none() {
        assert_eq!(
            parse_request("displays-off", &Payload::new()),
            Ok(SessionRequest::DisplaysOff)
        );
        assert_eq!(
            parse_request("lock", &Payload::new()),
            Ok(SessionRequest::Lock)
        );
        assert_eq!(
            parse_request("lock-and-suspend", &Payload::new()),
            Ok(SessionRequest::LockAndSuspend)
        );
    }

    #[test]
    fn an_unknown_verb_is_refused_with_its_own_name() {
        let refusal = parse_request("reboot", &Payload::new()).expect_err("unknown");
        assert!(refusal.contains("reboot"));
    }

    #[test]
    fn a_step_stops_at_both_edges() {
        assert_eq!(LevelChange::Step(5).applied_to(97), 100);
        assert_eq!(LevelChange::Step(-5).applied_to(2), 0);
        assert_eq!(LevelChange::Step(10).applied_to(40), 50);
    }

    #[test]
    fn a_device_already_above_the_ceiling_is_stepped_from_where_it_is() {
        // An overdriven session reports 150; stepping down must not first snap
        // it to 100, and stepping up must not raise it further.
        assert_eq!(LevelChange::Step(-10).applied_to(150), 140);
        assert_eq!(LevelChange::Step(10).applied_to(150), 150);
    }

    #[test]
    fn a_set_never_asks_for_more_than_the_nominal_maximum() {
        assert_eq!(LevelChange::Set(100).applied_to(150), 100);
        assert_eq!(LevelChange::Set(0).applied_to(60), 0);
    }

    #[test]
    fn a_provider_refuses_an_unknown_verb_in_its_own_name() {
        let refusal = parse_for("audio", "reboot", &Payload::new()).expect_err("unknown");
        assert!(refusal.contains("audio"));
        assert!(refusal.contains("reboot"));

        // A verb it could serve, with an option it cannot use, keeps the
        // sentence that names the option.
        let refusal = parse_for("audio", "volume-set", &Payload::new()).expect_err("no level");
        assert!(refusal.contains("level"));
        assert!(!refusal.contains("does not serve"));

        assert_eq!(
            parse_for("audio", "volume-set", &level_option(40)),
            Ok(SessionRequest::Volume(LevelChange::Set(40)))
        );
    }

    #[test]
    fn a_burst_becomes_the_one_change_it_adds_up_to() {
        // Ten notches on a wheel are one write, not ten.
        assert_eq!(
            LevelChange::Step(5).followed_by(LevelChange::Step(5)),
            LevelChange::Step(10)
        );
        // A set replaces whatever was still owed, in either order.
        assert_eq!(
            LevelChange::Step(-20).followed_by(LevelChange::Set(30)),
            LevelChange::Set(30)
        );
        assert_eq!(
            LevelChange::Set(30).followed_by(LevelChange::Step(5)),
            LevelChange::Set(35)
        );
        assert_eq!(
            LevelChange::Set(2).followed_by(LevelChange::Step(-10)),
            LevelChange::Set(0)
        );
        assert_eq!(
            LevelChange::Step(MAX_STEP).followed_by(LevelChange::Step(i16::MAX)),
            LevelChange::Step(i16::MAX)
        );
    }

    #[test]
    fn a_missing_provider_is_refused_by_the_capability_it_names() {
        assert!(no_provider(SessionRequest::Lock).contains("locker"));
        assert!(
            no_provider(SessionRequest::Mute(MuteDevice::Input, Switch::On)).contains("microphone")
        );
        assert!(no_provider(SessionRequest::NightLight(Switch::On)).contains("night light"));
        assert!(no_provider(SessionRequest::DisplaysOff).contains("display power"));
    }
}
