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
// Aliased because this module already imports `serde_json::Value` for the
// payloads it publishes; the journal's field values are a different vocabulary.
use celestina_shell_core::diagnostics::{Event, Level, Value as Field};
use celestina_shell_core::journal;
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::session::{self, LevelChange, SessionRequest};
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use rustix::fs::{flock, FlockOperation};

use super::tools::{lock_runtime, run_bounded_with_cancel};
use super::worker::Worker;

/// A monitor may take its time. This is generous against a warm read of about a
/// second and the near-ten-second first call, and still bounded: a monitor that
/// never answers must not hold the thread forever.
const DDC_TIMEOUT: Duration = Duration::from_secs(20);

/// Turns DDC off for this process.
///
/// `ddcutil` is the one thing this shell does that reaches the graphics card's
/// own I²C buses, and it is the only part of a provider helper that touches
/// real hardware rather than a session service. Automated runs need the helper
/// to start, register and publish exactly as it does in a session — but a run
/// that is only proving the host loads has no business opening a bus that the
/// desktop the author is sitting in front of is already using. Two GPU losses
/// have been recorded with concurrent `ddcutil` children on one bus.
///
/// Reading the name of the thing rather than a negation, `0` or `false` turns
/// it off, exactly as `CELESTINA_PANEL_MENU` does for the panel's menu.
const DDC_ENABLED_VAR: &str = "CELESTINA_DDC";

/// Whether a value from the environment asks for DDC to be off.
///
/// Absent means on: a session that says nothing gets the hardware it always
/// had. An unreadable value also means on, because a typo must not silently
/// remove a working control.
fn ddc_requested(value: Option<&str>) -> bool {
    match value.map(|value| value.trim().to_ascii_lowercase()) {
        None => true,
        Some(value) if value.is_empty() => true,
        Some(value) => !matches!(value.as_str(), "0" | "false"),
    }
}

/// Read once: the answer cannot change while this helper runs, and re-reading
/// it inside the worker loop would put a `getenv` in the DDC path.
fn ddc_is_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let enabled = ddc_requested(std::env::var(DDC_ENABLED_VAR).ok().as_deref());
        if !enabled {
            // Critical like every other DDC line: a journal that shows no DDC
            // activity must say whether that is because nothing happened or
            // because it was switched off.
            journal::record_from(
                "ddc",
                Event::new(Level::Critical, "ddc.disabled").with_text("variable", DDC_ENABLED_VAR),
            );
        }
        enabled
    })
}

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
    journal::record(Event::new(Level::Critical, "ddc.redetect.requested"));
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
/// Whether a DDC conversation is open right now.
///
/// The invariant this shell has always claimed is that exactly one worker ever
/// talks to `ddcutil`, so two operations can never overlap on the I²C buses of a
/// card whose driver has been seen dropping off the `PCIe` bus. A claim nobody
/// measures is a belief, so this measures it: an overlap is recorded as its own
/// critical event rather than asserted, because taking the shell down would
/// destroy the record that the overlap happened.
static DDC_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// How long an operation waits for the session's DDC lease before refusing.
///
/// Longer than [`DDC_TIMEOUT`] on purpose: the likeliest holder is another
/// process's own bounded conversation, so waiting one full conversation out is
/// the normal case, not the pathological one.
const DDC_LEASE_PATIENCE: Duration = Duration::from_secs(25);

/// The session-wide DDC lease. `DDC_IN_FLIGHT` above measures the one-worker
/// claim inside this process; this enforces it across processes, which the
/// instrument cannot.
///
/// The retained GPU losses were both preceded by concurrent `ddcutil` children
/// on one I²C bus, and this session's diagnostics have now recorded the exact
/// shape that produces them with nobody misbehaving: several freshly started
/// shells — a restart storm during a freeze, or the development nest beside
/// the real session — each running its own startup detect at once. An
/// advisory `flock` on one runtime file serializes every conversation this
/// suite starts, whoever starts it.
///
/// Residual and stated: a helper killed with SIGKILL releases the lease (the
/// kernel closes the descriptor) while its abandoned `ddcutil` child may still
/// be finishing. That window already has its own answer — the host spaces the
/// replacement by the abandoned child's lifetime — and a lease held by the
/// helper cannot close it without handing the descriptor to the child, which
/// would let every concurrently spawned process inherit and pin it.
struct DdcLease {
    _file: std::fs::File,
}

fn ddc_lease_path() -> std::path::PathBuf {
    let mut base = std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    base.push("celestina-ddc.lock");
    base
}

/// The lease, or `None` for an operation that must not run: another process
/// kept the bus for longer than a whole conversation, or shutdown was asked
/// for while waiting.
fn acquire_ddc_lease(shutdown: &AtomicBool) -> Option<DdcLease> {
    let path = ddc_lease_path();
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            // No file means no serialization, and unserialized DDC is the
            // thing that kills the card: refuse the operation, not the rule.
            journal::record_from(
                "ddc",
                Event::new(Level::Critical, "ddc.lease.unavailable")
                    .with_text("error", &error.to_string()),
            );
            return None;
        }
    };

    let deadline = Instant::now() + DDC_LEASE_PATIENCE;
    let mut waited = false;
    loop {
        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {
                if waited {
                    journal::record_from(
                        "ddc",
                        Event::new(Level::Critical, "ddc.lease.acquired-after-wait"),
                    );
                }
                return Some(DdcLease { _file: file });
            }
            Err(rustix::io::Errno::WOULDBLOCK) => {}
            Err(error) => {
                journal::record_from(
                    "ddc",
                    Event::new(Level::Critical, "ddc.lease.unavailable")
                        .with_text("error", &error.to_string()),
                );
                return None;
            }
        }
        if !waited {
            waited = true;
            // Another process of this suite is mid-conversation. That this
            // line exists at all is the point: the overlap used to happen.
            journal::record_from("ddc", Event::new(Level::Critical, "ddc.lease.wait"));
        }
        if shutdown.load(Ordering::Acquire) || Instant::now() >= deadline {
            journal::record_from("ddc", Event::new(Level::Critical, "ddc.lease.refused"));
            return None;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Runs one DDC operation, recorded from both ends.
///
/// These are `Critical` and therefore flushed: `ddcutil` is the one thing this
/// shell does that reaches the graphics card's own I²C buses, so "an operation
/// started and this file ends" is the single most useful line the journal could
/// ever hold about the freeze.
fn ddc_operation<T>(
    operation: &str,
    output: Option<&str>,
    number: Option<u8>,
    shutdown: &AtomicBool,
    run: impl FnOnce() -> T,
) -> Option<T> {
    let describe = |name: &str| {
        let mut event = Event::new(Level::Critical, name).with_text("operation", operation);
        if let Some(output) = output {
            event = event.with_text("output", output);
        }
        if let Some(number) = number {
            event = event.with("display", Field::Uint(u64::from(number)));
        }
        event
    };

    // The session-wide lease first: an operation another process would overlap
    // does not run at all. This one is a refusal where `DDC_IN_FLIGHT` is an
    // instrument, because the cross-process overlap is the recorded prelude to
    // losing the card from the bus, and no reading is worth that.
    let _lease = acquire_ddc_lease(shutdown)?;

    if DDC_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        // Not a refusal: the operation still runs, because refusing here would
        // change behaviour on the strength of an instrument. It is recorded so
        // the claim can be checked against a real session instead of assumed.
        journal::record_from("ddc", describe("ddc.overlap"));
    }

    journal::record_from("ddc", describe("ddc.start"));
    let started = Instant::now();
    let answer = run();
    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    DDC_IN_FLIGHT.store(false, Ordering::Release);
    journal::record_from(
        "ddc",
        describe("ddc.end").with("elapsed_ms", Field::Millis(elapsed)),
    );
    Some(answer)
}

fn detect(shutdown: &AtomicBool) -> Vec<DdcDisplay> {
    // No bus is opened and no child is spawned. The empty list is the same
    // answer a machine whose monitors do not speak DDC/CI already gives, so
    // every path after this one is the supported one rather than a new state
    // invented for automation.
    if !ddc_is_enabled() {
        return Vec::new();
    }

    let displays = ddc_operation("detect", None, None, shutdown, || {
        run_bounded_with_cancel(
            "ddcutil",
            &["detect", "--brief"],
            DDC_TIMEOUT,
            Some(shutdown),
        )
        .map(|listing| brightness::parse_detect(&listing))
        .unwrap_or_default()
    })
    // A refused lease reads as no monitors this round; the schedule and the
    // redetect requests ask again, on a bus that is no longer contended.
    .unwrap_or_default();

    // The technical inventory the buses were found as — connector and display
    // number only. `ddcutil detect` also prints EDID, serial numbers and the
    // monitor's model, and none of that is recorded: it identifies hardware in
    // somebody's room and answers no question this journal exists to answer.
    let mut found = Event::new(Level::Critical, "ddc.detected")
        .with("displays", Field::Uint(displays.len() as u64));
    for display in &displays {
        found = found.with(
            &format!("display_{}", display.number),
            Field::text(&display.connector),
        );
    }
    journal::record_from("ddc", found);
    displays
}

fn read(display: &DdcDisplay, shutdown: &AtomicBool) -> Option<u8> {
    let number = display.number.to_string();
    ddc_operation(
        "read",
        Some(&display.connector),
        Some(display.number),
        shutdown,
        || {
            run_bounded_with_cancel(
                "ddcutil",
                &["--display", &number, "getvcp", "10", "--brief"],
                DDC_TIMEOUT,
                Some(shutdown),
            )
            .and_then(|reading| brightness::parse_brightness(&reading))
        },
    )
    .flatten()
}

fn write(display: &DdcDisplay, value: u8, shutdown: &AtomicBool) -> bool {
    let number = display.number.to_string();
    let level = value.to_string();
    ddc_operation(
        "write",
        Some(&display.connector),
        Some(display.number),
        shutdown,
        || {
            run_bounded_with_cancel(
                "ddcutil",
                &["--display", &number, "setvcp", "10", &level],
                DDC_TIMEOUT,
                Some(shutdown),
            )
            .is_some()
        },
    )
    // A refused lease is a failed write; the pending target survives in the
    // worker's own retry shape, the same as a monitor that did not answer.
    .unwrap_or(false)
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
    journal::record_from("ddc", Event::new(Level::Critical, "ddc.worker.start"));
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
    use super::{ddc_operation, ddc_requested, request_redetect, REDETECT_REQUESTED};

    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };

    // Both cases deliberately exercise the process-global flag. Rust runs unit
    // tests in parallel, so they take one test-only lock rather than making
    // their assertions depend on which test the harness scheduled between two
    // atomic operations.
    static REDETECT_TEST: Mutex<()> = Mutex::new(());

    /// DDC is on unless something readable asks for it to be off.
    ///
    /// The default matters more than the switch: a session that says nothing
    /// must keep the hardware control it has always had, and a typo must not
    /// silently remove it either. Only an explicit, readable refusal counts.
    #[test]
    fn only_a_readable_refusal_turns_ddc_off() {
        assert!(ddc_requested(None));
        assert!(ddc_requested(Some("")));
        assert!(ddc_requested(Some("   ")));
        assert!(ddc_requested(Some("1")));
        assert!(ddc_requested(Some("true")));
        // Unreadable is not a refusal.
        assert!(ddc_requested(Some("perhaps")));
        assert!(ddc_requested(Some("00")));

        assert!(!ddc_requested(Some("0")));
        assert!(!ddc_requested(Some("false")));
        // Case and surrounding blanks are the shape a shell script produces.
        assert!(!ddc_requested(Some("FALSE")));
        assert!(!ddc_requested(Some(" 0 ")));
    }

    /// Two DDC operations of this shell can never overlap.
    ///
    /// The invariant is that one worker owns `ddcutil`, and this proves the
    /// instrument that watches it: sequential operations record no overlap, and
    /// a deliberately nested pair does. Nothing here runs `ddcutil`, opens a bus
    /// or touches hardware — the operation body is a closure that returns.
    #[test]
    fn two_ddc_operations_never_overlap_and_an_overlap_would_be_recorded() {
        use super::super::tools::{test_journal, test_journal_lines};

        test_journal();
        let _serialize = REDETECT_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // How the one worker actually behaves: one after another.
        let live = AtomicBool::new(false);
        assert!(ddc_operation("read", Some("DDC-FIXTURE-A"), Some(1), &live, || ()).is_some());
        assert!(ddc_operation("read", Some("DDC-FIXTURE-A"), Some(1), &live, || ()).is_some());
        let overlaps = |connector: &str| {
            test_journal_lines()
                .into_iter()
                .filter(|line| line["event"] == "ddc.overlap")
                .filter(|line| line["output"] == connector)
                .count()
        };
        assert_eq!(overlaps("DDC-FIXTURE-A"), 0);

        // And what a second owner meets now: the session lease. The nested
        // operation is the overlap shape — it contends on the same lock file
        // through its own descriptor, exactly as another process would — and
        // it is refused rather than run. `raised` stands in for that process's
        // shutdown so the refusal is immediate instead of a real wait.
        let raised = AtomicBool::new(true);
        let inner = ddc_operation("read", Some("DDC-FIXTURE-B"), Some(2), &live, || {
            ddc_operation("write", Some("DDC-FIXTURE-B"), Some(2), &raised, || ())
        });
        assert!(inner.expect("the outer operation runs").is_none());
        assert_eq!(
            overlaps("DDC-FIXTURE-B"),
            0,
            "a refused operation is exactly the overlap that never happened"
        );
        let refusals = test_journal_lines()
            .into_iter()
            .filter(|line| line["event"] == "ddc.lease.refused")
            .count();
        assert!(refusals >= 1, "and the refusal itself is on the record");

        // Both ends of every operation are on the disk, which is what makes
        // "started and never finished" readable after a freeze.
        let starts = test_journal_lines()
            .into_iter()
            .filter(|line| line["event"] == "ddc.start")
            .filter(|line| {
                line["output"]
                    .as_str()
                    .is_some_and(|o| o.starts_with("DDC-FIXTURE"))
            })
            .count();
        let ends = test_journal_lines()
            .into_iter()
            .filter(|line| line["event"] == "ddc.end")
            .filter(|line| {
                line["output"]
                    .as_str()
                    .is_some_and(|o| o.starts_with("DDC-FIXTURE"))
            })
            .count();
        // Three, not four: the refused operation never started, so it owes
        // the journal no bracket.
        assert_eq!(starts, 3);
        assert_eq!(ends, 3);
    }

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
