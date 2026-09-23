//! The one place Hematita runs something as root: `pkexec` around
//! `/usr/bin/kill`, on a worker thread, with a fixed argument shape. See
//! ADR 0010. Nothing here inherits privilege: pkexec runs `kill` and exits.

use std::process::Command;
use std::thread;

use hematita_core::services::{outcome_of_pkexec, Outcome};

const PKEXEC: &str = "/usr/bin/pkexec";
const KILL: &str = "/usr/bin/kill";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Terminate,
    Kill,
}

impl Signal {
    fn flag(self) -> &'static str {
        match self {
            Self::Terminate => "-TERM",
            Self::Kill => "-KILL",
        }
    }
}

/// Runs `pkexec kill -SIG PID` on a named thread and hands the outcome to
/// `report`, which is expected to queue it onto the Qt thread.
///
/// # Errors
///
/// The OS refused to create the thread.
pub fn signal_as_root(
    pid: u32,
    signal: Signal,
    report: impl FnOnce(Outcome) + Send + 'static,
) -> std::io::Result<()> {
    thread::Builder::new()
        .name("hematita-pkexec".to_owned())
        .spawn(move || {
            let status = Command::new(PKEXEC)
                .arg(KILL)
                .arg(signal.flag())
                .arg(pid.to_string())
                .status();
            report(match status {
                Ok(status) => outcome_of_pkexec(status.code()),
                Err(_) => Outcome::Failed,
            });
        })
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signal_is_the_flag_kill_takes_and_nothing_else() {
        assert_eq!(Signal::Terminate.flag(), "-TERM");
        assert_eq!(Signal::Kill.flag(), "-KILL");
    }
}
