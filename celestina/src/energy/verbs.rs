//! The energy verbs of `org.celestina.Shell1`: `caffeine`, `nightlight` and
//! `session`.
//!
//! This module holds the words and what a verb does to the settings and the
//! workers; it has no bus code. The bus answers `(true, "")` as soon as a verb
//! is queued, so a verb cannot return text to the caller: how it ended is an
//! [`Outcome`], logged by the loop at `info`, and at `warn` when the verb did
//! nothing (the duty is off, the settings were not saved, the request was not
//! queued). `session` verbs finish later, in logind or the compositor, and
//! their end arrives as an [`crate::energy::EnergyEvent`].

use celestina_shell::session::{PowerAction, Switch};
use celestina_shell::settings::Duty;

use crate::command::Verb;
use crate::energy::EnergyHandle;
use crate::error_chain;
use crate::providers::settings::ShellSettings;

/// The word of a switch in a verb.
#[must_use]
pub fn switch_word(switch: Switch) -> &'static str {
    match switch {
        Switch::On => "on",
        Switch::Off => "off",
        Switch::Toggle => "toggle",
    }
}

/// The switch a word spells.
#[must_use]
pub fn switch_from_word(word: &str) -> Option<Switch> {
    [Switch::On, Switch::Off, Switch::Toggle]
        .into_iter()
        .find(|s| switch_word(*s) == word)
}

/// The word of a session action in a verb.
#[must_use]
pub fn action_word(action: PowerAction) -> &'static str {
    match action {
        PowerAction::Suspend => "suspend",
        PowerAction::Reboot => "reboot",
        PowerAction::PowerOff => "power-off",
        PowerAction::LogOut => "log-out",
    }
}

/// The session action a word spells.
#[must_use]
pub fn action_from_word(word: &str) -> Option<PowerAction> {
    [
        PowerAction::Suspend,
        PowerAction::Reboot,
        PowerAction::PowerOff,
        PowerAction::LogOut,
    ]
    .into_iter()
    .find(|a| action_word(*a) == word)
}

/// The two persisted switches a verb flips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Caffeine,
    NightLight,
}

impl Flag {
    fn name(self) -> &'static str {
        match self {
            Self::Caffeine => "caffeine",
            Self::NightLight => "night light",
        }
    }

    fn duty_key(self) -> &'static str {
        match self {
            Self::Caffeine => "duties.caffeine",
            Self::NightLight => "duties.nightlight",
        }
    }
}

/// How an energy verb ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The switch was flipped, saved and handed to the workers.
    Changed(Flag, bool),
    /// The switch already had the asked state.
    Unchanged(Flag, bool),
    /// The duty is off: the verb did nothing to the system, and the state was
    /// not saved.
    DutyOff(Flag),
    /// The file could not be written; the previous state stays in force.
    NotSaved(Flag, String),
    /// A session action was queued.
    Requested(PowerAction),
    /// A session action was not queued.
    NotRequested(PowerAction, String),
}

impl Outcome {
    /// Writes the outcome to the journal.
    pub fn log(&self) {
        match self {
            Self::Changed(flag, on) => tracing::info!("{}: now {}", flag.name(), state(*on)),
            Self::Unchanged(flag, on) => {
                tracing::info!("{}: already {}", flag.name(), state(*on));
            }
            Self::DutyOff(flag) => tracing::warn!(
                "{}: nothing was done: the duty is off (set {} to \"on\" in shell.json)",
                flag.name(),
                flag.duty_key()
            ),
            Self::NotSaved(flag, why) => {
                tracing::warn!(
                    "{}: not changed, the settings were not saved: {why}",
                    flag.name()
                );
            }
            Self::Requested(action) => {
                tracing::info!("session {}: requested", action_word(*action));
            }
            Self::NotRequested(action, why) => {
                tracing::warn!("session {}: not requested: {why}", action_word(*action));
            }
        }
    }
}

fn state(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

/// Runs an energy verb; `None` for any other verb.
pub fn run(verb: Verb, settings: &mut ShellSettings, energy: &EnergyHandle) -> Option<Outcome> {
    match verb {
        Verb::Caffeine(switch) => Some(flip(Flag::Caffeine, switch, settings, energy)),
        Verb::NightLight(switch) => Some(flip(Flag::NightLight, switch, settings, energy)),
        Verb::Session(action) => Some(match energy.power(action) {
            Ok(()) => Outcome::Requested(action),
            Err(why) => Outcome::NotRequested(action, why),
        }),
        _ => None,
    }
}

/// Flips a persisted switch, durably before the workers hear of it, and only
/// while its duty is on.
fn flip(
    flag: Flag,
    switch: Switch,
    settings: &mut ShellSettings,
    energy: &EnergyHandle,
) -> Outcome {
    let now = settings.current();
    let (duty, current) = match flag {
        Flag::Caffeine => (now.duties.caffeine, now.energy.caffeine),
        Flag::NightLight => (now.duties.nightlight, now.energy.night_light),
    };
    if duty == Duty::Off {
        return Outcome::DutyOff(flag);
    }
    let wanted = switch.resolve(current);
    if wanted == current {
        return Outcome::Unchanged(flag, current);
    }
    let saved = settings.set_energy(|e| match flag {
        Flag::Caffeine => e.caffeine = wanted,
        Flag::NightLight => e.night_light = wanted,
    });
    match saved {
        Ok(_) => {
            energy.apply(settings.current());
            Outcome::Changed(flag, wanted)
        }
        Err(error) => Outcome::NotSaved(flag, error_chain(&error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::tests::{said, Fake};
    use celestina_shell::settings::{self, Settings};
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("celestina-verbs-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a test directory");
            Self(dir)
        }

        /// A settings file holding `json`.
        fn shell(&self, json: &str) -> ShellSettings {
            let file = settings::path_in(&self.0);
            std::fs::create_dir_all(file.parent().expect("parent")).expect("dir");
            std::fs::write(&file, json).expect("file");
            ShellSettings::load_from(Some(file))
        }

        fn reread(&self) -> Settings {
            let bytes = std::fs::read(settings::path_in(&self.0)).expect("the file");
            Settings::from_bytes(&bytes).expect("a settings file")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn words_round_trip() {
        for s in [Switch::On, Switch::Off, Switch::Toggle] {
            assert_eq!(switch_from_word(switch_word(s)), Some(s));
        }
        for a in [
            PowerAction::Suspend,
            PowerAction::Reboot,
            PowerAction::PowerOff,
            PowerAction::LogOut,
        ] {
            assert_eq!(action_from_word(action_word(a)), Some(a));
        }
        assert_eq!(switch_from_word("ON"), None);
        assert_eq!(action_from_word("poweroff"), None);
    }

    #[test]
    fn a_verb_with_its_duty_off_does_nothing_and_saves_nothing() {
        let scratch = Scratch::new("duty-off");
        let mut shell = scratch.shell("{}");
        let (energy, log) = Fake::handle();
        let before = std::fs::read(settings::path_in(&scratch.0)).expect("file");
        for (verb, flag) in [
            (Verb::Caffeine(Switch::Toggle), Flag::Caffeine),
            (Verb::NightLight(Switch::On), Flag::NightLight),
        ] {
            assert_eq!(run(verb, &mut shell, &energy), Some(Outcome::DutyOff(flag)));
        }
        assert_eq!(
            std::fs::read(settings::path_in(&scratch.0)).expect("file"),
            before
        );
        assert!(said(&log).is_empty());
        assert_eq!(energy.running(), Default::default());
    }

    #[test]
    fn toggling_persists_before_the_workers_hear_of_it() {
        let scratch = Scratch::new("toggle");
        let mut shell = scratch.shell(r#"{"duties":{"caffeine":"on","nightlight":"on"}}"#);
        let (energy, log) = Fake::handle();
        energy.apply(shell.current());

        assert_eq!(
            run(Verb::Caffeine(Switch::Toggle), &mut shell, &energy),
            Some(Outcome::Changed(Flag::Caffeine, true))
        );
        assert!(scratch.reread().energy.caffeine);
        assert!(said(&log).contains(&"logind caffeine true".to_owned()));
        assert_eq!(
            run(Verb::Caffeine(Switch::On), &mut shell, &energy),
            Some(Outcome::Unchanged(Flag::Caffeine, true))
        );
        assert_eq!(
            run(Verb::Caffeine(Switch::Toggle), &mut shell, &energy),
            Some(Outcome::Changed(Flag::Caffeine, false))
        );
        assert!(!scratch.reread().energy.caffeine);

        assert_eq!(
            run(Verb::NightLight(Switch::Toggle), &mut shell, &energy),
            Some(Outcome::Changed(Flag::NightLight, true))
        );
        assert!(scratch.reread().energy.night_light);
        assert!(said(&log).contains(&"night set Some(2700)".to_owned()));
        assert_eq!(
            run(Verb::NightLight(Switch::Off), &mut shell, &energy),
            Some(Outcome::Changed(Flag::NightLight, false))
        );
        assert!(said(&log).ends_with(&["night set None".to_owned()]));
    }

    #[test]
    fn a_failed_write_changes_nothing_anywhere() {
        let scratch = Scratch::new("failed-write");
        let mut shell = scratch.shell(r#"{"duties":{"caffeine":"on"}}"#);
        let (energy, log) = Fake::handle();
        // The settings directory turns into a file: the write cannot happen.
        let dir = scratch.0.join("celestina");
        std::fs::remove_dir_all(&dir).expect("remove");
        std::fs::write(&dir, b"in the way").expect("obstacle");
        let outcome = run(Verb::Caffeine(Switch::On), &mut shell, &energy);
        assert!(matches!(
            outcome,
            Some(Outcome::NotSaved(Flag::Caffeine, _))
        ));
        assert!(!shell.current().energy.caffeine);
        assert!(said(&log).is_empty());
    }

    #[test]
    fn session_verbs_are_routed_to_logind_and_need_no_duty() {
        let scratch = Scratch::new("session");
        let mut shell = scratch.shell("{}");
        let (energy, log) = Fake::handle();
        for (action, line) in [
            (PowerAction::Suspend, "logind power Suspend"),
            (PowerAction::Reboot, "logind power Reboot"),
            (PowerAction::PowerOff, "logind power PowerOff"),
        ] {
            assert_eq!(
                run(Verb::Session(action), &mut shell, &energy),
                Some(Outcome::Requested(action))
            );
            assert!(said(&log).contains(&line.to_owned()));
        }
        // Log out has no compositor in the fake: a stated refusal, not logind.
        assert!(matches!(
            run(Verb::Session(PowerAction::LogOut), &mut shell, &energy),
            Some(Outcome::NotRequested(PowerAction::LogOut, _))
        ));
        assert!(!said(&log).contains(&"logind power LogOut".to_owned()));
    }

    #[test]
    fn other_verbs_are_not_energy_verbs() {
        let mut shell = ShellSettings::load_from(None);
        let (energy, _log) = Fake::handle();
        assert_eq!(run(Verb::Quit, &mut shell, &energy), None);
        assert_eq!(run(Verb::IslandToggle, &mut shell, &energy), None);
    }
}
