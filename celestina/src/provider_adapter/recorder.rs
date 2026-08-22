//! Recording an output, for the bug that has to be shown rather than described.
//!
//! One recording exists at a time and this module owns its child process: the
//! helper is where a long-lived process belongs, and a recorder started from a
//! menu that is then dismissed must outlive that menu by definition.
//!
//! Two things shape the design. A recorder is stopped by asking it politely —
//! `SIGINT` is what makes `gpu-screen-recorder` write the file's index and
//! close it, where a kill leaves an unplayable file — so stopping waits, with
//! a deadline, and only then insists. And a recorder can also stop on its own:
//! an encoder that fails or a monitor that is unplugged ends the child without
//! anyone asking, so a reaper watches for that and corrects the reading. A
//! shell that says it is recording when it is not is worse than one that
//! cannot record at all.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use rustix::process::{kill_process, Pid, Signal};
use serde_json::Value;

use super::tools::{lock_runtime, run_bounded};
use super::worker::Worker;

pub const NAME: &str = "recorder";

/// The recorder this shell drives. Chosen because it is what the session has:
/// it captures a wlroots output by its connector name without a portal picker
/// in the way, which is the difference between "record this bug" and "record
/// this bug after answering a dialog that is now in the recording".
const TOOL: &str = "gpu-screen-recorder";

const START: &str = "record-start";
const STOP: &str = "record-stop";

/// The directory recordings are kept in, under the person's own videos folder.
/// A recording of a bug is a working file, not something to leave loose among
/// the videos somebody keeps on purpose.
const SUBDIRECTORY: &str = "Recordings";

/// Frames per second. A bug is often a moment of motion — a flicker, a jump —
/// and 30 frames can drop the very frame being reported.
const FRAMES_PER_SECOND: &str = "60";

/// How long a stop waits for the tool to finish its own file before insisting.
const CLOSE_GRACE: Duration = Duration::from_secs(5);

/// How often the reaper asks whether a recording ended without being asked to.
const REAP_INTERVAL: Duration = Duration::from_millis(500);

struct Recording {
    child: Child,
    output: String,
    path: PathBuf,
    since_ms: u64,
}

struct Surface {
    runtime: Arc<Mutex<ProviderRuntime>>,
    id: ProviderId,
}

static SURFACE: OnceLock<Surface> = OnceLock::new();
static ACTIVE: OnceLock<Mutex<Option<Recording>>> = OnceLock::new();

fn active() -> &'static Mutex<Option<Recording>> {
    ACTIVE.get_or_init(|| Mutex::new(None))
}

fn lock_active() -> std::sync::MutexGuard<'static, Option<Recording>> {
    match active().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Registers the provider and starts the reaper that keeps its reading honest.
pub fn spawn(
    runtime: &Arc<Mutex<ProviderRuntime>>,
    shutdown: &Arc<AtomicBool>,
) -> std::io::Result<Option<Worker>> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: recorder: unusable provider name");
        return Ok(None);
    };
    lock_runtime(runtime).register(id.clone());
    let _ = SURFACE.set(Surface {
        runtime: Arc::clone(runtime),
        id,
    });
    publish();

    let worker_shutdown = Arc::clone(shutdown);
    Worker::spawn(NAME, shutdown, move || {
        while !worker_shutdown.load(Ordering::Acquire) {
            std::thread::sleep(REAP_INTERVAL);
            if reap() {
                publish();
            }
        }
    })
    .map(Some)
}

/// Stops a recording the session is ending under, so the file is playable.
///
/// Called on the way out rather than left to process teardown: killing the
/// tool would leave exactly the unindexed file this module exists to avoid.
pub fn stop_for_shutdown() {
    if lock_active().is_some() {
        let _ = stop();
    }
}

/// # Errors
///
/// Returns the sentence the requester is owed: an unknown verb, a tool this
/// session does not have, a second recording, or a recorder that would not
/// start.
pub fn action(verb: &str, options: &Payload) -> Result<(), String> {
    match verb {
        START => start(requested_output(options)?),
        STOP => stop(),
        _ => Err(format!("'{NAME}' does not serve the verb '{verb}'")),
    }
}

/// The monitor to record, named by the requester rather than assumed: this
/// session has three, and recording the wrong one is a whole take wasted.
fn requested_output(options: &Payload) -> Result<String, String> {
    options
        .get("output")
        .and_then(Value::as_str)
        .filter(|output| !output.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("'{NAME}' needs the output to record"))
}

/// Starts a recording and says how it went, publishing either way.
///
/// The publish is deliberately out here, where nothing is locked: it reads the
/// same state `begin` holds while it works, and this mutex is not reentrant —
/// publishing from inside would deadlock the helper on its own recorder.
fn start(output: String) -> Result<(), String> {
    let outcome = begin(output);
    publish();
    outcome
}

fn begin(output: String) -> Result<(), String> {
    let mut recording = lock_active();
    if recording.is_some() {
        return Err("this session is already recording".to_owned());
    }
    if !available() {
        return Err(format!("this session has no {TOOL} to record with"));
    }

    let path = destination(&output);
    let file = path
        .to_str()
        .ok_or_else(|| "the recording's own path is not text".to_owned())?;
    let child = Command::new(TOOL)
        .args([
            "-w",
            &output,
            "-f",
            FRAMES_PER_SECOND,
            "-cursor",
            "yes",
            "-o",
            file,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        // Remembered rather than only returned: the surface that asked has
        // closed itself by now, so a person who missed the moment still finds
        // out from the toolbox why nothing is recording.
        .inspect_err(|_| remember_failure(Some(FAILED_TO_START)))
        .map_err(|error| format!("cannot start {TOOL}: {error}"))?;

    remember_failure(None);
    *recording = Some(Recording {
        child,
        output,
        path,
        since_ms: now_ms(),
    });
    Ok(())
}

fn stop() -> Result<(), String> {
    let mut held = lock_active();
    let Some(mut recording) = held.take() else {
        return Err("this session is not recording".to_owned());
    };
    // Released before the wait below: a stop that takes its full grace must not
    // hold every other command in this helper behind it.
    drop(held);

    let outcome = close(&mut recording);
    last_path_remember(&recording.path);
    publish();
    outcome
}

/// Asks the tool to finish, waits for it, and only insists if it will not.
fn close(recording: &mut Recording) -> Result<(), String> {
    let interrupted = Pid::from_raw(
        i32::try_from(recording.child.id())
            .map_err(|_| "the recorder has no usable id".to_owned())?,
    )
    .ok_or_else(|| "the recorder has no usable id".to_owned())
    .and_then(|pid| {
        kill_process(pid, Signal::INT)
            .map_err(|error| format!("cannot ask {TOOL} to stop: {error}"))
    });

    if interrupted.is_ok() {
        let deadline = Instant::now() + CLOSE_GRACE;
        while Instant::now() < deadline {
            match recording.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => std::thread::sleep(REAP_INTERVAL),
                Err(error) => return Err(format!("cannot wait for {TOOL}: {error}")),
            }
        }
    }

    // Whatever it has written stays on disk; what it has not written is a
    // file the person may not be able to play, and they are told so rather
    // than left to discover it.
    let _ = recording.child.kill();
    let _ = recording.child.wait();
    remember_failure(Some(FAILED_TO_CLOSE));
    Err(format!(
        "{TOOL} did not close its file in time; the recording may be unplayable"
    ))
}

/// Notices a recording that ended without being asked to. Returns whether the
/// reading has to change.
fn reap() -> bool {
    let mut held = lock_active();
    let Some(recording) = held.as_mut() else {
        return false;
    };
    match recording.child.try_wait() {
        Ok(Some(_)) | Err(_) => {
            let path = recording.path.clone();
            *held = None;
            drop(held);
            last_path_remember(&path);
            true
        }
        Ok(None) => false,
    }
}

/// Why the last attempt did not do what was asked, as a token rather than a
/// sentence: the reason a helper writes is English by contract and belongs in
/// the log, while what a person reads is the shell's own words. A failed
/// command answers the requester too, but the menu that asked is closed by
/// then — this is what is left for them to find.
static FAILURE: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();

/// The recorder would not start at all.
const FAILED_TO_START: &str = "start-failed";
/// It was asked to finish and did not, so its file may have no index.
const FAILED_TO_CLOSE: &str = "close-failed";

fn remember_failure(cause: Option<&'static str>) {
    let cell = FAILURE.get_or_init(|| Mutex::new(None));
    if let Ok(mut failure) = cell.lock() {
        *failure = cause;
    }
}

fn failure() -> Option<&'static str> {
    FAILURE
        .get()
        .and_then(|cell| cell.lock().ok())
        .and_then(|failure| *failure)
}

/// The last file this session wrote, so the toolbox can say where it went
/// after the recording is no longer running.
static LAST_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn last_path_remember(path: &Path) {
    let cell = LAST_PATH.get_or_init(|| Mutex::new(None));
    if let Ok(mut last) = cell.lock() {
        *last = Some(path.to_path_buf());
    }
}

fn last_path() -> Option<PathBuf> {
    LAST_PATH
        .get()
        .and_then(|cell| cell.lock().ok())
        .and_then(|last| last.clone())
}

fn available() -> bool {
    which(TOOL).is_some()
}

/// Whether a program exists on this session's `PATH`, without running it.
fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

/// Where a recording goes, and what it is called.
///
/// The person's own videos directory, asked for the way the desktop asks
/// rather than assumed: a localized session names it in the local language,
/// which no constant in this repository would have guessed. One folder of
/// its own lives inside it.
fn destination(output: &str) -> PathBuf {
    let directory = run_bounded("xdg-user-dir", &["VIDEOS"])
        .map(|answer| answer.trim().to_owned())
        .filter(|answer| !answer.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(SUBDIRECTORY);
    let _ = std::fs::create_dir_all(&directory);
    directory.join(recording_name(&stamp(), output))
}

/// A name that sorts by when it was taken and says what it shows.
fn recording_name(stamp: &str, output: &str) -> String {
    let output: String = output
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect();
    format!("celestina-{stamp}-{output}.mp4")
}

/// The moment, in this session's own timezone.
///
/// Asked of `date` rather than computed: a civil date needs the timezone
/// database and its rules for the day they change, and this shell has no
/// business carrying a second copy of either. An answer that does not arrive
/// falls back to the epoch, which is ugly and still unique.
fn stamp() -> String {
    run_bounded("date", &["+%Y-%m-%d_%H-%M-%S"])
        .map(|answer| answer.trim().to_owned())
        .filter(|answer| !answer.is_empty())
        .unwrap_or_else(|| now_ms().to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(0))
}

fn publish() {
    let Some(surface) = SURFACE.get() else {
        return;
    };
    let mut payload = Payload::new();
    // Said rather than implied by absence: a toolbox that offers recording on
    // a session with no recorder is a promise it cannot keep, and one that
    // silently hides the row explains nothing.
    payload.insert("available".to_owned(), Value::from(available()));

    let held = lock_active();
    if let Some(recording) = held.as_ref() {
        payload.insert("recording".to_owned(), Value::from(true));
        payload.insert("output".to_owned(), Value::from(recording.output.clone()));
        payload.insert("since".to_owned(), Value::from(recording.since_ms));
        payload.insert(
            "path".to_owned(),
            Value::from(recording.path.to_string_lossy().into_owned()),
        );
    } else {
        payload.insert("recording".to_owned(), Value::from(false));
        if let Some(cause) = failure() {
            payload.insert("failure".to_owned(), Value::from(cause));
        }
        if let Some(path) = last_path() {
            payload.insert(
                "path".to_owned(),
                Value::from(path.to_string_lossy().into_owned()),
            );
        }
    }
    drop(held);

    if let Err(error) = lock_runtime(&surface.runtime).publish(&surface.id, payload) {
        eprintln!("celestina-provider-adapter: recorder: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_is_named_for_when_it_was_taken_and_what_it_shows() {
        assert_eq!(
            recording_name("2026-08-21_22-59-43", "DP-1"),
            "celestina-2026-08-21_22-59-43-DP-1.mp4"
        );
    }

    /// A connector name reaches this from the compositor, and it ends up as a
    /// path. Nothing in it may leave the directory it was meant for.
    #[test]
    fn an_outputs_name_cannot_reach_out_of_its_directory() {
        let name = recording_name("stamp", "../../etc/DP-1");

        assert!(!name.contains('/'));
        assert!(!name.contains(".."));
        assert_eq!(name, "celestina-stamp-------etc-DP-1.mp4");
    }

    #[test]
    fn a_verb_this_provider_does_not_serve_is_refused_in_its_own_name() {
        let refusal = action("record-sideways", &Payload::new()).expect_err("unknown verb");

        assert!(refusal.contains(NAME));
        assert!(refusal.contains("record-sideways"));
    }

    #[test]
    fn recording_needs_the_output_to_record() {
        let refusal = action(START, &Payload::new()).expect_err("no output");

        assert!(refusal.contains("output"));
    }

    #[test]
    fn stopping_what_was_never_started_says_so() {
        let refusal = action(STOP, &Payload::new()).expect_err("not recording");

        assert!(refusal.contains("not recording"));
    }
}
