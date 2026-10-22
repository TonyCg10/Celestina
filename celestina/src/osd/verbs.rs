//! The level verbs of `org.celestina.Shell1` (CEL-2-C, plan decisions 7 and
//! 9): `volume up|down|set <0-100>|mute`, `microphone mute|set <0-100>` and
//! `brightness up|down|set <0-100> [output]`.
//!
//! This module holds the words, the plan a verb makes and how it ended; it
//! has no bus code. The bus answers `(true, "")` as soon as a verb is queued,
//! so a verb cannot return text to the caller: how it ended is an
//! [`Outcome`], logged by the loop at `info`, and at `warn` when the verb did
//! nothing.
//!
//! One switch, `duties.osd`, owns every audio and DDC child, so with the duty
//! off every one of these verbs is *rejected*, not run. A brightness verb is
//! a change only: the DDC worker detects lazily for the first one and keeps
//! what it found (never a detection per press). The step is 5 and the
//! ceiling 100, both the audio worker's and the brightness workers' own (the
//! keys' `-l 1.0`).

use celestina_shell::audio::Direction;
use celestina_shell::settings::plain_name;

use super::readings::{Readings, Target};
use super::OsdRuntime;
use crate::command::Verb;
use crate::providers::audio::{AudioCommand, Device};
use crate::providers::brightness::{BrightnessCommand, Change};

/// The highest level a verb sets.
pub const MAX_PERCENT: u8 = 100;

/// `volume ...`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeVerb {
    Up,
    Down,
    Set(u8),
    Mute,
}

/// `microphone ...`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicrophoneVerb {
    Mute,
    Set(u8),
}

/// `brightness <change> [output]`'s change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrightnessVerb {
    Up,
    Down,
    Set(u8),
}

/// A percentage word: one to three ASCII digits, at most [`MAX_PERCENT`]. No
/// sign, no space, no leading plus.
fn percent_word(word: &str) -> Option<u8> {
    if word.is_empty() || word.len() > 3 || !word.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    word.parse::<u8>().ok().filter(|p| *p <= MAX_PERCENT)
}

/// The words after `volume`.
#[must_use]
pub fn parse_volume(rest: &[&str]) -> Option<VolumeVerb> {
    match rest {
        ["up"] => Some(VolumeVerb::Up),
        ["down"] => Some(VolumeVerb::Down),
        ["mute"] => Some(VolumeVerb::Mute),
        ["set", percent] => percent_word(percent).map(VolumeVerb::Set),
        _ => None,
    }
}

/// The words after `microphone`.
#[must_use]
pub fn parse_microphone(rest: &[&str]) -> Option<MicrophoneVerb> {
    match rest {
        ["mute"] => Some(MicrophoneVerb::Mute),
        ["set", percent] => percent_word(percent).map(MicrophoneVerb::Set),
        _ => None,
    }
}

/// The words after `brightness`: the change and the optional output.
#[must_use]
pub fn parse_brightness(rest: &[&str]) -> Option<(BrightnessVerb, Option<String>)> {
    let output = |word: &str| plain_name(word).then(|| word.to_owned());
    match rest {
        ["up"] => Some((BrightnessVerb::Up, None)),
        ["up", out] => Some((BrightnessVerb::Up, Some(output(out)?))),
        ["down"] => Some((BrightnessVerb::Down, None)),
        ["down", out] => Some((BrightnessVerb::Down, Some(output(out)?))),
        ["set", percent] => Some((BrightnessVerb::Set(percent_word(percent)?), None)),
        ["set", percent, out] => Some((
            BrightnessVerb::Set(percent_word(percent)?),
            Some(output(out)?),
        )),
        _ => None,
    }
}

/// The words that spell a volume verb.
#[must_use]
pub fn volume_words(verb: VolumeVerb) -> Vec<String> {
    let words: Vec<String> = match verb {
        VolumeVerb::Up => vec!["volume".into(), "up".into()],
        VolumeVerb::Down => vec!["volume".into(), "down".into()],
        VolumeVerb::Mute => vec!["volume".into(), "mute".into()],
        VolumeVerb::Set(p) => vec!["volume".into(), "set".into(), p.to_string()],
    };
    words
}

/// The words that spell a microphone verb.
#[must_use]
pub fn microphone_words(verb: MicrophoneVerb) -> Vec<String> {
    match verb {
        MicrophoneVerb::Mute => vec!["microphone".into(), "mute".into()],
        MicrophoneVerb::Set(p) => vec!["microphone".into(), "set".into(), p.to_string()],
    }
}

/// The words that spell a brightness verb.
#[must_use]
pub fn brightness_words(verb: BrightnessVerb, output: Option<&str>) -> Vec<String> {
    let mut words: Vec<String> = match verb {
        BrightnessVerb::Up => vec!["brightness".into(), "up".into()],
        BrightnessVerb::Down => vec!["brightness".into(), "down".into()],
        BrightnessVerb::Set(p) => vec!["brightness".into(), "set".into(), p.to_string()],
    };
    words.extend(output.map(str::to_owned));
    words
}

/// The audio command a volume or microphone verb is, if `verb` is one.
#[must_use]
pub fn audio_command(verb: &Verb) -> Option<AudioCommand> {
    match verb {
        Verb::Volume(VolumeVerb::Up) => Some(AudioCommand::Step {
            device: Device::Output,
            direction: Direction::Up,
        }),
        Verb::Volume(VolumeVerb::Down) => Some(AudioCommand::Step {
            device: Device::Output,
            direction: Direction::Down,
        }),
        Verb::Volume(VolumeVerb::Set(percent)) => Some(AudioCommand::SetVolume {
            device: Device::Output,
            percent: *percent,
        }),
        Verb::Volume(VolumeVerb::Mute) => Some(AudioCommand::ToggleMute {
            device: Device::Output,
        }),
        Verb::Microphone(MicrophoneVerb::Mute) => Some(AudioCommand::ToggleMute {
            device: Device::Input,
        }),
        Verb::Microphone(MicrophoneVerb::Set(percent)) => Some(AudioCommand::SetVolume {
            device: Device::Input,
            percent: *percent,
        }),
        _ => None,
    }
}

/// The change a brightness verb asks of a monitor.
#[must_use]
pub fn change_of(verb: BrightnessVerb) -> Change {
    match verb {
        BrightnessVerb::Up => Change::Step(Direction::Up),
        BrightnessVerb::Down => Change::Step(Direction::Down),
        BrightnessVerb::Set(percent) => Change::Set(percent),
    }
}

/// Why a verb did nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// `duties.osd` is off: one switch owns every audio and DDC child.
    DutyOff,
    /// The duty is on but the worker the verb needs did not start.
    NoWorker,
    /// A brightness verb with no output named and none focused.
    NoOutput,
    /// A brightness verb for an output this shell does not know.
    UnknownOutput(String),
    /// The worker's queue was full or the worker had stopped (logged there).
    Refused,
}

/// How a level verb ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The command was queued for the worker.
    Queued(String),
    /// Nothing was done, and why.
    Rejected(String, Rejection),
}

impl Outcome {
    /// Writes the outcome to the journal.
    pub fn log(&self) {
        match self {
            Self::Queued(verb) => tracing::info!("{verb}: queued"),
            Self::Rejected(verb, Rejection::DutyOff) => tracing::warn!(
                "{verb}: rejected: the osd duty is off (set duties.osd to \"on\" in shell.json)"
            ),
            Self::Rejected(verb, Rejection::NoWorker) => {
                tracing::warn!("{verb}: rejected: the duty's worker is not running");
            }
            Self::Rejected(verb, Rejection::NoOutput) => {
                tracing::warn!("{verb}: rejected: no output was named and none is focused");
            }
            Self::Rejected(verb, Rejection::UnknownOutput(output)) => {
                tracing::warn!("{verb}: rejected: no output named {output:?}");
            }
            Self::Rejected(verb, Rejection::Refused) => {
                tracing::warn!("{verb}: rejected: the worker did not take the command");
            }
        }
    }
}

/// The monitor and change a brightness verb means: `named` output, else the
/// `focused` one.
///
/// # Errors
///
/// [`Rejection::NoOutput`] with neither, [`Rejection::UnknownOutput`] for an
/// output the shell does not have.
pub fn brightness_plan(
    verb: BrightnessVerb,
    named: Option<&str>,
    focused: Option<&str>,
    readings: &Readings,
) -> Result<(Target, Change), Rejection> {
    let output = named.or(focused).ok_or(Rejection::NoOutput)?;
    let target = readings
        .target(output)
        .ok_or_else(|| Rejection::UnknownOutput(output.to_owned()))?;
    Ok((target, change_of(verb)))
}

/// What the verbs need of the duty's workers; [`OsdRuntime`] is the real
/// one, and tests give a recording fake.
pub trait Workers {
    /// Whether the duty's workers exist at all (the duty is on).
    fn running(&self) -> bool;
    /// Queues an audio command: `None` without an audio worker, else whether
    /// the worker took it.
    fn audio(&self, command: AudioCommand) -> Option<bool>;
    /// Queues a brightness command: `None` without brightness workers, else
    /// whether a worker took it.
    fn brightness(&self, command: BrightnessCommand) -> Option<bool>;
}

impl Workers for OsdRuntime {
    fn running(&self) -> bool {
        OsdRuntime::running(self)
    }

    fn audio(&self, command: AudioCommand) -> Option<bool> {
        OsdRuntime::audio(self).map(|worker| worker.request(command).is_ok())
    }

    fn brightness(&self, command: BrightnessCommand) -> Option<bool> {
        OsdRuntime::brightness(self).map(|worker| worker.request(command).is_ok())
    }
}

/// The workers of a duty that never started: nothing runs, so every level
/// verb is rejected as the duty being off.
#[derive(Clone, Copy, Debug)]
pub struct NoWorkers;

impl Workers for NoWorkers {
    fn running(&self) -> bool {
        false
    }

    fn audio(&self, _: AudioCommand) -> Option<bool> {
        None
    }

    fn brightness(&self, _: BrightnessCommand) -> Option<bool> {
        None
    }
}

/// Runs a level verb against the duty's workers; `None` for any other verb.
/// `focused` is the output niri reports focused, the default of a brightness
/// verb.
pub fn run(
    verb: &Verb,
    workers: &dyn Workers,
    readings: &Readings,
    focused: Option<&str>,
) -> Option<Outcome> {
    let spelled = verb.words().join(" ");
    let done = |ok: Option<bool>| {
        Some(match ok {
            Some(true) => Outcome::Queued(spelled.clone()),
            Some(false) => Outcome::Rejected(spelled.clone(), Rejection::Refused),
            None => Outcome::Rejected(spelled.clone(), Rejection::NoWorker),
        })
    };
    let audio = audio_command(verb);
    if audio.is_none() && !matches!(verb, Verb::Brightness { .. }) {
        return None;
    }
    if !workers.running() {
        return Some(Outcome::Rejected(spelled, Rejection::DutyOff));
    }
    if let Some(command) = audio {
        return done(workers.audio(command));
    }
    let Verb::Brightness { change, output } = verb else {
        return None;
    };
    let (target, change) = match brightness_plan(*change, output.as_deref(), focused, readings) {
        Ok(plan) => plan,
        Err(why) => return Some(Outcome::Rejected(spelled, why)),
    };
    // A change only: the DDC worker detects lazily for the first one and
    // keeps what it found, so a key held down never asks for a detection.
    done(workers.brightness(BrightnessCommand::Change {
        source: target.source,
        monitor: target.monitor,
        change,
    }))
}

#[cfg(test)]
mod tests {
    use celestina_wayland::OutputId;

    use super::*;
    use crate::providers::brightness::BrightnessSource;

    fn words(line: &str) -> Vec<&str> {
        line.split(' ').collect()
    }

    #[test]
    fn volume_words_parse_and_round_trip() {
        let table = [
            ("up", VolumeVerb::Up),
            ("down", VolumeVerb::Down),
            ("mute", VolumeVerb::Mute),
            ("set 0", VolumeVerb::Set(0)),
            ("set 55", VolumeVerb::Set(55)),
            ("set 100", VolumeVerb::Set(100)),
        ];
        for (rest, verb) in table {
            assert_eq!(parse_volume(&words(rest)), Some(verb), "{rest}");
            let spelled = volume_words(verb);
            assert_eq!(spelled[0], "volume");
            let tail: Vec<&str> = spelled[1..].iter().map(String::as_str).collect();
            assert_eq!(parse_volume(&tail), Some(verb));
        }
    }

    #[test]
    fn a_percentage_is_plain_digits_up_to_the_ceiling() {
        for bad in [
            "set 101", "set 1000", "set -5", "set +5", "set 5.0", "set ", "set 050x", "set abc",
            "set 256", "set ٣",
        ] {
            assert_eq!(parse_volume(&words(bad)), None, "{bad:?}");
        }
        // A leading zero is still plain digits.
        assert_eq!(parse_volume(&words("set 050")), Some(VolumeVerb::Set(50)));
        for bad in ["", "louder", "up now", "mute 1", "set", "set 1 2"] {
            assert_eq!(parse_volume(&words(bad)), None, "{bad:?}");
        }
        assert_eq!(parse_volume(&[]), None);
    }

    #[test]
    fn microphone_has_no_up_or_down() {
        assert_eq!(parse_microphone(&["mute"]), Some(MicrophoneVerb::Mute));
        assert_eq!(
            parse_microphone(&["set", "30"]),
            Some(MicrophoneVerb::Set(30))
        );
        for bad in [&["up"][..], &["down"], &["set"], &["set", "101"], &[]] {
            assert_eq!(parse_microphone(bad), None, "{bad:?}");
        }
        assert_eq!(
            microphone_words(MicrophoneVerb::Set(7)),
            ["microphone", "set", "7"]
        );
    }

    #[test]
    fn brightness_takes_an_optional_plain_output_last() {
        assert_eq!(parse_brightness(&["up"]), Some((BrightnessVerb::Up, None)));
        assert_eq!(
            parse_brightness(&["down", "DP-1"]),
            Some((BrightnessVerb::Down, Some("DP-1".to_owned())))
        );
        assert_eq!(
            parse_brightness(&["set", "40", "HDMI-A-1"]),
            Some((BrightnessVerb::Set(40), Some("HDMI-A-1".to_owned())))
        );
        for bad in [
            &[][..],
            &["set"],
            &["set", "101"],
            &["set", "x", "DP-1"],
            &["up", ""],
            &["up", "a/b"],
            &["up", ".hidden"],
            &["up", "DP-1", "extra"],
            &["mute"],
        ] {
            assert_eq!(parse_brightness(bad), None, "{bad:?}");
        }
        for (verb, output) in [
            (BrightnessVerb::Up, None),
            (BrightnessVerb::Set(9), Some("DP-1")),
        ] {
            let spelled = brightness_words(verb, output);
            let tail: Vec<&str> = spelled[1..].iter().map(String::as_str).collect();
            assert_eq!(
                parse_brightness(&tail),
                Some((verb, output.map(str::to_owned)))
            );
        }
    }

    #[test]
    fn verbs_become_the_workers_commands_with_the_keys_step_and_ceiling() {
        use AudioCommand::{SetVolume, Step, ToggleMute};
        let cases = [
            (
                Verb::Volume(VolumeVerb::Up),
                Step {
                    device: Device::Output,
                    direction: Direction::Up,
                },
            ),
            (
                Verb::Volume(VolumeVerb::Down),
                Step {
                    device: Device::Output,
                    direction: Direction::Down,
                },
            ),
            (
                Verb::Volume(VolumeVerb::Set(100)),
                SetVolume {
                    device: Device::Output,
                    percent: 100,
                },
            ),
            (
                Verb::Volume(VolumeVerb::Mute),
                ToggleMute {
                    device: Device::Output,
                },
            ),
            (
                Verb::Microphone(MicrophoneVerb::Mute),
                ToggleMute {
                    device: Device::Input,
                },
            ),
            (
                Verb::Microphone(MicrophoneVerb::Set(20)),
                SetVolume {
                    device: Device::Input,
                    percent: 20,
                },
            ),
        ];
        for (verb, command) in cases {
            assert_eq!(audio_command(&verb), Some(command), "{verb:?}");
        }
        assert_eq!(audio_command(&Verb::Quit), None);
        assert_eq!(change_of(BrightnessVerb::Up), Change::Step(Direction::Up));
        assert_eq!(change_of(BrightnessVerb::Set(30)), Change::Set(30));
    }

    fn readings(outputs: &[&str]) -> Readings {
        let mut r = Readings::new();
        for (i, name) in outputs.iter().enumerate() {
            r.output_added(OutputId(i as u32 + 1), name);
        }
        r
    }

    #[test]
    fn a_brightness_verb_means_the_named_output_else_the_focused_one() {
        let r = readings(&["DP-1", "HDMI-A-1"]);
        let ddc = |monitor: &str| Target {
            source: BrightnessSource::Ddc,
            monitor: monitor.to_owned(),
        };
        assert_eq!(
            brightness_plan(BrightnessVerb::Up, None, Some("DP-1"), &r),
            Ok((ddc("DP-1"), Change::Step(Direction::Up)))
        );
        assert_eq!(
            brightness_plan(BrightnessVerb::Set(10), Some("HDMI-A-1"), Some("DP-1"), &r),
            Ok((ddc("HDMI-A-1"), Change::Set(10)))
        );
    }

    #[test]
    fn an_unknown_or_missing_output_is_a_rejection() {
        let r = readings(&["DP-1"]);
        assert_eq!(
            brightness_plan(BrightnessVerb::Up, Some("DP-9"), Some("DP-1"), &r),
            Err(Rejection::UnknownOutput("DP-9".to_owned()))
        );
        assert_eq!(
            brightness_plan(BrightnessVerb::Up, None, None, &r),
            Err(Rejection::NoOutput)
        );
        // A focused output the shell has not been told of is unknown too.
        assert_eq!(
            brightness_plan(BrightnessVerb::Up, None, Some("DP-2"), &r),
            Err(Rejection::UnknownOutput("DP-2".to_owned()))
        );
    }

    /// Records what reached the workers.
    #[derive(Default)]
    struct Fake {
        on: bool,
        audio: bool,
        brightness: bool,
        refuse: bool,
        audio_sent: std::cell::RefCell<Vec<AudioCommand>>,
        brightness_sent: std::cell::RefCell<Vec<BrightnessCommand>>,
    }

    impl Workers for Fake {
        fn running(&self) -> bool {
            self.on
        }

        fn audio(&self, command: AudioCommand) -> Option<bool> {
            self.audio.then(|| {
                self.audio_sent.borrow_mut().push(command);
                !self.refuse
            })
        }

        fn brightness(&self, command: BrightnessCommand) -> Option<bool> {
            self.brightness.then(|| {
                self.brightness_sent.borrow_mut().push(command);
                !self.refuse
            })
        }
    }

    fn all_verbs() -> [Verb; 3] {
        [
            Verb::Volume(VolumeVerb::Up),
            Verb::Microphone(MicrophoneVerb::Mute),
            Verb::Brightness {
                change: BrightnessVerb::Up,
                output: None,
            },
        ]
    }

    #[test]
    fn with_the_duty_off_every_level_verb_is_rejected_and_nothing_reaches_a_worker() {
        let r = readings(&["DP-1"]);
        let off = Fake {
            audio: true,
            brightness: true,
            ..Fake::default()
        };
        for verb in &all_verbs() {
            let outcome = run(verb, &off, &r, Some("DP-1"));
            assert!(
                matches!(outcome, Some(Outcome::Rejected(_, Rejection::DutyOff))),
                "{verb:?}: {outcome:?}"
            );
        }
        assert!(off.audio_sent.borrow().is_empty() && off.brightness_sent.borrow().is_empty());
        assert_eq!(run(&Verb::Quit, &off, &r, None), None);
        assert_eq!(run(&Verb::LauncherToggle, &off, &r, None), None);
    }

    #[test]
    fn with_the_duty_on_the_verbs_reach_their_workers() {
        let r = readings(&["DP-1"]);
        let on = Fake {
            on: true,
            audio: true,
            brightness: true,
            ..Fake::default()
        };
        for verb in &all_verbs() {
            assert!(
                matches!(run(verb, &on, &r, Some("DP-1")), Some(Outcome::Queued(_))),
                "{verb:?}"
            );
        }
        assert_eq!(
            *on.audio_sent.borrow(),
            [
                AudioCommand::Step {
                    device: Device::Output,
                    direction: Direction::Up
                },
                AudioCommand::ToggleMute {
                    device: Device::Input
                }
            ]
        );
        // The brightness verb is a change only: the worker detects lazily
        // for the first one and keeps what it found, so a key held down is
        // never a detection per press.
        assert_eq!(
            *on.brightness_sent.borrow(),
            [BrightnessCommand::Change {
                source: BrightnessSource::Ddc,
                monitor: "DP-1".to_owned(),
                change: Change::Step(Direction::Up)
            }]
        );
    }

    #[test]
    fn a_backlight_change_is_not_a_ddc_consumer() {
        let mut r = readings(&["eDP-1"]);
        r.brightness_event(&crate::providers::brightness::BrightnessEvent {
            monitor: "intel_backlight".to_owned(),
            percent: Some(50),
            source: BrightnessSource::Backlight,
        });
        let on = Fake {
            on: true,
            brightness: true,
            ..Fake::default()
        };
        let verb = Verb::Brightness {
            change: BrightnessVerb::Set(20),
            output: None,
        };
        assert!(matches!(
            run(&verb, &on, &r, Some("eDP-1")),
            Some(Outcome::Queued(_))
        ));
        assert_eq!(
            *on.brightness_sent.borrow(),
            [BrightnessCommand::Change {
                source: BrightnessSource::Backlight,
                monitor: "intel_backlight".to_owned(),
                change: Change::Set(20)
            }]
        );
    }

    #[test]
    fn a_missing_worker_or_a_full_queue_or_a_bad_output_is_a_rejection() {
        let r = readings(&["DP-1"]);
        let no_worker = Fake {
            on: true,
            ..Fake::default()
        };
        for verb in &all_verbs() {
            assert!(
                matches!(
                    run(verb, &no_worker, &r, Some("DP-1")),
                    Some(Outcome::Rejected(_, Rejection::NoWorker))
                ),
                "{verb:?}"
            );
        }
        let full = Fake {
            on: true,
            audio: true,
            brightness: true,
            refuse: true,
            ..Fake::default()
        };
        for verb in &all_verbs() {
            assert!(
                matches!(
                    run(verb, &full, &r, Some("DP-1")),
                    Some(Outcome::Rejected(_, Rejection::Refused))
                ),
                "{verb:?}"
            );
        }
        let on = Fake {
            on: true,
            brightness: true,
            ..Fake::default()
        };
        let unknown = Verb::Brightness {
            change: BrightnessVerb::Up,
            output: Some("DP-9".to_owned()),
        };
        assert_eq!(
            run(&unknown, &on, &r, Some("DP-1")),
            Some(Outcome::Rejected(
                "brightness up DP-9".to_owned(),
                Rejection::UnknownOutput("DP-9".to_owned())
            ))
        );
        // Rejected before any worker heard of it.
        assert!(on.brightness_sent.borrow().is_empty());
    }
}
