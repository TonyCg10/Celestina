//! `selenita --pick-output`: the ScreenCast portal's output chooser
//! (SEL-4-A).
//!
//! xdg-desktop-portal-wlr runs its `chooser_cmd` through `/bin/sh -c` and
//! reads one line from its standard output, `Monitor: <name>` or the bare
//! output name. With this flag as that command, a recording started with
//! `--record-focused` casts the monitor niri had focused without a dialog:
//! the recording worker leaves a one-shot request
//! (`selenita_core::pick`) in `$XDG_RUNTIME_DIR/selenita/pick-output`, and
//! a fresh one is consumed and answered here. Without one (a plain
//! recording, a stale or broken request, no runtime folder) this process
//! becomes the previous chooser, whose dialog asks as before:
//! `"$HOME/.local/bin/celestina" --pick-output`, or the command line in
//! `SELENITA_FALLBACK_CHOOSER`, run through `/bin/sh -c` with the same
//! standard input, output and error.
//!
//! `main` calls [`run`] first of all, before Qt, the bus or the settings:
//! the chooser is quick, never claims Selenita's name and never opens a
//! window. Diagnostics go to standard error (the portal's journal).

use std::ffi::OsString;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::SystemTime;

use selenita_core::pick::{self, PickRequest, Taken};

/// The environment variable that names another fallback chooser.
pub const FALLBACK_VARIABLE: &str = "SELENITA_FALLBACK_CHOOSER";
/// The chooser the portal ran before Selenita's, which still asks for every
/// recording that did not leave a request.
pub const DEFAULT_FALLBACK: &str = "exec \"$HOME/.local/bin/celestina\" --pick-output";
/// The shell the portal itself runs its chooser through.
const SHELL: &str = "/bin/sh";

/// Answers the chooser and ends the process: a fresh request's output, or
/// the fallback chooser in this process's place.
pub fn run() -> ! {
    if let Some(request) = fresh_request() {
        answer(&request);
    }
    fall_back()
}

/// The fresh request in the runtime folder, consumed; `None` (and anything
/// stale or broken removed) otherwise.
fn fresh_request() -> Option<PickRequest> {
    let runtime = match celestina_core::xdg::runtime_dir() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("selenita: no runtime folder for a pick request: {error}");
            return None;
        }
    };
    match pick::take_request(&pick::request_file(&runtime), SystemTime::now()) {
        Ok(Taken::Fresh(request)) => Some(request),
        Ok(Taken::Absent) => None,
        Ok(Taken::Stale(request)) => {
            eprintln!(
                "selenita: a stale pick request for {} was dropped",
                request.output()
            );
            None
        }
        Ok(Taken::Invalid(error)) => {
            eprintln!("selenita: a broken pick request was dropped: {error}");
            None
        }
        Err(error) => {
            eprintln!("selenita: {error}");
            None
        }
    }
}

/// Prints the request's chooser line and ends with status 0.
fn answer(request: &PickRequest) -> ! {
    let mut out = std::io::stdout().lock();
    let written = writeln!(out, "{}", request.chooser_line()).and_then(|()| out.flush());
    match written {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("selenita: cannot answer the portal: {error}");
            std::process::exit(1)
        }
    }
}

/// Becomes the fallback chooser; returns only to end with status 1 when it
/// could not be started.
fn fall_back() -> ! {
    let command = fallback_command(std::env::var_os(FALLBACK_VARIABLE));
    let error = through_shell(&command).exec();
    eprintln!(
        "selenita: cannot run the fallback chooser {}: {error}",
        command.to_string_lossy()
    );
    std::process::exit(1)
}

/// `command` run as the portal runs a chooser, `/bin/sh -c`, with this
/// process's standard input, output and error.
fn through_shell(command: &OsString) -> Command {
    let mut shell = Command::new(SHELL);
    shell.arg("-c").arg(command);
    shell
}

/// The fallback's command line: the variable's value when it holds one,
/// else [`DEFAULT_FALLBACK`].
fn fallback_command(value: Option<OsString>) -> OsString {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from(DEFAULT_FALLBACK))
}

// The binary's own `--pick-output` (a fresh request answered, the fallback
// exec'd with its output and status) is run by `scripts/smoke.sh`: an
// integration test cannot link in this package, whose build links the
// CXX-Qt objects into every target.
#[cfg(test)]
mod tests {
    use super::{fallback_command, through_shell, DEFAULT_FALLBACK};
    use std::ffi::OsString;
    use std::io::Write;
    use std::process::Stdio;

    /// The fallback runs through the shell as the portal's own chooser
    /// would: its standard output is the answer, its status the exit, and
    /// it reads the same standard input.
    #[test]
    fn the_fallback_runs_through_the_shell_with_the_same_streams() {
        let output = through_shell(&fallback_command(Some(OsString::from(
            "echo Monitor: FAKE-2",
        ))))
        .output()
        .expect("ran");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"Monitor: FAKE-2\n");
        let output = through_shell(&OsString::from("exit 3"))
            .output()
            .expect("ran");
        assert_eq!(output.status.code(), Some(3));
        let mut child = through_shell(&OsString::from("read -r line; echo \"Monitor: $line\""))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawned");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(b"DP-7\n")
            .expect("written");
        let output = child.wait_with_output().expect("ended");
        assert_eq!(output.stdout, b"Monitor: DP-7\n");
    }

    #[test]
    fn the_fallback_is_the_previous_chooser_unless_one_is_named() {
        assert_eq!(fallback_command(None), OsString::from(DEFAULT_FALLBACK));
        assert_eq!(
            fallback_command(Some(OsString::new())),
            OsString::from(DEFAULT_FALLBACK)
        );
        assert_eq!(
            fallback_command(Some(OsString::from("echo Monitor: FAKE-2"))),
            OsString::from("echo Monitor: FAKE-2")
        );
        assert!(DEFAULT_FALLBACK.contains("/.local/bin/celestina\" --pick-output"));
    }
}
