// language-contract: allow-non-english
// (the tests type international input: accents, symbols, emoji)
//! One authentication attempt: the verifier child, its deadline, and the only
//! place that can say "authenticated" (design §4.3 (c), §4.5, §4.10, §6 items
//! 1, 2 and 13, TM-1, TM-2, TM-8, TM-9, TM-10, TM-21).
//!
//! * [`PinnedBinary`] holds the verifier open from the moment the lock starts.
//!   The bytes that run are the bytes that were checked: the child is
//!   executed from `/proc/self/fd/N` of that descriptor, so replacing the file
//!   on disk changes nothing, and a script (which the kernel would hand to its
//!   interpreter through a path that no longer exists) is refused at the pin.
//! * [`Attempts`] runs one attempt at a time on a worker thread: spawn with
//!   a cleared environment and a process group of its own, write the secret,
//!   close, wait on the child's pidfd up to the deadline, then kill the pidfd
//!   and the group. The verdict reaches the loop as soon as the kill is sent;
//!   the reap then waits at most [`Timings::reap_wait`] and no new attempt
//!   starts before the old child is reaped.
//! * [`Authenticated`] is a zero-sized token whose only constructor is private
//!   to this module and is called from [`classify`] for exit status 0 of the
//!   child this module spawned. The unlock site ([`crate::unlock`]) takes it.
//!
//! A write error on the child's stdin (it exited without reading) is not a
//! verdict: the exit status is. Every kill targets the child's pidfd or its
//! process group, both of the child this worker spawned and has not reaped, so
//! neither can have been reused.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use celestina_lock_proto::{
    AttemptKind, Service, Verdict, ATTEMPT_DEADLINE, PROTOCOL_VERSION, REAP_WAIT,
};
use rustix::event::{poll, PollFd, PollFlags};
use rustix::process::{
    getuid, kill_process_group, pidfd_open, pidfd_send_signal, Pid, PidfdFlags, Signal,
};
use rustix::time::Timespec;

use crate::field::Secret;

/// The verifier's `PATH`, and its whole environment (design §4.2).
const VERIFIER_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/bin";

/// The proof that the verifier this module spawned for an attempt exited with
/// status 0. Zero-sized, not `Clone`, not `Copy`, no `Default`, and its only
/// constructor is [`Authenticated::mint`], private to this module and called
/// from [`classify`] alone. [`crate::unlock::unlock`] consumes it.
#[must_use = "the token is the right to unlock, once"]
pub struct Authenticated(());

impl Authenticated {
    fn mint() -> Self {
        Self(())
    }
}

impl fmt::Debug for Authenticated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Authenticated")
    }
}

/// Why an attempt did not authenticate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Denial {
    /// The password was wrong (counts for the delay).
    Refused,
    /// The password was right but the account was refused (counts).
    Account,
    /// PAM could not decide, the verifier failed, crashed or could not be
    /// started, or its request was malformed.
    Unavailable,
    /// The verifier did not answer within the deadline and was killed.
    Timeout,
    /// The stack asked for something the lock cannot answer.
    Unsupported,
    /// The PAM service file is not installed.
    ServiceMissing,
}

impl Denial {
    /// The verdict the backoff records: a timeout is an unavailable stack.
    #[must_use]
    pub const fn verdict(self) -> Verdict {
        match self {
            Self::Refused => Verdict::Refused,
            Self::Account => Verdict::AccountRefused,
            Self::Unavailable | Self::Timeout => Verdict::Unavailable,
            Self::Unsupported => Verdict::Unsupported,
            Self::ServiceMissing => Verdict::ServiceMissing,
        }
    }

    /// The `attempt <kind>` line the shell is told.
    #[must_use]
    pub const fn kind(self) -> AttemptKind {
        match self {
            Self::Refused => AttemptKind::Refused,
            Self::Account => AttemptKind::Account,
            Self::Unavailable => AttemptKind::Unavailable,
            Self::Timeout => AttemptKind::Timeout,
            Self::Unsupported => AttemptKind::Unsupported,
            Self::ServiceMissing => AttemptKind::ServiceMissing,
        }
    }
}

/// How an attempt ended: the token, or the reason there is none.
#[derive(Debug)]
pub enum Outcome {
    Authenticated(Authenticated),
    Denied(Denial),
}

/// The outcome a finished child stands for. The one place `Authenticated` is
/// made: exit status 0, by a normal exit.
fn classify(code: Option<i32>, signaled: bool) -> Outcome {
    match Verdict::from_exit(code, signaled) {
        Verdict::Authenticated => Outcome::Authenticated(Authenticated::mint()),
        Verdict::Refused => Outcome::Denied(Denial::Refused),
        Verdict::AccountRefused => Outcome::Denied(Denial::Account),
        Verdict::Unavailable | Verdict::BadRequest => Outcome::Denied(Denial::Unavailable),
        Verdict::Unsupported => Outcome::Denied(Denial::Unsupported),
        Verdict::ServiceMissing => Outcome::Denied(Denial::ServiceMissing),
    }
}

/// Why a binary could not be pinned.
#[derive(Debug, thiserror::Error)]
pub enum PinError {
    #[error("cannot open the verifier")]
    Open(#[source] io::Error),
    #[error("cannot inspect the verifier")]
    Stat(#[source] io::Error),
    #[error("the verifier is not a regular file")]
    NotRegular,
    #[error("the verifier belongs to uid {0}, not to the user or root")]
    Owner(u32),
    #[error("the verifier is writable by group or others (mode {0:o})")]
    Writable(u32),
    #[error("the verifier is not executable")]
    NotExecutable,
    #[error("the verifier is setuid or setgid (mode {0:o})")]
    Setuid(u32),
    #[error("the verifier is not an ELF executable")]
    NotElf,
}

/// The verifier, held open so the bytes checked are the bytes run.
#[derive(Debug)]
pub struct PinnedBinary {
    file: File,
}

impl PinnedBinary {
    /// Opens `path` and checks the descriptor, not the path: a regular file,
    /// owned by the user or root, not writable by group or others, not setuid
    /// or setgid, executable, and an ELF image.
    ///
    /// # Errors
    ///
    /// A [`PinError`] naming the first check that failed.
    pub fn open(path: &Path) -> Result<Self, PinError> {
        let file = File::open(path).map_err(PinError::Open)?;
        let meta = file.metadata().map_err(PinError::Stat)?;
        if !meta.file_type().is_file() {
            return Err(PinError::NotRegular);
        }
        let owner = meta.uid();
        if owner != 0 && owner != getuid().as_raw() {
            return Err(PinError::Owner(owner));
        }
        let mode = meta.mode();
        if mode & 0o022 != 0 {
            return Err(PinError::Writable(mode & 0o7777));
        }
        if mode & 0o6000 != 0 {
            return Err(PinError::Setuid(mode & 0o7777));
        }
        if mode & 0o111 == 0 {
            return Err(PinError::NotExecutable);
        }
        let mut magic = [0u8; 4];
        file.read_exact_at(&mut magic, 0)
            .map_err(|_| PinError::NotElf)?;
        if magic != *b"\x7fELF" {
            return Err(PinError::NotElf);
        }
        Ok(Self { file })
    }

    /// The path the held descriptor is executed from. Valid in the child
    /// between `fork` and `exec` because the descriptor is inherited.
    fn exec_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
    }
}

/// The verifier to run and the arguments it gets.
#[derive(Debug)]
pub struct Verifier {
    binary: PinnedBinary,
    args: Vec<OsString>,
}

impl Verifier {
    /// The production contract: `--protocol 1 --service <name>` (design §4.3
    /// (c)).
    #[must_use]
    pub fn for_service(binary: PinnedBinary, service: Service) -> Self {
        Self {
            binary,
            args: vec![
                OsString::from("--protocol"),
                OsString::from(PROTOCOL_VERSION.to_string()),
                OsString::from("--service"),
                OsString::from(service.name()),
            ],
        }
    }

    /// A verifier with arguments of the caller's choosing: the tests and the
    /// rehearsal probe, whose "verifier" is not `celestina-lock-verify`.
    #[cfg(any(test, feature = "fake-verifier"))]
    #[must_use]
    pub fn with_args(binary: PinnedBinary, args: Vec<OsString>) -> Self {
        Self { binary, args }
    }
}

/// How long one attempt may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timings {
    /// The deadline after which the verifier is killed.
    pub deadline: Duration,
    /// How long the reap of a killed verifier is waited for before it is
    /// logged and left to the next attempt's busy check.
    pub reap_wait: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            deadline: ATTEMPT_DEADLINE,
            reap_wait: REAP_WAIT,
        }
    }
}

/// Why an attempt could not start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// The previous verifier has not been reaped yet.
    #[error("the previous verifier has not been reaped")]
    Busy,
    #[error("cannot start the verify thread")]
    Thread(#[source] io::Error),
}

/// The attempts of one lock: at most one verifier child exists at any time.
#[derive(Debug)]
pub struct Attempts {
    verifier: Arc<Verifier>,
    timings: Timings,
    busy: Arc<AtomicBool>,
}

impl Attempts {
    #[must_use]
    pub fn new(verifier: Verifier, timings: Timings) -> Self {
        Self {
            verifier: Arc::new(verifier),
            timings,
            busy: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Whether a verifier child is still alive or not yet reaped.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// Starts attempt `attempt` with `secret`. `sink` is called exactly once,
    /// from the worker thread, with the attempt's number and outcome: at the
    /// child's exit, or as soon as the deadline's kill is sent.
    ///
    /// # Errors
    ///
    /// [`StartError::Busy`] while the previous child is not reaped (the secret
    /// is dropped, hence zeroed, and `sink` is never called);
    /// [`StartError::Thread`] when the worker cannot be created.
    pub fn start(
        &self,
        attempt: u64,
        secret: Secret,
        sink: impl FnOnce(u64, Outcome) + Send + 'static,
    ) -> Result<(), StartError> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(StartError::Busy);
        }
        let verifier = Arc::clone(&self.verifier);
        let busy = Arc::clone(&self.busy);
        let timings = self.timings;
        // The worker logs where the loop logs: it runs under the subscriber
        // that is current now (the process's own, in production).
        let dispatch = tracing::Dispatch::default();
        let spawned = thread::Builder::new()
            .name("lock-verify".to_owned())
            .spawn(move || {
                let mut sink = Some(sink);
                let mut send = |outcome: Outcome| {
                    if let Some(sink) = sink.take() {
                        sink(attempt, outcome);
                    }
                };
                tracing::dispatcher::with_default(&dispatch, || {
                    run_attempt(&verifier, timings, secret, &mut send);
                });
                busy.store(false, Ordering::Release);
            });
        match spawned {
            Ok(_) => Ok(()),
            Err(source) => {
                self.busy.store(false, Ordering::Release);
                Err(StartError::Thread(source))
            }
        }
    }
}

/// Spawns the verifier, feeds it, and reports its outcome through `send`
/// (once). Returns when the child is reaped.
fn run_attempt(
    verifier: &Verifier,
    timings: Timings,
    secret: Secret,
    send: &mut impl FnMut(Outcome),
) {
    let mut child = match spawn(verifier) {
        Ok(child) => child,
        Err(err) => {
            tracing::error!(kind = ?err.kind(), "cannot start the verifier");
            send(Outcome::Denied(Denial::Unavailable));
            return;
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        // The verifier may exit without reading (a bad request): a broken
        // pipe is not a verdict, its exit status is.
        if let Err(err) = stdin.write_all(secret.as_bytes()) {
            tracing::debug!(kind = ?err.kind(), "the verifier did not read its input");
        }
    }
    drop(secret);

    let pid = Pid::from_child(&child);
    let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
        Ok(pidfd) => pidfd,
        Err(err) => {
            tracing::error!(%err, "cannot watch the verifier");
            kill(None, pid);
            send(Outcome::Denied(Denial::Unavailable));
            reap_after_kill(&mut child, None, timings.reap_wait);
            return;
        }
    };

    match wait_readable(&pidfd, Some(timings.deadline)) {
        Ok(true) => {
            // Leave nothing of the attempt behind in its group, while the
            // zombie leader still keeps the group id from being reused.
            let _ = kill_process_group(pid, Signal::KILL);
            match child.wait() {
                Ok(status) => send(classify(status.code(), status.signal().is_some())),
                Err(err) => {
                    tracing::error!(%err, "cannot reap the verifier");
                    send(Outcome::Denied(Denial::Unavailable));
                }
            }
        }
        Ok(false) => {
            kill(Some(&pidfd), pid);
            send(Outcome::Denied(Denial::Timeout));
            reap_after_kill(&mut child, Some(&pidfd), timings.reap_wait);
        }
        Err(err) => {
            tracing::error!(%err, "cannot wait for the verifier");
            kill(Some(&pidfd), pid);
            send(Outcome::Denied(Denial::Unavailable));
            reap_after_kill(&mut child, Some(&pidfd), timings.reap_wait);
        }
    }
}

fn spawn(verifier: &Verifier) -> io::Result<Child> {
    Command::new(verifier.binary.exec_path())
        .args(&verifier.args)
        .env_clear()
        .env("PATH", VERIFIER_PATH)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // A group of its own, so the kill reaches whatever PAM started.
        .process_group(0)
        .spawn()
}

/// SIGKILL to the child through its pidfd and to its process group.
fn kill(pidfd: Option<&OwnedFd>, pid: Pid) {
    if let Some(pidfd) = pidfd {
        let _ = pidfd_send_signal(pidfd, Signal::KILL);
    }
    let _ = kill_process_group(pid, Signal::KILL);
}

/// Waits for a killed child, at most `reap_wait` before logging, then for as
/// long as it takes: the worker, and with it the busy flag, are held until the
/// child is reaped, so no second verifier starts beside it.
fn reap_after_kill(child: &mut Child, pidfd: Option<&OwnedFd>, reap_wait: Duration) {
    if let Some(pidfd) = pidfd {
        match wait_readable(pidfd, Some(reap_wait)) {
            Ok(true) => {}
            Ok(false) => {
                tracing::error!("the killed verifier is not gone yet; no new attempt starts");
                let _ = wait_readable(pidfd, None);
            }
            Err(err) => tracing::error!(%err, "cannot wait for the killed verifier"),
        }
    }
    if let Err(err) = child.wait() {
        tracing::error!(%err, "cannot reap the killed verifier");
    }
}

/// Whether `fd` became readable within `timeout` (`None` waits forever).
/// Interrupted waits resume with the time that is left.
fn wait_readable(fd: &OwnedFd, timeout: Option<Duration>) -> io::Result<bool> {
    let end = timeout.map(|timeout| Instant::now() + timeout);
    loop {
        let remaining = end.map(|end| end.saturating_duration_since(Instant::now()));
        let spec = remaining.map(|remaining| Timespec {
            tv_sec: i64::try_from(remaining.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(remaining.subsec_nanos()),
        });
        let mut fds = [PollFd::new(fd, PollFlags::IN)];
        match poll(&mut fds, spec.as_ref()) {
            Ok(0) => {
                if end.is_some_and(|end| Instant::now() >= end) {
                    return Ok(false);
                }
            }
            Ok(_) => return Ok(true),
            Err(rustix::io::Errno::INTR) => {}
            Err(err) => return Err(err.into()),
        }
    }
}

/// A token for the tests of the machine, which cannot reach the private
/// constructor. Compiled only for tests.
#[cfg(test)]
pub(crate) fn test_token() -> Authenticated {
    Authenticated::mint()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::mpsc::{channel, Receiver};

    /// A scratch directory of this test run.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "celestina-lock-verify-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    /// A "verifier" that is `/bin/sh -c <script>`: an ELF, so it pins.
    fn fake(script: &str) -> Verifier {
        Verifier::with_args(
            PinnedBinary::open(Path::new("/bin/sh")).expect("pin /bin/sh"),
            vec!["-c".into(), script.into(), "fake-verifier".into()],
        )
    }

    fn timings(deadline_ms: u64) -> Timings {
        Timings {
            deadline: Duration::from_millis(deadline_ms),
            reap_wait: Duration::from_secs(2),
        }
    }

    fn secret(text: &str) -> Secret {
        Secret::from_vec(text.as_bytes().to_vec())
    }

    type Answers = Receiver<(u64, Outcome)>;

    fn run(attempts: &Attempts, attempt: u64, text: &str) -> Answers {
        let (tx, rx) = channel();
        attempts
            .start(attempt, secret(text), move |attempt, outcome| {
                let _ = tx.send((attempt, outcome));
            })
            .expect("start");
        rx
    }

    fn answer(rx: &Answers) -> (u64, Outcome) {
        rx.recv_timeout(Duration::from_secs(10)).expect("an answer")
    }

    fn wait_idle(attempts: &Attempts) {
        let end = Instant::now() + Duration::from_secs(10);
        while attempts.is_busy() {
            assert!(Instant::now() < end, "the worker never finished");
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Runs `path`, retrying while another test's fork still holds the
    /// executable's write descriptor ("text file busy").
    fn run_path_retrying(path: &Path) -> Option<i32> {
        for _ in 0..40 {
            match Command::new(path).status() {
                Ok(status) => return status.code(),
                Err(err) if err.raw_os_error() == Some(26) => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return None,
            }
        }
        None
    }

    #[test]
    fn the_fd_path_runs_an_elf_and_a_script_does_not() {
        // Q9: `Command` executes `/proc/self/fd/N` of a close-on-exec
        // descriptor, which is what glibc's `fexecve` does.
        let elf = PinnedBinary::open(Path::new("/usr/bin/true")).expect("pin");
        let status = Command::new(elf.exec_path()).status().expect("run");
        assert!(status.success());
        let elf = PinnedBinary::open(Path::new("/usr/bin/false")).expect("pin");
        assert_eq!(
            Command::new(elf.exec_path()).status().expect("run").code(),
            Some(1)
        );
        // A script handed to its interpreter through a descriptor that is
        // closed on exec does not run: that is why the pin refuses one.
        let dir = scratch("script");
        let path = dir.join("script.sh");
        fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        let file = File::open(&path).expect("open");
        let via_fd = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
        let ran_ok = Command::new(&via_fd)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        assert!(!ran_ok, "a script must not run through the descriptor path");
        assert!(matches!(PinnedBinary::open(&path), Err(PinError::NotElf)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn t_e3_a_pinned_binary_replaced_on_disk_still_runs_the_pinned_bytes() {
        let dir = scratch("replace");
        let target = dir.join("verifier");
        fs::copy("/usr/bin/true", &target).expect("copy true");
        let pinned = PinnedBinary::open(&target).expect("pin");
        // A deploy replaces the file: new bytes at the same path.
        let other = dir.join("other");
        fs::copy("/usr/bin/false", &other).expect("copy false");
        fs::rename(&other, &target).expect("replace");
        assert_eq!(
            run_path_retrying(&target),
            Some(1),
            "the path now runs the new bytes"
        );
        let attempts = Attempts::new(Verifier::with_args(pinned, Vec::new()), timings(5_000));
        let mut outcome = Outcome::Denied(Denial::Unavailable);
        for round in 0..40 {
            let (n, answered) = answer(&run(&attempts, round, "x"));
            assert_eq!(n, round);
            wait_idle(&attempts);
            outcome = answered;
            // "Text file busy": another test's fork still holds the copy's
            // write descriptor for a moment. Not a property of the lock.
            if !matches!(outcome, Outcome::Denied(Denial::Unavailable)) {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        assert!(
            matches!(outcome, Outcome::Authenticated(_)),
            "the pinned bytes (true) ran, not the replacement (false): {outcome:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_pin_checks_the_descriptor_not_the_name() {
        let dir = scratch("pin");
        let path = dir.join("v");
        fs::copy("/usr/bin/true", &path).expect("copy");
        assert!(PinnedBinary::open(&path).is_ok());
        let mode =
            |bits| fs::set_permissions(&path, fs::Permissions::from_mode(bits)).expect("chmod");
        mode(0o775);
        assert!(matches!(
            PinnedBinary::open(&path),
            Err(PinError::Writable(_))
        ));
        mode(0o757);
        assert!(matches!(
            PinnedBinary::open(&path),
            Err(PinError::Writable(_))
        ));
        mode(0o4755);
        assert!(matches!(
            PinnedBinary::open(&path),
            Err(PinError::Setuid(_))
        ));
        mode(0o644);
        assert!(matches!(
            PinnedBinary::open(&path),
            Err(PinError::NotExecutable)
        ));
        assert!(matches!(
            PinnedBinary::open(&dir),
            Err(PinError::NotRegular)
        ));
        assert!(matches!(
            PinnedBinary::open(&dir.join("missing")),
            Err(PinError::Open(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fake_that_exits_0_yields_authenticated_only_for_its_own_attempt() {
        let attempts = Attempts::new(fake("read -r secret; [ \"$secret\" = ok ]"), timings(5_000));
        let (n, outcome) = answer(&run(&attempts, 7, "no"));
        assert_eq!(n, 7);
        assert!(matches!(outcome, Outcome::Denied(Denial::Refused)));
        wait_idle(&attempts);
        let (n, outcome) = answer(&run(&attempts, 8, "ok"));
        assert_eq!(n, 8, "the answer carries its own attempt's number");
        assert!(matches!(outcome, Outcome::Authenticated(_)));
        wait_idle(&attempts);
    }

    #[test]
    fn every_exit_status_of_the_table_maps_to_its_denial() {
        let table = [
            (1, Denial::Refused),
            (2, Denial::Unavailable),
            (3, Denial::Account),
            (4, Denial::Unsupported),
            (5, Denial::ServiceMissing),
            (6, Denial::Unavailable),
            (7, Denial::Unavailable),
            (127, Denial::Unavailable),
            (255, Denial::Unavailable),
        ];
        for (code, expected) in table {
            let attempts = Attempts::new(fake(&format!("exit {code}")), timings(5_000));
            let (_, outcome) = answer(&run(&attempts, 1, "x"));
            assert!(
                matches!(outcome, Outcome::Denied(d) if d == expected),
                "{code}: {outcome:?}"
            );
            wait_idle(&attempts);
        }
    }

    #[test]
    fn a_signalled_or_unknown_exit_is_unavailable() {
        for script in [
            "kill -9 $$",
            "kill -TERM $$",
            "exec /nonexistent-binary-for-the-test",
        ] {
            let attempts = Attempts::new(fake(script), timings(5_000));
            let (_, outcome) = answer(&run(&attempts, 1, "x"));
            assert!(
                matches!(outcome, Outcome::Denied(Denial::Unavailable)),
                "{script}: {outcome:?}"
            );
            wait_idle(&attempts);
        }
    }

    #[test]
    fn a_verifier_that_exits_without_reading_is_judged_by_its_exit_code() {
        // The secret is written after the child may already be gone: a
        // broken pipe is not a verdict. Many rounds to catch the race.
        let attempts = Attempts::new(fake("exit 1"), timings(5_000));
        let big = "a".repeat(512);
        for round in 0..30 {
            let (_, outcome) = answer(&run(&attempts, round, &big));
            assert!(
                matches!(outcome, Outcome::Denied(Denial::Refused)),
                "{round}: {outcome:?}"
            );
            wait_idle(&attempts);
        }
    }

    #[test]
    fn t_v6_a_fake_verifier_that_sleeps_is_killed_at_the_shortened_deadline_group_included_and_the_verdict_is_unavailable(
    ) {
        let dir = scratch("sleeper");
        let pid_file = dir.join("grandchild.pid");
        let script = format!("sleep 60 & echo $! > '{}'; wait", pid_file.display());
        let attempts = Attempts::new(fake(&script), timings(400));
        let start = Instant::now();
        let rx = run(&attempts, 3, "x");
        let (n, outcome) = answer(&rx);
        let took = start.elapsed();
        assert_eq!(n, 3);
        assert!(
            matches!(outcome, Outcome::Denied(Denial::Timeout)),
            "{outcome:?}"
        );
        assert_eq!(Denial::Timeout.verdict(), Verdict::Unavailable);
        assert!(
            took >= Duration::from_millis(400) && took < Duration::from_secs(5),
            "answered at the deadline, not before and not much after: {took:?}"
        );
        // Exactly one answer, ever.
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
        wait_idle(&attempts);
        // The grandchild of the verifier's group is gone too.
        let grandchild: i32 = fs::read_to_string(&pid_file)
            .expect("the grandchild wrote its pid")
            .trim()
            .parse()
            .expect("a pid");
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let state = fs::read_to_string(format!("/proc/{grandchild}/stat")).ok();
            let gone = state.is_none_or(|stat| {
                stat.rsplit(')')
                    .next()
                    .is_some_and(|rest| rest.trim_start().starts_with('Z'))
            });
            if gone {
                break;
            }
            assert!(Instant::now() < end, "the grandchild {grandchild} survived");
            thread::sleep(Duration::from_millis(20));
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_second_attempt_starts_before_the_previous_reap() {
        let attempts = Attempts::new(fake("sleep 0.3"), timings(5_000));
        let rx = run(&attempts, 1, "x");
        assert!(attempts.is_busy());
        let (tx, _unused) = channel::<(u64, Outcome)>();
        let refused = attempts.start(2, secret("y"), move |a, o| {
            let _ = tx.send((a, o));
        });
        assert!(matches!(refused, Err(StartError::Busy)));
        let (n, _) = answer(&rx);
        assert_eq!(n, 1);
        wait_idle(&attempts);
        assert!(run_ok(&attempts));
    }

    fn run_ok(attempts: &Attempts) -> bool {
        let (tx, rx) = channel();
        attempts
            .start(9, secret("z"), move |a, o| {
                let _ = tx.send((a, o));
            })
            .is_ok()
            && rx.recv_timeout(Duration::from_secs(10)).is_ok()
    }

    #[test]
    fn the_killed_verifier_is_reaped_before_the_worker_is_free_again() {
        let attempts = Attempts::new(fake("sleep 60"), timings(200));
        let rx = run(&attempts, 1, "x");
        let (_, outcome) = answer(&rx);
        assert!(matches!(outcome, Outcome::Denied(Denial::Timeout)));
        // The verdict came with the kill; the worker frees itself only after
        // the reap.
        wait_idle(&attempts);
        assert!(!attempts.is_busy());
    }

    #[test]
    fn t_e1_the_verifier_gets_only_path_and_the_secret_only_on_stdin() {
        let dir = scratch("env");
        let environ = dir.join("environ");
        let cmdline = dir.join("cmdline");
        let stdin = dir.join("stdin");
        let script = format!(
            "cat /proc/$$/environ > '{}'; cat /proc/$$/cmdline > '{}'; cat > '{}'",
            environ.display(),
            cmdline.display(),
            stdin.display()
        );
        let attempts = Attempts::new(fake(&script), timings(5_000));
        let typed = "pässwörd-Ωq9!";
        let (_, outcome) = answer(&run(&attempts, 1, typed));
        assert!(matches!(outcome, Outcome::Authenticated(_)));
        wait_idle(&attempts);
        assert_eq!(
            fs::read(&environ).expect("environ"),
            b"PATH=/usr/local/sbin:/usr/local/bin:/usr/bin\0"
        );
        assert_eq!(fs::read(&stdin).expect("stdin"), typed.as_bytes());
        let cmd = fs::read(&cmdline).expect("cmdline");
        assert!(
            !String::from_utf8_lossy(&cmd).contains("Ωq9"),
            "argv holds no secret"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_production_contract_is_protocol_1_and_a_closed_service_name() {
        let pinned = PinnedBinary::open(Path::new("/usr/bin/true")).expect("pin");
        let verifier = Verifier::for_service(pinned, Service::Rehearsal);
        assert_eq!(
            verifier.args,
            ["--protocol", "1", "--service", "celestina-lock-rehearsal"]
        );
    }

    #[test]
    fn a_verifier_that_cannot_be_started_is_unavailable() {
        // An "ELF" the kernel cannot execute: it pins (the magic is there)
        // and the exec fails.
        let dir = scratch("nospawn");
        let path = dir.join("v");
        fs::write(&path, b"\x7fELF-but-not-an-image").expect("write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        let pinned = PinnedBinary::open(&path).expect("pin");
        let attempts = Attempts::new(Verifier::with_args(pinned, Vec::new()), timings(5_000));
        let (_, outcome) = answer(&run(&attempts, 1, "x"));
        assert!(
            matches!(outcome, Outcome::Denied(Denial::Unavailable)),
            "{outcome:?}"
        );
        wait_idle(&attempts);
        let _ = fs::remove_dir_all(&dir);
    }
}
