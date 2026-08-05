//! Night light and staying awake: the two session states this shell holds by
//! keeping somebody else's program alive.
//!
//! Neither is a reading. `wlsunset` owns the gamma ramps and `systemd-inhibit`
//! owns a logind lock, so what the panel is told is simply whether this helper
//! still has that child — see [`super::held`] for why that distinction is the
//! whole design. They share one module because they share one lifecycle: the
//! same poll notices either one dying, and the same shutdown releases both.
//!
//! Nothing turns either of them on by itself. The idle chain in particular
//! stays off until somebody asks for it, which is the only honest default for a
//! state whose whole effect is stopping the session from sleeping.

use std::io;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::session::{self, SessionRequest};
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use super::held::{wanted, Hold};
use super::tools::lock_runtime;

/// Nothing here changes on its own, so the poll exists only to notice a holder
/// that died: rarely enough to cost nothing, often enough that the panel is not
/// left claiming a state the session lost.
const INTERVAL: Duration = Duration::from_secs(2);

pub const NIGHT_LIGHT: &str = "night-light";
pub const CAFFEINE: &str = "caffeine";

/// One fixed warm temperature rather than a location-driven curve: this shell
/// offers night light as something you turn on, not as a schedule it decides
/// for you. `wlsunset` insists the high temperature be above the low one, so
/// the pair is 2701/2700 K and both ends of its ramp are the same warmth.
const NIGHT_LIGHT_TOOL: &str = "wlsunset";
const NIGHT_LIGHT_HIGH: &str = "2701";
const NIGHT_LIGHT_LOW: &str = "2700";

/// `--mode=block` because a delay inhibitor only postpones sleep; what the
/// verb promises is that the session does not idle or suspend while it is on.
const CAFFEINE_TOOL: &str = "systemd-inhibit";

static NIGHT: OnceLock<Mutex<Hold>> = OnceLock::new();
static AWAKE: OnceLock<Mutex<Hold>> = OnceLock::new();

fn night() -> &'static Mutex<Hold> {
    NIGHT.get_or_init(|| {
        Mutex::new(Hold::new(
            NIGHT_LIGHT_TOOL,
            vec![
                "-T".to_owned(),
                NIGHT_LIGHT_HIGH.to_owned(),
                "-t".to_owned(),
                NIGHT_LIGHT_LOW.to_owned(),
            ],
        ))
    })
}

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

fn hold_for(provider: &str) -> Option<&'static Mutex<Hold>> {
    match provider {
        NIGHT_LIGHT => Some(night()),
        CAFFEINE => Some(awake()),
        _ => None,
    }
}

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let (Ok(night_id), Ok(caffeine_id)) = (ProviderId::new(NIGHT_LIGHT), ProviderId::new(CAFFEINE))
    else {
        eprintln!("celestina-provider-adapter: session holds: unusable provider name");
        return Ok(());
    };

    let mut state = lock_runtime(runtime);
    state.register(night_id.clone());
    state.register(caffeine_id.clone());
    drop(state);

    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name("session-holds".to_owned())
        .spawn(move || run(&runtime, &night_id, &caffeine_id))?;
    Ok(())
}

/// Applies one switch verb and publishes what the session is actually left in.
///
/// # Errors
///
/// Returns the requester's sentence for a verb this provider does not serve or
/// a tool that could not be started. A missing tool is a refusal: the state is
/// never reported as on because somebody asked for it.
pub fn action(
    provider: &str,
    verb: &str,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    let Some(shared) = hold_for(provider) else {
        return Err(session::unserved_verb(provider, verb));
    };

    // `parse_for` already refused anything this provider does not serve, so
    // the only shapes left are the two switches these holds are.
    let (SessionRequest::NightLight(state) | SessionRequest::Caffeine(state)) =
        session::parse_for(provider, verb, options)?
    else {
        return Err(session::unserved_verb(provider, verb));
    };

    let mut hold = lock_hold(shared);
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
    let remembered = if provider == NIGHT_LIGHT {
        super::settings::remember(|settings| settings.night_light = now_held)
    } else {
        super::settings::remember(|settings| settings.caffeine = now_held)
    };
    if let Err(error) = remembered {
        // The session is in the state that was asked for; only its survival
        // past this session failed, and that is worth saying without undoing
        // what did work.
        eprintln!("celestina-provider-adapter: {provider}: {error}");
    }
    Ok(())
}

/// Releases both holds. The helper calls this before it exits, so a shell that
/// stops never leaves the screen tinted or the session unable to sleep with
/// nothing left that knows how to undo it.
pub fn release_all() {
    lock_hold(night()).release();
    lock_hold(awake()).release();
}

fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, held: bool) {
    let mut payload = Payload::new();
    payload.insert("active".to_owned(), Value::from(held));
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: {}: {error}", id.as_str());
    }
}

/// Puts the session back into the states the person chose last time.
///
/// A failure here is reported and left: a tool that has since been uninstalled
/// must not stop the rest of the shell from starting, and the published state
/// will simply say the hold is not held.
fn restore() {
    let chosen = super::settings::current();
    for (wanted, hold, what) in [
        (chosen.night_light, night(), NIGHT_LIGHT),
        (chosen.caffeine, awake(), CAFFEINE),
    ] {
        if !wanted {
            continue;
        }
        if let Err(error) = lock_hold(hold).set(true) {
            eprintln!("celestina-provider-adapter: {what}: {error}");
        }
    }
}

fn run(runtime: &Mutex<ProviderRuntime>, night_id: &ProviderId, caffeine_id: &ProviderId) {
    restore();
    loop {
        // Asking is what notices a holder that died, so this is a poll of this
        // helper's own children rather than of the session.
        let night_held = lock_hold(night()).is_held();
        let awake_held = lock_hold(awake()).is_held();
        publish(runtime, night_id, night_held);
        publish(runtime, caffeine_id, awake_held);
        thread::sleep(INTERVAL);
    }
}
