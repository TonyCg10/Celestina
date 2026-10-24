//! The `lock` worker: the shell's side of the lock process (CEL-2-H design
//! §4.1, §4.2, §4.3 (b), §4.4, §4.10).
//!
//! One thread owns everything about the child that blocks: pinning the lock's
//! binary, spawning it, reading its status lines, waiting for its pidfd and
//! the liveness check before a suspend proof is handed on. It waits with one
//! `poll` over the child's stdout, the child's pidfd and an `eventfd` that
//! the command sender and the shutdown poke, so it never wakes for nothing.
//! It decides nothing: every line, exit and failure becomes an [`Event`] for
//! the policy on the loop, and every spawn, kill and suspend arrives as a
//! [`Command`] the policy asked for.
//!
//! What the worker keeps true (the policy relies on all of it):
//!
//! * Events are about the child the last `Spawn` started. A new child starts
//!   only when no child is held: the previous one's `Exit` was published
//!   before its record was dropped. The one exception is a child the policy
//!   killed and gave up on (it went idle): a spawn asked for meanwhile waits
//!   until that child is reaped, and then the dead child's exit is not
//!   published, because the policy would read it as the new child's.
//! * The lock is executed from the descriptor that was checked, as
//!   `/proc/self/fd/N`, with a cleared environment, a process group of its
//!   own and a sealed memory file as stdin. It is found beside the shell's own
//!   executable; no `PATH`, no variable names it.
//! * The child outlives this worker and the shell: its stdout is a pipe whose
//!   reader may vanish, and the lock ignores `EPIPE`. When the worker stops
//!   with a child running, a detached thread only reaps it.
//! * A suspend proof reaches logind only if the child is alive and has not
//!   said `unlocked` (the policy's generation is checked again by the logind
//!   worker).
//! * Nothing here is killed by name: the only signal the worker sends is
//!   `SIGKILL` through the pidfd of a child it spawned and has not reaped, and
//!   only when the policy asks (the child never asked to lock).
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_lock_proto::{render, service_check, LineReader, Service, StatusLine, Verdict};
use celestina_shell::lock::{ChildExit, Event, GenerationReader, LockedProof};
use celestina_wayland::lock::{LAUNCHER_SHELL, LAUNCHER_VARIABLE};
use rustix::event::{eventfd, poll, EventfdFlags, PollFd, PollFlags};
use rustix::fs::{fcntl_setfl, OFlags};
use rustix::process::{getuid, pidfd_open, pidfd_send_signal, Pid, PidfdFlags, Signal};
use rustix::time::Timespec;

use super::bundle;
use crate::providers::Worker;
use crate::wallpaper::thumbnail::ThumbnailStore;

/// The lock's file name beside the shell's executable.
pub const LOCK_FILE: &str = "celestina-lock";
/// The verifier's file name beside the shell's executable.
pub const VERIFIER_FILE: &str = "celestina-lock-verify";
/// The requests that may wait for the worker. The policy asks for few things
/// and one at a time; past this a request repeats itself.
pub const QUEUE: usize = 4;
/// How long stopping the worker waits for it.
pub const JOIN_WAIT: Duration = Duration::from_secs(2);
/// The most dropped lines logged per read of the child's stdout.
const MAX_DROPPED_LOGS: usize = 8;

/// Why the lock's binary could not be pinned.
#[derive(Debug, thiserror::Error)]
pub enum PinError {
    #[error("cannot open the binary")]
    Open(#[source] io::Error),
    #[error("cannot inspect the binary")]
    Stat(#[source] io::Error),
    #[error("the binary is not a regular file")]
    NotRegular,
    #[error("the binary belongs to uid {0}, not to the user or root")]
    Owner(u32),
    #[error("the binary is writable by group or others (mode {0:o})")]
    Writable(u32),
    #[error("the binary is not executable")]
    NotExecutable,
    #[error("the binary is setuid or setgid (mode {0:o})")]
    Setuid(u32),
    #[error("the binary is not an ELF executable")]
    NotElf,
}

/// A binary held open so the bytes that were checked are the bytes that run.
#[derive(Debug)]
pub struct Pinned {
    file: File,
}

impl Pinned {
    /// Opens `path` and checks the descriptor, not the path: a regular file,
    /// owned by the user or root, not writable by group or others, not setuid
    /// or setgid, executable, and an ELF image (a script would be handed to
    /// its interpreter through a path that no longer exists).
    ///
    /// # Errors
    ///
    /// The first check that failed.
    pub fn open(path: &Path) -> Result<Self, PinError> {
        let file = File::open(path).map_err(PinError::Open)?;
        let pinned = Self { file };
        pinned.check()?;
        Ok(pinned)
    }

    /// Checks the held descriptor again.
    ///
    /// # Errors
    ///
    /// The first check that failed.
    pub fn check(&self) -> Result<(), PinError> {
        let meta = self.file.metadata().map_err(PinError::Stat)?;
        judge(
            meta.file_type().is_file(),
            meta.uid(),
            meta.mode(),
            getuid().as_raw(),
        )?;
        let mut magic = [0u8; 4];
        self.file
            .read_exact_at(&mut magic, 0)
            .map_err(|_| PinError::NotElf)?;
        if magic != *b"\x7fELF" {
            return Err(PinError::NotElf);
        }
        Ok(())
    }

    /// The path the held descriptor is executed from; valid in the child
    /// between `fork` and `exec` because the descriptor is inherited.
    fn exec_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
    }
}

/// The checks a pinned binary's `fstat` must pass: a regular file, owned by
/// the user (`me`) or root, not writable by group or others, not setuid or
/// setgid, executable.
fn judge(is_file: bool, owner: u32, mode: u32, me: u32) -> Result<(), PinError> {
    if !is_file {
        return Err(PinError::NotRegular);
    }
    if owner != 0 && owner != me {
        return Err(PinError::Owner(owner));
    }
    if mode & 0o022 != 0 {
        return Err(PinError::Writable(mode & 0o7777));
    }
    if mode & 0o6000 != 0 {
        return Err(PinError::Setuid(mode & 0o7777));
    }
    if mode & 0o111 == 0 {
        return Err(PinError::NotExecutable);
    }
    Ok(())
}

/// What the worker needs to find, start and judge the lock.
#[derive(Clone)]
pub struct Config {
    /// The directory the lock and the verifier are found in.
    pub dir: PathBuf,
    /// The lock's file name in [`Config::dir`].
    pub lock_file: &'static str,
    /// The verifier's file name in [`Config::dir`].
    pub verifier_file: &'static str,
    /// The arguments the lock gets. Production passes none.
    pub args: Vec<OsString>,
    /// The whole environment the lock gets.
    pub env: Vec<(OsString, OsString)>,
    /// Which of the PAM service's candidate files exist (`stat`, never
    /// opened).
    pub pam_existing: Arc<dyn Fn() -> Vec<PathBuf> + Send + Sync>,
    /// The wallpaper state's thumbnails; every spawn copies them into the
    /// bundle (CEL-2-H-b). Empty with the wallpaper duty off.
    pub thumbnails: ThumbnailStore,
    /// Reads the local UTC offset in seconds for one spawn; `None` when it
    /// cannot be read (CEL-2-H-b, design §4.8, M1). It may block on a child
    /// process, and is called only on the worker, never on the loop.
    pub utc_offset: Arc<dyn Fn() -> Option<i32> + Send + Sync>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("dir", &self.dir)
            .field("lock_file", &self.lock_file)
            .field("verifier_file", &self.verifier_file)
            .field("args", &self.args)
            .finish_non_exhaustive()
    }
}

impl Config {
    /// The production configuration: the binaries beside this executable
    /// (after a deploy replaced it the kernel reports `... (deleted)`; only
    /// the directory is used), and the three variables of the allowlist.
    ///
    /// # Errors
    ///
    /// When this executable's own path cannot be read.
    pub fn production(thumbnails: ThumbnailStore) -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(Self {
            dir,
            lock_file: LOCK_FILE,
            verifier_file: VERIFIER_FILE,
            args: Vec::new(),
            env: allowlisted_environment(|name| std::env::var_os(name)),
            pam_existing: Arc::new(production_pam_files),
            thumbnails,
            utc_offset: Arc::new(production_utc_offset),
        })
    }
}

/// The Island's `date +%z` parse, without its cell: afresh on every call, so a
/// lock started after a daylight-saving change shows the new offset.
fn production_utc_offset() -> Option<i32> {
    crate::island::clock::read_utc_offset().and_then(|seconds| i32::try_from(seconds).ok())
}

/// The environment the lock is started with, and nothing else (design §4.2):
/// where to connect and where the runtime directory is, passed on from this
/// process, and `MESA_SHADER_CACHE_DISABLE=true`, which the design lists for
/// a GPU painter the CPU painter replaced (it is inert today), and the
/// launcher marker of the connection guard. No `HOME`, no
/// `XDG_*` beyond the runtime directory, no `LANG`, no `LD_*`, no `RUST_LOG`.
fn allowlisted_environment(read: impl Fn(&str) -> Option<OsString>) -> Vec<(OsString, OsString)> {
    let mut env = Vec::new();
    for name in ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"] {
        if let Some(value) = read(name) {
            env.push((OsString::from(name), value));
        }
    }
    env.push((
        OsString::from("MESA_SHADER_CACHE_DISABLE"),
        OsString::from("true"),
    ));
    // The declaration the lock's connection guard looks for (round 5, ADR
    // 0022 A4): this lock was started by the shell, not by hand. A statement
    // of intent against accidents (a bare `celestina-lock` run in a terminal
    // with the desktop's `WAYLAND_DISPLAY`), not a defence against a process
    // that can set its own environment. The pair is the toolkit's constants,
    // so the two sides cannot drift.
    env.push((
        OsString::from(LAUNCHER_VARIABLE),
        OsString::from(LAUNCHER_SHELL),
    ));
    env
}

/// The production service's candidate files that exist.
fn production_pam_files() -> Vec<PathBuf> {
    Service::Production
        .paths()
        .into_iter()
        .filter(|path| std::fs::symlink_metadata(path).is_ok())
        .collect()
}

/// What suspends the machine once the worker has judged a proof worth
/// handing on.
pub trait Suspender: Send {
    /// Hands `proof` to whoever calls logind.
    ///
    /// # Errors
    ///
    /// Why it was not queued.
    fn suspend(&self, proof: LockedProof) -> Result<(), String>;
}

/// What the policy asks of the worker.
#[derive(Debug)]
pub enum Request {
    /// Start a lock child after `after`.
    Spawn { prior_failures: u8, after: Duration },
    /// Kill the child through its pidfd (it never asked to lock).
    Kill,
    /// Hand a suspend proof on if the child is still alive.
    Suspend(LockedProof),
}

/// Why a request was not queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// [`QUEUE`] requests are waiting already.
    #[error("the lock worker's queue is full")]
    Full,
    /// The worker has stopped.
    #[error("the lock worker has stopped")]
    Stopped,
}

/// The way into the worker: queues a request and wakes it, never waits.
#[derive(Debug)]
pub struct SupervisorHandle {
    commands: SyncSender<Request>,
    wake: Arc<OwnedFd>,
    stop: Arc<AtomicBool>,
}

impl SupervisorHandle {
    fn poke(&self) {
        poke(&self.wake);
    }

    fn send(&self, command: Request) -> Result<(), RequestError> {
        let result = self.commands.try_send(command).map_err(|err| match err {
            TrySendError::Full(_) => RequestError::Full,
            TrySendError::Disconnected(_) => RequestError::Stopped,
        });
        if result.is_ok() {
            self.poke();
        }
        result
    }

    /// Asks for a child after `after`.
    ///
    /// # Errors
    ///
    /// When the request is not queued.
    pub fn spawn_child(&self, prior_failures: u8, after: Duration) -> Result<(), RequestError> {
        self.send(Request::Spawn {
            prior_failures,
            after,
        })
    }

    /// Asks for the child to be killed through its pidfd.
    ///
    /// # Errors
    ///
    /// When the request is not queued.
    pub fn kill_child(&self) -> Result<(), RequestError> {
        self.send(Request::Kill)
    }

    /// Hands a proof to the worker, which checks the child before it passes
    /// it on. A proof that is not queued is dropped.
    ///
    /// # Errors
    ///
    /// When the request is not queued.
    pub fn suspend(&self, proof: LockedProof) -> Result<(), RequestError> {
        self.send(Request::Suspend(proof))
    }
}

/// A running worker and its handle.
#[derive(Debug)]
pub struct Supervisor {
    handle: SupervisorHandle,
    worker: Worker,
}

impl Supervisor {
    /// The handle.
    #[must_use]
    pub fn handle(&self) -> &SupervisorHandle {
        &self.handle
    }

    /// Stops the worker and waits for it, at most [`JOIN_WAIT`]. A lock
    /// child that is running is not touched: only PAM ends it.
    pub fn stop(self) {
        let Supervisor { handle, worker } = self;
        handle.stop.store(true, Ordering::Release);
        handle.poke();
        worker.join_within(JOIN_WAIT);
    }
}

fn poke(wake: &OwnedFd) {
    // A full counter or a closed reader means the worker is awake or gone.
    let _ = rustix::io::write(wake, &1u64.to_ne_bytes());
}

/// Starts the worker. `publish` receives every [`Event`] and returns `false`
/// once the loop is gone (the worker then stops). Nothing is spawned until
/// the policy asks; the binaries are pinned and the PAM file looked for first.
///
/// # Errors
///
/// The failure to make the eventfd or start the thread.
pub fn spawn(
    config: Config,
    publish: impl Fn(Event) -> bool + Send + 'static,
    suspender: Box<dyn Suspender>,
    generation: GenerationReader,
) -> io::Result<Supervisor> {
    let wake = Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?);
    let (commands, queue) = mpsc::sync_channel(QUEUE);
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let wake = Arc::clone(&wake);
        let flag = Arc::clone(&stop);
        Worker::spawn("lock", &stop, move || {
            let mut run = Run {
                config,
                publish: Box::new(publish),
                suspender,
                generation,
                wake,
                stop: flag,
                lock: None,
                child: None,
                pending: None,
            };
            run.serve(&queue);
        })?
    };
    Ok(Supervisor {
        handle: SupervisorHandle {
            commands,
            wake,
            stop,
        },
        worker,
    })
}

/// The child the worker holds.
struct Held {
    child: Child,
    pidfd: OwnedFd,
    /// The child's stdout (non-blocking), until it reaches end of file.
    stdout: Option<OwnedFd>,
    lines: LineReader,
    /// The child said `unlocked`: no proof is handed on any more.
    said_unlocked: bool,
    /// The policy asked for it to be killed and the signal was sent. The
    /// policy expects nothing more from it: its remaining lines and its exit
    /// are never published, and a spawn asked for meanwhile starts only after
    /// the reaping.
    killed: bool,
}

impl Held {
    /// Takes charge of a child that was just spawned with a piped stdout:
    /// its pidfd, its stdout made non-blocking.
    fn adopt(mut child: Child) -> Result<Held, LaunchError> {
        let pid = Pid::from_child(&child);
        let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            Err(errno) => {
                // The child has not asked for anything yet; it is ours and
                // unreaped, so the pid is its own.
                let _ = child.kill();
                let _ = child.wait();
                return Err(LaunchError::Pidfd(io::Error::from(errno)));
            }
        };
        let stdout = child.stdout.take().map(OwnedFd::from);
        if let Some(stdout) = &stdout {
            if let Err(errno) = fcntl_setfl(stdout, OFlags::NONBLOCK) {
                let _ = pidfd_send_signal(&pidfd, Signal::KILL);
                let _ = child.wait();
                return Err(LaunchError::Pipe(io::Error::from(errno)));
            }
        }
        Ok(Held {
            child,
            pidfd,
            stdout,
            lines: LineReader::new(),
            said_unlocked: false,
            killed: false,
        })
    }
}

/// A spawn that waits for its delay.
struct Pending {
    at: Instant,
    prior_failures: u8,
}

struct Run {
    config: Config,
    publish: Box<dyn Fn(Event) -> bool + Send>,
    suspender: Box<dyn Suspender>,
    generation: GenerationReader,
    wake: Arc<OwnedFd>,
    stop: Arc<AtomicBool>,
    lock: Option<Pinned>,
    child: Option<Held>,
    pending: Option<Pending>,
}

impl Run {
    fn say(&self, event: Event) -> bool {
        (self.publish)(event)
    }

    fn serve(&mut self, queue: &Receiver<Request>) {
        if !self.pin() {
            return;
        }
        while !self.stop.load(Ordering::Acquire) {
            let ready = self.wait();
            if self.stop.load(Ordering::Acquire) {
                break;
            }
            if ready.wake {
                drain_wake(&self.wake);
            }
            loop {
                match queue.try_recv() {
                    Ok(command) => {
                        if !self.command(command) {
                            self.finish();
                            return;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.finish();
                        return;
                    }
                }
            }
            if ready.stdout && !self.read_stdout() {
                self.finish();
                return;
            }
            if self.pidfd_ready() && !self.child_exited() {
                self.finish();
                return;
            }
            if self
                .pending
                .as_ref()
                .is_some_and(|p| Instant::now() >= p.at)
                && self.child.is_none()
            {
                if let Some(pending) = self.pending.take() {
                    if !self.start(pending.prior_failures) {
                        self.finish();
                        return;
                    }
                }
            }
        }
        self.finish();
    }

    /// Pins the lock and checks the verifier and the PAM file; tells the
    /// policy. `false` when the loop is gone.
    fn pin(&mut self) -> bool {
        let pinned = self.check_binaries();
        let ok = pinned.is_some();
        self.lock = pinned;
        if !self.say(Event::Pinned(ok)) {
            return false;
        }
        if ok {
            let existing = (self.config.pam_existing)();
            if service_check(Service::Production, &existing) == Err(Verdict::ServiceMissing) {
                return self.say(Event::PamFileMissing);
            }
        }
        true
    }

    /// The lock's descriptor (held, or opened now) and the verifier's file,
    /// both checked. Nothing is published here.
    fn check_binaries(&mut self) -> Option<Pinned> {
        let lock_path = self.config.dir.join(self.config.lock_file);
        let verifier_path = self.config.dir.join(self.config.verifier_file);
        let lock = match self.lock.take() {
            // A deploy that replaced the file on disk changes nothing: the
            // held descriptor is what runs, and it is checked again.
            Some(held) => match held.check() {
                Ok(()) => held,
                Err(err) => {
                    tracing::error!(%err, "the lock binary no longer passes its checks");
                    return None;
                }
            },
            None => match Pinned::open(&lock_path) {
                Ok(pinned) => pinned,
                Err(err) => {
                    tracing::warn!(%err, path = %lock_path.display(), "the lock binary cannot be pinned");
                    return None;
                }
            },
        };
        // Only the file is checked here: the lock pins the verifier itself.
        if let Err(err) = Pinned::open(&verifier_path) {
            tracing::warn!(%err, path = %verifier_path.display(), "the verifier cannot be pinned");
            self.lock = Some(lock);
            return None;
        }
        Some(lock)
    }

    /// Handles one command. `false` when the loop is gone.
    fn command(&mut self, command: Request) -> bool {
        match command {
            Request::Spawn {
                prior_failures,
                after,
            } => {
                // A child the policy has already given up on (killed, not yet
                // reaped) does not count: the spawn waits for its reaping, so
                // a kill followed by a new request is not a failure (I2).
                let held_and_wanted = self.child.as_ref().is_some_and(|held| !held.killed);
                if held_and_wanted || self.pending.is_some() {
                    tracing::error!("a lock child was asked for while one is held");
                    return self.say(Event::SpawnFailed);
                }
                if after.is_zero() && self.child.is_none() {
                    self.start(prior_failures)
                } else {
                    self.pending = Some(Pending {
                        at: Instant::now() + after,
                        prior_failures,
                    });
                    true
                }
            }
            Request::Kill => {
                // A spawn that has not started yet is simply forgotten.
                self.pending = None;
                if let Some(held) = self.child.as_mut() {
                    match pidfd_send_signal(&held.pidfd, Signal::KILL) {
                        Ok(()) => held.killed = true,
                        Err(err) => {
                            tracing::warn!(%err, "cannot kill the lock child through its pidfd");
                        }
                    }
                }
                true
            }
            Request::Suspend(proof) => {
                self.hand_on(proof);
                true
            }
        }
    }

    /// Starts the child. `false` when the loop is gone.
    fn start(&mut self, prior_failures: u8) -> bool {
        if self.check_binaries_for_spawn().is_none() {
            // The failure first: the policy then notices the person who asked
            // (it forgets their request when it hears the pin is gone), and
            // only afterwards becomes unavailable (finding M1).
            return self.say(Event::SpawnFailed) && self.say(Event::Pinned(false));
        }
        match self.launch(prior_failures) {
            Ok(held) => {
                self.child = Some(held);
                true
            }
            Err(err) => {
                tracing::error!(error = %crate::error_chain(&err), "cannot start the lock");
                self.say(Event::SpawnFailed)
            }
        }
    }

    fn check_binaries_for_spawn(&mut self) -> Option<()> {
        let pinned = self.check_binaries();
        let ok = pinned.is_some();
        self.lock = pinned;
        ok.then_some(())
    }

    fn launch(&mut self, prior_failures: u8) -> Result<Held, LaunchError> {
        let lock = self.lock.as_ref().ok_or(LaunchError::NotPinned)?;
        // Read afresh for every spawn, so a lock restarted after a wallpaper
        // change shows the new picture. The copy is the supervisor worker's,
        // never the loop's.
        // The UTC offset is read afresh too, once per spawn and here, on the
        // worker (it runs `date`): a failed read sends `offset_known = 0` and
        // the lock hides its clock rather than show a wrong hour.
        let offset = (self.config.utc_offset)();
        if offset.is_none() {
            tracing::warn!("the UTC offset could not be read: the lock will show no clock");
        }
        let bundle = bundle::sealed(&bundle::with_thumbnails(
            prior_failures,
            offset,
            &self.config.thumbnails,
        ))
        .map_err(LaunchError::Bundle)?;
        let mut command = Command::new(lock.exec_path());
        command
            .arg0(self.config.lock_file)
            .args(&self.config.args)
            .env_clear()
            .envs(self.config.env.iter().map(|(k, v)| (k, v)))
            // A group of its own: a signal to the shell's group does not
            // reach the lock.
            .process_group(0)
            .stdin(Stdio::from(bundle))
            .stdout(Stdio::piped());
        // What the child inherits (round 4, finding I-α; round 5, finding
        // I-2): `Command` closes nothing of its own, so the child gets every
        // descriptor of the shell that is not close-on-exec. Ours all are.
        // The one that was not is the logind inhibitor's descriptor in the
        // `Inhibit` reply: zbus receives it without close-on-exec, the
        // sleep worker's stream keeps the reply message until its next
        // `drain`, and a `spawn` in that window would have handed the child a
        // copy that lives as long as the lock. `sleep::inhibit` now marks the
        // message's own descriptor close-on-exec right after `recvmsg`, so the
        // stream no longer holds an inheritable copy (test
        // `lock::sleep::tests::the_inhibitor_reply_leaves_no_inheritable_descriptor_in_the_stream`).
        // What remains (residual X3, round 6) is the span between `recvmsg`
        // and that `fcntl`: the lock and the verifier cure themselves by
        // closing everything they were not handed, but another child of the
        // shell started in that span could still inherit the descriptor.
        // No `pre_exec` here (that is `unsafe` and forbidden); the child
        // still closes what it was not handed, first thing it does
        // (`celestina-lock`'s `hardening::close_inherited_descriptors`, and the
        // verifier's), as the second layer. Tests:
        // `celestina-lock/tests/descriptors.rs`.
        let child = command.spawn().map_err(LaunchError::Spawn)?;
        Held::adopt(child)
    }

    /// Reads what the child's stdout holds and publishes its lines. `false`
    /// when the loop is gone.
    fn read_stdout(&mut self) -> bool {
        let mut lines = Vec::new();
        if let Some(held) = self.child.as_mut() {
            read_available(held, &mut lines);
            if held.killed {
                // Read so the pipe drains, but the policy is done with it.
                return true;
            }
        }
        self.publish_lines(lines)
    }

    fn publish_lines(&mut self, lines: Vec<StatusLine>) -> bool {
        for line in lines {
            // The lines carry a protocol word and an attempt kind, never a
            // secret (design §4.3 (b)).
            tracing::debug!(line = %render(&line), "lock: status line");
            if line == StatusLine::Unlocked {
                if let Some(held) = self.child.as_mut() {
                    held.said_unlocked = true;
                }
            }
            if !self.say(Event::Line(line)) {
                return false;
            }
        }
        true
    }

    fn pidfd_ready(&self) -> bool {
        self.child
            .as_ref()
            .is_some_and(|held| readable(&held.pidfd))
    }

    /// The child ended: its last lines, then its exit. `false` when the loop
    /// is gone.
    fn child_exited(&mut self) -> bool {
        let Some(mut held) = self.child.take() else {
            return true;
        };
        let status = match held.child.wait() {
            Ok(status) => status,
            Err(err) => {
                tracing::error!(%err, "cannot reap the lock child");
                self.child = Some(held);
                return true;
            }
        };
        if held.killed {
            // Never told (finding I-B). The policy moved on when it killed
            // this child (the generation was bumped, and `Action::KillChild`
            // promises no further event from it): an exit published now
            // would be read as the next child's, whether that spawn is
            // already waiting behind this one or arrives after the reaping.
            tracing::info!("the lock child the policy gave up on was reaped");
            return true;
        }
        let mut lines = Vec::new();
        read_available(&mut held, &mut lines);
        let exit = ChildExit::from_wait(status.code(), status.signal().is_some());
        let said = self.publish_lines(lines);
        said && self.say(Event::Exit(exit))
    }

    /// Hands `proof` to logind if the child is alive and has not unlocked.
    fn hand_on(&mut self, proof: LockedProof) {
        // Lines the child already wrote count: an `unlocked` waiting in the
        // pipe has not reached the policy's generation yet.
        let mut lines = Vec::new();
        if let Some(held) = self.child.as_mut() {
            read_available(held, &mut lines);
            if held.killed {
                lines.clear();
            }
        }
        if !self.publish_lines(lines) {
            return;
        }
        let alive = self
            .child
            .as_ref()
            .is_some_and(|held| !held.killed && !held.said_unlocked && !readable(&held.pidfd));
        if !alive {
            tracing::warn!("not suspending: the lock child is gone or has unlocked");
            return;
        }
        if !self.generation.is_current(&proof) {
            tracing::warn!("not suspending: the lock changed since the proof was made");
            return;
        }
        if let Err(why) = self.suspender.suspend(proof) {
            tracing::warn!("the suspend was not queued: {why}");
        }
    }

    /// Waits for the eventfd, the child's stdout, its pidfd or the pending
    /// spawn's time.
    fn wait(&self) -> Ready {
        // A spawn waiting behind a child being reaped is woken by its pidfd.
        let waiting = self.pending.as_ref().filter(|_| self.child.is_none());
        let timeout = waiting.map(|pending| {
            let left = pending.at.saturating_duration_since(Instant::now());
            Timespec {
                tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
                tv_nsec: i64::from(left.subsec_nanos()),
            }
        });
        let stdout = self.child.as_ref().and_then(|held| held.stdout.as_ref());
        let pidfd = self.child.as_ref().map(|held| &held.pidfd);
        let mut fds = vec![PollFd::new(&*self.wake, PollFlags::IN)];
        let stdout_at = stdout.map(|fd| {
            fds.push(PollFd::new(fd, PollFlags::IN));
            fds.len() - 1
        });
        if let Some(fd) = pidfd {
            fds.push(PollFd::new(fd, PollFlags::IN));
        }
        loop {
            match poll(&mut fds, timeout.as_ref()) {
                Ok(_) => break,
                Err(rustix::io::Errno::INTR) => {
                    if self.stop.load(Ordering::Acquire) {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!(%err, "the lock worker cannot poll");
                    // Do not spin: a failing poll waits a moment.
                    std::thread::sleep(Duration::from_millis(100));
                    break;
                }
            }
        }
        Ready {
            wake: !fds[0].revents().is_empty(),
            stdout: stdout_at.is_some_and(|at| !fds[at].revents().is_empty()),
        }
    }

    /// The worker is done. A child that runs is left alone; a detached
    /// thread reaps it when PAM ends it.
    fn finish(&mut self) {
        self.pending = None;
        if let Some(held) = self.child.take() {
            let Held { mut child, .. } = held;
            let reaper = std::thread::Builder::new()
                .name("lock-reap".to_owned())
                .spawn(move || {
                    let _ = child.wait();
                });
            if let Err(err) = reaper {
                tracing::warn!(%err, "no thread to reap the lock child; it stays a zombie");
            }
            tracing::info!("the lock worker stops; the lock keeps running");
        }
    }
}

struct Ready {
    wake: bool,
    stdout: bool,
}

#[derive(Debug, thiserror::Error)]
enum LaunchError {
    #[error("the lock binary is not pinned")]
    NotPinned,
    #[error("cannot make the backdrop bundle")]
    Bundle(#[source] bundle::BundleError),
    #[error("cannot start the process")]
    Spawn(#[source] io::Error),
    #[error("cannot watch the process")]
    Pidfd(#[source] io::Error),
    #[error("cannot set up the status pipe")]
    Pipe(#[source] io::Error),
}

fn drain_wake(wake: &OwnedFd) {
    let mut count = [0u8; 8];
    let _ = rustix::io::read(wake, &mut count);
}

/// Whether `fd` is readable right now.
fn readable(fd: &OwnedFd) -> bool {
    let mut fds = [PollFd::new(fd, PollFlags::IN)];
    let now = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    loop {
        match poll(&mut fds, Some(&now)) {
            Ok(count) => return count > 0,
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => return false,
        }
    }
}

/// Reads everything the child's stdout holds without blocking and appends
/// the lines it spells to `lines`; a line that does not parse is logged and
/// dropped. At end of file the pipe is forgotten.
fn read_available(held: &mut Held, lines: &mut Vec<StatusLine>) {
    let mut dropped = 0usize;
    let mut buf = [0u8; LineReader::CHUNK];
    while let Some(stdout) = held.stdout.as_ref() {
        match rustix::io::read(stdout, &mut buf) {
            Ok(0) => {
                held.stdout = None;
            }
            Ok(count) => {
                for line in held.lines.feed(&buf[..count]) {
                    match line {
                        Ok(line) => lines.push(line),
                        Err(err) => {
                            dropped += 1;
                            if dropped <= MAX_DROPPED_LOGS {
                                tracing::warn!(%err, "a status line of the lock was dropped");
                            }
                        }
                    }
                }
            }
            Err(rustix::io::Errno::AGAIN) => return,
            Err(rustix::io::Errno::INTR) => {}
            Err(err) => {
                tracing::warn!(%err, "cannot read the lock's status pipe");
                held.stdout = None;
            }
        }
    }
}

// The scanner helper shared by the lock's source guards (round 7: the one
// owner of "production code without its test items").
#[cfg(test)]
#[path = "../../crates/celestina-wayland/tests/support/test_gates.rs"]
mod test_gates;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    use celestina_shell::lock::{Event as PolicyEvent, LockPolicy};

    #[test]
    fn a_binary_failing_fstat_is_not_available() {
        let me = 1000;
        assert!(judge(true, me, 0o755, me).is_ok());
        assert!(judge(true, 0, 0o755, me).is_ok(), "root owns the system's");
        assert!(matches!(
            judge(false, me, 0o755, me),
            Err(PinError::NotRegular)
        ));
        assert!(matches!(
            judge(true, 4242, 0o755, me),
            Err(PinError::Owner(4242))
        ));
        assert!(matches!(
            judge(true, me, 0o775, me),
            Err(PinError::Writable(_))
        ));
        assert!(matches!(
            judge(true, me, 0o757, me),
            Err(PinError::Writable(_))
        ));
        assert!(matches!(
            judge(true, me, 0o4755, me),
            Err(PinError::Setuid(_))
        ));
        assert!(matches!(
            judge(true, me, 0o2755, me),
            Err(PinError::Setuid(_))
        ));
        assert!(matches!(
            judge(true, me, 0o644, me),
            Err(PinError::NotExecutable)
        ));
    }

    #[test]
    fn t_e1_the_lock_gets_only_the_allowlisted_environment() {
        let hostile = |name: &str| -> Option<OsString> {
            Some(OsString::from(match name {
                "WAYLAND_DISPLAY" => "wayland-9",
                "XDG_RUNTIME_DIR" => "/run/user/1",
                // Everything else the caller's environment may hold.
                "HOME"
                | "LD_PRELOAD"
                | "LD_LIBRARY_PATH"
                | "RUST_LOG"
                | "LANG"
                | "PATH"
                | "CELESTINA_LOCK"
                | "XDG_CONFIG_HOME"
                | "__EGL_VENDOR_LIBRARY_FILENAMES" => "x",
                _ => "y",
            }))
        };
        let env = allowlisted_environment(hostile);
        let names: Vec<&str> = env.iter().filter_map(|(k, _)| k.to_str()).collect();
        assert_eq!(
            names,
            [
                "WAYLAND_DISPLAY",
                "XDG_RUNTIME_DIR",
                "MESA_SHADER_CACHE_DISABLE",
                "CELESTINA_LOCK_LAUNCHER"
            ]
        );
        assert_eq!(env[0].1, "wayland-9");
        assert_eq!(env[2].1, "true");
        // The marker the connection guard asks for, spelled out here too so a
        // rename of the toolkit's constant shows up as a failure.
        assert_eq!(env[3].1, "shell");
        // A variable that is not set is not invented.
        let bare = allowlisted_environment(|_| None);
        assert_eq!(bare.len(), 2, "only the two fixed ones remain");
        // The marker is set by the supervisor, never copied from the caller:
        // a hostile value is replaced.
        let hostile_marker = allowlisted_environment(|name| {
            (name == "CELESTINA_LOCK_LAUNCHER").then(|| OsString::from("nope"))
        });
        assert_eq!(hostile_marker.len(), 2);
    }

    #[test]
    fn no_override_variable_is_read_by_the_supervisor() {
        // The only `env::var` of the module is the production allowlist's.
        let source = include_str!("supervisor.rs");
        let production = super::test_gates::without_test_blocks(source);
        let reads: Vec<&str> = production
            .lines()
            .filter(|line| line.contains("env::var"))
            .collect();
        assert_eq!(reads.len(), 1, "{reads:?}");
        assert!(reads[0].contains("std::env::var_os(name)"));
    }

    /// Everything the run published, in order, and every proof it handed on.
    #[derive(Clone, Default)]
    struct Seen {
        events: Arc<Mutex<Vec<Event>>>,
        proofs: Arc<Mutex<Vec<u64>>>,
    }

    struct Capture(Seen);

    impl Suspender for Capture {
        fn suspend(&self, proof: LockedProof) -> Result<(), String> {
            self.0
                .proofs
                .lock()
                .expect("proofs")
                .push(proof.generation());
            Ok(())
        }
    }

    fn run_over(seen: &Seen, generation: GenerationReader) -> Run {
        let events = Arc::clone(&seen.events);
        Run {
            config: Config {
                dir: PathBuf::new(),
                lock_file: LOCK_FILE,
                verifier_file: VERIFIER_FILE,
                args: Vec::new(),
                env: Vec::new(),
                pam_existing: Arc::new(Vec::new),
                thumbnails: ThumbnailStore::default(),
                utc_offset: Arc::new(|| None),
            },
            publish: Box::new(move |event| {
                events.lock().expect("events").push(event);
                true
            }),
            suspender: Box::new(Capture(seen.clone())),
            generation,
            wake: Arc::new(eventfd(0, EventfdFlags::CLOEXEC).expect("eventfd")),
            stop: Arc::new(AtomicBool::new(false)),
            lock: None,
            child: None,
            pending: None,
        }
    }

    fn held(script: &str) -> Held {
        let child = Command::new("/usr/bin/bash")
            .args(["-c", script])
            .stdout(Stdio::piped())
            .spawn()
            .expect("bash starts");
        Held::adopt(child).expect("adopted")
    }

    /// Ends a test's child through its own pidfd and reaps it.
    fn end(mut held: Held) {
        let _ = pidfd_send_signal(&held.pidfd, Signal::KILL);
        let _ = held.child.wait();
    }

    /// A policy in `Locked`, a proof from it, and the policy kept so the
    /// generation can move.
    fn proof() -> (LockPolicy, LockedProof) {
        super::super::test_support::locked()
    }

    /// Waits until `condition` holds.
    fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn events_of(seen: &Seen) -> Vec<Event> {
        seen.events.lock().expect("events").clone()
    }

    #[test]
    fn a_spawn_asked_right_after_a_kill_waits_for_the_reaping_instead_of_failing() {
        // Finding I2: the policy kills the child it gave up on and is idle at
        // once; the next request must not meet "one is held".
        let seen = Seen::default();
        let (policy, _) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        run.child = Some(held("echo unlocked; sleep 30"));
        assert!(run.command(Request::Kill));
        assert!(run.command(Request::Spawn {
            prior_failures: 0,
            after: Duration::ZERO,
        }));
        assert!(events_of(&seen).is_empty(), "{:?}", events_of(&seen));
        assert!(run.pending.is_some(), "the spawn waits");
        // The child dies; its reaping publishes nothing the policy is owed.
        let pidfd_ready = |run: &Run| run.pidfd_ready();
        eventually("the killed child to die", || pidfd_ready(&run));
        assert!(run.child_exited());
        assert!(run.child.is_none());
        assert!(
            events_of(&seen).is_empty(),
            "no stray exit for the policy: {:?}",
            events_of(&seen)
        );
        assert!(run.pending.is_some(), "and the spawn is still waiting");
    }

    #[test]
    fn a_killed_childs_exit_is_never_told_even_with_no_spawn_waiting() {
        // Finding I-B: the generation moved when the policy killed it, and a
        // spawn may arrive after the reaping; an exit published here would
        // meet the next child's policy state.
        let seen = Seen::default();
        let (policy, _) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        run.child = Some(held("echo requested; sleep 30"));
        std::thread::sleep(Duration::from_millis(300));
        assert!(run.command(Request::Kill));
        eventually("the killed child to die", || run.pidfd_ready());
        assert!(run.child_exited());
        assert!(run.child.is_none(), "reaped");
        assert!(
            events_of(&seen).is_empty(),
            "neither its lines nor its end are told: {:?}",
            events_of(&seen)
        );
    }

    #[test]
    fn a_spawn_while_a_wanted_child_is_held_is_still_a_failure() {
        let seen = Seen::default();
        let (policy, _) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        run.child = Some(held("sleep 30"));
        assert!(run.command(Request::Spawn {
            prior_failures: 0,
            after: Duration::ZERO,
        }));
        assert_eq!(events_of(&seen), [Event::SpawnFailed]);
        end(run.child.take().expect("held"));
    }

    #[test]
    fn the_lines_of_a_killed_child_are_dropped() {
        let seen = Seen::default();
        let (policy, _) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        run.child = Some(held("echo requested; sleep 30"));
        std::thread::sleep(Duration::from_millis(300));
        assert!(run.command(Request::Kill));
        assert!(run.read_stdout());
        assert!(events_of(&seen).is_empty(), "{:?}", events_of(&seen));
        end(run.child.take().expect("held"));
    }

    #[test]
    fn a_spawn_that_cannot_start_reports_the_failure_before_the_pin_is_gone() {
        // Finding M1: with the pin first the policy forgets who asked.
        let seen = Seen::default();
        let (policy, _) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        assert!(run.start(0));
        assert_eq!(events_of(&seen), [Event::SpawnFailed, Event::Pinned(false)]);
    }

    #[test]
    fn t_p10_the_proof_is_dropped_when_the_childs_pidfd_is_readable_at_hand_off() {
        let (policy, live_proof) = proof();
        let seen = Seen::default();
        let mut run = run_over(&seen, policy.generation_reader());
        // A live child: the proof goes on.
        run.child = Some(held("sleep 30"));
        run.hand_on(live_proof);
        assert_eq!(seen.proofs.lock().expect("proofs").len(), 1);
        end(run.child.take().expect("held"));

        // A child that has exited but was not reaped yet: its pidfd is
        // readable, the proof is dropped.
        let (policy, dead_proof) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        let dead = held("exit 0");
        eventually("the child to exit", || readable(&dead.pidfd));
        run.child = Some(dead);
        run.hand_on(dead_proof);
        assert_eq!(
            seen.proofs.lock().expect("proofs").len(),
            1,
            "the dead child's proof was dropped"
        );

        // No child at all.
        let (policy, orphan_proof) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        run.hand_on(orphan_proof);
        assert_eq!(seen.proofs.lock().expect("proofs").len(), 1);
    }

    #[test]
    fn a_stale_proof_or_an_unlocked_line_in_the_pipe_is_dropped_at_hand_off() {
        let seen = Seen::default();
        // Stale: the policy's generation moved after the proof was made.
        let (mut policy, stale) = proof();
        policy.handle(
            Instant::now(),
            PolicyEvent::Line(celestina_lock_proto::StatusLine::Unlocked),
        );
        let mut run = run_over(&seen, policy.generation_reader());
        run.child = Some(held("sleep 30"));
        run.hand_on(stale);
        assert!(seen.proofs.lock().expect("proofs").is_empty());
        end(run.child.take().expect("held"));

        // An `unlocked` the child already wrote, which the policy has not
        // seen yet: the generation still matches, the pipe says otherwise.
        let (policy, fresh) = proof();
        let mut run = run_over(&seen, policy.generation_reader());
        run.child = Some(held("echo unlocked; sleep 30"));
        // Give the child time to write.
        std::thread::sleep(Duration::from_millis(300));
        run.hand_on(fresh);
        end(run.child.take().expect("held"));
        assert!(
            seen.proofs.lock().expect("proofs").is_empty(),
            "the proof was dropped"
        );
        assert!(
            seen.events
                .lock()
                .expect("events")
                .contains(&Event::Line(celestina_lock_proto::StatusLine::Unlocked)),
            "and the policy still hears the line"
        );
    }
}
