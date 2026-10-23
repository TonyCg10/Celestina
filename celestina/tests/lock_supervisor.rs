//! The lock supervisor against fake lock binaries (CEL-2-H design §4.3 (b),
//! §4.4, §9.1: T-S2, T-Z1, T-E1/T-E3 on the shell's side, the watchdog kill).
//!
//! The "lock" here is a copy of `bash` named `celestina-lock` in a directory
//! of the test's own, running a script that prints status lines and exits.
//! Nothing talks to a compositor, the session bus or the system bus; no
//! process is killed by name. Children are ended by their own script or by the
//! pid they record.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina::lock::supervisor::{spawn, Config, Supervisor, Suspender};
use celestina_lock_proto::{decode, AttemptKind, FileMeta, LockExit, Status, StatusLine};
use celestina_shell::lock::{Action, ChildExit, Event, LockPolicy, LockedProof};

const WAIT: Duration = Duration::from_secs(10);
const BASH: &str = "/usr/bin/bash";

/// A directory with a fake `celestina-lock` and a fake verifier.
struct Rig {
    dir: PathBuf,
}

impl Rig {
    /// A rig whose binaries are links to the system's: nothing is written, so
    /// no concurrent `fork` of another test can hold a write descriptor open
    /// on a file this one executes (`ETXTBSY`).
    fn new(name: &str) -> Rig {
        let rig = Rig::bare(name);
        std::os::unix::fs::symlink(BASH, rig.dir.join("celestina-lock")).expect("a fake lock");
        std::os::unix::fs::symlink("/usr/bin/true", rig.dir.join("celestina-lock-verify"))
            .expect("a fake verifier");
        rig
    }

    /// A rig whose binaries are copies, for tests that change their modes or
    /// replace them and never execute the copy.
    fn copied(name: &str) -> Rig {
        let rig = Rig::bare(name);
        fs::copy(BASH, rig.dir.join("celestina-lock")).expect("a fake lock");
        fs::copy("/usr/bin/true", rig.dir.join("celestina-lock-verify")).expect("a verifier");
        for name in ["celestina-lock", "celestina-lock-verify"] {
            fs::set_permissions(rig.dir.join(name), fs::Permissions::from_mode(0o755))
                .expect("mode");
        }
        rig
    }

    fn bare(name: &str) -> Rig {
        let dir = std::env::temp_dir().join(format!(
            "celestina-lock-supervisor-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Rig { dir }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn config(&self, script: &str) -> Config {
        Config {
            dir: self.dir.clone(),
            lock_file: "celestina-lock",
            verifier_file: "celestina-lock-verify",
            args: vec!["-c".into(), script.into()],
            env: vec![
                ("WAYLAND_DISPLAY".into(), "wayland-test".into()),
                ("XDG_RUNTIME_DIR".into(), self.dir.clone().into()),
                ("MESA_SHADER_CACHE_DISABLE".into(), "true".into()),
                ("CELESTINA_LOCK_LAUNCHER".into(), "shell".into()),
            ],
            // The production service file is "installed".
            pam_existing: Arc::new(|| vec![PathBuf::from("/etc/pam.d/celestina-lock")]),
        }
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct NoSuspend;

impl Suspender for NoSuspend {
    fn suspend(&self, _proof: LockedProof) -> Result<(), String> {
        Ok(())
    }
}

/// A started supervisor and the events it published.
struct Run {
    supervisor: Option<Supervisor>,
    events: Receiver<Event>,
    _policy: LockPolicy,
}

fn start(config: Config) -> Run {
    let policy = LockPolicy::new();
    let (sent, events) = channel();
    let supervisor = spawn(
        config,
        move |event| sent.send(event).is_ok(),
        Box::new(NoSuspend),
        policy.generation_reader(),
    )
    .expect("the worker starts");
    Run {
        supervisor: Some(supervisor),
        events,
        _policy: policy,
    }
}

impl Run {
    fn handle(&self) -> &celestina::lock::supervisor::SupervisorHandle {
        self.supervisor.as_ref().expect("running").handle()
    }

    fn next(&self) -> Event {
        self.events.recv_timeout(WAIT).expect("an event in time")
    }

    /// Events up to and including the first `Exit`.
    fn until_exit(&self) -> Vec<Event> {
        let mut seen = Vec::new();
        loop {
            let event = self.next();
            let done = matches!(event, Event::Exit(_));
            seen.push(event);
            if done {
                return seen;
            }
        }
    }

    fn stop(&mut self) {
        if let Some(supervisor) = self.supervisor.take() {
            supervisor.stop();
        }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Waits until the process whose pid `file` records is gone: reaped by the
/// worker, since a zombie still has its `/proc` entry.
fn gone_soon(file: &Path) {
    let end = Instant::now() + WAIT;
    let pid = loop {
        if let Ok(text) = fs::read_to_string(file) {
            if let Some(line) = text.lines().last() {
                break line.trim().to_owned();
            }
        }
        assert!(Instant::now() < end, "no pid was recorded");
        std::thread::sleep(Duration::from_millis(20));
    };
    while Path::new(&format!("/proc/{pid}")).exists() {
        assert!(Instant::now() < end, "the child {pid} was never reaped");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A script that records its pid, runs `before` and then sleeps.
fn recording(rig: &Rig, before: &str) -> String {
    format!(
        "echo $$ >> {}; {before} exec sleep 30",
        rig.file("pids").display()
    )
}

/// Kills the held child and checks the policy hears nothing of it, however
/// long the reaping takes (finding I-B); returns once it is reaped.
fn kill_and_expect_silence(run: &Run, rig: &Rig) {
    run.handle().kill_child().expect("queued");
    gone_soon(&rig.file("pids"));
    assert!(
        run.events.recv_timeout(Duration::from_millis(300)).is_err(),
        "a killed child's exit or lines must never reach the policy"
    );
}

/// Waits until `path` exists.
fn exists_soon(path: &Path) {
    let end = Instant::now() + WAIT;
    while !path.exists() {
        assert!(Instant::now() < end, "{} never appeared", path.display());
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_worker_pins_the_binaries_and_asks_for_nothing_until_told() {
    let rig = Rig::new("pin");
    let run = start(rig.config("exit 0"));
    assert_eq!(run.next(), Event::Pinned(true));
    // The PAM file exists (as far as the injected check goes): no notice.
    assert!(run.events.recv_timeout(Duration::from_millis(300)).is_err());
}

#[test]
fn a_missing_pam_file_is_reported_after_pinning_and_does_not_unpin() {
    let rig = Rig::new("pam");
    let mut config = rig.config("exit 0");
    config.pam_existing = Arc::new(Vec::new);
    let run = start(config);
    assert_eq!(run.next(), Event::Pinned(true));
    assert_eq!(run.next(), Event::PamFileMissing);
}

#[test]
fn a_script_or_a_non_executable_file_is_refused_at_the_pin() {
    // A script is what the pin exists to refuse: the kernel would hand it to
    // its interpreter through a path that no longer exists.
    let rig = Rig::copied("script");
    fs::write(rig.file("celestina-lock"), "#!/bin/sh\nexit 0\n").expect("a script");
    fs::set_permissions(
        rig.file("celestina-lock"),
        fs::Permissions::from_mode(0o755),
    )
    .expect("mode");
    assert_eq!(start(rig.config("exit 0")).next(), Event::Pinned(false));

    let rig = Rig::copied("modes");
    for mode in [0o644, 0o775, 0o757, 0o4755] {
        fs::set_permissions(rig.file("celestina-lock"), fs::Permissions::from_mode(mode))
            .expect("mode");
        assert_eq!(
            start(rig.config("exit 0")).next(),
            Event::Pinned(false),
            "mode {mode:o}"
        );
    }
    // A missing verifier is no better.
    fs::set_permissions(
        rig.file("celestina-lock"),
        fs::Permissions::from_mode(0o755),
    )
    .expect("mode");
    fs::remove_file(rig.file("celestina-lock-verify")).expect("remove");
    assert_eq!(start(rig.config("exit 0")).next(), Event::Pinned(false));
}

#[test]
fn t_s2_line_parsing_bounds_and_the_exit_table() {
    let rig = Rig::new("lines");
    // Lines of the protocol; a 64-byte line, a 65-byte line, a line with an
    // embedded NUL and an unknown one (all dropped, the stream resynchronises
    // on the next line); then an exit.
    let script = r#"
        echo 'hello 1'
        echo requested
        printf 'attempt %s\n' refused
        printf '%064d\n' 0
        printf '%065d\n' 0
        printf 'locked\0\n'
        echo 'who knows'
        printf 'attempt timeout\n'
        echo locked
        echo unlocked
        exit 0
    "#;
    let run = start(rig.config(script));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(
        run.until_exit(),
        vec![
            Event::Line(StatusLine::Hello(1)),
            Event::Line(StatusLine::Requested),
            Event::Line(StatusLine::Attempt(AttemptKind::Refused)),
            Event::Line(StatusLine::Attempt(AttemptKind::Timeout)),
            Event::Line(StatusLine::Locked),
            Event::Line(StatusLine::Unlocked),
            Event::Exit(ChildExit::Status(LockExit::Unlocked)),
        ]
    );
}

#[test]
fn a_line_without_its_newline_is_never_acted_on() {
    // Review Focus 4: a line counts only once its terminator arrived. The
    // child ends in the middle of `unlocked`: nothing is read from it.
    let rig = Rig::new("nonewline");
    let run = start(rig.config("echo requested; printf unlocked; exit 0"));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(
        run.until_exit(),
        vec![
            Event::Line(StatusLine::Requested),
            Event::Exit(ChildExit::Status(LockExit::Unlocked)),
        ]
    );
}

#[test]
fn every_exit_of_the_table_and_a_signal_are_read() {
    let rig = Rig::new("exits");
    let table = [
        (0, ChildExit::Status(LockExit::Unlocked)),
        (1, ChildExit::Status(LockExit::EarlyError)),
        (2, ChildExit::Status(LockExit::Usage)),
        (10, ChildExit::Status(LockExit::FinishedBeforeLocked)),
        (11, ChildExit::Status(LockExit::NoLockManager)),
        (12, ChildExit::Status(LockExit::NoConnection)),
        (13, ChildExit::Status(LockExit::FinishedAfterLocked)),
        (15, ChildExit::Status(LockExit::SetupFailed)),
        (16, ChildExit::Status(LockExit::ConnectionLost)),
        // Anything else after `requested` means the lock died.
        (3, ChildExit::Died),
        (127, ChildExit::Died),
        (255, ChildExit::Died),
    ];
    for (code, expected) in table {
        let run = start(rig.config(&format!("exit {code}")));
        assert_eq!(run.next(), Event::Pinned(true));
        run.handle().spawn_child(0, Duration::ZERO).expect("queued");
        assert_eq!(run.until_exit(), vec![Event::Exit(expected)], "exit {code}");
    }
    // A signal.
    let run = start(rig.config("kill -KILL $$"));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.until_exit(), vec![Event::Exit(ChildExit::Died)]);
}

#[test]
fn the_lines_written_just_before_an_exit_arrive_before_it() {
    let rig = Rig::new("lastlines");
    let run = start(rig.config("echo locked; echo unlocked; exit 0"));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    let seen = run.until_exit();
    assert_eq!(
        seen,
        vec![
            Event::Line(StatusLine::Locked),
            Event::Line(StatusLine::Unlocked),
            Event::Exit(ChildExit::Status(LockExit::Unlocked)),
        ]
    );
}

#[test]
fn a_delayed_spawn_waits_its_delay() {
    let rig = Rig::new("delay");
    let run = start(rig.config("echo requested; exit 0"));
    assert_eq!(run.next(), Event::Pinned(true));
    let asked = Instant::now();
    run.handle()
        .spawn_child(0, Duration::from_millis(400))
        .expect("queued");
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    assert!(
        asked.elapsed() >= Duration::from_millis(400),
        "started after {:?}",
        asked.elapsed()
    );
}

#[test]
fn spawn_uses_a_process_group_env_clear_and_the_allowlist() {
    let rig = Rig::new("env");
    let out = rig.file("environ");
    let group = rig.file("group");
    // The environment as the kernel handed it over (no shell additions), and
    // whether the child leads a process group of its own.
    let script = format!(
        "tr '\\0' '\\n' < /proc/$$/environ | sort > {out}; \
         ps -o pid=,pgid= -p $$ > {group}; echo requested; exit 0",
        out = out.display(),
        group = group.display()
    );
    let run = start(rig.config(&script));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    run.until_exit();
    let environ = fs::read_to_string(&out).expect("the child wrote its environment");
    let mut expected = vec![
        "CELESTINA_LOCK_LAUNCHER=shell".to_owned(),
        "MESA_SHADER_CACHE_DISABLE=true".to_owned(),
        "WAYLAND_DISPLAY=wayland-test".to_owned(),
        format!("XDG_RUNTIME_DIR={}", rig.dir.display()),
    ];
    expected.sort();
    let found: Vec<&str> = environ.lines().collect();
    assert_eq!(found, expected, "the whole environment, nothing else");
    // The child is its own group leader: pid == pgid.
    let ids = fs::read_to_string(&group).expect("the child wrote its group");
    let mut ids = ids.split_whitespace();
    let (pid, pgid) = (ids.next(), ids.next());
    assert!(pid.is_some() && pid == pgid, "{pid:?} {pgid:?}");
    // And the shell's own group is not that one.
    let own = fs::read_to_string("/proc/self/stat").expect("stat");
    let own_group = own
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().nth(2).map(str::to_owned));
    assert_ne!(own_group.as_deref(), pgid);
}

#[test]
fn the_pinned_descriptor_is_what_executes() {
    let rig = Rig::new("pinned");
    let run = start(rig.config("echo requested; exit 0"));
    assert_eq!(run.next(), Event::Pinned(true));
    // A deploy replaces the file on disk after the pin: a different program
    // that would print something else and exit 1.
    fs::copy("/usr/bin/false", rig.file("replacement")).expect("a replacement");
    fs::set_permissions(rig.file("replacement"), fs::Permissions::from_mode(0o755)).expect("mode");
    fs::rename(rig.file("replacement"), rig.file("celestina-lock")).expect("replace");
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    // The checked bytes ran (the script of the original bash), not the new
    // file (`false` would exit 1 with no line).
    assert_eq!(
        run.until_exit(),
        vec![
            Event::Line(StatusLine::Requested),
            Event::Exit(ChildExit::Status(LockExit::Unlocked)),
        ]
    );
}

#[test]
fn t_z1_the_memfd_offset_is_0_at_spawn_and_the_childs_first_read_returns_the_magic() {
    let rig = Rig::new("bundle");
    let dump = rig.file("stdin");
    // `cat` copies stdin from wherever its offset is: the whole file only if
    // the offset was 0.
    let run = start(rig.config(&format!("cat > {}; exit 0", dump.display())));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(3, Duration::ZERO).expect("queued");
    run.until_exit();
    let bytes = fs::read(&dump).expect("the child read its stdin");
    assert_eq!(&bytes[..4], b"CLKB", "the first read is the magic");
    let size = u64::try_from(bytes.len()).expect("len");
    let decoded = decode(
        &bytes,
        FileMeta {
            size,
            seals_ok: true,
        },
    );
    assert_eq!(decoded.status, Status::Complete);
    assert_eq!(decoded.bundle.prior_failures, 3, "the failures are carried");
    assert!(decoded.bundle.outputs.is_empty(), "header only in H-a");
    assert!(!decoded.bundle.offset_known);
}

#[test]
fn a_kill_request_ends_a_child_that_never_asked_to_lock() {
    let rig = Rig::new("kill");
    let run = start(rig.config(&recording(&rig, "")));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    // Nothing yet: it is asleep.
    assert!(run.events.recv_timeout(Duration::from_millis(300)).is_err());
    kill_and_expect_silence(&run, &rig);
}

#[test]
fn the_policy_kills_a_silent_spawn_after_5_s_and_leaves_a_requested_one_alone() {
    // The policy and the worker together, on a clock the test moves.
    let rig = Rig::new("watchdog");
    let run = start(rig.config(&recording(&rig, "")));
    assert_eq!(run.next(), Event::Pinned(true));
    let mut policy = LockPolicy::new();
    let t0 = Instant::now();
    policy.handle(t0, Event::Duty(true));
    policy.handle(t0, Event::Pinned(true));
    let actions = policy.handle(t0, Event::Lock);
    assert!(actions
        .iter()
        .any(|action| matches!(action, Action::Spawn { .. })));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    let late = policy.handle(t0 + Duration::from_secs(6), Event::Tick);
    assert!(late.contains(&Action::KillChild), "{late:?}");
    // The child must have written its pid before it is killed.
    exists_soon(&rig.file("pids"));
    kill_and_expect_silence(&run, &rig);

    // A child that said `requested` is not killed by the 10 s watchdog.
    let rig = Rig::new("watchdog-requested");
    let run = start(rig.config(&recording(&rig, "echo requested;")));
    assert_eq!(run.next(), Event::Pinned(true));
    let mut policy = LockPolicy::new();
    policy.handle(t0, Event::Duty(true));
    policy.handle(t0, Event::Pinned(true));
    policy.handle(t0, Event::Lock);
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    policy.handle(t0, Event::Line(StatusLine::Requested));
    let late = policy.handle(t0 + Duration::from_secs(11), Event::Tick);
    assert!(!late.contains(&Action::KillChild), "{late:?}");
    assert!(run.events.recv_timeout(Duration::from_millis(300)).is_err());
    // The test ends its own child through the worker's pidfd, not by name.
    kill_and_expect_silence(&run, &rig);
}

#[test]
fn the_child_outlives_the_supervisor_and_ignores_epipe() {
    let rig = Rig::new("outlives");
    let pid_file = rig.file("pid");
    let go = rig.file("go");
    let wrote = rig.file("wrote");
    let done = rig.file("done");
    let script = format!(
        "trap '' PIPE; echo $$ > {pid}; echo 'hello 1'; echo requested; \
         while [ ! -e {go} ]; do sleep 0.02; done; \
         echo locked; echo wrote > {wrote}; \
         while [ ! -e {done} ]; do sleep 0.02; done; exit 0",
        pid = pid_file.display(),
        go = go.display(),
        wrote = wrote.display(),
        done = done.display()
    );
    let mut run = start(rig.config(&script));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.next(), Event::Line(StatusLine::Hello(1)));
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    exists_soon(&pid_file);
    let pid: i32 = fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .parse()
        .expect("a pid");
    // The shell's side goes away: the worker stops, the pipe's reader with it.
    run.stop();
    // The child writes `locked` into the closed pipe (EPIPE) and carries on.
    fs::write(&go, b"").expect("go");
    exists_soon(&wrote);
    assert!(
        Path::new(&format!("/proc/{pid}")).exists(),
        "the lock survived the supervisor"
    );
    // End it the way its own script does, then wait for it to be gone.
    fs::write(&done, b"").expect("done");
    let end = Instant::now() + WAIT;
    while Path::new(&format!("/proc/{pid}")).exists()
        && !fs::read_to_string(format!("/proc/{pid}/stat"))
            .map(|stat| stat.contains(") Z"))
            .unwrap_or(true)
    {
        assert!(Instant::now() < end, "the lock never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_spawn_while_a_child_is_held_is_a_failure_not_a_second_child() {
    let rig = Rig::new("second");
    let run = start(rig.config(&recording(&rig, "echo requested;")));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.next(), Event::SpawnFailed);
    kill_and_expect_silence(&run, &rig);
}

#[test]
fn the_worker_stops_within_the_bound() {
    let rig = Rig::new("stop");
    let mut run = start(rig.config("exit 0"));
    assert_eq!(run.next(), Event::Pinned(true));
    let started = Instant::now();
    run.stop();
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_spawn_after_the_killed_childs_reaping_starts_and_the_old_exit_is_never_told() {
    // Finding I-B, the order that broke: Kill, the worker reaps and (before
    // the fix) publishes the old child's exit, then the Spawn arrives and
    // starts; the late exit met the policy in `Spawning` and ended it.
    let rig = Rig::new("kill-then-spawn");
    let run = start(rig.config(&recording(&rig, "echo requested;")));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    run.handle().kill_child().expect("queued");
    // Long enough for the reaping and any publication.
    std::thread::sleep(Duration::from_millis(600));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    // The first event after the kill is the NEW child's line, not an exit.
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    kill_and_expect_silence(&run, &rig);
}

#[test]
fn a_spawn_queued_behind_the_kill_starts_after_the_reaping_without_any_exit() {
    // Finding M-d: the other order, the Spawn waits in the queue behind the
    // killed child and starts once it is reaped.
    let rig = Rig::new("spawn-behind-kill");
    let run = start(rig.config(&recording(&rig, "echo requested;")));
    assert_eq!(run.next(), Event::Pinned(true));
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    run.handle().kill_child().expect("queued");
    run.handle().spawn_child(0, Duration::ZERO).expect("queued");
    assert_eq!(run.next(), Event::Line(StatusLine::Requested));
    let pids = fs::read_to_string(rig.file("pids")).expect("pids");
    assert_eq!(pids.lines().count(), 2, "a second child ran: {pids:?}");
    kill_and_expect_silence(&run, &rig);
}
