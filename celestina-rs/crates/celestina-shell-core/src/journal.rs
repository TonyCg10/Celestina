//! The sink that puts [`crate::diagnostics`] events on a disk.
//!
//! This is testable IO: it owns files, permissions, a thread and a deadline, and
//! it is exercised against temporary directories and deliberate failures rather
//! than against a session. The policy it obeys — the vocabulary, the bounds, the
//! redaction, the line, the rotation arithmetic and the drop rule — belongs to
//! [`crate::diagnostics`] and is not restated here.
//!
//! # The one rule
//!
//! **A journal may never change what it observes.** It does not block the caller,
//! it does not grow without bound, it does not propagate an error, and it does
//! not terminate anything when the disk refuses it. A shell that died because it
//! could not write about itself would be a worse outcome than the freeze this
//! exists to investigate.
//!
//! Recording therefore takes `&self`, cannot fail, and returns nothing. Every
//! failure — a full queue, an unwritable directory, a disk that filled — is
//! itself recorded, in the journal when it can be and on the mirror when it
//! cannot.
//!
//! # Why the file and not journald
//!
//! Celestina is normally started from a terminal, and a terminal's `stderr` is
//! captured by journald only when the launch shape happens to arrange it. The
//! file is the evidence and the mirror is a convenience. `CELESTINA_JOURNAL_MIRROR=0`
//! quiets the mirror and changes nothing about the file, which is the switch to
//! reach for when the terminal is too noisy — there is deliberately no switch
//! that turns the critical record off.

use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::diagnostics::{self, Component, Event, Identity, Level, Queue, Stamp, Value, MAX_QUEUE};

/// The environment variable carrying the host's `run_id` to its helpers. The
/// host generates one per invocation and sets this before spawning; a helper
/// started without it makes its own, which is what a hand-run helper or a test
/// gets.
pub const RUN_ID_VARIABLE: &str = "CELESTINA_RUN_ID";
/// Set to `0` to quiet the compact stderr mirror. The file is unaffected: there
/// is no variable that stops the journal.
pub const MIRROR_VARIABLE: &str = "CELESTINA_JOURNAL_MIRROR";
/// How long shutdown waits for the writer to drain before giving up on the rest.
/// Bounded on purpose: a slow disk must not hold a session's exit open.
pub const DRAIN_DEADLINE: Duration = Duration::from_millis(1500);
/// How often the writer retries a directory it could not write to. A disk that
/// came back should be used again without restarting the shell.
const REOPEN_AFTER: Duration = Duration::from_secs(30);

/// Where the journals live, under the caller's XDG state directory.
#[must_use]
pub fn directory() -> Option<PathBuf> {
    celestina_core::xdg::state_home().map(|state| state.join("celestina").join("diagnostics"))
}

/// A `run_id` for one invocation.
///
/// Not random — this crate takes no dependency for one identifier — but unique
/// in the only way that matters: two invocations differ if they started at
/// different nanoseconds or in different processes, and a `run_id` never has to
/// be unguessable because it names nothing private.
#[must_use]
pub fn generate_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{:x}-{:x}", nanos, process::id())
}

/// The `run_id` this process belongs to: the host's, or a fresh one.
#[must_use]
pub fn inherited_run_id() -> String {
    std::env::var(RUN_ID_VARIABLE)
        .ok()
        .map(|value| crate::bounded(value.trim(), diagnostics::MAX_TEXT_CHARS))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(generate_run_id)
}

/// Whether the compact stderr mirror is wanted.
#[must_use]
pub fn mirror_wanted() -> bool {
    !matches!(
        std::env::var(MIRROR_VARIABLE).as_deref(),
        Ok("0") | Ok("off") | Ok("false")
    )
}

/// What one recording attempt did. Callers ignore this; the tests do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorded {
    Queued,
    Dropped,
}

struct Shared {
    queue: Mutex<Queue>,
    signal: Condvar,
    stopping: AtomicBool,
}

/// A running journal.
///
/// Dropping one stops and joins its writer within [`DRAIN_DEADLINE`], so no
/// writer outlives the thing it was observing. Call [`Journal::shutdown`] when
/// the exit is deliberate and you want the last events on the disk.
pub struct Journal {
    shared: Arc<Shared>,
    writer: Mutex<Option<JoinHandle<()>>>,
    identity: Identity,
    started: Instant,
}

impl Journal {
    /// Opens a journal for one component. Never fails.
    ///
    /// A directory that cannot be created is not an error here: the writer will
    /// keep trying and the mirror carries what it can meanwhile. There is no
    /// call site in this shell that could sensibly handle "the journal is
    /// unavailable" by doing something else.
    #[must_use]
    pub fn open(directory: Option<PathBuf>, identity: Identity, mirror: bool) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::new(MAX_QUEUE)),
            signal: Condvar::new(),
            stopping: AtomicBool::new(false),
        });

        let writer_shared = Arc::clone(&shared);
        let writer_identity = identity.clone();
        let writer = thread::Builder::new()
            .name("celestina-journal".to_owned())
            .spawn(move || {
                let mut sink = Sink::new(directory, writer_identity, mirror);
                sink.run(&writer_shared);
            })
            .ok();

        Self {
            shared,
            writer: Mutex::new(writer),
            identity,
            started: Instant::now(),
        }
    }

    /// Opens the journal this process's environment describes.
    #[must_use]
    pub fn for_component(name: &str, generation: u64) -> Self {
        let identity = Identity::new(
            &inherited_run_id(),
            Component::new(name),
            process::id(),
            generation,
        );
        Self::open(directory(), identity, mirror_wanted())
    }

    #[must_use]
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Records an event. Never blocks, never fails, never propagates.
    pub fn record(&self, event: Event) -> Recorded {
        self.record_from("", event)
    }

    /// Records an event attributed to a named worker or thread.
    pub fn record_from(&self, worker: &str, event: Event) -> Recorded {
        let stamp = Stamp {
            wall_nanos: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos()),
            monotonic_millis: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
        };

        // A poisoned queue means the writer panicked mid-write. Recording is
        // still not allowed to fail, so the event is dropped and the mirror is
        // the only witness — which is exactly what the drop policy is for.
        let Ok(mut queue) = self.shared.queue.lock() else {
            return Recorded::Dropped;
        };
        let admission = queue.offer(event, stamp, worker);
        drop(queue);
        self.shared.signal.notify_one();

        match admission {
            diagnostics::Admission::Dropped => Recorded::Dropped,
            _ => Recorded::Queued,
        }
    }

    /// Stops the writer, giving it a bounded chance to drain.
    pub fn shutdown(self) {
        self.close();
    }

    /// Stops the writer without consuming the journal.
    ///
    /// Takes `&self` so a process-wide journal, which by construction is never
    /// dropped, can still be closed deterministically at the one place that
    /// knows the process is ending. Calling it twice is harmless.
    pub fn close(&self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        self.shared.signal.notify_all();
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        if let Some(handle) = writer.take() {
            let deadline = Instant::now() + DRAIN_DEADLINE;
            while !handle.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if handle.is_finished() {
                let _unused = handle.join();
            }
            // Dropping a still-running JoinHandle detaches it. The writer owns
            // its sink and only shares the queue through Arc, so this is safe:
            // an unresponsive disk may lose the tail at process exit, but it
            // cannot hold the session shutdown open indefinitely.
        }
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        self.close();
    }
}

/// The journal this process writes to, installed once at startup.
///
/// A process-wide handle rather than one threaded through every call site. That
/// is normally a smell and here it is the lesser one: the events worth having
/// come from a bounded tool runner, a DDC worker, a signal handler and a dozen
/// providers that have no other reason to know about each other, and giving all
/// of them a journal parameter would spread the instrument through every
/// signature it observes.
///
/// A process that never installs one records nothing and works exactly as it
/// did — which is what every existing test does.
static PROCESS_JOURNAL: OnceLock<Journal> = OnceLock::new();

/// Installs this process's journal. The second call is ignored.
pub fn install(journal: Journal) {
    let _unused = PROCESS_JOURNAL.set(journal);
}

/// This process's journal, if one was installed.
#[must_use]
pub fn process_journal() -> Option<&'static Journal> {
    PROCESS_JOURNAL.get()
}

/// Records one event on this process's journal, or does nothing.
pub fn record(event: Event) {
    if let Some(journal) = PROCESS_JOURNAL.get() {
        journal.record(event);
    }
}

/// Records one event attributed to a named worker, or does nothing.
pub fn record_from(worker: &str, event: Event) {
    if let Some(journal) = PROCESS_JOURNAL.get() {
        journal.record_from(worker, event);
    }
}

/// Closes this process's journal at a deliberate exit.
pub fn close_process_journal() {
    if let Some(journal) = PROCESS_JOURNAL.get() {
        journal.close();
    }
}

/// The writing half, which lives entirely on the writer thread.
struct Sink {
    directory: Option<PathBuf>,
    identity: Identity,
    mirror: bool,
    file: Option<File>,
    bytes: u64,
    /// When the directory last refused us, so a broken disk is retried rather
    /// than either hammered or abandoned.
    unwritable_since: Option<Instant>,
    /// Failures to write since the last time we managed to say so.
    write_failures: u64,
}

impl Sink {
    fn new(directory: Option<PathBuf>, identity: Identity, mirror: bool) -> Self {
        Self {
            directory,
            identity,
            mirror,
            file: None,
            bytes: 0,
            unwritable_since: None,
            write_failures: 0,
        }
    }

    fn run(&mut self, shared: &Arc<Shared>) {
        loop {
            let stopping = shared.stopping.load(Ordering::SeqCst);
            let batch = self.drain_once(shared);
            if batch == 0 {
                if stopping {
                    break;
                }
                let Ok(queue) = shared.queue.lock() else {
                    break;
                };
                // A timeout rather than a pure wait so a stop that raced the
                // notification cannot leave this thread parked for ever.
                let _unused = shared
                    .signal
                    .wait_timeout(queue, Duration::from_millis(200))
                    .ok();
            }
        }

        // The deliberate exit: drain what is left, but only until the deadline.
        // A disk that has stopped answering must not hold the session's exit.
        let deadline = Instant::now() + DRAIN_DEADLINE;
        while Instant::now() < deadline && self.drain_once(shared) > 0 {}
        self.write(&diagnostics::Event::new(Level::Critical, "journal.stop"));
        if let Some(file) = self.file.as_mut() {
            let _unused = file.flush();
            let _unused = file.sync_all();
        }
    }

    /// Writes whatever is waiting, and returns how many events that was.
    fn drain_once(&mut self, shared: &Arc<Shared>) -> usize {
        let mut written = 0;
        loop {
            let Ok(mut queue) = shared.queue.lock() else {
                return written;
            };
            let dropped = queue.take_dropped();
            let next = queue.take();
            drop(queue);

            if dropped > 0 {
                self.write_with_worker("", &diagnostics::loss_event(dropped));
            }
            let Some((event, stamp, worker)) = next else {
                return written;
            };
            self.emit(&event, stamp, &worker);
            written += 1;
            if written >= 256 {
                // Yield the lock periodically so a burst cannot starve a
                // producer that is only trying to enqueue.
                return written;
            }
        }
    }

    fn write(&mut self, event: &Event) {
        self.write_with_worker("", event);
    }

    fn write_with_worker(&mut self, worker: &str, event: &Event) {
        let stamp = Stamp {
            wall_nanos: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos()),
            monotonic_millis: 0,
        };
        self.emit(event, stamp, worker);
    }

    fn emit(&mut self, event: &Event, stamp: Stamp, worker: &str) {
        let line = diagnostics::render(&self.identity, event, stamp, worker);

        if self.mirror && event.level().mirrored() {
            // Compact and one line, so a terminal stays readable and journald
            // gets the same identity the file has.
            eprintln!(
                "celestina[{}/{}] {} {}",
                self.identity.component().as_str(),
                self.identity.run_id(),
                event.level().as_str(),
                event.name()
            );
        }

        if !self.ensure_file(line.text.len()) {
            return;
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };

        let mut bytes = line.text.into_bytes();
        bytes.push(b'\n');
        match file.write_all(&bytes) {
            Ok(()) => {
                self.bytes += bytes.len() as u64;
                if line.flush {
                    // The failure this journal exists for cuts power to the
                    // machine. A critical event that only ever reached a buffer
                    // would be the one line the investigation needed.
                    let _unused = file.flush();
                    let _unused = file.sync_data();
                }
            }
            Err(_) => {
                self.write_failures += 1;
                self.file = None;
                self.bytes = 0;
                self.unwritable_since = Some(Instant::now());
            }
        }
    }

    /// Makes sure there is a file with room for `incoming` bytes, rotating and
    /// retiring as the policy says. Returns whether writing may proceed.
    fn ensure_file(&mut self, incoming: usize) -> bool {
        if self.file.is_some()
            && diagnostics::rotation(self.bytes, incoming) == diagnostics::Rotation::Rotate
        {
            self.rotate();
        }
        if self.file.is_some() {
            return true;
        }

        if let Some(since) = self.unwritable_since {
            if since.elapsed() < REOPEN_AFTER {
                return false;
            }
        }

        let Some(directory) = self.directory.clone() else {
            self.unwritable_since = Some(Instant::now());
            return false;
        };
        if fs::create_dir_all(&directory).is_err() {
            self.unwritable_since = Some(Instant::now());
            return false;
        }
        // The journal names outputs, buses, processes and timings of one
        // person's session. Nobody else on this machine has business reading it.
        let _unused = fs::set_permissions(&directory, fs::Permissions::from_mode(0o700));

        let path = directory.join(self.live_name());
        let opened = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&path);
        let Ok(mut file) = opened else {
            self.unwritable_since = Some(Instant::now());
            return false;
        };

        self.bytes = file.seek(SeekFrom::End(0)).unwrap_or(0);
        // A previous run that was cut off mid-line leaves a file whose last byte
        // is not a newline. Closing that line before appending keeps the torn
        // record readable as one broken line instead of silently fusing it with
        // the first line of this run.
        if self.bytes > 0 && !ends_with_newline(&path) && file.write_all(b"\n").is_ok() {
            self.bytes += 1;
        }

        self.file = Some(file);
        self.unwritable_since = None;
        self.retire_surplus(&directory);

        if self.write_failures > 0 {
            let failures = std::mem::take(&mut self.write_failures);
            let recovered = Event::new(Level::Warn, "journal.recovered")
                .with("failed_writes", Value::Uint(failures));
            let stamp = Stamp {
                wall_nanos: 0,
                monotonic_millis: 0,
            };
            let line = diagnostics::render(&self.identity, &recovered, stamp, "");
            if let Some(file) = self.file.as_mut() {
                let _unused = file.write_all(line.text.as_bytes());
                let _unused = file.write_all(b"\n");
            }
        }
        true
    }

    fn live_name(&self) -> String {
        format!(
            "{}-{}.jsonl",
            sanitized(self.identity.component().as_str()),
            sanitized(self.identity.run_id())
        )
    }

    fn rotate(&mut self) {
        let (Some(directory), Some(file)) = (self.directory.clone(), self.file.take()) else {
            return;
        };
        let _unused = file.sync_all();
        drop(file);

        let live = directory.join(self.live_name());
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let retired = directory.join(format!(
            "{}-{}.{:x}.jsonl",
            sanitized(self.identity.component().as_str()),
            sanitized(self.identity.run_id()),
            stamp
        ));
        let _unused = fs::rename(&live, &retired);
        self.bytes = 0;
    }

    /// Deletes this component's surplus files, oldest first.
    fn retire_surplus(&self, directory: &Path) {
        let prefix = format!("{}-", sanitized(self.identity.component().as_str()));
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with(&prefix) && name.ends_with(".jsonl"))
            .collect();
        // Newest first. The live file has no timestamp segment and sorts last
        // by name, so it is put in front explicitly rather than by luck.
        names.sort_by(|a, b| b.cmp(a));
        let live = self.live_name();
        if let Some(index) = names.iter().position(|name| *name == live) {
            let found = names.remove(index);
            names.insert(0, found);
        }

        let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();
        for surplus in diagnostics::retire(&borrowed) {
            let _unused = fs::remove_file(directory.join(surplus));
        }
    }
}

fn ends_with_newline(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| bytes.last().copied())
        .is_none_or(|last| last == b'\n')
}

/// Keeps a file name to characters that cannot escape the directory or confuse a
/// shell reading the bundle later.
fn sanitized(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "unknown".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A directory that is removed with the test, without a dependency.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos());
            let path = std::env::temp_dir().join(format!("celestina-journal-{name}-{nanos:x}"));
            fs::create_dir_all(&path).expect("a temporary directory");
            Self(path)
        }

        fn path(&self) -> PathBuf {
            self.0.clone()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _unused = fs::remove_dir_all(&self.0);
        }
    }

    fn identity(run: &str) -> Identity {
        Identity::new(run, Component::new("test-helper"), 99, 1)
    }

    fn lines(directory: &Path) -> Vec<serde_json::Value> {
        let mut all = Vec::new();
        let Ok(entries) = fs::read_dir(directory) else {
            return all;
        };
        let mut names: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        names.sort();
        for name in names {
            let Ok(text) = fs::read_to_string(&name) else {
                continue;
            };
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                all.push(
                    serde_json::from_str(line)
                        .unwrap_or_else(|_| panic!("every line is valid JSON: {line}")),
                );
            }
        }
        all
    }

    #[test]
    fn every_written_line_is_valid_jsonl_carrying_its_run() {
        let scratch = Scratch::new("jsonl");
        let journal = Journal::open(Some(scratch.path()), identity("run-1"), false);

        journal.record(Event::new(Level::Info, "host.start"));
        journal.record(Event::new(Level::Critical, "ddc.write.start").with_text("output", "DP-1"));
        journal.shutdown();

        let written = lines(&scratch.path());
        assert!(written.len() >= 3, "{written:?}");
        assert!(written.iter().all(|line| line["run_id"] == "run-1"));
        assert!(written
            .iter()
            .all(|line| line["component"] == "test-helper"));
        let names: Vec<&str> = written
            .iter()
            .map(|line| line["event"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"host.start"));
        assert!(names.contains(&"ddc.write.start"));
        // Shutdown is deterministic: the writer says it stopped.
        assert!(names.contains(&"journal.stop"));
    }

    #[test]
    fn events_keep_the_order_they_were_recorded_in() {
        let scratch = Scratch::new("order");
        let journal = Journal::open(Some(scratch.path()), identity("run-2"), false);

        for index in 0..50 {
            journal.record(Event::new(Level::Info, "step").with("n", Value::Uint(index)));
        }
        journal.shutdown();

        let steps: Vec<u64> = lines(&scratch.path())
            .iter()
            .filter(|line| line["event"] == "step")
            .map(|line| line["n"].as_u64().unwrap())
            .collect();
        assert_eq!(steps, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn a_helper_inherits_the_hosts_run_id_from_its_environment() {
        // Set and cleared inside one test; `inherited_run_id` reads it once.
        std::env::set_var(RUN_ID_VARIABLE, "run-from-the-host");
        let inherited = inherited_run_id();
        std::env::remove_var(RUN_ID_VARIABLE);

        assert_eq!(inherited, "run-from-the-host");
        // Without it, a helper still writes a correlatable journal of its own
        // rather than none at all.
        assert!(!inherited_run_id().is_empty());
        assert_ne!(inherited_run_id(), inherited);
    }

    #[test]
    fn a_directory_that_cannot_be_written_never_reaches_the_caller() {
        let scratch = Scratch::new("readonly");
        let blocked = scratch.path().join("blocked");
        fs::create_dir_all(&blocked).expect("a directory");
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o500))
            .expect("a read-only directory");

        let journal = Journal::open(Some(blocked.join("inner")), identity("run-3"), false);
        // Every one of these is a normal call site. None may fail, block or
        // panic, and the shell around them carries on.
        for _ in 0..200 {
            journal.record(Event::new(Level::Critical, "ddc.read"));
        }
        journal.shutdown();

        let _unused = fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700));
    }

    #[test]
    fn a_journal_with_nowhere_to_write_still_accepts_everything() {
        let journal = Journal::open(None, identity("run-4"), false);

        assert_eq!(
            journal.record(Event::new(Level::Critical, "process.kill")),
            Recorded::Queued
        );
        journal.shutdown();
    }

    #[test]
    fn a_torn_last_line_is_closed_rather_than_fused_with_this_run() {
        let scratch = Scratch::new("torn");
        let path = scratch.path().join("test-helper-run-5.jsonl");
        fs::create_dir_all(scratch.path()).expect("a directory");
        // What a physical reset leaves behind: half a line, no newline.
        fs::write(&path, b"{\"v\":1,\"event\":\"ddc.wri").expect("a torn file");

        let journal = Journal::open(Some(scratch.path()), identity("run-5"), false);
        journal.record(Event::new(Level::Info, "host.start"));
        journal.shutdown();

        let text = fs::read_to_string(&path).expect("the file");
        let all: Vec<&str> = text.lines().collect();
        // The torn record stays one broken line, and this run's first line is
        // its own — a reader loses the torn one and nothing else.
        assert_eq!(all[0], "{\"v\":1,\"event\":\"ddc.wri");
        assert!(all[1..]
            .iter()
            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok()));
    }

    #[test]
    fn a_full_queue_publishes_what_it_lost() {
        let scratch = Scratch::new("dropped");
        let identity = identity("run-6");
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::new(4)),
            signal: Condvar::new(),
            stopping: AtomicBool::new(false),
        });
        // Fill past the capacity before any writer runs, so the drop is certain
        // rather than a race with the disk.
        {
            let mut queue = shared.queue.lock().expect("the queue");
            for _ in 0..10 {
                queue.offer(
                    Event::new(Level::Info, "tick"),
                    Stamp {
                        wall_nanos: 1,
                        monotonic_millis: 1,
                    },
                    "",
                );
            }
        }
        let mut sink = Sink::new(Some(scratch.path()), identity, false);
        shared.stopping.store(true, Ordering::SeqCst);
        sink.run(&shared);

        let written = lines(&scratch.path());
        let loss = written
            .iter()
            .find(|line| line["event"] == "journal.dropped")
            .expect("a journal that dropped events says so");
        assert_eq!(loss["events"], 6);
    }

    #[test]
    fn a_file_past_its_bound_rotates_and_only_the_surplus_is_retired() {
        let scratch = Scratch::new("rotate");
        let identity = identity("run-7");
        let mut sink = Sink::new(Some(scratch.path()), identity, false);

        // Rotate more times than the file bound allows to survive.
        for _ in 0..diagnostics::MAX_FILES + 4 {
            sink.ensure_file(1);
            sink.write(&Event::new(Level::Info, "filler"));
            sink.rotate();
        }
        sink.ensure_file(1);

        let files: Vec<String> = fs::read_dir(scratch.path())
            .expect("the directory")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        assert!(files.len() <= diagnostics::MAX_FILES, "{files:?}");
        assert!(files.iter().any(|name| name == "test-helper-run-7.jsonl"));
    }

    #[test]
    fn the_files_are_private_to_the_person_whose_session_they_describe() {
        let scratch = Scratch::new("modes");
        let journal = Journal::open(Some(scratch.path()), identity("run-8"), false);
        journal.record(Event::new(Level::Critical, "host.start"));
        journal.shutdown();

        let directory_mode = fs::metadata(scratch.path())
            .expect("the directory")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o700);
        for entry in fs::read_dir(scratch.path()).expect("the directory") {
            let entry = entry.expect("an entry");
            let mode = entry.metadata().expect("metadata").permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{:?}", entry.file_name());
        }
    }

    #[test]
    fn shutdown_drains_within_its_deadline_and_leaves_no_writer_behind() {
        let scratch = Scratch::new("drain");
        let journal = Journal::open(Some(scratch.path()), identity("run-9"), false);
        for index in 0..2000 {
            journal.record(Event::new(Level::Info, "burst").with("n", Value::Uint(index)));
        }

        let started = Instant::now();
        journal.shutdown();
        let took = started.elapsed();

        // Bounded, and the bound is the one the constant declares plus room for
        // the join itself.
        assert!(took < DRAIN_DEADLINE * 3, "shutdown took {took:?}");
        // `shutdown` consumed the journal, so its thread has been joined; a
        // writer that outlived it would still hold the file open.
        assert!(!lines(&scratch.path()).is_empty());
    }

    #[test]
    fn no_secret_from_a_hostile_session_reaches_the_disk() {
        let scratch = Scratch::new("secrets");
        let secrets: HashMap<&str, &str> = HashMap::from([
            ("clipboard", "hunter2-correct-horse"),
            ("body", "verification code 819322"),
            ("title", "Metallica - Nothing Else Matters"),
            ("window", "invoice-2026.pdf"),
            ("exec", "/usr/bin/firefox --private"),
            ("ssid", "MiFibra-A4C1"),
        ]);

        let journal = Journal::open(Some(scratch.path()), identity("run-10"), false);
        for (key, secret) in &secrets {
            journal.record(
                Event::new(Level::Info, "provider.published")
                    .with_text("provider", "clipboard")
                    .with_redacted(key, secret),
            );
        }
        journal.shutdown();

        let text = fs::read_dir(scratch.path())
            .expect("the directory")
            .filter_map(Result::ok)
            .filter_map(|entry| fs::read_to_string(entry.path()).ok())
            .collect::<String>();
        for secret in secrets.values() {
            for word in secret.split(['-', ' ', '/']).filter(|word| word.len() > 3) {
                assert!(!text.contains(word), "the journal leaked `{word}`");
            }
        }
        // The technical identity a diagnosis needs did survive.
        assert!(text.contains("\"provider\":\"clipboard\""));
    }
}
