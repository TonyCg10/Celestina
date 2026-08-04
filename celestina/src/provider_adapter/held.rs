//! A state the session holds by keeping a program alive.
//!
//! Night light and the idle inhibitor are the same shape: something is true
//! for exactly as long as a child process is running, and false the moment it
//! is not. Neither owns a value to poll — `wlsunset` is holding the gamma ramps
//! and `systemd-inhibit` is holding a logind lock — so the honest state is
//! simply whether this helper still has that child.
//!
//! What makes it worth one type rather than two modules is where these go
//! wrong. A child that dies keeps its state advertised as on; a helper that
//! exits without killing it leaves a session tinted orange or unable to sleep
//! with nothing left that knows how to undo it. So a hold checks liveness every
//! time it is asked, and it is released on shutdown, on failure, and when the
//! program refuses to start at all.

use std::process::{Child, Command, Stdio};

use celestina_shell_core::session::Switch;

/// One held state: the program that holds it, and the child holding it now.
pub struct Hold {
    program: &'static str,
    args: Vec<String>,
    child: Option<Child>,
}

impl Hold {
    /// Describes a hold without taking it. Nothing runs until [`Hold::set`].
    pub fn new(program: &'static str, args: Vec<String>) -> Self {
        Self {
            program,
            args,
            child: None,
        }
    }

    /// Whether the state is held right now.
    ///
    /// A child that exited on its own — killed by the session, crashed, or
    /// refused by the compositor after starting — is not a held state, and
    /// asking is what notices it.
    pub fn is_held(&mut self) -> bool {
        let Some(child) = self.child.as_mut() else {
            return false;
        };

        if let Ok(None) = child.try_wait() {
            // Still running: the state is genuinely held.
            return true;
        }

        // Gone — exited on its own, or unwaitable. Either way this helper no
        // longer holds anything, so it stops claiming the state.
        self.child = None;
        false
    }

    /// Takes or releases the hold, and returns whether it is held afterwards.
    ///
    /// # Errors
    ///
    /// Returns the sentence the requester should be shown when the program
    /// cannot be started — a missing tool is a refusal, never a state quietly
    /// reported as on.
    pub fn set(&mut self, held: bool) -> Result<bool, String> {
        if !held {
            self.release();
            return Ok(false);
        }
        if self.is_held() {
            // Starting a second holder would leave one of them unreleasable:
            // this helper only remembers the child it last spawned.
            return Ok(true);
        }

        let child = Command::new(self.program)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("cannot start {}: {error}", self.program))?;
        self.child = Some(child);
        Ok(true)
    }

    /// Releases the hold now, waiting for the child so it never lingers as a
    /// zombie and so the thing it was holding — gamma, a logind lock — is
    /// really given back before this returns.
    pub fn release(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };

        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.release();
    }
}

/// The state a switch verb leaves a hold in, given what it is now.
///
/// `Toggle` is the only one that needs the current state, which is why this is
/// a function over it rather than something each caller works out again.
#[must_use]
pub fn wanted(state: Switch, held: bool) -> bool {
    match state {
        Switch::On => true,
        Switch::Off => false,
        Switch::Toggle => !held,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sleeping() -> Hold {
        Hold::new("sleep", vec!["30".to_owned()])
    }

    #[test]
    fn nothing_is_held_until_it_is_taken() {
        let mut hold = sleeping();
        assert!(!hold.is_held());
    }

    #[test]
    fn a_hold_lasts_until_it_is_released() {
        let mut hold = sleeping();
        assert_eq!(hold.set(true), Ok(true));
        assert!(hold.is_held());

        assert_eq!(hold.set(false), Ok(false));
        assert!(!hold.is_held());
    }

    #[test]
    fn taking_a_hold_twice_keeps_one_holder() {
        let mut hold = sleeping();
        assert_eq!(hold.set(true), Ok(true));
        assert_eq!(hold.set(true), Ok(true));

        // Releasing once is enough, which is only true if the second `set`
        // never spawned a child this type has forgotten how to kill.
        hold.release();
        assert!(!hold.is_held());
    }

    #[test]
    fn a_holder_that_exited_is_not_a_held_state() {
        let mut hold = Hold::new("true", Vec::new());
        assert_eq!(hold.set(true), Ok(true));

        // `true` returns immediately; the state it was supposed to hold is
        // gone, and the provider must say so instead of advertising it.
        for _ in 0..50 {
            if !hold.is_held() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("a finished holder is still reported as holding");
    }

    #[test]
    fn a_program_that_cannot_start_is_refused_rather_than_claimed() {
        let mut hold = Hold::new("celestina-no-such-program", Vec::new());
        let refusal = hold.set(true).expect_err("a missing program is refused");

        assert!(refusal.contains("celestina-no-such-program"));
        assert!(!hold.is_held());
    }

    #[test]
    fn a_toggle_is_the_only_verb_that_needs_the_current_state() {
        assert!(wanted(Switch::On, false));
        assert!(wanted(Switch::On, true));
        assert!(!wanted(Switch::Off, true));
        assert!(wanted(Switch::Toggle, false));
        assert!(!wanted(Switch::Toggle, true));
    }

    #[test]
    fn releasing_what_was_never_held_is_not_an_error() {
        let mut hold = sleeping();
        hold.release();
        assert!(!hold.is_held());
    }
}
