//! The compositor's key bindings, read from niri's `config.kdl` once each time
//! the overview's Shortcuts mode opens (CEL-2-L, design §5 and §6).
//!
//! The file is the person's own but still hostile input, and the `kdl` parser
//! recurses: small crafted text overflows any stack and aborts the process
//! that parses it, which no lexical pre-scan fully prevents. So the shell
//! never parses it itself. [`load`] runs the whole read (open, bounds,
//! includes, parse) in a child process: the shell's own binary re-executed
//! with the internal mode [`PROBE_FLAG`] (decided by the author on
//! 2026-10-17). The child has an empty environment, no stdin and no stderr;
//! it prints one JSON answer ([`encode`]) on stdout and exits. The parent
//! reads at most [`MAX_PROBE_OUTPUT`] bytes, waits at most [`PROBE_DEADLINE`]
//! and kills the child past it; any other exit, a signal, or output that is
//! too long or does not decode ([`decode`]) is
//! [`KeybindsUnavailable::Invalid`]; a child that cannot be started at all is
//! [`KeybindsUnavailable::Unreadable`]. The parent never touches the path
//! itself: whether it is missing, not a file or too large is the child's
//! answer, inside the child's deadline. What the child answers is never
//! trusted further: [`decode`] re-applies the model's bounds. The child also
//! lowers its own core-dump, address-space and CPU limits before it opens
//! anything ([`confine`]), so a hostile file costs neither a queued core dump
//! nor unbounded memory or time.
//!
//! In the child, [`load_here`] opens the file without blocking (a FIFO named
//! `config.kdl` cannot hang it), requires a regular file of at most
//! [`MAX_FILE_BYTES`] and reads through a cap. Parsing is
//! `celestina_shell::keybinds`'s. `include` paths the parser reports are
//! followed when, after symlinks are resolved, they stay inside the root
//! config's directory, at most [`MAX_INCLUDE_DEPTH`] levels deep and
//! [`MAX_INCLUDES`] files in all; a missing, unreadable or invalid include is
//! skipped, since the root file alone already tells the person something. The
//! groups of every file are merged inside the model's own bounds.
//!
//! There is no watcher and no standing thread: [`spawn_load`] starts one named
//! worker that runs the child once and publishes [`Message::KeybindsLoaded`],
//! tagged with the overview's opening so a late answer is recognised. The
//! worker's stop flag is checked every [`POLL`] while the child runs, and a
//! stopped worker kills and reaps its child.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::bounded::bounded;
use celestina_shell::keybinds::{
    parse_file, Bind, BindGroup, KeybindError, MAX_BINDS, MAX_CHORD_UNITS, MAX_FILE_BYTES,
    MAX_GROUPS, MAX_INCLUDES, MAX_INCLUDE_DEPTH, MAX_LABEL_UNITS,
};
use celestina_wayland::Sender;
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::fs::OFlags;
use rustix::io::Errno;
use rustix::process::{setrlimit, Resource, Rlimit};
use serde_json::{json, Value};

use super::Worker;
use crate::message::Message;

/// The internal argument that makes `celestina` the bindings probe: the
/// binary reads the config at the path that follows, prints the answer and
/// exits. Not a verb and not in the usage text; only [`load`] passes it.
pub const PROBE_FLAG: &str = "--keybinds-probe";
/// How long the probe has to answer, from its start to its exit.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(3);
/// How often the parent looks at its stop flag while the child runs.
const POLL: Duration = Duration::from_millis(50);
/// The child's address-space limit.
const PROBE_MEMORY_LIMIT: u64 = 1 << 30;
/// The child's CPU-time limit, in seconds (the deadline is 3 s of wall time).
const PROBE_CPU_SECONDS: u64 = 5;
/// The most bytes of answer the parent reads. The largest honest answer
/// ([`MAX_BINDS`] binds whose four fields are at their caps, every character
/// escaped by JSON) is about 1 MiB.
pub const MAX_PROBE_OUTPUT: usize = 2 * 1024 * 1024;
/// The answer's format; a probe of another format is not believed.
const WIRE_VERSION: u64 = 1;

/// What the shortcuts page has to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Keybinds {
    /// The merged groups, normal binds first when the file lists them first.
    Groups(Vec<BindGroup>),
    /// There is nothing to show, and why.
    Unavailable(KeybindsUnavailable),
}

/// Why no bindings could be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeybindsUnavailable {
    /// No config path could be resolved, or no file is there.
    Missing,
    /// The path is not a regular file, or reading it failed.
    Unreadable,
    /// The file is over [`MAX_FILE_BYTES`].
    TooLarge,
    /// The file is not UTF-8 text or not valid KDL, or the probe that reads
    /// it failed.
    Invalid,
}

impl KeybindsUnavailable {
    /// The reason's token in the probe's answer.
    fn word(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
            Self::TooLarge => "too-large",
            Self::Invalid => "invalid",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        [
            Self::Missing,
            Self::Unreadable,
            Self::TooLarge,
            Self::Invalid,
        ]
        .into_iter()
        .find(|why| why.word() == word)
    }
}

/// The environment the config path depends on, passed in so a test uses a
/// private one.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// `$NIRI_CONFIG`.
    pub niri_config: Option<OsString>,
    /// `$XDG_CONFIG_HOME`.
    pub xdg_config_home: Option<OsString>,
    /// `$HOME`.
    pub home: Option<OsString>,
}

impl Env {
    /// The process's own environment.
    #[must_use]
    pub fn from_process() -> Env {
        Env {
            niri_config: std::env::var_os("NIRI_CONFIG"),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
            home: std::env::var_os("HOME"),
        }
    }
}

/// The config path: `$NIRI_CONFIG`, then `$XDG_CONFIG_HOME/niri/config.kdl`,
/// then `$HOME/.config/niri/config.kdl`. An empty value is unset, and a
/// relative `$XDG_CONFIG_HOME` is ignored, as the XDG specification says. A
/// relative `$NIRI_CONFIG` is made absolute against the current directory, so
/// the config's includes (which are confined to its directory) are not lost;
/// when the current directory is unknown it is ignored, with a log.
#[must_use]
pub fn resolve_path(env: &Env) -> Option<PathBuf> {
    let set =
        |value: &Option<OsString>| value.as_ref().filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(path) = set(&env.niri_config) {
        match std::path::absolute(&path) {
            Ok(path) => return Some(path),
            Err(err) => {
                tracing::warn!(%err, "keybinds: NIRI_CONFIG is relative and the current directory is unknown; ignored");
            }
        }
    }
    if let Some(base) = set(&env.xdg_config_home).filter(|p| p.is_absolute()) {
        return Some(base.join("niri").join("config.kdl"));
    }
    set(&env.home).map(|home| home.join(".config").join("niri").join("config.kdl"))
}

/// Reads the bindings of the config the process's environment names, through
/// the probe. Blocking (up to [`PROBE_DEADLINE`], or until `cancel` is
/// raised): call it off the UI thread ([`spawn_load`] does).
#[must_use]
pub fn load_default(cancel: &AtomicBool) -> Keybinds {
    match resolve_path(&Env::from_process()) {
        Some(path) => load(&path, cancel),
        None => Keybinds::Unavailable(KeybindsUnavailable::Missing),
    }
}

/// Reads the bindings of the config at `path` and of the files it includes,
/// in a probe child of this very binary. Blocking and bounded; never panics,
/// and a parser that crashes takes only the child with it. Raising `cancel`
/// kills the child and returns early.
#[must_use]
pub fn load(path: &Path, cancel: &AtomicBool) -> Keybinds {
    match own_binary() {
        Some(exe) => load_with(&exe, path, cancel),
        None => {
            tracing::warn!("keybinds: the shell's own binary cannot be found; no probe");
            Keybinds::Unavailable(KeybindsUnavailable::Unreadable)
        }
    }
}

/// The binary to re-execute as the probe. `/proc/self/exe` first: it still
/// names the running image after a deployment has replaced the file on disk,
/// where the path `current_exe` reports would be gone.
fn own_binary() -> Option<PathBuf> {
    let proc_exe = Path::new("/proc/self/exe");
    if proc_exe.exists() {
        return Some(proc_exe.to_path_buf());
    }
    std::env::current_exe().ok()
}

/// [`load`] through the probe binary `exe` (a test passes the built binary).
#[must_use]
pub fn load_with(exe: &Path, path: &Path, cancel: &AtomicBool) -> Keybinds {
    // A relative path is made absolute here, not in the child, so the
    // includes' directory is the one the caller meant.
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut command = Command::new(exe);
    command.arg(PROBE_FLAG).arg(&path).env_clear();
    match run_probe(command, PROBE_DEADLINE, MAX_PROBE_OUTPUT, cancel) {
        Ok(answer) => decode(&answer).unwrap_or_else(|| {
            tracing::warn!("keybinds: the probe's answer does not decode");
            Keybinds::Unavailable(KeybindsUnavailable::Invalid)
        }),
        Err(failure) => {
            tracing::warn!("keybinds: the probe failed: {failure}");
            Keybinds::Unavailable(failure.unavailable())
        }
    }
}

/// The probe's side: the body of `celestina --keybinds-probe PATH`. Reads
/// `path` in this process, prints the answer on stdout and exits 0; exits 1
/// when the answer cannot be written.
#[must_use]
pub fn probe_main(path: &Path) -> ExitCode {
    confine();
    let answer = encode(&load_here(path));
    let mut out = io::stdout().lock();
    match out.write_all(answer.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

/// Lowers this process's resource limits before it reads a hostile file: no
/// core dump (a crash on crafted text would otherwise queue one), at most
/// [`PROBE_MEMORY_LIMIT`] of address space and [`PROBE_CPU_SECONDS`] of CPU.
/// Best effort: a limit the kernel refuses to lower is left as it was, since
/// the parent's deadline and size cap still bound the child.
fn confine() {
    for (resource, limit) in [
        (Resource::Core, 0),
        (Resource::As, PROBE_MEMORY_LIMIT),
        (Resource::Cpu, PROBE_CPU_SECONDS),
    ] {
        let _ = setrlimit(
            resource,
            Rlimit {
                current: Some(limit),
                maximum: Some(limit),
            },
        );
    }
}

/// The probe's answer as one JSON object:
/// `{"v":1,"groups":[{"name":…,"enter":…,"binds":[{"chord":…,"title":…,"action":…,"program":…}]}]}`
/// or `{"v":1,"unavailable":"missing|unreadable|too-large|invalid"}`.
#[must_use]
pub fn encode(keybinds: &Keybinds) -> String {
    let value = match keybinds {
        Keybinds::Groups(groups) => {
            let groups: Vec<Value> = groups
                .iter()
                .map(|group| {
                    let binds: Vec<Value> = group
                        .binds
                        .iter()
                        .map(|bind| {
                            json!({
                                "chord": bind.chord,
                                "title": bind.title,
                                "action": bind.action,
                                "program": bind.program,
                            })
                        })
                        .collect();
                    json!({ "name": group.name, "enter": group.enter, "binds": binds })
                })
                .collect();
            json!({ "v": WIRE_VERSION, "groups": groups })
        }
        Keybinds::Unavailable(why) => json!({ "v": WIRE_VERSION, "unavailable": why.word() }),
    };
    value.to_string()
}

/// Reads a probe's answer, or `None` when it is not one. Every string is cut
/// to the model's bound for its kind and the groups and binds are kept within
/// [`MAX_GROUPS`] and [`MAX_BINDS`], as a parse in this process would.
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<Keybinds> {
    if bytes.len() > MAX_PROBE_OUTPUT {
        return None;
    }
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let object = value.as_object()?;
    if object.get("v")?.as_u64()? != WIRE_VERSION {
        return None;
    }
    if let Some(why) = object.get("unavailable") {
        return KeybindsUnavailable::from_word(why.as_str()?).map(Keybinds::Unavailable);
    }
    // A string, or `null` for an absent one; anything else is not an answer.
    let text = |value: Option<&Value>, units: usize| -> Option<Option<String>> {
        match value {
            None | Some(Value::Null) => Some(None),
            Some(Value::String(text)) => Some(Some(bounded(text, units))),
            Some(_) => None,
        }
    };
    let mut merged = Merged::default();
    for group in object.get("groups")?.as_array()? {
        let group = group.as_object()?;
        let mut binds = Vec::new();
        for bind in group.get("binds")?.as_array()? {
            if binds.len() >= MAX_BINDS {
                break;
            }
            let bind = bind.as_object()?;
            binds.push(Bind {
                chord: text(bind.get("chord"), MAX_CHORD_UNITS)??,
                title: text(bind.get("title"), MAX_LABEL_UNITS)?,
                action: text(bind.get("action"), MAX_LABEL_UNITS)??,
                program: text(bind.get("program"), MAX_LABEL_UNITS)?,
            });
        }
        merged.add(vec![BindGroup {
            name: text(group.get("name"), MAX_LABEL_UNITS)?,
            enter: text(group.get("enter"), MAX_CHORD_UNITS)?,
            binds,
        }]);
    }
    Some(Keybinds::Groups(merged.groups))
}

/// Why a probe gave no answer.
#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    /// The child could not be started or waited for.
    #[error("cannot run the probe")]
    Spawn(#[source] io::Error),
    /// Reading its answer failed.
    #[error("cannot read the probe's answer")]
    Read(#[source] io::Error),
    /// It said more than the parent reads.
    #[error("the probe's answer is over {0} bytes")]
    Oversize(usize),
    /// It did not finish in time and was killed.
    #[error("the probe did not finish within {} ms", .0.as_millis())]
    Timeout(Duration),
    /// It exited with a failure or was killed by a signal.
    #[error("the probe ended with {0}")]
    Failed(std::process::ExitStatus),
    /// The caller gave up and the child was killed.
    #[error("the probe was cancelled")]
    Cancelled,
}

impl ProbeError {
    /// What the shortcuts page says for a probe that gave no answer: a probe
    /// that could not be run or read, or was given up, is unreadable; one that
    /// ran and failed, said too much or took too long met bad text.
    fn unavailable(&self) -> KeybindsUnavailable {
        match self {
            Self::Spawn(_) | Self::Read(_) | Self::Cancelled => KeybindsUnavailable::Unreadable,
            Self::Oversize(_) | Self::Timeout(_) | Self::Failed(_) => KeybindsUnavailable::Invalid,
        }
    }
}

/// Runs `command` with stdin and stderr closed and stdout piped, and returns
/// what it printed. The child is killed when it prints more than `cap` bytes
/// or is not done by `deadline`, or as soon as `cancel` is raised (looked at
/// every [`POLL`]); it is always waited for, so no zombie stays.
///
/// # Errors
///
/// [`ProbeError`]: it could not run, said too much, took too long, failed or
/// was cancelled.
pub fn run_probe(
    mut command: Command,
    deadline: Duration,
    cap: usize,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ProbeError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let started = Instant::now();
    let mut child = command.spawn().map_err(ProbeError::Spawn)?;
    let read = match child.stdout.take() {
        Some(stdout) => read_capped(OwnedFd::from(stdout), started, deadline, cap, cancel),
        None => Err(ProbeError::Spawn(io::Error::other("no stdout pipe"))),
    };
    let answer = match read {
        Ok(answer) => answer,
        Err(failure) => {
            end(&mut child);
            return Err(failure);
        }
    };
    // The pipe is closed: the child is exiting, or closed it and lingers.
    loop {
        if cancel.load(Ordering::Acquire) {
            end(&mut child);
            return Err(ProbeError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(answer),
            Ok(Some(status)) => return Err(ProbeError::Failed(status)),
            Ok(None) if started.elapsed() >= deadline => {
                end(&mut child);
                return Err(ProbeError::Timeout(deadline));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(err) => {
                end(&mut child);
                return Err(ProbeError::Spawn(err));
            }
        }
    }
}

/// Kills `child` (it may already be gone) and reaps it.
fn end(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Reads `pipe` to its end, at most `cap` bytes, waiting no later than
/// `deadline` after `started` and looking at `cancel` every [`POLL`].
fn read_capped(
    pipe: OwnedFd,
    started: Instant,
    deadline: Duration,
    cap: usize,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ProbeError> {
    let mut file = File::from(pipe);
    let mut answer = Vec::new();
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(ProbeError::Cancelled);
        }
        let Some(remaining) = deadline
            .checked_sub(started.elapsed())
            .filter(|left| !left.is_zero())
        else {
            return Err(ProbeError::Timeout(deadline));
        };
        let timeout = Timespec::try_from(remaining.min(POLL)).unwrap_or(Timespec {
            tv_sec: 0,
            tv_nsec: 10_000_000,
        });
        let ready = {
            let mut fds = [PollFd::new(&file, PollFlags::IN)];
            match poll(&mut fds, Some(&timeout)) {
                Ok(count) => count > 0,
                Err(Errno::INTR) => false,
                Err(errno) => return Err(ProbeError::Read(io::Error::from(errno))),
            }
        };
        if !ready {
            continue;
        }
        // Readable (or hung up): one read returns what is there, or 0 at the
        // end, without blocking.
        match file.read(&mut chunk) {
            Ok(0) => return Ok(answer),
            Ok(count) => {
                answer.extend_from_slice(chunk.get(..count).unwrap_or_default());
                if answer.len() > cap {
                    return Err(ProbeError::Oversize(cap));
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(ProbeError::Read(err)),
        }
    }
}

/// Reads the bindings of the config at `path` and of the files it includes,
/// in this process. Only the probe ([`probe_main`]) and pure tests call this:
/// the parser it runs can abort the process on crafted text.
#[must_use]
pub fn load_here(path: &Path) -> Keybinds {
    let root = match read_text(path) {
        Ok(text) => text,
        Err(why) => return Keybinds::Unavailable(why),
    };
    let parsed = match parse_file(&root) {
        Ok(parsed) => parsed,
        Err(KeybindError::TooLarge { .. }) => {
            return Keybinds::Unavailable(KeybindsUnavailable::TooLarge)
        }
        Err(KeybindError::Syntax(_)) => return Keybinds::Unavailable(KeybindsUnavailable::Invalid),
    };
    let mut merged = Merged::default();
    merged.add(parsed.groups);
    // The directory includes must stay in; unresolvable means no include is
    // followed (the root was read, so this is only a vanished directory).
    let jail = path
        .parent()
        .map(Path::to_path_buf)
        .and_then(|dir| dir.canonicalize().ok());
    if let (Some(jail), Some(dir)) = (jail, path.parent()) {
        let mut seen = HashSet::new();
        if let Ok(own) = path.canonicalize() {
            seen.insert(own);
        }
        let mut read = 0usize;
        follow(
            &Follow { jail: &jail },
            dir,
            &parsed.includes,
            1,
            &mut seen,
            &mut read,
            &mut merged,
        );
    }
    Keybinds::Groups(merged.groups)
}

struct Follow<'a> {
    jail: &'a Path,
}

/// Reads each of `includes` (relative to `dir`) at `depth`, recursively.
fn follow(
    ctx: &Follow<'_>,
    dir: &Path,
    includes: &[String],
    depth: usize,
    seen: &mut HashSet<PathBuf>,
    read: &mut usize,
    merged: &mut Merged,
) {
    if depth > MAX_INCLUDE_DEPTH {
        return;
    }
    for name in includes {
        if *read >= MAX_INCLUDES {
            return;
        }
        let Ok(resolved) = dir.join(name).canonicalize() else {
            continue;
        };
        if !resolved.starts_with(ctx.jail) || !seen.insert(resolved.clone()) {
            continue;
        }
        *read += 1;
        let Ok(text) = read_text(&resolved) else {
            continue;
        };
        let Ok(parsed) = parse_file(&text) else {
            continue;
        };
        merged.add(parsed.groups);
        if let Some(own_dir) = resolved.parent() {
            follow(
                ctx,
                own_dir,
                &parsed.includes,
                depth + 1,
                seen,
                read,
                merged,
            );
        }
    }
}

/// The groups of several files, one group per name, inside the model's
/// bounds.
#[derive(Default)]
struct Merged {
    groups: Vec<BindGroup>,
    binds: usize,
}

impl Merged {
    fn add(&mut self, groups: Vec<BindGroup>) {
        for group in groups {
            let at = match self.groups.iter().position(|g| g.name == group.name) {
                Some(at) => at,
                None if self.groups.len() >= MAX_GROUPS => continue,
                None => {
                    self.groups.push(BindGroup {
                        name: group.name.clone(),
                        enter: group.enter.clone(),
                        binds: Vec::new(),
                    });
                    self.groups.len() - 1
                }
            };
            let Some(target) = self.groups.get_mut(at) else {
                continue;
            };
            if target.enter.is_none() {
                target.enter = group.enter;
            }
            for bind in group.binds {
                if self.binds >= MAX_BINDS {
                    return;
                }
                target.binds.push(bind);
                self.binds += 1;
            }
        }
    }
}

/// Opens `path` without blocking and reads at most [`MAX_FILE_BYTES`] of it
/// as text.
fn read_text(path: &Path) -> Result<String, KeybindsUnavailable> {
    let unreadable = |err: &io::Error| match err.kind() {
        io::ErrorKind::NotFound => KeybindsUnavailable::Missing,
        _ => KeybindsUnavailable::Unreadable,
    };
    // `stat` first: a FIFO, device or directory is refused before it is opened.
    let before = std::fs::metadata(path).map_err(|e| unreadable(&e))?;
    if !before.is_file() {
        return Err(KeybindsUnavailable::Unreadable);
    }
    let descriptor = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
        rustix::fs::Mode::empty(),
    )
    .map_err(|errno| unreadable(&io::Error::from(errno)))?;
    let file = File::from(descriptor);
    let metadata = file.metadata().map_err(|e| unreadable(&e))?;
    if !metadata.is_file() {
        return Err(KeybindsUnavailable::Unreadable);
    }
    let limit = u64::try_from(MAX_FILE_BYTES).unwrap_or(u64::MAX);
    if metadata.len() > limit {
        return Err(KeybindsUnavailable::TooLarge);
    }
    let mut bytes = Vec::new();
    // One byte past the limit tells a file that grew after the check.
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| unreadable(&e))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(KeybindsUnavailable::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| KeybindsUnavailable::Invalid)
}

/// Reads the bindings of the environment's config through the probe, on a
/// named worker thread, and publishes [`Message::KeybindsLoaded`] once,
/// tagged with `generation` (the overview's opening). Call it each time the
/// Shortcuts mode opens; keep the returned [`Worker`] until the message
/// arrives, and drop it off the loop's thread (dropping it waits for the
/// probe, at most [`PROBE_DEADLINE`]).
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_load(sender: Sender<Message>, generation: u64) -> io::Result<Worker> {
    spawn_load_from(None, sender, generation)
}

/// [`spawn_load`] for an explicit config `path` (`None`: the environment's).
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_load_from(
    path: Option<PathBuf>,
    sender: Sender<Message>,
    generation: u64,
) -> io::Result<Worker> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let cancel = Arc::clone(&shutdown);
    Worker::spawn("shell-keybinds", &shutdown, move || {
        let keybinds = match path {
            Some(path) => load(&path, &cancel),
            None => load_default(&cancel),
        };
        // The loop being gone is the only failure: nothing to publish to.
        let _ = sender.send(Message::KeybindsLoaded {
            generation,
            keybinds,
        });
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Scratch {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "celestina-keybinds-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create scratch dir");
            Scratch(dir)
        }

        fn write(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(&path, text).expect("write file");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn groups(keybinds: Keybinds) -> Vec<BindGroup> {
        match keybinds {
            Keybinds::Groups(groups) => groups,
            Keybinds::Unavailable(why) => panic!("unavailable: {why:?}"),
        }
    }

    fn chords(group: &BindGroup) -> Vec<&str> {
        group.binds.iter().map(|b| b.chord.as_str()).collect()
    }

    fn env(niri: Option<&str>, xdg: Option<&str>, home: Option<&str>) -> Env {
        Env {
            niri_config: niri.map(OsString::from),
            xdg_config_home: xdg.map(OsString::from),
            home: home.map(OsString::from),
        }
    }

    #[test]
    fn the_path_prefers_niri_config_then_xdg_then_home() {
        let all = env(Some("/n/c.kdl"), Some("/x"), Some("/h"));
        assert_eq!(resolve_path(&all), Some(PathBuf::from("/n/c.kdl")));
        let xdg = env(None, Some("/x"), Some("/h"));
        assert_eq!(
            resolve_path(&xdg),
            Some(PathBuf::from("/x/niri/config.kdl"))
        );
        let home = env(None, None, Some("/h"));
        assert_eq!(
            resolve_path(&home),
            Some(PathBuf::from("/h/.config/niri/config.kdl"))
        );
        assert_eq!(resolve_path(&env(None, None, None)), None);
    }

    #[test]
    fn empty_and_relative_values_are_ignored() {
        let empty = env(Some(""), Some(""), Some("/h"));
        assert_eq!(
            resolve_path(&empty),
            Some(PathBuf::from("/h/.config/niri/config.kdl"))
        );
        let relative = env(None, Some("rel"), Some("/h"));
        assert_eq!(
            resolve_path(&relative),
            Some(PathBuf::from("/h/.config/niri/config.kdl"))
        );
    }

    #[test]
    fn the_fixture_loads_into_its_groups() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../celestina-rs/crates/celestina-shell/tests/fixtures/niri-binds.kdl");
        let loaded = groups(load_here(&path));
        let names: Vec<_> = loaded.iter().map(|g| g.name.as_deref()).collect();
        assert_eq!(names, [Some("monitor"), Some("size"), None]);
        assert!(loaded
            .iter()
            .any(|g| g.name.is_none() && chords(g).contains(&"Mod+Q")));
    }

    #[test]
    fn a_missing_file_is_missing() {
        let dir = Scratch::new();
        assert_eq!(
            load_here(&dir.0.join("nope.kdl")),
            Keybinds::Unavailable(KeybindsUnavailable::Missing)
        );
    }

    #[test]
    fn an_oversize_file_is_too_large_without_being_read() {
        let dir = Scratch::new();
        let path = dir.write("c.kdl", &" ".repeat(MAX_FILE_BYTES + 1));
        assert_eq!(
            load_here(&path),
            Keybinds::Unavailable(KeybindsUnavailable::TooLarge)
        );
        let at_limit = dir.write("d.kdl", &" ".repeat(MAX_FILE_BYTES));
        assert_eq!(load_here(&at_limit), Keybinds::Groups(Vec::new()));
    }

    #[test]
    fn a_fifo_is_refused_without_blocking() {
        let dir = Scratch::new();
        let path = dir.0.join("config.kdl");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &path,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .expect("make a fifo");
        assert_eq!(
            load_here(&path),
            Keybinds::Unavailable(KeybindsUnavailable::Unreadable)
        );
        // A directory is not a config either.
        assert_eq!(
            load_here(&dir.0),
            Keybinds::Unavailable(KeybindsUnavailable::Unreadable)
        );
    }

    #[test]
    fn invalid_kdl_and_invalid_utf8_are_invalid() {
        let dir = Scratch::new();
        let bad = dir.write("a.kdl", "binds {");
        assert_eq!(
            load_here(&bad),
            Keybinds::Unavailable(KeybindsUnavailable::Invalid)
        );
        let path = dir.0.join("b.kdl");
        fs::write(&path, [0xff, 0xfe, 0x00]).expect("write bytes");
        assert_eq!(
            load_here(&path),
            Keybinds::Unavailable(KeybindsUnavailable::Invalid)
        );
    }

    #[test]
    fn a_deeply_nested_file_is_invalid_not_a_crash() {
        let dir = Scratch::new();
        let braces = dir.write("a.kdl", &"a{".repeat(500));
        assert_eq!(
            load_here(&braces),
            Keybinds::Unavailable(KeybindsUnavailable::Invalid)
        );
        let comments = dir.write("b.kdl", &"/*".repeat(1000));
        assert_eq!(
            load_here(&comments),
            Keybinds::Unavailable(KeybindsUnavailable::Invalid)
        );
    }

    #[test]
    fn includes_inside_the_directory_are_merged_by_group() {
        let dir = Scratch::new();
        dir.write(
            "more.kdl",
            "binds { Mod+B { spawn \"b\"; } }\ninclude \"deep/last.kdl\"",
        );
        dir.write("deep/last.kdl", "binds { Mod+C { spawn \"c\"; } }");
        let root = dir.write(
            "config.kdl",
            "binds { Mod+A { spawn \"a\"; } }\ninclude \"more.kdl\"\ninclude \"missing.kdl\"",
        );
        let loaded = groups(load_here(&root));
        assert_eq!(loaded.len(), 1);
        assert_eq!(chords(&loaded[0]), ["Mod+A", "Mod+B", "Mod+C"]);
    }

    #[test]
    fn includes_outside_the_directory_are_not_followed() {
        let dir = Scratch::new();
        dir.write("outside.kdl", "binds { Mod+X { spawn \"x\"; } }");
        let inner = dir.0.join("inner");
        let outside = dir.0.join("outside.kdl");
        let root = dir.write(
            "inner/config.kdl",
            &format!(
                "binds {{ Mod+A {{ spawn \"a\"; }} }}\ninclude \"../outside.kdl\"\ninclude \"{}\"\ninclude \"link.kdl\"",
                outside.display()
            ),
        );
        std::os::unix::fs::symlink(&outside, inner.join("link.kdl")).expect("symlink");
        let loaded = groups(load_here(&root));
        assert_eq!(chords(&loaded[0]), ["Mod+A"]);
    }

    #[test]
    fn include_depth_and_cycles_are_bounded() {
        let dir = Scratch::new();
        dir.write(
            "l1.kdl",
            "binds { L1 { spawn \"1\"; } }\ninclude \"l2.kdl\"",
        );
        dir.write(
            "l2.kdl",
            "binds { L2 { spawn \"2\"; } }\ninclude \"l3.kdl\"",
        );
        dir.write("l3.kdl", "binds { L3 { spawn \"3\"; } }");
        let root = dir.write("config.kdl", "include \"l1.kdl\"\ninclude \"config.kdl\"");
        let loaded = groups(load_here(&root));
        assert_eq!(chords(&loaded[0]), ["L1", "L2"], "depth 3 is not read");
    }

    #[test]
    fn at_most_the_included_file_count_is_read() {
        let dir = Scratch::new();
        let mut root = String::new();
        for i in 0..(MAX_INCLUDES + 1) {
            dir.write(
                &format!("f{i}.kdl"),
                &format!("binds {{ K{i} {{ spawn \"x\"; }} }}"),
            );
            root.push_str(&format!("include \"f{i}.kdl\"\n"));
        }
        let root = dir.write("config.kdl", &root);
        // The parser keeps MAX_INCLUDES paths of one file; the reader reads
        // no more than that either way.
        let loaded = groups(load_here(&root));
        let total: usize = loaded.iter().map(|g| g.binds.len()).sum();
        assert_eq!(total, MAX_INCLUDES);
    }

    #[test]
    fn merging_respects_the_model_bounds() {
        let bind = |n: usize| Bind {
            chord: format!("K{n}"),
            title: None,
            action: "quit".to_owned(),
            program: None,
        };
        let mut merged = Merged::default();
        for g in 0..(MAX_GROUPS + 5) {
            merged.add(vec![BindGroup {
                name: Some(format!("g{g}")),
                enter: None,
                binds: vec![bind(g)],
            }]);
        }
        assert_eq!(merged.groups.len(), MAX_GROUPS);
        let mut merged = Merged::default();
        merged.add(vec![BindGroup {
            name: None,
            enter: None,
            binds: (0..MAX_BINDS).map(bind).collect(),
        }]);
        merged.add(vec![BindGroup {
            name: None,
            enter: None,
            binds: vec![bind(9999)],
        }]);
        assert_eq!(merged.groups[0].binds.len(), MAX_BINDS);
    }

    fn bind(chord: &str, title: Option<&str>, action: &str, program: Option<&str>) -> Bind {
        Bind {
            chord: chord.to_owned(),
            title: title.map(str::to_owned),
            action: action.to_owned(),
            program: program.map(str::to_owned),
        }
    }

    #[test]
    fn the_answer_round_trips() {
        let groups = vec![
            BindGroup {
                name: Some("resize \"wide\"\n".to_owned()),
                enter: Some("Mod+R".to_owned()),
                binds: vec![bind("Left", None, "set-column-width", None)],
            },
            BindGroup {
                name: None,
                enter: None,
                binds: vec![
                    bind(
                        "Mod+T",
                        Some("Abrir terminal \u{1F600}"),
                        "spawn",
                        Some("kitty"),
                    ),
                    bind("Mod+H", Some("x"), "", None),
                ],
            },
        ];
        let answer = Keybinds::Groups(groups);
        assert_eq!(decode(encode(&answer).as_bytes()), Some(answer));
        for why in [
            KeybindsUnavailable::Missing,
            KeybindsUnavailable::Unreadable,
            KeybindsUnavailable::TooLarge,
            KeybindsUnavailable::Invalid,
        ] {
            let answer = Keybinds::Unavailable(why);
            assert_eq!(decode(encode(&answer).as_bytes()), Some(answer));
        }
        assert_eq!(
            decode(encode(&Keybinds::Groups(Vec::new())).as_bytes()),
            Some(Keybinds::Groups(Vec::new()))
        );
    }

    #[test]
    fn what_is_not_an_answer_does_not_decode() {
        for garbled in [
            &b""[..],
            b"\xff\xfe",
            b"[]",
            b"{\"groups\":[]}",
            b"{\"v\":2,\"groups\":[]}",
            b"{\"v\":1}",
            b"{\"v\":1,\"unavailable\":\"gone\"}",
            b"{\"v\":1,\"unavailable\":3}",
            b"{\"v\":1,\"groups\":[{\"binds\":[{\"chord\":1,\"action\":\"x\"}]}]}",
            b"{\"v\":1,\"groups\":[{\"binds\":[{\"chord\":\"A\"}]}]}",
            b"{\"v\":1,\"groups\":[{\"name\":[],\"binds\":[]}]}",
            b"{\"v\":1,\"groups\":[{\"name\":null}]}",
            b"{\"v\":1,\"groups\":[]",
        ] {
            assert_eq!(
                decode(garbled),
                None,
                "{}",
                String::from_utf8_lossy(garbled)
            );
        }
        let oversize = vec![b' '; MAX_PROBE_OUTPUT + 1];
        assert_eq!(decode(&oversize), None);
    }

    #[test]
    fn a_decoded_answer_is_held_to_the_model_bounds() {
        // A probe that lies: too many groups, too many binds, long strings.
        let long = "x".repeat(10_000);
        let mut groups: Vec<BindGroup> = (0..MAX_GROUPS + 8)
            .map(|g| BindGroup {
                name: Some(format!("m{g}{long}")),
                enter: Some(long.clone()),
                binds: vec![bind(&long, Some(&long), &long, Some(&long))],
            })
            .collect();
        groups.push(BindGroup {
            name: None,
            enter: None,
            binds: (0..MAX_BINDS + 50)
                .map(|n| bind(&format!("K{n}"), None, "quit", None))
                .collect(),
        });
        let lie = encode(&Keybinds::Groups(groups));
        let Some(Keybinds::Groups(kept)) = decode(lie.as_bytes()) else {
            panic!("a well-formed answer decodes");
        };
        assert_eq!(kept.len(), MAX_GROUPS);
        assert!(kept.iter().map(|g| g.binds.len()).sum::<usize>() <= MAX_BINDS);
        let first = &kept[0];
        assert!(first
            .name
            .as_ref()
            .is_some_and(|n| n.chars().count() <= MAX_LABEL_UNITS));
        assert!(first
            .enter
            .as_ref()
            .is_some_and(|e| e.chars().count() <= MAX_CHORD_UNITS));
        let b = &first.binds[0];
        assert_eq!(b.chord.chars().count(), MAX_CHORD_UNITS);
        assert_eq!(b.action.chars().count(), MAX_LABEL_UNITS);
        assert!(b
            .title
            .as_ref()
            .is_some_and(|t| t.chars().count() == MAX_LABEL_UNITS));
        assert!(b
            .program
            .as_ref()
            .is_some_and(|p| p.chars().count() == MAX_LABEL_UNITS));
    }

    #[test]
    fn the_largest_honest_answer_fits_the_read_cap() {
        // Every field at its cap and made of characters JSON escapes as
        // `\u00XX`, six bytes each.
        let worst = |units: usize| "\u{1}".repeat(units);
        let groups: Vec<BindGroup> = (0..MAX_GROUPS)
            .map(|g| BindGroup {
                name: Some(worst(MAX_LABEL_UNITS)),
                enter: Some(worst(MAX_CHORD_UNITS)),
                binds: if g == 0 {
                    (0..MAX_BINDS)
                        .map(|_| {
                            bind(
                                &worst(MAX_CHORD_UNITS),
                                Some(&worst(MAX_LABEL_UNITS)),
                                &worst(MAX_LABEL_UNITS),
                                Some(&worst(MAX_LABEL_UNITS)),
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            })
            .collect();
        assert!(encode(&Keybinds::Groups(groups)).len() <= MAX_PROBE_OUTPUT);
    }

    fn sh(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script);
        command
    }

    fn never() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn a_probe_that_never_answers_is_killed_at_the_deadline() {
        let started = Instant::now();
        let outcome = run_probe(
            sh("exec sleep 30"),
            Duration::from_millis(200),
            1024,
            &never(),
        );
        assert!(
            matches!(outcome, Err(ProbeError::Timeout(_))),
            "{outcome:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        // One that answers and then lingers is killed too.
        let started = Instant::now();
        let outcome = run_probe(
            sh("echo partial; exec sleep 30 >&-"),
            Duration::from_millis(200),
            1024,
            &never(),
        );
        assert!(
            matches!(outcome, Err(ProbeError::Timeout(_))),
            "{outcome:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_probe_that_says_too_much_fails_or_exits_badly_is_refused() {
        let flag = never();
        let outcome = run_probe(
            sh("head -c 5000 /dev/zero"),
            Duration::from_secs(3),
            1000,
            &flag,
        );
        assert!(
            matches!(outcome, Err(ProbeError::Oversize(1000))),
            "{outcome:?}"
        );
        let outcome = run_probe(sh("echo '{}'; exit 3"), Duration::from_secs(3), 1000, &flag);
        assert!(matches!(outcome, Err(ProbeError::Failed(_))), "{outcome:?}");
        let outcome = run_probe(sh("kill -ABRT $$"), Duration::from_secs(3), 1000, &flag);
        assert!(matches!(outcome, Err(ProbeError::Failed(_))), "{outcome:?}");
        let outcome = run_probe(
            Command::new("/nonexistent/celestina"),
            Duration::from_secs(3),
            1000,
            &flag,
        );
        assert!(matches!(outcome, Err(ProbeError::Spawn(_))), "{outcome:?}");
        let outcome = run_probe(sh("printf hello"), Duration::from_secs(3), 1000, &flag);
        assert_eq!(outcome.ok(), Some(b"hello".to_vec()));
    }

    /// Whether a process with this id still exists (a zombie counts: it has
    /// not been reaped).
    fn alive(pid: u32) -> bool {
        Path::new(&format!("/proc/{pid}")).exists()
    }

    #[test]
    fn a_raised_stop_flag_kills_and_reaps_the_child() {
        let dir = Scratch::new();
        let pid_file = dir.0.join("pid");
        let script = format!("echo $$ > {}; exec sleep 30", pid_file.display());
        let cancel = Arc::new(AtomicBool::new(false));
        let raiser = {
            let cancel = Arc::clone(&cancel);
            let pid_file = pid_file.clone();
            std::thread::spawn(move || {
                // Wait for the child to be up, then give up on it.
                while fs::read_to_string(&pid_file).map_or(true, |text| text.trim().is_empty()) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                cancel.store(true, Ordering::Release);
            })
        };
        let started = Instant::now();
        let outcome = run_probe(sh(&script), Duration::from_secs(20), 1024, &cancel);
        assert!(raiser.join().is_ok());
        assert!(matches!(outcome, Err(ProbeError::Cancelled)), "{outcome:?}");
        assert!(started.elapsed() < Duration::from_secs(2));
        let pid: u32 = fs::read_to_string(&pid_file)
            .expect("the child wrote its pid")
            .trim()
            .parse()
            .expect("a pid");
        assert!(!alive(pid), "the child was killed and reaped");
        // Already raised before the run: nothing lingers either.
        let outcome = run_probe(
            sh("exec sleep 30"),
            Duration::from_secs(20),
            1024,
            &AtomicBool::new(true),
        );
        assert!(matches!(outcome, Err(ProbeError::Cancelled)), "{outcome:?}");
    }

    #[test]
    fn a_stopped_load_worker_returns_promptly() {
        // A probe that sleeps far past the test, in place of the binary.
        let dir = Scratch::new();
        let slow = dir.write("slow.sh", "#!/bin/sh\nexec sleep 30\n");
        fs::set_permissions(&slow, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .expect("chmod");
        let cancel = Arc::new(AtomicBool::new(false));
        let stopper = {
            let cancel = Arc::clone(&cancel);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                cancel.store(true, Ordering::Release);
            })
        };
        let started = Instant::now();
        let answer = load_with(&slow, &dir.0.join("config.kdl"), &cancel);
        assert!(stopper.join().is_ok());
        assert_eq!(
            answer,
            Keybinds::Unavailable(KeybindsUnavailable::Unreadable)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_probe_that_cannot_run_is_unreadable_and_one_that_fails_is_invalid() {
        let dir = Scratch::new();
        let good = dir.write("config.kdl", "binds { Mod+Q { quit; } }");
        // No probe binary at all: the shell cannot read, which is not the
        // file's fault.
        assert_eq!(
            load_with(Path::new("/nonexistent/celestina"), &good, &never()),
            Keybinds::Unavailable(KeybindsUnavailable::Unreadable)
        );
        // A binary that runs and fails (here `false`) met text it could not
        // read.
        assert_eq!(
            load_with(Path::new("/bin/false"), &good, &never()),
            Keybinds::Unavailable(KeybindsUnavailable::Invalid)
        );
        // And one that answers with something that is not an answer.
        assert_eq!(
            load_with(Path::new("/bin/true"), &good, &never()),
            Keybinds::Unavailable(KeybindsUnavailable::Invalid)
        );
    }

    #[test]
    fn a_relative_niri_config_is_made_absolute() {
        let relative = env(Some("sub/config.kdl"), None, Some("/h"));
        let resolved = resolve_path(&relative).expect("a path");
        assert!(resolved.is_absolute(), "{resolved:?}");
        assert_eq!(
            resolved,
            std::env::current_dir()
                .expect("a current directory")
                .join("sub/config.kdl")
        );
    }

    #[test]
    fn confining_the_probe_lowers_its_limits() {
        // `confine` lowers the process's own limits for good, so it runs in
        // a child of this test binary that reports them.
        let output = Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                "--exact",
                "providers::keybinds::tests::confine_child",
                "--nocapture",
            ])
            .env("CELESTINA_CONFINE_CHILD", "1")
            .stdin(Stdio::null())
            .output()
            .expect("run the child");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.contains("limits core=0 as=1073741824 cpu=5"),
            "{text} {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn confine_child() {
        if std::env::var_os("CELESTINA_CONFINE_CHILD").is_none() {
            return;
        }
        confine();
        let get = |resource| {
            rustix::process::getrlimit(resource)
                .current
                .map_or_else(|| "none".to_owned(), |limit| limit.to_string())
        };
        println!(
            "limits core={} as={} cpu={}",
            get(Resource::Core),
            get(Resource::As),
            get(Resource::Cpu)
        );
    }
}
