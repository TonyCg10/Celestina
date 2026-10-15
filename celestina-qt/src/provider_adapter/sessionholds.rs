//! Staying awake: the session state this shell holds by keeping a program
//! alive.
//!
//! This is not a reading. `systemd-inhibit` owns a logind lock, so what the
//! panel is told is simply whether this helper still has that child — see
//! [`super::held`] for why that distinction is the whole design. Night light
//! is deliberately separate: its Wayland gamma worker owns a gradual numeric
//! transition rather than a process whose liveness is the state.
//!
//! Nothing turns the idle chain on by itself. It stays off until somebody asks
//! for it, which is the only honest default for a state whose whole effect is
//! stopping the session from sleeping.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::session::{self, SessionRequest};
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use super::held::{wanted, Hold};
use super::tools::lock_runtime;
use super::worker::Worker;

/// Nothing here changes on its own, so the poll exists only to notice a holder
/// that died: rarely enough to cost nothing, often enough that the panel is not
/// left claiming a state the session lost.
const INTERVAL: Duration = Duration::from_secs(2);
/// How many pieces the poll interval is slept in. A shutdown request is noticed
/// within one piece, so the helper's exit is not held up by a full interval.
const SLEEP_SLICES: u32 = 8;

pub const CAFFEINE: &str = "caffeine";

/// `--mode=block` because a delay inhibitor only postpones sleep; what the
/// verb promises is that the session does not idle or suspend while it is on.
const CAFFEINE_TOOL: &str = "systemd-inhibit";

static AWAKE: OnceLock<Mutex<Hold>> = OnceLock::new();

fn awake() -> &'static Mutex<Hold> {
    AWAKE.get_or_init(|| {
        Mutex::new(Hold::new(
            CAFFEINE_TOOL,
            vec![
                "--what=idle:sleep".to_owned(),
                "--who=Celestina".to_owned(),
                "--why=The session was asked to stay awake".to_owned(),
                "--mode=block".to_owned(),
                "sleep".to_owned(),
                "infinity".to_owned(),
            ],
        ))
    })
}

fn lock_hold(hold: &'static Mutex<Hold>) -> std::sync::MutexGuard<'static, Hold> {
    match hold.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn spawn(
    runtime: &Arc<Mutex<ProviderRuntime>>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<Option<Worker>> {
    let Ok(caffeine_id) = ProviderId::new(CAFFEINE) else {
        eprintln!("celestina-provider-adapter: caffeine: unusable provider name");
        return Ok(None);
    };
    lock_runtime(runtime).register(caffeine_id.clone());

    let runtime = Arc::clone(runtime);
    let worker_shutdown = Arc::clone(shutdown);
    // This thread starts the remembered hold, so it has to stop before release.
    // Left detached, it could take the hold back after `release_all` had already
    // given it up and leave a child with nothing able to end it.
    Worker::spawn("session-holds", shutdown, move || {
        run(&runtime, &caffeine_id, &worker_shutdown);
    })
    .map(Some)
}

/// Applies one caffeine verb and publishes what the session is actually left
/// in.
///
/// # Errors
///
/// Returns the requester's sentence for a verb this provider does not serve or
/// a tool that could not be started. A missing tool is a refusal: the state is
/// never reported as on because somebody asked for it.
pub fn action(
    verb: &str,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    let SessionRequest::Caffeine(state) = session::parse_for(CAFFEINE, verb, options)? else {
        return Err(session::unserved_verb(CAFFEINE, verb));
    };

    let mut hold = lock_hold(awake());
    let current = hold.is_held();
    let outcome = hold.set(wanted(state, current));
    let now_held = hold.is_held();
    // Whatever happened — taken, released or refused — the panel is told what
    // is true now rather than what was asked for.
    publish(runtime, id, now_held);
    drop(hold);

    outcome?;
    // The session really changed, so the choice that describes it is recorded.
    // A preference that persisted while the change itself failed would be a
    // promise nothing kept, which is why this is after the outcome.
    if let Err(error) = super::settings::remember(|settings| settings.caffeine = now_held) {
        // The session is in the state that was asked for; only its survival
        // past this session failed, and that is worth saying without undoing
        // what did work.
        eprintln!("celestina-provider-adapter: caffeine: {error}");
    }
    Ok(())
}

/// Releases the hold. The helper calls this before it exits, so a shell that
/// stops never leaves the session unable to sleep with nothing left that knows
/// how to undo it.
pub fn release_all() {
    lock_hold(awake()).release();
}

fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, held: bool) {
    let mut payload = Payload::new();
    payload.insert("active".to_owned(), Value::from(held));
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: {}: {error}", id.as_str());
    }
}

/// Puts the session back into the state the person chose last time.
fn restore(shutdown: &AtomicBool) {
    if !super::settings::current().caffeine || shutdown.load(Ordering::Acquire) {
        return;
    }
    if let Err(error) = lock_hold(awake()).set(true) {
        eprintln!("celestina-provider-adapter: caffeine: {error}");
    }
}

fn run(runtime: &Mutex<ProviderRuntime>, caffeine_id: &ProviderId, shutdown: &AtomicBool) {
    restore(shutdown);
    while !shutdown.load(Ordering::Acquire) {
        // Asking is what notices a holder that died, so this is a poll of this
        // helper's own child rather than of the session.
        let awake_held = lock_hold(awake()).is_held();
        publish(runtime, caffeine_id, awake_held);
        // Slept in slices so a shutdown is noticed within one of them rather
        // than after the full poll interval.
        for _ in 0..SLEEP_SLICES {
            if shutdown.load(Ordering::Acquire) {
                return;
            }
            thread::sleep(INTERVAL / SLEEP_SLICES);
        }
    }
}
