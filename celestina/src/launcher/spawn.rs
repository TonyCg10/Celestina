//! Starting an application the launcher found (design §5.5, Launcher): the
//! entry's `Exec` split and expanded by `celestina_core::desktop_entry`
//! (never a shell, so a `.desktop` file's `$()` or `;` does nothing), run
//! through `setsid -f` in a session of its own with the launcher's
//! environment, its standard streams on `/dev/null`, and never through the
//! compositor.
//!
//! `setsid -f` forks and exits at once, so the application is reparented
//! away from the shell; the short-lived `setsid` itself is reaped on a
//! detached thread, so no zombie is left behind and the loop never waits.

use std::{
    io,
    process::{Command, Stdio},
    thread,
};

use celestina_core::desktop_entry::{exec_argv, DesktopEntry};

/// The program that detaches the application from the shell's session.
const SETSID: [&str; 2] = ["setsid", "-f"];

/// The session's terminal for an entry with `Terminal=true`, the same one
/// the halted launcher used (`celestina-qt/src/provider_adapter/launcher.rs`
/// at c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413), followed by its option to
/// run a command.
const TERMINAL: [&str; 2] = ["kitty", "-e"];

/// Why an application was not started.
#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    /// The entry's `Exec` names no program.
    #[error("{name} names no program to run")]
    NoProgram { name: String },
    /// `setsid` could not be started.
    #[error("cannot start {name}")]
    Start {
        name: String,
        #[source]
        source: io::Error,
    },
}

/// The command line that starts `entry`: `setsid -f`, the session's
/// terminal for a terminal application, then the entry's `Exec` with its
/// field codes expanded for a launch that names no file. Empty when the
/// `Exec` names no program.
#[must_use]
pub fn spawn_argv(entry: &DesktopEntry) -> Vec<String> {
    let Some(program) = exec_argv(entry) else {
        return Vec::new();
    };
    let terminal: &[&str] = if entry.terminal { &TERMINAL } else { &[] };
    SETSID
        .iter()
        .chain(terminal)
        .map(|word| (*word).to_owned())
        .chain(program)
        .collect()
}

/// Starts `entry` as [`spawn_argv`] spells it, from the home directory
/// when there is one.
///
/// # Errors
///
/// [`SpawnError`] when the entry names no program or `setsid` cannot start.
pub fn spawn(entry: &DesktopEntry) -> Result<(), SpawnError> {
    let argv = spawn_argv(entry);
    let Some((program, args)) = argv.split_first() else {
        return Err(SpawnError::NoProgram {
            name: entry.name.clone(),
        });
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        command.current_dir(home);
    }
    let mut child = command.spawn().map_err(|source| SpawnError::Start {
        name: entry.name.clone(),
        source,
    })?;
    let id = entry.id.clone();
    // `setsid -f` exits right after its fork; reaping it here keeps the
    // shell free of zombies without the loop waiting on it.
    let reaper = thread::Builder::new()
        .name("launcher-reap".to_owned())
        .spawn(move || match child.wait() {
            Ok(status) if !status.success() => {
                tracing::warn!(%id, %status, "launcher: setsid failed");
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(%id, %err, "launcher: setsid not reaped"),
        });
    if let Err(err) = reaper {
        tracing::warn!(%err, "launcher: no thread to reap setsid; it stays a zombie until exit");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(exec: &str) -> DesktopEntry {
        DesktopEntry {
            id: "app.desktop".to_owned(),
            name: "App".to_owned(),
            exec: exec.to_owned(),
            is_application: true,
            ..DesktopEntry::default()
        }
    }

    #[test]
    fn spawn_builds_setsid_argv_from_exec_with_field_codes_stripped() {
        assert_eq!(
            spawn_argv(&entry("firefox %u --new-window %F")),
            ["setsid", "-f", "firefox", "--new-window"]
        );
        // Quoting is the specification's, never a shell's.
        assert_eq!(
            spawn_argv(&entry("\"/opt/My App/run\" --name=%c $(rm -rf ~)")),
            [
                "setsid",
                "-f",
                "/opt/My App/run",
                "--name=App",
                "$(rm",
                "-rf",
                "~)"
            ]
        );
        // `%%` is a literal percent sign.
        assert_eq!(
            spawn_argv(&entry("printf 100%%")),
            ["setsid", "-f", "printf", "100%"]
        );
    }

    #[test]
    fn a_terminal_application_runs_inside_the_terminal() {
        let mut top = entry("htop");
        top.terminal = true;
        assert_eq!(spawn_argv(&top), ["setsid", "-f", "kitty", "-e", "htop"]);
    }

    #[test]
    fn an_exec_without_a_program_starts_nothing() {
        assert!(spawn_argv(&entry("")).is_empty());
        assert!(spawn_argv(&entry("%f %U")).is_empty());
        assert!(matches!(
            spawn(&entry("")),
            Err(SpawnError::NoProgram { .. })
        ));
    }
}
