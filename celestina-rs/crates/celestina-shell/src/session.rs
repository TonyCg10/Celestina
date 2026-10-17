// Copied from `celestina-rs/crates/celestina-shell-core/src/session.rs` at
// 7175df087e4f2c7dda33e6ca3dfc05d0e3926ce9, adapted: only `Switch` and
// `PowerAction` are kept, with their doc comments and derives unchanged. The
// level and mute vocabulary, `SessionRequest`, the provider-envelope parsing
// (`parse_request`, `parse_for`, `serves`, `no_provider`) and the option
// helpers are dropped; they depend on the halted provider envelope.

//! The two session vocabularies this shell needs before anything is done about
//! them: a two-state feature (`on`, `off`, `toggle`) and the requests a person
//! cannot take back (log out, reboot, power off, suspend).
//!
//! Both are plain values. Parsing a binding into them and carrying them out
//! belong to the surfaces that own those steps, so this module does no IO.

/// A state a binding may ask a two-state feature to take. `Toggle` is a
/// separate verb rather than a missing value, so a lost keystroke can never be
/// read as "the other one".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    On,
    Off,
    Toggle,
}

impl Switch {
    /// The state this switch leaves, given the one the feature is in now.
    ///
    /// `On` and `Off` name their answer outright; only `Toggle` depends on
    /// `current`, and it always takes the opposite of it.
    #[must_use]
    pub fn resolve(self, current: bool) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
            Self::Toggle => !current,
        }
    }
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
    /// Sleep. Fail-closed like the lock verbs: a session that suspends
    /// unlocked wakes up unlocked, so this is refused while no locker provider
    /// exists.
    Suspend,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_is_copied_and_compared_by_value() {
        let on = Switch::On;
        let copy = on;
        assert_eq!(on, copy);
        assert_ne!(Switch::On, Switch::Off);
        assert_ne!(Switch::Off, Switch::Toggle);
    }

    #[test]
    fn on_and_off_ignore_the_current_state() {
        for current in [false, true] {
            assert!(Switch::On.resolve(current));
            assert!(!Switch::Off.resolve(current));
        }
    }

    #[test]
    fn toggle_takes_the_opposite_of_the_current_state() {
        assert!(Switch::Toggle.resolve(false));
        assert!(!Switch::Toggle.resolve(true));
    }

    #[test]
    fn the_power_actions_are_the_four_the_session_offers() {
        let actions = [
            PowerAction::LogOut,
            PowerAction::Reboot,
            PowerAction::PowerOff,
            PowerAction::Suspend,
        ];
        for (index, action) in actions.iter().enumerate() {
            for (other_index, other) in actions.iter().enumerate() {
                assert_eq!(action == other, index == other_index);
            }
        }
    }
}
