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
    /// The level this change names by itself, when it names one.
    ///
    /// A `Set` carries its whole answer; only a `Step` has to know where the
    /// device is. A caller that must run a process to find that out asks this
    /// first, so an absolute request costs no reading at all — which is one
    /// fewer child per move of a slider that is dragged, not stepped.
    #[must_use]
    pub fn absolute(self) -> Option<u8> {
        match self {
            Self::Set(level) => Some(level.min(MAX_LEVEL)),
            Self::Step(_) => None,
        }
    }

    /// The level this change leaves, given the one the device reports now.
    ///
    /// A step lands on a round number rather than `current + step`: from 22, a
    /// step of five reaches 25 going up and 20 going down. What a person turns
    /// a wheel for is a level, not an offset, and a device that starts on 22 —
    /// because something else set it there — otherwise carries that stray 2
    /// through every step it is ever given.
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
                let current = i16::from(current);
                let size = step.saturating_abs().max(1);
                let target = if step > 0 {
                    // The next multiple strictly above where it is.
                    current
                        .div_euclid(size)
                        .saturating_add(1)
                        .saturating_mul(size)
                } else {
                    // And strictly below, which is why this counts from one
                    // less: a device already on a multiple must move off it.
                    current
                        .saturating_sub(1)
                        .div_euclid(size)
                        .saturating_mul(size)
                };
                u8::try_from(target.clamp(0, ceiling)).unwrap_or(MAX_LEVEL)
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

/// Ending the session, or the machine's day.
///
/// These are the requests a person cannot take back, which is why they are
/// typed rather than a string reaching a shell: `power-off` and `reboot` must
/// never be what a mistyped verb falls through to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAction {
    /// End this session and return to the greeter. The compositor does it;
    /// this shell asks.
    LogOut,
    Reboot,
    PowerOff,
    /// Sleep. Fail-closed like [`SessionRequest::Lock`]: a session that
    /// suspends unlocked wakes up unlocked, so this is refused while no locker
    /// provider exists.
    Suspend,
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
    /// How warm night light should be, in kelvin. Separate from the switch:
    /// choosing a temperature says nothing about whether the light is on, and
    /// a person adjusting it while it is off is setting what it will be.
    NightLightTemperature(u32),
    /// Keeping the session awake on purpose.
    Caffeine(Switch),
    /// Blanking the outputs now. There is no "on": any input wakes them, and a
    /// verb that claimed to turn them back on would be describing the
    /// compositor's behaviour rather than causing it.
    DisplaysOff,
    Lock,
    LockAndSuspend,
    Power(PowerAction),
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

fn kelvin(options: &Payload) -> Result<u32, String> {
    let minimum = crate::nightlight::Whitepoint::MINIMUM_KELVIN;
    let maximum = crate::nightlight::Whitepoint::MAXIMUM_KELVIN;
    let value = options
        .get("kelvin")
        .ok_or_else(|| format!("this verb needs a 'kelvin' between {minimum} and {maximum}"))?;
    let kelvin = value
        .as_i64()
        .filter(|kelvin| (i64::from(minimum)..=i64::from(maximum)).contains(kelvin))
        .ok_or_else(|| format!("'kelvin' must be a whole number from {minimum} to {maximum}"))?;

    u32::try_from(kelvin).map_err(|_| "'kelvin' is out of range".to_owned())
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
        "night-light-temperature" => Ok(SessionRequest::NightLightTemperature(kelvin(options)?)),
        "volume-set" => Ok(SessionRequest::Volume(LevelChange::Set(level(options)?))),
        "volume-step" => Ok(SessionRequest::Volume(LevelChange::Step(step(options)?))),
        "brightness-set" => Ok(SessionRequest::Brightness(LevelChange::Set(level(
            options,
        )?))),
        "brightness-step" => Ok(SessionRequest::Brightness(LevelChange::Step(step(
            options,
        )?))),
        "displays-off" => Ok(SessionRequest::DisplaysOff),
        "log-out" => Ok(SessionRequest::Power(PowerAction::LogOut)),
        "reboot" => Ok(SessionRequest::Power(PowerAction::Reboot)),
        "power-off" => Ok(SessionRequest::Power(PowerAction::PowerOff)),
        "suspend" => Ok(SessionRequest::Power(PowerAction::Suspend)),
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
    let request = parse_request(verb, options).map_err(|reason| {
        if reason.contains(verb) {
            unserved_verb(provider, verb)
        } else {
            reason
        }
    })?;

    // A verb this vocabulary knows is still not this provider's to serve. The
    // check lives here rather than in each provider because "who serves what"
    // is one fact, and a provider repeating it is a copy that can drift — the
    // audio provider must refuse `reboot` for the same reason it refuses
    // `defenestrate`, and neither must reach a device.
    if serves(provider, request) {
        Ok(request)
    } else {
        Err(unserved_verb(provider, verb))
    }
}

/// Whether `provider` is the one that carries `request`.
///
/// Requests with no provider of their own — ending the session, locking it,
/// blanking the outputs — belong to the host, which asks the compositor or the
/// session manager directly. No provider name serves them.
#[must_use]
pub fn serves(provider: &str, request: SessionRequest) -> bool {
    match request {
        SessionRequest::Volume(_) | SessionRequest::Mute(..) => provider == "audio",
        SessionRequest::Brightness(_) => provider == "brightness",
        SessionRequest::NightLight(_) | SessionRequest::NightLightTemperature(_) => {
            provider == "night-light"
        }
        SessionRequest::Caffeine(_) => provider == "caffeine",
        SessionRequest::DisplaysOff
        | SessionRequest::Lock
        | SessionRequest::LockAndSuspend
        | SessionRequest::Power(_) => false,
    }
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
        SessionRequest::NightLight(_) | SessionRequest::NightLightTemperature(_) => "night light",
        SessionRequest::Caffeine(_) => "the idle inhibitor",
        SessionRequest::DisplaysOff => "the compositor's display power control",
        SessionRequest::Lock
        | SessionRequest::LockAndSuspend
        | SessionRequest::Power(PowerAction::Suspend) => "a session locker",
        SessionRequest::Power(PowerAction::LogOut) => "the compositor",
        SessionRequest::Power(PowerAction::Reboot | PowerAction::PowerOff) => "the session manager",
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
    fn the_verbs_that_end_a_session_are_typed_one_by_one() {
        for (verb, action) in [
            ("log-out", PowerAction::LogOut),
            ("reboot", PowerAction::Reboot),
            ("power-off", PowerAction::PowerOff),
            ("suspend", PowerAction::Suspend),
        ] {
            assert_eq!(
                parse_request(verb, &Payload::new()),
                Ok(SessionRequest::Power(action))
            );
        }
        // Nothing falls through to one of these: a near miss is refused, not
        // rounded to the nearest irreversible action.
        assert!(parse_request("power", &Payload::new()).is_err());
        assert!(parse_request("power-off-now", &Payload::new()).is_err());
        assert!(parse_request("shutdown", &Payload::new()).is_err());
    }

    #[test]
    fn suspending_needs_the_same_locker_lock_does() {
        // A session that suspends unlocked wakes up unlocked.
        assert!(no_provider(SessionRequest::Power(PowerAction::Suspend)).contains("locker"));
        assert!(no_provider(SessionRequest::Power(PowerAction::Reboot)).contains("session manager"));
    }

    #[test]
    fn an_unknown_verb_is_refused_with_its_own_name() {
        let refusal = parse_request("defenestrate", &Payload::new()).expect_err("unknown");
        assert!(refusal.contains("defenestrate"));
    }

    #[test]
    fn a_step_stops_at_both_edges() {
        assert_eq!(LevelChange::Step(5).applied_to(97), 100);
        assert_eq!(LevelChange::Step(-5).applied_to(2), 0);
        assert_eq!(LevelChange::Step(10).applied_to(40), 50);
    }

    /// What the wheel is for: the level, not the offset it was given.
    #[test]
    fn a_step_lands_on_a_round_number() {
        assert_eq!(LevelChange::Step(5).applied_to(22), 25);
        assert_eq!(LevelChange::Step(-5).applied_to(22), 20);
        // A level already on a multiple still moves a whole step.
        assert_eq!(LevelChange::Step(5).applied_to(25), 30);
        assert_eq!(LevelChange::Step(-5).applied_to(25), 20);
        // And the stray value is spent once, not carried forever.
        assert_eq!(
            LevelChange::Step(-5).applied_to(LevelChange::Step(-5).applied_to(22)),
            15
        );
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

    /// The distinction a provider pays for in processes: a `set` can be carried
    /// out without asking the device anything, a `step` cannot.
    #[test]
    fn only_a_step_has_to_know_where_the_device_is() {
        assert_eq!(LevelChange::Set(40).absolute(), Some(40));
        assert_eq!(LevelChange::Set(150).absolute(), Some(MAX_LEVEL));
        assert_eq!(LevelChange::Step(-5).absolute(), None);

        // And when it does name a level, it is the same level `applied_to`
        // would have reached from anywhere at all.
        for current in [0, 37, MAX_LEVEL, 150] {
            assert_eq!(
                LevelChange::Set(40).absolute(),
                Some(LevelChange::Set(40).applied_to(current))
            );
        }
    }

    #[test]
    fn a_provider_refuses_an_unknown_verb_in_its_own_name() {
        let refusal = parse_for("audio", "defenestrate", &Payload::new()).expect_err("unknown");
        assert!(refusal.contains("audio"));
        assert!(refusal.contains("defenestrate"));

        // Deliberately a verb this vocabulary *does* know: the audio provider
        // must refuse it in its own name rather than letting it through.
        let foreign = parse_for("audio", "reboot", &Payload::new()).expect_err("not audio's");
        assert!(foreign.contains("audio"));
        assert!(foreign.contains("reboot"));
        let wrong_device =
            parse_for("audio", "brightness-step", &step_option(5)).expect_err("not audio's");
        assert!(wrong_device.contains("audio"));

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
