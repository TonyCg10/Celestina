//! `scripts/nest-run.sh`: the host-wide lock of the measuring runs and the
//! signal handling that ends a smoke together with its nest. The scripts run
//! here are small drivers that source it; no compositor, no shell binary and
//! no niri is involved, and `XDG_RUNTIME_DIR` points at a scratch directory so
//! the author's real lock file is never touched.
//!
//! This is a test binary of its own on purpose: `tests/lock_scripts.rs` keeps
//! exactly one test that starts a process, and these start several.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn scripts() -> PathBuf {
    manifest().join("scripts")
}

/// The program words that start a driver with INT, QUIT and HUP at their
/// default disposition. A test run started from a background job, `nohup` or
/// a CI runner inherits them ignored, and a POSIX shell cannot trap a signal
/// that was ignored on entry; `env --default-signal` (coreutils 8.31 and
/// later; it execs, so the pid is the driver's) restores them before `sh`
/// starts. Without that option the driver starts as it is.
fn default_signals() -> &'static [&'static str] {
    static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let supported = *SUPPORTED.get_or_init(|| {
        Command::new("env")
            .args(["--default-signal=INT,QUIT,HUP", "true"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    });
    if supported {
        &["env", "--default-signal=INT,QUIT,HUP"]
    } else {
        &[]
    }
}

/// `[env --default-signal=...] sh FILE`, as a command.
fn driver_command(file: &Path) -> Command {
    let words = default_signals();
    let mut command = match words.split_first() {
        Some((program, rest)) => {
            let mut command = Command::new(program);
            command.args(rest).arg("sh");
            command
        }
        None => Command::new("sh"),
    };
    command.arg(file);
    command
}

/// A scratch directory with a runtime directory inside it, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("celestina-nest-run-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("run")).expect("scratch directory");
        Scratch(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Writes a driver script (read by `sh`, never executed itself).
    fn driver(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, format!("set -eu\n. \"$NEST_RUN\"\n{body}\n")).expect("driver");
        path
    }

    /// `sh FILE` with the scratch runtime directory, a short lock wait and no
    /// marker inherited from whoever runs the tests.
    fn sh(&self, file: &Path) -> Command {
        let mut command = driver_command(file);
        command
            .env("NEST_RUN", scripts().join("nest-run.sh"))
            .env("XDG_RUNTIME_DIR", self.path("run"))
            .env("MARKS", self.path("marks"))
            .env("READY", self.path("ready"))
            .env("STOP", self.path("stop"))
            .env("OUT", self.path("out"))
            .env("CLEAN", self.path("clean"))
            .env("PIDFILE", self.path("pid"))
            .env("INNER", self.path("inner.sh"))
            .env_remove("CELESTINA_NEST_LOCK_HELD")
            .env_remove("CELESTINA_NEST_LOCK_WAIT")
            .stdin(Stdio::null());
        command
    }

    /// Starts a driver with its output in files of the scratch directory.
    fn spawn(&self, mut command: Command, tag: &str) -> Child {
        command
            .stdout(fs::File::create(self.path(&format!("{tag}.out"))).expect("stdout file"))
            .stderr(fs::File::create(self.path(&format!("{tag}.err"))).expect("stderr file"))
            .spawn()
            .expect("sh starts")
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap_or_default()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Polls until `condition` holds or the deadline passes.
fn poll(seconds: u64, what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_exit(child: &mut Child, seconds: u64, what: &str) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status;
        }
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn signal(name: &str, pid: u32) {
    let status = Command::new("kill")
        .arg(format!("-{name}"))
        .arg(pid.to_string())
        .status()
        .expect("kill runs");
    assert!(status.success(), "kill -{name} {pid}");
}

/// Whether a process is gone or only a zombie nobody has reaped yet.
fn gone(pid: u32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/status")) {
        Err(_) => true,
        Ok(text) => text.lines().any(|line| {
            line.strip_prefix("State:")
                .is_some_and(|state| state.trim_start().starts_with('Z'))
        }),
    }
}

/// A driver that takes the lock as `who`, records its start, tells the test it
/// holds the lock and holds it until the test creates `$STOP`.
const HOLDER: &str = r#"
nest_lock_acquire "$1"
echo "start-$1" >> "$MARKS"
echo "held=${CELESTINA_NEST_LOCK_HELD:-unset}" > "$OUT"
: > "$READY"
while [ ! -e "$STOP" ] && [ -d "$XDG_RUNTIME_DIR" ]; do sleep 0.05; done
echo "end-$1" >> "$MARKS"
"#;

/// A driver that only takes the lock and notes its start and end.
const TAKER: &str = r#"
nest_lock_acquire "$1"
echo "start-$1" >> "$MARKS"
echo "end-$1" >> "$MARKS"
"#;

fn start_holder(scratch: &Scratch, who: &str) -> Child {
    let driver = scratch.driver("holder.sh", HOLDER);
    let mut command = scratch.sh(&driver);
    command.arg(who);
    let child = scratch.spawn(command, "holder");
    poll(10, "the holder to take the lock", || {
        scratch.path("ready").exists()
    });
    child
}

#[test]
fn a_second_run_waits_for_the_first_and_never_overlaps_it() {
    let scratch = Scratch::new("overlap");
    let mut first = start_holder(&scratch, "driver-a");
    let marker = scratch.read("out");
    let marker = marker
        .trim()
        .strip_prefix("held=")
        .expect("held=")
        .to_owned();
    assert!(
        marker.starts_with(scratch.path("run").to_str().unwrap()) && Path::new(&marker).is_dir(),
        "the marker names the hold directory: {marker}"
    );
    let owner =
        fs::read_to_string(scratch.path("run/celestina-nest.lock.owner")).expect("owner file");
    assert_eq!(owner.lines().count(), 1, "one line: {owner:?}");
    assert!(
        owner.starts_with(&format!("{} driver-a ", first.id())),
        "the owner line names the pid and the caller: {owner:?}"
    );

    // No patience: it fails at once and names the holder.
    let taker = scratch.driver("taker.sh", TAKER);
    let mut impatient = scratch.sh(&taker);
    impatient
        .arg("driver-b")
        .env("CELESTINA_NEST_LOCK_WAIT", "0");
    let output = impatient.output().expect("sh runs");
    assert_eq!(output.status.code(), Some(69), "{output:?}");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("driver-a"), "names the holder: {message}");
    assert!(!scratch.read("marks").contains("start-driver-b"));

    // With patience it waits (one line saying who holds it) and gets the lock
    // only once the first run has ended.
    let mut patient = scratch.sh(&taker);
    patient
        .arg("driver-b")
        .env("CELESTINA_NEST_LOCK_WAIT", "30");
    let mut second = scratch.spawn(patient, "patient");
    poll(10, "the waiting line", || {
        scratch.read("patient.err").contains("driver-a")
    });
    assert!(
        !scratch.read("marks").contains("start-driver-b"),
        "the second run started while the first held the lock"
    );
    fs::write(scratch.path("stop"), "").expect("stop file");
    assert!(wait_for_exit(&mut first, 10, "the first run").success());
    assert!(wait_for_exit(&mut second, 10, "the second run").success());
    assert_eq!(
        scratch.read("marks").lines().collect::<Vec<_>>(),
        [
            "start-driver-a",
            "end-driver-a",
            "start-driver-b",
            "end-driver-b"
        ],
        "the two runs overlapped"
    );
    let waiting = scratch.read("patient.err");
    assert_eq!(
        waiting
            .lines()
            .filter(|line| line.contains("driver-a"))
            .count(),
        1,
        "one waiting line, not a stream: {waiting}"
    );
}

#[test]
fn the_lock_is_free_within_three_seconds_of_a_killed_holder() {
    let scratch = Scratch::new("kill");
    let mut first = start_holder(&scratch, "driver-a");
    first.kill().expect("kill -9 of the driver");
    first.wait().expect("reaped");
    let taker = scratch.driver("taker.sh", TAKER);
    poll(3, "the lock to be free after kill -9", || {
        let mut attempt = scratch.sh(&taker);
        attempt.arg("driver-b").env("CELESTINA_NEST_LOCK_WAIT", "0");
        attempt.output().expect("sh runs").status.success()
    });
}

#[test]
fn a_script_started_by_a_holder_does_not_wait_for_it() {
    let scratch = Scratch::new("marker");
    let mut first = start_holder(&scratch, "driver-a");
    let marker = scratch.read("out");
    let marker = marker
        .trim()
        .strip_prefix("held=")
        .expect("held=")
        .to_owned();
    let taker = scratch.driver("taker.sh", TAKER);
    let mut inner = scratch.sh(&taker);
    inner
        .arg("driver-inner")
        .env("CELESTINA_NEST_LOCK_HELD", &marker)
        .env("CELESTINA_NEST_LOCK_WAIT", "0");
    let started = Instant::now();
    let output = inner.output().expect("sh runs");
    assert!(output.status.success(), "{output:?}");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(scratch.read("marks").contains("end-driver-inner"));
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut first, 10, "the first run");
}

#[test]
fn a_marker_that_names_no_live_hold_directory_is_ignored() {
    let scratch = Scratch::new("stale-marker");
    let mut first = start_holder(&scratch, "driver-a");
    let taker = scratch.driver("taker.sh", TAKER);
    for value in ["1", "/nonexistent/celestina-nest-hold.x"] {
        let mut command = scratch.sh(&taker);
        command
            .arg("driver-b")
            .env("CELESTINA_NEST_LOCK_HELD", value)
            .env("CELESTINA_NEST_LOCK_WAIT", "0");
        let output = command.output().expect("sh runs");
        assert_eq!(output.status.code(), Some(69), "marker {value}: {output:?}");
    }
    assert!(!scratch.read("marks").contains("start-driver-b"));
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut first, 10, "the first run");
}

/// A taker with no patience: whether it got the lock.
fn lock_is_free(scratch: &Scratch) -> bool {
    let taker = scratch.driver("probe-taker.sh", TAKER);
    let mut command = scratch.sh(&taker);
    command.arg("probe").env("CELESTINA_NEST_LOCK_WAIT", "0");
    command.output().expect("sh runs").status.success()
}

fn hold_directories(scratch: &Scratch) -> usize {
    fs::read_dir(scratch.path("run"))
        .expect("run directory")
        .filter(|entry| {
            entry.as_ref().is_ok_and(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("celestina-nest-hold.")
            })
        })
        .count()
}

#[test]
fn a_killed_outer_script_keeps_the_lock_while_its_attempt_and_nest_run() {
    let scratch = Scratch::new("orphan");
    fs::write(
        scratch.path("inner.sh"),
        r#"set -eu
. "$NEST_RUN"
nest_lock_acquire inner
trap 'echo inner-exit >> "$MARKS"' EXIT
nest 30 sh -c 'echo $$ > "$1.part"; mv "$1.part" "$1"; sleep 3' sh "$PIDFILE"
nest 30 sh -c 'echo second-nest >> "$1"' sh "$MARKS"
"#,
    )
    .expect("inner");
    let outer = scratch.driver(
        "outer.sh",
        r#"
nest_lock_acquire outer
nest_child sh "$INNER"
"#,
    );
    let mut child = scratch.spawn(scratch.sh(&outer), "outer");
    poll(10, "the nest to start", || scratch.path("pid").exists());
    let program: u32 = scratch.read("pid").trim().parse().expect("the nest's pid");
    child.kill().expect("kill -9 of the outer script");
    child.wait().expect("reaped");

    let notice = scratch.driver("notice.sh", "nest_lock_notice");
    let told = scratch.sh(&notice).output().expect("sh runs");
    assert!(
        String::from_utf8_lossy(&told.stderr).contains("script gone"),
        "an orphaned run is named: {told:?}"
    );
    let mut checks = 0;
    while !gone(program) {
        assert!(
            !lock_is_free(&scratch),
            "the lock was free while the nest still ran"
        );
        checks += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(checks >= 1, "the nest ended before it was checked");
    poll(10, "the orphaned attempt to end", || {
        scratch.read("marks").contains("inner-exit")
    });
    assert!(
        !scratch.read("marks").contains("second-nest"),
        "the orphaned attempt started another nest"
    );
    poll(5, "the lock to be free once nothing runs", || {
        lock_is_free(&scratch)
    });
    poll(5, "the hold directory to be removed", || {
        hold_directories(&scratch) == 0
    });
}

/// A driver in a session of its own, so a signal to its process group reaches
/// nothing else, with a nest that outlives the signal for three seconds.
fn spawn_group_driver(scratch: &Scratch, nest_program: &str) -> Child {
    let driver = scratch.driver(
        "group.sh",
        &format!(
            r#"
cleanup() {{ nest_lock_release; }}
nest_lock_acquire group
nest_signals cleanup
nest 30 sh -c '{nest_program}' sh "$PIDFILE"
"#
        ),
    );
    let mut command = Command::new("setsid");
    command
        .args(default_signals())
        .arg("sh")
        .arg(&driver)
        .env("NEST_RUN", scripts().join("nest-run.sh"))
        .env("XDG_RUNTIME_DIR", scratch.path("run"))
        .env("MARKS", scratch.path("marks"))
        .env("PIDFILE", scratch.path("pid"))
        .env_remove("CELESTINA_NEST_LOCK_HELD")
        .env_remove("CELESTINA_NEST_LOCK_WAIT")
        .stdin(Stdio::null());
    scratch.spawn(command, "group")
}

#[test]
fn a_kill_to_the_whole_process_group_keeps_the_lock_until_the_nest_is_gone() {
    let scratch = Scratch::new("group-kill");
    let mut child = spawn_group_driver(
        &scratch,
        r#"echo $$ > "$1.part"; mv "$1.part" "$1"; sleep 3"#,
    );
    poll(10, "the nest to start", || scratch.path("pid").exists());
    let program: u32 = scratch.read("pid").trim().parse().expect("the nest's pid");
    let status = Command::new("kill")
        .args(["-s", "KILL", "--"])
        .arg(format!("-{}", child.id()))
        .status()
        .expect("kill runs");
    assert!(status.success(), "KILL to the group");
    child.wait().expect("reaped");

    let mut checks = 0;
    while !gone(program) {
        assert!(
            !lock_is_free(&scratch),
            "the lock was free while the orphaned nest ran"
        );
        checks += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(checks >= 1, "the nest ended before it was checked");
    poll(5, "the lock to be free once the nest ended", || {
        lock_is_free(&scratch)
    });
    poll(5, "the hold directory to be removed", || {
        hold_directories(&scratch) == 0
    });
}

#[test]
fn releasing_a_lock_whose_directory_is_gone_does_not_end_the_cleanup() {
    let scratch = Scratch::new("release-gone");
    let driver = scratch.driver(
        "gone.sh",
        r#"
nest_lock_acquire gone
rm -rf "$CELESTINA_NEST_LOCK_HELD"
nest_lock_release
echo survived > "$OUT"
"#,
    );
    let output = scratch.sh(&driver).output().expect("sh runs");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(scratch.read("out").trim(), "survived");
}

#[test]
fn a_refused_nest_says_so_on_the_scripts_own_stderr() {
    let scratch = Scratch::new("refused");
    let driver = scratch.driver(
        "refused.sh",
        r#"
nest_lock_acquire refused
rm -f "$CELESTINA_NEST_LOCK_HELD/ready"
nest 5 true > /dev/null 2>&1 || exit $?
echo started > "$OUT"
"#,
    );
    let output = scratch.sh(&driver).output().expect("sh runs");
    assert_eq!(output.status.code(), Some(70), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("lock is gone"),
        "{output:?}"
    );
    assert_eq!(scratch.read("out"), "");
}

#[test]
fn a_run_without_a_private_runtime_directory_is_refused() {
    let scratch = Scratch::new("no-runtime");
    let taker = scratch.driver("taker.sh", TAKER);
    let mut command = scratch.sh(&taker);
    command.arg("driver-a").env_remove("XDG_RUNTIME_DIR");
    let output = command.output().expect("sh runs");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("XDG_RUNTIME_DIR"));
}

#[test]
fn the_next_run_removes_the_directory_of_a_waiter_that_was_killed() {
    let scratch = Scratch::new("sweep");
    let dead = scratch.path("run/celestina-nest-hold.dead00");
    fs::create_dir(&dead).expect("hold directory");
    fs::write(dead.join("owner"), "999999999\n").expect("owner");
    let taker = scratch.driver("taker.sh", TAKER);
    let output = scratch
        .sh(&taker)
        .arg("driver-a")
        .output()
        .expect("sh runs");
    assert!(output.status.success(), "{output:?}");
    assert!(!dead.exists(), "the dead waiter's directory is still there");
}

#[test]
fn a_term_to_the_whole_process_group_does_not_free_the_lock_before_the_nest_ended() {
    let scratch = Scratch::new("group-term");
    let driver = scratch.driver(
        "group.sh",
        r#"
cleanup() { nest_lock_release; }
nest_lock_acquire group
nest_signals cleanup
nest 30 sh -c 'trap "sleep 2; exit 0" TERM; echo $$ > "$1.part"; mv "$1.part" "$1"; sleep 30 & wait' sh "$PIDFILE"
"#,
    );
    let mut command = Command::new("setsid");
    command
        .args(default_signals())
        .arg("sh")
        .arg(&driver)
        .env("NEST_RUN", scripts().join("nest-run.sh"))
        .env("XDG_RUNTIME_DIR", scratch.path("run"))
        .env("MARKS", scratch.path("marks"))
        .env("PIDFILE", scratch.path("pid"))
        .env_remove("CELESTINA_NEST_LOCK_HELD")
        .env_remove("CELESTINA_NEST_LOCK_WAIT")
        .stdin(Stdio::null());
    let mut child = scratch.spawn(command, "group");
    poll(10, "the nest to start", || scratch.path("pid").exists());
    let program: u32 = scratch.read("pid").trim().parse().expect("the nest's pid");
    let status = Command::new("kill")
        .args(["-s", "TERM", "--"])
        .arg(format!("-{}", child.id()))
        .status()
        .expect("kill runs");
    assert!(status.success(), "TERM to the group");

    let mut checks = 0;
    while !gone(program) {
        assert!(
            !lock_is_free(&scratch),
            "the lock was free while the nest was stopping"
        );
        checks += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(checks >= 1, "the nest ended before it was checked");
    assert_eq!(
        wait_for_exit(&mut child, 10, "the driver").code(),
        Some(143)
    );
    poll(5, "the lock to be free once the nest ended", || {
        lock_is_free(&scratch)
    });
}

#[test]
fn a_wait_that_is_not_a_whole_number_is_a_usage_error() {
    let scratch = Scratch::new("usage");
    let taker = scratch.driver("taker.sh", TAKER);
    let mut command = scratch.sh(&taker);
    command
        .arg("driver-a")
        .env("CELESTINA_NEST_LOCK_WAIT", "soon");
    let output = command.output().expect("sh runs");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("CELESTINA_NEST_LOCK_WAIT"));
}

#[test]
fn the_smoke_takes_the_lock_before_anything_else() {
    let scratch = Scratch::new("smoke");
    let mut first = start_holder(&scratch, "driver-a");
    let smoke = scripts().join("smoke.sh");
    let run = |scratch: &Scratch, wait: &str| {
        let mut command = scratch.sh(&smoke);
        command
            .args(["--binary", "/nonexistent/celestina"])
            .env("CELESTINA_NEST_LOCK_WAIT", wait);
        command.output().expect("sh runs")
    };
    let busy = run(&scratch, "0");
    assert_eq!(busy.status.code(), Some(69), "{busy:?}");
    let message = String::from_utf8_lossy(&busy.stderr);
    assert!(message.contains("driver-a"), "names the holder: {message}");
    assert!(!message.contains("is not an executable"), "{message}");

    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut first, 10, "the holder");
    let free = run(&scratch, "30");
    assert_eq!(free.status.code(), Some(1), "{free:?}");
    assert!(
        String::from_utf8_lossy(&free.stderr).contains("is not an executable"),
        "{free:?}"
    );
}

/// A driver that runs a nest under `nest_signals`.
const NESTED: &str = r#"
cleanup() { echo ran >> "$CLEAN"; }
nest_signals cleanup
nest 30 sh -c 'sleep 30 & echo $! > "$1.part"; mv "$1.part" "$1"; wait' sh "$PIDFILE"
"#;

fn a_signal_ends_the_run_and_its_nest(name: &str, code: i32) {
    let scratch = Scratch::new(&format!("signal-{}", name.to_lowercase()));
    let driver = scratch.driver("nested.sh", NESTED);
    let mut child = scratch.spawn(scratch.sh(&driver), "nested");
    poll(10, "the nest's program to start", || {
        scratch.path("pid").exists()
    });
    let sleeper: u32 = scratch.read("pid").trim().parse().expect("the sleep's pid");
    assert!(!gone(sleeper), "the sleep is running before the signal");

    signal(name, child.id());
    let status = wait_for_exit(&mut child, 3, "the driver to end on the signal");
    assert_eq!(status.code(), Some(code), "exit status after {name}");
    poll(3, "the sleep to be gone", || gone(sleeper));
    assert_eq!(
        scratch.read("clean").lines().collect::<Vec<_>>(),
        ["ran"],
        "the cleanup runs exactly once"
    );
}

#[test]
fn term_ends_the_run_and_its_nest() {
    a_signal_ends_the_run_and_its_nest("TERM", 143);
}

#[test]
fn int_ends_the_run_and_its_nest() {
    a_signal_ends_the_run_and_its_nest("INT", 130);
}

#[test]
fn term_ends_an_attempt_the_smoke_started_in_the_background() {
    let scratch = Scratch::new("child");
    let driver = scratch.driver(
        "child.sh",
        r#"
cleanup() { echo ran >> "$CLEAN"; }
nest_signals cleanup
nest_child sh -c 'echo $$ > "$1.part"; mv "$1.part" "$1"; while :; do sleep 1; done' sh "$PIDFILE"
"#,
    );
    let mut child = scratch.spawn(scratch.sh(&driver), "child");
    poll(10, "the attempt to start", || scratch.path("pid").exists());
    let attempt: u32 = scratch
        .read("pid")
        .trim()
        .parse()
        .expect("the attempt's pid");
    signal("TERM", child.id());
    let status = wait_for_exit(&mut child, 3, "the driver to end on TERM");
    assert_eq!(status.code(), Some(143));
    assert!(gone(attempt), "the attempt outlived the driver");
    assert_eq!(scratch.read("clean").lines().count(), 1);
}

#[test]
fn an_interactive_nest_is_told_about_a_measuring_run_and_only_then() {
    let scratch = Scratch::new("notice");
    let notice = scratch.driver("notice.sh", "nest_lock_notice");
    let quiet = scratch.sh(&notice).output().expect("sh runs");
    assert!(quiet.status.success());
    assert!(quiet.stderr.is_empty(), "nothing is held: {quiet:?}");

    let mut first = start_holder(&scratch, "driver-a");
    let loud = scratch.sh(&notice).output().expect("sh runs");
    assert!(loud.status.success(), "the notice never blocks: {loud:?}");
    let message = String::from_utf8_lossy(&loud.stderr);
    assert!(
        message.contains("measuring run") && message.contains("driver-a"),
        "{message}"
    );
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut first, 10, "the holder");
}

#[test]
fn a_nest_that_ends_by_itself_returns_its_status_and_cleans_up_once() {
    let scratch = Scratch::new("status");
    let driver = scratch.driver(
        "nested.sh",
        r#"
cleanup() { echo ran >> "$CLEAN"; }
nest_signals cleanup
status=0
nest 30 FOO=bar sh -c 'test "$FOO" = bar && exit 7' || status=$?
echo "status=$status" > "$OUT"
"#,
    );
    let output = scratch.sh(&driver).output().expect("sh runs");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(scratch.read("out").trim(), "status=7");
    assert_eq!(scratch.read("clean").lines().count(), 1);
}

/// The logical statements of a script's code: continued lines joined.
fn statements(name: &str) -> Vec<String> {
    let path = scripts().join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    statements_of(&text)
}

/// The logical statements of a script's text: comments dropped, continued
/// lines joined.
fn statements_of(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open = String::new();
    for line in text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
    {
        match line.trim_end().strip_suffix('\\') {
            Some(head) => {
                open.push_str(head);
                open.push(' ');
            }
            None => {
                open.push_str(line);
                out.push(std::mem::take(&mut open));
            }
        }
    }
    out
}

/// `scripts/smoke.sh` plus every `*.sh` of `scripts/smoke/`, sorted, as
/// (path relative to `scripts/`, text). Fails when the phase directory is
/// missing or holds fewer than the 15 files of the layout (`common.sh` and
/// the 14 phases).
fn smoke_sources() -> Vec<(String, String)> {
    let dir = scripts().join("smoke");
    let entries = fs::read_dir(&dir).unwrap_or_else(|err| panic!("{}: {err}", dir.display()));
    let mut found: Vec<(String, String)> = entries
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sh"))
        .map(|path| {
            let text =
                fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            let name = path.file_name().expect("a file name").to_string_lossy();
            (format!("smoke/{name}"), text)
        })
        .collect();
    assert!(
        found.len() >= 15,
        "scripts/smoke/ holds {} .sh files; the layout needs common.sh and 14 phases",
        found.len()
    );
    let driver = scripts().join("smoke.sh");
    found.push((
        "smoke.sh".to_owned(),
        fs::read_to_string(&driver).unwrap_or_else(|err| panic!("{}: {err}", driver.display())),
    ));
    found.sort();
    found
}

/// Whether a statement sources the phases' shared file.
fn sources_common(statement: &str) -> bool {
    let statement = statement.trim_start();
    statement.starts_with(". ") && statement.contains("smoke/common.sh")
}

/// The rules every script that starts nests keeps, checked on its statements:
/// `dev-session.sh` only as `nest LIMIT …` with no `timeout` of its own, no
/// INT or TERM trap, and `nest` only as a plain statement. Returns how many
/// statements start a dev session.
fn check_nest_statements(name: &str, statements: &[String]) -> usize {
    let mut starts = 0;
    for statement in statements {
        if statement.contains("dev-session.sh") {
            starts += 1;
            let words: Vec<&str> = statement.split_whitespace().collect();
            assert!(
                words.first() == Some(&"nest"),
                "{name} starts dev-session.sh other than as `nest LIMIT …`: {statement}"
            );
            assert!(
                !words.contains(&"timeout"),
                "{name} still wraps a nest in timeout itself: {statement}"
            );
        }
        if statement.trim_start().starts_with("trap ") {
            assert!(
                !statement.contains("INT") && !statement.contains("TERM"),
                "{name} installs its own signal trap (nest_signals owns them): {statement}"
            );
        }
        for bad in ["| nest ", "(nest ", "$(nest ", "`nest "] {
            assert!(
                !statement.contains(bad),
                "{name}: nest must be a plain statement of the main shell: {statement}"
            );
        }
    }
    starts
}

/// The index of the first statement whose first word is `word`.
fn first_call(statements: &[String], word: &str) -> Option<usize> {
    statements
        .iter()
        .position(|s| s.split_whitespace().next() == Some(word))
}

fn installs_cleanup(statements: &[String]) -> bool {
    statements
        .iter()
        .any(|s| s.trim() == "nest_signals cleanup")
}

#[test]
fn the_nest_scripts_start_dev_session_only_through_nest_and_own_no_signal_trap() {
    let mut phase_starts = 0;
    for (name, text) in smoke_sources() {
        let statements = statements_of(&text);
        let starts = check_nest_statements(&name, &statements);

        // The lock comes before any nest. A phase file reaches it through
        // `smoke/common.sh`, which takes it; this is how the rule is read: the
        // statement that sources `common.sh` stands for common.sh's
        // statements at that point, so common.sh itself must hold the
        // `nest_lock_acquire ` statement (and start no nest before it), and
        // each phase must source it before its first `nest `.
        let first_lock = first_call(&statements, "nest_lock_acquire");
        let first_nest = first_call(&statements, "nest");
        match name.as_str() {
            "smoke.sh" => {
                assert_eq!(
                    starts, 0,
                    "smoke.sh is a driver; the dev sessions belong to the phase files"
                );
                assert!(
                    installs_cleanup(&statements),
                    "smoke.sh must install its traps with nest_signals cleanup"
                );
                let first_lock = first_lock.expect("smoke.sh never takes the nest lock");
                let first_child =
                    first_call(&statements, "nest_child").expect("smoke.sh never calls nest_child");
                assert!(
                    first_lock < first_child,
                    "smoke.sh starts an attempt before it takes the lock"
                );
            }
            "smoke/common.sh" => {
                let first_lock = first_lock.expect("smoke/common.sh never takes the nest lock");
                assert!(
                    first_nest.is_none_or(|nest| first_lock < nest),
                    "smoke/common.sh starts a nest before it takes the lock"
                );
            }
            _ => {
                phase_starts += starts;
                let sourced = statements
                    .iter()
                    .position(|s| sources_common(s))
                    .unwrap_or_else(|| panic!("{name} never sources smoke/common.sh"));
                assert!(
                    first_nest.is_none_or(|nest| sourced < nest),
                    "{name} starts a nest before it sources smoke/common.sh (the lock)"
                );
            }
        }
    }
    assert!(
        phase_starts >= 1,
        "the phase files start no dev session at all"
    );

    // The probe is one file: it starts its nests and takes the lock itself.
    let script = "wallpaper-backdrop-probe.sh";
    let statements = statements(script);
    let starts = check_nest_statements(script, &statements);
    assert!(starts >= 1, "{script} starts no nest at all");
    assert!(
        installs_cleanup(&statements),
        "{script} must install its traps with nest_signals cleanup"
    );
    let first_lock = first_call(&statements, "nest_lock_acquire")
        .unwrap_or_else(|| panic!("{script} never takes the nest lock"));
    let first_nest =
        first_call(&statements, "nest").unwrap_or_else(|| panic!("{script} never calls nest"));
    assert!(
        first_lock < first_nest,
        "{script} starts a nest before it takes the lock"
    );
}

#[test]
fn only_the_outer_drivers_take_the_lock() {
    let takes = |name: &str| {
        statements(name)
            .iter()
            .any(|s| s.trim_start().starts_with("nest_lock_acquire "))
    };
    assert!(takes("smoke.sh"));
    assert!(takes("wallpaper-backdrop-probe.sh"));
    assert!(takes("lock-rehearsal.sh"));
    assert!(
        statements("lock-rehearsal.sh")
            .iter()
            .any(|s| s.trim_start().starts_with("nest_register ")),
        "the rehearsal's nests must be in the registry"
    );
    assert!(
        !takes("dev-session.sh"),
        "an interactive nest never blocks a landing"
    );
    assert!(
        !takes("smoke-media.sh"),
        "a client run inside a nest takes no lock"
    );
}

const PHASES: [&str; 14] = [
    "notify",
    "launcher",
    "bar",
    "wallpaper",
    "energy",
    "glance",
    "clipboard",
    "connectivity",
    "osd",
    "phone",
    "media",
    "overview",
    "polkit",
    "cutover",
];

/// `sh scripts/smoke.sh ARGS` with the lock held by someone else and no wait:
/// a smoke that took the lock would end with 69. Runs while `holder` holds it.
fn smoke_while_locked(scratch: &Scratch, args: &[&str]) -> std::process::Output {
    let mut command = scratch.sh(&scripts().join("smoke.sh"));
    command
        .args(args)
        .env("CELESTINA_NEST_LOCK_WAIT", "0")
        .output()
        .expect("sh runs")
}

#[test]
fn the_smoke_lists_its_phases() {
    let scratch = Scratch::new("list");
    let mut holder = start_holder(&scratch, "driver-a");
    let output = smoke_while_locked(&scratch, &["--list"]);
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut holder, 10, "the holder");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let listed: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(listed, PHASES, "{output:?}");
}

#[test]
fn the_smoke_refuses_an_unknown_phase() {
    let scratch = Scratch::new("unknown");
    let mut holder = start_holder(&scratch, "driver-a");
    let output = smoke_while_locked(
        &scratch,
        &["--binary", "/nonexistent/celestina", "--only", "nope"],
    );
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut holder, 10, "the holder");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("nope"), "names the phase: {message}");
    assert!(
        !message.contains("driver-a"),
        "it asked for the lock: {message}"
    );
}

#[test]
fn the_smoke_refuses_an_empty_or_repeated_selection() {
    let scratch = Scratch::new("selection");
    let mut holder = start_holder(&scratch, "driver-a");
    for (selection, named) in [("", "--only"), ("bar,bar", "bar")] {
        let output = smoke_while_locked(
            &scratch,
            &["--binary", "/nonexistent/celestina", "--only", selection],
        );
        assert_eq!(output.status.code(), Some(2), "{selection:?}: {output:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(
            message.contains(named),
            "{selection:?} is refused without naming {named}: {message}"
        );
        assert!(
            !message.contains("driver-a"),
            "it asked for the lock: {message}"
        );
    }
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut holder, 10, "the holder");
}

#[test]
fn every_listed_phase_has_its_file() {
    let scratch = Scratch::new("files");
    let output = scratch
        .sh(&scripts().join("smoke.sh"))
        .arg("--list")
        .output()
        .expect("sh runs");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let listed: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(!listed.is_empty(), "--list printed nothing: {output:?}");
    let mut expected: Vec<String> = listed
        .iter()
        .map(|id| {
            let file = format!("{id}.sh");
            let path = scripts().join("smoke").join(&file);
            let text =
                fs::read_to_string(&path).unwrap_or_else(|_| panic!("no scripts/smoke/{file}"));
            // The file names its own id; it is what the refusal and the
            // driver's retry hook read.
            assert!(
                text.lines().any(|line| line == format!("SMOKE_PHASE={id}")),
                "scripts/smoke/{file} does not say SMOKE_PHASE={id}"
            );
            file
        })
        .collect();
    expected.push("common.sh".to_owned());
    expected.sort();
    let dir = scripts().join("smoke");
    let mut present: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    present.sort();
    assert_eq!(present, expected, "scripts/smoke/ holds other files");
}

#[test]
fn a_phase_refuses_to_run_outside_the_driver() {
    let scratch = Scratch::new("phase");
    let output = scratch
        .sh(&scripts().join("smoke/bar.sh"))
        .env_remove("SMOKE_PHASE_WORK")
        .output()
        .expect("sh runs");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("run it through smoke.sh --only bar"),
        "{output:?}"
    );
}

/// What `smoke.sh --next-phase SECOND IDS` picks: the phase that would wait
/// least at that second of the minute for its first window; on a tie, the
/// narrowest window, a phase with no window counting as the widest; then the
/// order of `--list`.
fn next_phase(scratch: &Scratch, second: &str, ids: &str) -> std::process::Output {
    smoke_while_locked(scratch, &["--next-phase", second, ids])
}

#[test]
fn the_smoke_runs_next_the_phase_that_waits_least() {
    let scratch = Scratch::new("next");
    let mut holder = start_holder(&scratch, "driver-a");
    for (second, ids, expected) in [
        // clipboard fits now (2..22) and is the narrowest of those that fit.
        ("10", "clipboard,launcher,bar", "clipboard"),
        // clipboard would wait 32 s; bar fits (2..48) and is narrower than a
        // phase with no window.
        ("30", "clipboard,launcher,bar", "bar"),
        // Nothing with a window fits: a phase with none fills the gap.
        ("30", "clipboard,launcher", "launcher"),
        // Both wait 12 s for the next minute: the narrower window first.
        ("50", "bar,clipboard", "clipboard"),
        // Both wait 1 s: the narrower window first.
        ("1", "notify,cutover", "cutover"),
        // Neither has a window: the order of --list.
        ("50", "polkit,osd", "osd"),
        // A second written with a leading zero is the same second.
        ("05", "launcher,clipboard", "clipboard"),
        ("09", "launcher,clipboard", "clipboard"),
        // Second 0: every window opens at 2, so a phase with none goes first.
        ("0", "clipboard,launcher", "launcher"),
        // One past a window (49 for bar): it waits for the next minute.
        ("49", "bar,launcher", "launcher"),
        // Second 59: both wait 3 s, the narrower first.
        ("59", "bar,clipboard", "clipboard"),
    ] {
        let output = next_phase(&scratch, second, ids);
        assert_eq!(output.status.code(), Some(0), "{second} {ids}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "at second {second} out of {ids}"
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("driver-a"),
            "it asked for the lock: {output:?}"
        );
    }
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut holder, 10, "the holder");
}

#[test]
fn every_phase_with_a_minute_wait_declares_its_window() {
    // A phase whose first wait the driver cannot read would be scheduled as
    // one with no window. At second 55 no window is open, so a phase with a
    // window loses to one without: if it wins, its window was not read.
    let scratch = Scratch::new("windows");
    let mut holder = start_holder(&scratch, "driver-a");
    for id in PHASES {
        let text = fs::read_to_string(scripts().join("smoke").join(format!("{id}.sh")))
            .expect("phase file");
        if !text.contains("$(date +%S)") {
            continue;
        }
        let output = next_phase(&scratch, "55", &format!("{id},polkit"));
        assert_eq!(output.status.code(), Some(0), "{id}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "polkit",
            "{id} waits for the minute but the driver read no window for it"
        );
    }
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut holder, 10, "the holder");
}

#[test]
fn the_smoke_refuses_a_bad_order_or_choice() {
    let scratch = Scratch::new("order");
    let mut holder = start_holder(&scratch, "driver-a");
    let binary = ["--binary", "/nonexistent/celestina"];
    let cases: [(&[&str], &str); 8] = [
        (&["--order", "nope"], "nope"),
        (&["--order", "bar,bar"], "bar"),
        (&["--order", ""], "--order"),
        (&["--order", "bar", "--only", "osd"], "usage"),
        (&["--next-phase", "60", "bar"], "second"),
        (&["--next-phase", "x", "bar"], "second"),
        (&["--next-phase", "008", "bar"], "second"),
        (&["--next-phase", "10", "nope"], "nope"),
    ];
    for (args, named) in cases {
        let mut all: Vec<&str> = Vec::new();
        if args[0] != "--next-phase" {
            all.extend(binary);
        }
        all.extend(args);
        let output = smoke_while_locked(&scratch, &all);
        assert_eq!(output.status.code(), Some(2), "{all:?}: {output:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(
            message.contains(named),
            "{all:?} does not name {named}: {message}"
        );
        assert!(
            !message.contains("driver-a"),
            "it asked for the lock: {message}"
        );
    }
    fs::write(scratch.path("stop"), "").expect("stop file");
    wait_for_exit(&mut holder, 10, "the holder");
}
