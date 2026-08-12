//! Running the session's own tools, and holding the runtime while doing it.
//!
//! Every provider here reads the desktop through a program the session already
//! has installed. What they share is not what those programs say but how they
//! are run: bounded, killed if they outstay their welcome, and never able to
//! block the helper by hanging.

use std::io;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::diagnostics::{Event, Level, Value};
use celestina_shell_core::inventory::Answer;
use celestina_shell_core::journal;
use celestina_shell_core::runtime::ProviderRuntime;

/// A tool that cannot answer in this long is a tool the panel does without.
/// It is generous for a process spawn and short enough that a stuck player or
/// a wedged adapter never becomes a stuck provider.
const TOOL_TIMEOUT: Duration = Duration::from_millis(750);

/// A poisoned runtime lock still owns usable state; recovering it keeps the
/// panel fed instead of taking the helper down with the thread that panicked —
/// the documented mutex pattern of this suite.
pub fn lock_runtime(runtime: &Mutex<ProviderRuntime>) -> MutexGuard<'_, ProviderRuntime> {
    match runtime.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Runs a short-lived tool and returns its output, killing it if it outstays
/// its deadline: a hung tool must not take its provider down with it.
pub fn run_bounded(program: &str, args: &[&str]) -> Option<String> {
    probe_bounded(program, args).into_text()
}

/// The same, for a reading that cares *why* there was no output.
///
/// A program that is not installed and one that missed its deadline are the
/// same absence to a summary widget, which shows nothing either way. They are
/// not the same to a list: on a session without the tool an empty list is
/// permanently correct, and on a session with a slow one it is a lie that
/// erases what was already known.
pub fn probe_bounded(program: &str, args: &[&str]) -> Answer {
    probe_bounded_with_cancel(program, args, TOOL_TIMEOUT, None)
}

/// The same, for a tool whose own pace is the reason it is slow. DDC is a
/// physical conversation with a monitor and takes about a second; giving it the
/// panel's timeout would mean never reading a brightness at all.
pub fn run_bounded_with(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    run_bounded_with_cancel(program, args, timeout, None)
}

/// Runs a bounded tool owned by a cancellable provider. Shutdown kills and
/// reaps the direct child before the worker returns, so terminating the helper
/// cannot reparent a probe that was still talking to hardware.
pub fn run_bounded_with_cancel(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
) -> Option<String> {
    probe_bounded_with_cancel(program, args, timeout, cancelled).into_text()
}

/// The one implementation. Everything above is a narrowing of this answer for
/// a caller that does not need the distinction.
pub fn probe_bounded_with_cancel(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
) -> Answer {
    // Every external process this helper runs passes through here, so this is
    // where the journal watches them. The GPU has been lost from the bus while
    // this shell was running and `ddcutil` reaches an I²C bus on that same card,
    // so a process that started and never came back is exactly the shape of
    // thing an investigation needs to be able to see. That is why these are
    // `Critical`: they are flushed rather than left in a buffer a power cut
    // would take.
    //
    // The arguments are recorded because they are this shell's own and entirely
    // technical — `getvcp 10 --bus 5` is the operation, not a person's data.
    // Arguments that come from somewhere else never reach this function; see
    // `launch_argv`, which deliberately records none.
    let started = Instant::now();
    let command = command_text(program, args);

    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        journal::record(
            process_event("process.cancelled", &command).with("before_spawn", Value::Bool(true)),
        );
        return Answer::Unreadable;
    }

    journal::record(process_event("process.spawn", &command));

    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        // The only failure this can tell apart with confidence. A permission
        // error or an exhausted process table is a session that is unwell, not
        // one without the program.
        Err(error) => {
            // The technical reason is never discarded. A spawn that failed for
            // a reason nobody wrote down is how "the tool is missing" hides an
            // exhausted process table.
            journal::record(
                process_event("process.spawn.failed", &command)
                    .with_text("kind", &format!("{:?}", error.kind()))
                    .with_text("error", &error.to_string()),
            );
            if error.kind() == io::ErrorKind::NotFound {
                return Answer::Missing;
            }
            return Answer::Unreadable;
        }
    };

    let pid = u64::from(child.id());
    journal::record(process_event("process.started", &command).with("child_pid", Value::Uint(pid)));

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                let stopping = cancelled.is_some_and(|flag| flag.load(Ordering::Acquire));
                if stopping || Instant::now() >= deadline {
                    journal::record(
                        process_event(
                            if stopping {
                                "process.cancelled"
                            } else {
                                "process.timeout"
                            },
                            &command,
                        )
                        .with("child_pid", Value::Uint(pid))
                        .with("elapsed_ms", Value::Millis(elapsed_ms(started)))
                        .with("timeout_ms", Value::Millis(millis(timeout))),
                    );
                    stop_and_reap(&mut child);
                    journal::record(
                        process_event("process.reaped", &command)
                            .with("child_pid", Value::Uint(pid))
                            .with("killed", Value::Bool(true)),
                    );
                    return Answer::Unreadable;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                journal::record(
                    process_event("process.wait.failed", &command)
                        .with("child_pid", Value::Uint(pid))
                        .with_text("error", &error.to_string()),
                );
                stop_and_reap(&mut child);
                journal::record(
                    process_event("process.reaped", &command)
                        .with("child_pid", Value::Uint(pid))
                        .with("killed", Value::Bool(true)),
                );
                return Answer::Unreadable;
            }
        }
    }

    let Ok(output) = child.wait_with_output() else {
        journal::record(
            process_event("process.output.failed", &command).with("child_pid", Value::Uint(pid)),
        );
        return Answer::Unreadable;
    };
    // The output itself is never recorded: `ddcutil detect` names monitors and
    // a player's status names what somebody is listening to. Its size is.
    journal::record(
        process_exit_event(&command, output.status.success())
            .with("child_pid", Value::Uint(pid))
            .with(
                "code",
                Value::Int(i64::from(output.status.code().unwrap_or(-1))),
            )
            .with("ok", Value::Bool(output.status.success()))
            .with("elapsed_ms", Value::Millis(elapsed_ms(started)))
            .with_redacted("stdout", &String::from_utf8_lossy(&output.stdout)),
    );
    if !output.status.success() {
        return Answer::Unreadable;
    }
    Answer::Text(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// One external invocation, as the journal names it.
///
/// The program and its arguments joined with spaces, bounded by the field's own
/// limit. This is only ever called with this shell's own tool invocations.
fn command_text(program: &str, args: &[&str]) -> String {
    let mut text = program.to_owned();
    for argument in args {
        text.push(' ');
        text.push_str(argument);
    }
    text
}

/// Programs that reach the graphics card, and so keep the flushed level.
///
/// The reason these events are `Critical` is the GPU: the card has been lost
/// from the bus while this shell was running, and a child that started and
/// never came back is the shape an investigation needs to survive a power cut.
/// That argument is about `ddcutil` and the I²C buses it opens. It was never
/// about `wpctl`.
const GPU_ADJACENT_PROGRAMS: [&str; 1] = ["ddcutil"];

fn reaches_the_graphics_card(command: &str) -> bool {
    let program = command.split(' ').next().unwrap_or(command);
    // Compared on the file name so an absolute path is the same program.
    let name = program.rsplit('/').next().unwrap_or(program);
    GPU_ADJACENT_PROGRAMS.contains(&name)
}

/// The ordinary lifecycle of a child that cannot touch the card.
///
/// Recorded, and not flushed. `Critical` means `write` plus `flush` plus
/// `sync_data` for every line, and the audio provider polls by spawning
/// `wpctl` every two seconds — three flushed events each. Measured on the
/// author's idle session that was 290 KB/s reaching the disk for a file
/// growing 2.3 KB/s: roughly 126 times amplification, about 25 GB a day of
/// writes for poll bookkeeping. The events still reach the file in order; they
/// simply travel in the buffer the sink already has.
/// Deliberately not `process.reaped`: that one is only ever recorded after a
/// timeout or a broken wait, which is the overlap `AUD-1-C` exists to make
/// visible, and it is rare enough to deserve the disk.
const ROUTINE_LIFECYCLE: [&str; 3] = ["process.spawn", "process.started", "process.exit"];

fn process_event(name: &str, command: &str) -> Event {
    // Anything that is not the ordinary lifecycle is an anomaly — a spawn that
    // failed, a timeout, a kill, a wait that broke — and those are rare and
    // worth the disk. So is every event about a GPU-adjacent child.
    let level = if ROUTINE_LIFECYCLE.contains(&name) && !reaches_the_graphics_card(command) {
        Level::Info
    } else {
        Level::Critical
    };
    Event::new(level, name).with_text("command", command)
}

/// A child that ended badly is an anomaly whatever it was, so it keeps the
/// flushed level even when the ordinary exit beside it does not.
fn process_exit_event(command: &str, ok: bool) -> Event {
    if ok {
        process_event("process.exit", command)
    } else {
        Event::new(Level::Critical, "process.exit").with_text("command", command)
    }
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn stop_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs an external tool the session already uses, detached from this helper's
/// pipes. A short-lived thread waits on it so a launched tool never lingers as
/// a zombie, and the helper never blocks on one.
pub fn launch(program: &str) -> Result<(), String> {
    launch_argv(&[program])
}

/// The same, for a launch that needs its own arguments — an application's
/// `Exec`, or a URL handed to `xdg-open`. `argv` is never run through a shell:
/// it is exactly the words execve receives, which is what keeps a `.desktop`
/// file's own text from being interpreted as anything more than a program name
/// and its arguments.
///
/// # Errors
///
/// Returns `Err` for an empty `argv`, or one whose program could not be
/// started at all; a launch is a request, and this is what a request that
/// could not even be made looks like.
pub fn launch_argv(argv: &[&str]) -> Result<(), String> {
    let [program, arguments @ ..] = argv else {
        return Err("nothing to launch".to_owned());
    };

    // Deliberately different from `probe_bounded_with_cancel`: this `argv` came
    // out of a `.desktop` file the person chose to run, so neither the command
    // line nor its arguments are recorded. What is recorded is that a launch
    // happened, how many arguments it carried and whether it started — enough to
    // place it in the timeline, and nothing about what was opened.
    journal::record(
        Event::new(Level::Info, "launch.request")
            .with("argument_count", Value::Uint(arguments.len() as u64))
            .with_redacted("argv", &argv.join(" ")),
    );

    let child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .inspect_err(|error| {
            journal::record(
                Event::new(Level::Warn, "launch.failed")
                    .with_text("kind", &format!("{:?}", error.kind())),
            );
        })
        .map_err(|error| format!("cannot start {program}: {error}"))?;
    journal::record(
        Event::new(Level::Info, "launch.started")
            .with("child_pid", Value::Uint(u64::from(child.id()))),
    );

    thread::Builder::new()
        .name("reaper".to_owned())
        .spawn(move || {
            let mut child = child;
            let _ = child.wait();
        })
        .map_err(|error| format!("cannot wait on {program}: {error}"))?;
    Ok(())
}

/// The journal every test in this binary writes to.
///
/// One process, one journal, so it is installed once into a temporary directory
/// and every test reads the same file. Tests filter by the command they ran,
/// which is what keeps them independent while sharing it.
#[cfg(test)]
pub fn test_journal() -> std::path::PathBuf {
    use celestina_shell_core::diagnostics::{Component, Identity};
    use celestina_shell_core::journal::Journal;
    use std::sync::OnceLock;

    static DIRECTORY: OnceLock<std::path::PathBuf> = OnceLock::new();
    DIRECTORY
        .get_or_init(|| {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos());
            let path = std::env::temp_dir().join(format!("celestina-tools-journal-{nanos:x}"));
            let identity = Identity::new("run-test", Component::new("provider-adapter"), 1, 1);
            journal::install(Journal::open(Some(path.clone()), identity, false));
            path
        })
        .clone()
}

/// Every line this binary's journal has managed to write so far.
#[cfg(test)]
pub fn test_journal_lines() -> Vec<serde_json::Value> {
    // The writer is a thread; give it a moment to reach the disk before reading.
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(20));
        if let Some(journal) = journal::process_journal() {
            journal.record(Event::new(Level::Trace, "journal.probe"));
        }
        let found = read_journal_lines();
        if !found.is_empty() {
            return found;
        }
    }
    read_journal_lines()
}

#[cfg(test)]
fn read_journal_lines() -> Vec<serde_json::Value> {
    let mut all = Vec::new();
    let Ok(entries) = std::fs::read_dir(test_journal()) else {
        return all;
    };
    for entry in entries.filter_map(Result::ok) {
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            if let Ok(value) = serde_json::from_str(line) {
                all.push(value);
            }
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn events_named(name: &str) -> Vec<serde_json::Value> {
        test_journal_lines()
            .into_iter()
            .filter(|line| line["event"] == name)
            .collect()
    }

    /// What a routine poll costs the disk, and what still survives a power cut.
    ///
    /// `Critical` is `write` + `flush` + `sync_data` per line. The audio
    /// provider polls by spawning `wpctl` every two seconds, so recording its
    /// ordinary lifecycle at that level put 290 KB/s on the author's idle SSD
    /// for a journal file growing 2.3 KB/s — about 25 GB a day of poll
    /// bookkeeping. Nothing here stops being recorded; the ordinary lines
    /// simply travel in the sink's buffer.
    #[test]
    fn only_the_gpu_and_the_anomalies_are_worth_an_fsync() {
        // A poll that succeeds is recorded without reaching for the disk.
        for name in ["process.spawn", "process.started", "process.exit"] {
            assert_eq!(
                process_event(name, "wpctl get-volume @DEFAULT_AUDIO_SINK@").level(),
                Level::Info,
                "{name} of a routine tool must not flush"
            );
        }

        // The reason the level exists at all: a child that can take the
        // graphics card down with it, whatever it is doing.
        for name in ["process.spawn", "process.started", "process.exit"] {
            assert_eq!(
                process_event(name, "ddcutil detect --brief").level(),
                Level::Critical,
                "{name} of a DDC child must survive a power cut"
            );
            assert_eq!(
                process_event(name, "/usr/bin/ddcutil getvcp 10").level(),
                Level::Critical,
                "an absolute path is the same program"
            );
        }

        // Every anomaly keeps the disk, for any program: these are rare, and
        // each one is the line an investigation would need.
        for name in [
            "process.spawn.failed",
            "process.timeout",
            "process.cancelled",
            "process.wait.failed",
            "process.output.failed",
            // Only ever recorded after a timeout or a broken wait — the
            // overlap `AUD-1-C` exists to make visible.
            "process.reaped",
        ] {
            assert_eq!(
                process_event(name, "wpctl get-volume @DEFAULT_AUDIO_SINK@").level(),
                Level::Critical,
                "{name} is not routine"
            );
        }

        // And an ordinary-looking exit that failed is an anomaly too.
        assert_eq!(
            process_exit_event("wpctl get-volume @DEFAULT_AUDIO_SINK@", true).level(),
            Level::Info
        );
        assert_eq!(
            process_exit_event("wpctl get-volume @DEFAULT_AUDIO_SINK@", false).level(),
            Level::Critical
        );
    }

    #[test]
    fn a_bounded_process_is_recorded_from_spawn_to_exit() {
        test_journal();
        // A fake process, not a session tool: nothing here runs `ddcutil`,
        // touches hardware or reaches a bus.
        let answer = probe_bounded_with_cancel(
            "/bin/sh",
            &["-c", "printf journal-fixture-ok"],
            Duration::from_secs(5),
            None,
        );
        assert_eq!(answer.into_text().as_deref(), Some("journal-fixture-ok"));

        let exits: Vec<serde_json::Value> = events_named("process.exit")
            .into_iter()
            .filter(|line| {
                line["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("journal-fixture-ok"))
            })
            .collect();
        let exit = exits.first().expect("the exit of the fixture is recorded");
        assert_eq!(exit["code"], 0);
        assert_eq!(exit["ok"], true);
        assert!(exit["child_pid"].as_u64().unwrap() > 0);
        assert!(exit["elapsed_ms"].as_u64().is_some());
        // The output was measured and not kept.
        assert_eq!(exit["stdout_bytes"], "journal-fixture-ok".len());
        assert!(exit.get("stdout").is_none());
    }

    #[test]
    fn a_process_that_outstays_its_deadline_is_recorded_killed_and_reaped() {
        test_journal();
        let answer = probe_bounded_with_cancel(
            "/bin/sh",
            &["-c", "sleep 30 # journal-fixture-hang"],
            Duration::from_millis(120),
            None,
        );
        assert!(answer.into_text().is_none());

        let mine = |name: &str| {
            events_named(name).into_iter().any(|line| {
                line["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("journal-fixture-hang"))
            })
        };
        assert!(mine("process.timeout"), "the timeout is recorded");
        assert!(mine("process.reaped"), "the kill and reap are recorded");
    }

    #[test]
    fn a_cancelled_process_says_it_was_cancelled_rather_than_timed_out() {
        test_journal();
        let cancelled = AtomicBool::new(true);

        let answer = probe_bounded_with_cancel(
            "/bin/sh",
            &["-c", "true # journal-fixture-cancel"],
            Duration::from_secs(5),
            Some(&cancelled),
        );

        assert!(answer.into_text().is_none());
        assert!(events_named("process.cancelled").into_iter().any(|line| {
            line["command"]
                .as_str()
                .is_some_and(|command| command.contains("journal-fixture-cancel"))
        }));
    }

    #[test]
    fn a_spawn_failure_never_discards_its_technical_reason() {
        test_journal();
        let answer = probe_bounded_with_cancel(
            "/nonexistent/journal-fixture-missing",
            &[],
            Duration::from_secs(1),
            None,
        );
        assert!(matches!(answer, Answer::Missing));

        let failure = events_named("process.spawn.failed")
            .into_iter()
            .find(|line| {
                line["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("journal-fixture-missing"))
            })
            .expect("a spawn that failed is recorded with why");
        // "Missing" is a conclusion; the kind is the fact behind it, and an
        // exhausted process table must never hide inside that conclusion.
        assert_eq!(failure["kind"], "NotFound");
        assert!(!failure["error"].as_str().unwrap().is_empty());
    }

    #[test]
    fn a_launch_records_that_it_happened_and_nothing_about_what_was_opened() {
        test_journal();
        let secret = "/home/toni/Documents/journalfixtureprivate.pdf";

        let _unused = launch_argv(&["/bin/true", secret]);

        let request = events_named("launch.request")
            .into_iter()
            .next_back()
            .expect("a launch is placed in the timeline");
        assert_eq!(request["argument_count"], 1);
        // The size is there; the path is not, and neither is the program.
        assert!(request["argv_bytes"].as_u64().unwrap() > 0);
        let text = serde_json::to_string(&test_journal_lines()).expect("serializable");
        assert!(!text.contains("journalfixtureprivate"));
    }

    /// A program the session does not have. The whole point of the enum: this
    /// must not look like a tool that was merely slow, because a list built on
    /// it is permanently empty rather than temporarily unknown.
    #[test]
    fn a_program_that_is_not_installed_says_so_rather_than_timing_out() {
        let answer = probe_bounded("celestina-tool-that-does-not-exist", &[]);

        assert_eq!(answer, Answer::Missing);
        assert_eq!(answer.text(), None);
    }

    /// A tool that outstays its deadline. `sleep` is in coreutils, it touches
    /// nothing, and it is the one thing here that is guaranteed to be slow.
    #[test]
    fn a_program_that_outstays_its_deadline_is_unreadable() {
        let started = Instant::now();
        let answer = run_probe("sleep", &["30"], Duration::from_millis(80));

        assert_eq!(answer, Answer::Unreadable);
        // Killed at its deadline rather than waited out.
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_program_that_fails_is_unreadable_rather_than_empty() {
        // `false` exits non-zero and prints nothing, which is exactly the
        // shape that must not be mistaken for "it answered with no rows".
        assert_eq!(probe_bounded("false", &[]), Answer::Unreadable);
    }

    /// The distinction that makes an empty inventory publishable: a tool that
    /// ran, succeeded and printed nothing really did answer.
    #[test]
    fn a_program_that_succeeds_with_no_output_answered_with_nothing() {
        let answer = probe_bounded("true", &[]);

        assert_eq!(answer, Answer::Text(String::new()));
        assert_eq!(answer.text(), Some(""));
    }

    /// A short-lived probe with its own deadline, so a timeout case does not
    /// have to wait out the shared one.
    fn run_probe(program: &str, args: &[&str], timeout: Duration) -> Answer {
        probe_bounded_with_cancel(program, args, timeout, None)
    }

    #[test]
    fn cancellation_stops_a_bounded_child_before_its_deadline() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let request = Arc::clone(&cancelled);
        let cancellation_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            request.store(true, Ordering::Release);
        });

        let started = Instant::now();
        // The child is killed and reaped before this returns, so nothing is
        // left behind for the helper to trip over on shutdown.
        let answer =
            probe_bounded_with_cancel("sleep", &["5"], Duration::from_secs(10), Some(&cancelled));

        // Cancelled, not absent: `sleep` is installed and was running.
        assert_eq!(answer, Answer::Unreadable);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(cancellation_thread.join().is_ok());

        // And a probe that is cancelled before it starts never spawns at all.
        assert_eq!(
            probe_bounded_with_cancel("sleep", &["5"], Duration::from_secs(10), Some(&cancelled)),
            Answer::Unreadable
        );
    }
}
