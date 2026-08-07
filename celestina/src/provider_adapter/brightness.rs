//! Monitor brightness over DDC, which is slow enough to shape its own design.
//!
//! A single `ddcutil` read takes about a second on this hardware once warm, and
//! close to ten on the first call of a session. That rules out polling and it
//! rules out doing this anywhere near the shared command worker: a wheel notch
//! must not make the volume command behind it wait a second.
//!
//! So brightness has a thread of its own and works from a target. A step is
//! recorded instantly and answered instantly; the thread applies the newest
//! target per monitor — a burst of ten notches is one write, not ten — and then
//! reads back what the monitor actually settled on. The panel only ever shows
//! that read-back.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::brightness::{self, DdcDisplay};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::session::{self, LevelChange, SessionRequest};
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use super::tools::{lock_runtime, run_bounded_with_cancel};
use super::worker::Worker;

/// A monitor may take its time. This is generous against a warm read of about a
/// second and the near-ten-second first call, and still bounded: a monitor that
/// never answers must not hold the thread forever.
const DDC_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the thread waits between looks at its target. Short enough that a
/// wheel feels answered, long enough that a burst arrives as one target.
const APPLY_DELAY: Duration = Duration::from_millis(250);

pub const NAME: &str = "brightness";

/// What each monitor has been asked to become, by connector name. Empty means
/// nothing is owed. It is a module singleton because there is exactly one
/// helper process and one brightness provider in it; threading it through the
/// command dispatch would put a slow monitor's business in every provider's
/// signature.
static PENDING: OnceLock<Mutex<HashMap<String, LevelChange>>> = OnceLock::new();
/// Whether an output has appeared or gone since the worker last looked.
///
/// A flag rather than a queue: a burst of outputs is one rediscovery, and a
/// request arriving while `ddcutil` is mid-conversation is simply still set
/// when the loop comes back around. Nothing here starts a detection — only the
/// one worker thread reads this, and it reads it between operations, so there
/// is never a second `ddcutil` child.
static REDETECT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Records that the set of outputs changed. Called from the command thread and
/// answered by the worker on its own schedule; it runs nothing itself.
pub fn request_redetect() {
    REDETECT_REQUESTED.store(true, Ordering::Release);
}

fn pending() -> &'static Mutex<HashMap<String, LevelChange>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_pending() -> std::sync::MutexGuard<'static, HashMap<String, LevelChange>> {
    match pending().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn spawn(
    runtime: &Arc<Mutex<ProviderRuntime>>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<Worker> {
    let id = ProviderId::new(NAME).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "brightness has an unusable provider name",
        )
    })?;

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    let worker_shutdown = Arc::clone(shutdown);
    // The worker owns the thread: an early return anywhere after this point
    // still requests cancellation and waits until an active DDC child has been
    // reaped, because dropping the guard does exactly what joining it does.
    Worker::spawn(NAME, shutdown, move || {
        run(&runtime, &id, &worker_shutdown);
    })
}

/// Records a target and returns at once. The wheel is answered by the panel's
/// next reading, not by this call — which is the only honest thing a control
/// over a one-second conversation can do.
pub fn action(verb: &str, options: &Payload) -> Result<(), String> {
    let SessionRequest::Brightness(change) = session::parse_for(NAME, verb, options)? else {
        // `parse_for` already refused everything this provider does not serve.
        return Err(session::unserved_verb(NAME, verb));
    };

    // A monitor is named, never assumed: this session has more than one and
    // stepping the wrong one is worse than refusing.
    let output = options
        .get("output")
        .and_then(Value::as_str)
        .filter(|output| !output.is_empty())
        .ok_or_else(|| format!("'{NAME}' needs the output to change"))?;

    let mut owed = lock_pending();
    let combined = owed
        .get(output)
        .map_or(change, |pending| pending.followed_by(change));
    owed.insert(output.to_owned(), combined);
    Ok(())
}

/// The monitors that answer DDC at all. Detection is the expensive call, so it
/// happens once and then only when the schedule or a request says so — a
/// monitor plugged in later is picked up because the host asked, not because
/// this polls for it.
fn detect(shutdown: &AtomicBool) -> Vec<DdcDisplay> {
    run_bounded_with_cancel(
        "ddcutil",
        &["detect", "--brief"],
        DDC_TIMEOUT,
        Some(shutdown),
    )
    .map(|listing| brightness::parse_detect(&listing))
    .unwrap_or_default()
}

fn read(display: &DdcDisplay, shutdown: &AtomicBool) -> Option<u8> {
    let number = display.number.to_string();
    run_bounded_with_cancel(
        "ddcutil",
        &["--display", &number, "getvcp", "10", "--brief"],
        DDC_TIMEOUT,
        Some(shutdown),
    )
    .and_then(|reading| brightness::parse_brightness(&reading))
}

fn write(display: &DdcDisplay, value: u8, shutdown: &AtomicBool) -> bool {
    let number = display.number.to_string();
    let level = value.to_string();
    run_bounded_with_cancel(
        "ddcutil",
        &["--display", &number, "setvcp", "10", &level],
        DDC_TIMEOUT,
        Some(shutdown),
    )
    .is_some()
}

/// Publishes one entry per monitor that speaks DDC. A monitor that speaks it
/// but has not answered is `null` — unknown, which is not the same as dark —
/// and a monitor that does not speak it at all is simply absent.
fn publish(
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    levels: &HashMap<String, Option<u8>>,
) {
    if levels.is_empty() {
        lock_runtime(runtime).withdraw(id);
        return;
    }

    let mut payload = Payload::new();
    for (connector, level) in levels {
        payload.insert(connector.clone(), level.map_or(Value::Null, Value::from));
    }

    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: brightness: {error}");
    }
}

fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, shutdown: &AtomicBool) {
    let mut displays = detect(shutdown);
    if shutdown.load(Ordering::Acquire) {
        return;
    }
    let mut levels: HashMap<String, Option<u8>> = displays
        .iter()
        .map(|display| (display.connector.clone(), None))
        .collect();
    // Say which monitors have a brightness before saying what it is: the panel
    // shows "unknown" for a second rather than nothing at all.
    publish(runtime, id, &levels);

    for display in &displays {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        levels.insert(display.connector.clone(), read(display, shutdown));
        publish(runtime, id, &levels);
    }

    let mut refreshed = Instant::now();
    while !shutdown.load(Ordering::Acquire) {
        // One target per monitor, however many notches produced it.
        let targets: Vec<(String, LevelChange)> = lock_pending().drain().collect();
        for (connector, change) in targets {
            let Some(display) = displays
                .iter()
                .find(|display| display.connector == connector)
            else {
                continue;
            };
            let read_back = levels.get(&connector).copied().flatten();
            let current = match (change, read_back) {
                // An absolute target needs no reading; a step from a value
                // nobody has read would be guessing.
                (LevelChange::Set(_), _) => read_back.unwrap_or_default(),
                (LevelChange::Step(_), Some(level)) => level,
                (LevelChange::Step(_), None) => {
                    eprintln!(
                        "celestina-provider-adapter: brightness: {connector} has not answered yet"
                    );
                    continue;
                }
            };

            let wanted = change.applied_to(current);
            // A monitor that has not answered is written to even when the
            // target matches the placeholder: `unknown` is not `already there`.
            if (wanted != current || read_back.is_none()) && write(display, wanted, shutdown) {
                // What the monitor settled on, not what it was asked for.
                levels.insert(connector.clone(), read(display, shutdown));
                publish(runtime, id, &levels);
            }
        }

        // Taken, not read: consuming the request here is what coalesces a
        // burst into one detection and what keeps a request made during the
        // detection below from being lost.
        let requested = REDETECT_REQUESTED.swap(false, Ordering::AcqRel);
        if brightness::detection_is_due(!displays.is_empty(), requested, refreshed.elapsed()) {
            displays = detect(shutdown);
            levels = displays
                .iter()
                .map(|display| (display.connector.clone(), read(display, shutdown)))
                .collect();
            publish(runtime, id, &levels);
            refreshed = Instant::now();
        }

        thread::sleep(APPLY_DELAY);
    }
}

#[cfg(test)]
mod tests {
    use super::{request_redetect, REDETECT_REQUESTED};

    use std::sync::{atomic::Ordering, Mutex};

    // Both cases deliberately exercise the process-global flag. Rust runs unit
    // tests in parallel, so they take one test-only lock rather than making
    // their assertions depend on which test the harness scheduled between two
    // atomic operations.
    static REDETECT_TEST: Mutex<()> = Mutex::new(());

    /// Requests coalesce and are consumed exactly once.
    ///
    /// This runs no `ddcutil` and needs none: what is being proved is that a
    /// burst of outputs is one rediscovery and that the worker's own `swap` is
    /// what ends it. The conversation with a monitor is deliberately not
    /// reachable from a test.
    #[test]
    fn a_burst_of_outputs_is_one_rediscovery() {
        let _serial = REDETECT_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        REDETECT_REQUESTED.store(false, Ordering::Release);

        request_redetect();
        request_redetect();
        request_redetect();

        // The worker's own take.
        assert!(REDETECT_REQUESTED.swap(false, Ordering::AcqRel));
        // And nothing is owed a second time.
        assert!(!REDETECT_REQUESTED.swap(false, Ordering::AcqRel));
    }

    #[test]
    fn a_request_made_during_an_operation_survives_until_the_next_turn() {
        let _serial = REDETECT_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        REDETECT_REQUESTED.store(false, Ordering::Release);

        // The worker takes the request and starts a detection.
        request_redetect();
        assert!(REDETECT_REQUESTED.swap(false, Ordering::AcqRel));
        // An output arrives while that detection is still running. Nothing
        // starts a second one; the flag is simply still set next time round.
        request_redetect();
        assert!(REDETECT_REQUESTED.swap(false, Ordering::AcqRel));

        REDETECT_REQUESTED.store(false, Ordering::Release);
    }
}
