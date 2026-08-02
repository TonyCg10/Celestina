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
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::brightness::{self, DdcDisplay};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::Value;

use super::tools::{lock_runtime, run_bounded_with};

/// A monitor may take its time. This is generous against a warm read of about a
/// second and the near-ten-second first call, and still bounded: a monitor that
/// never answers must not hold the thread forever.
const DDC_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the thread waits between looks at its target. Short enough that a
/// wheel feels answered, long enough that a burst arrives as one target.
const APPLY_DELAY: Duration = Duration::from_millis(250);
/// Nothing but this panel and the monitor's own buttons change brightness, so a
/// re-read exists only to notice the buttons — rarely, because it is expensive.
const REFRESH: Duration = Duration::from_secs(300);
/// DDC comes and goes on real hardware: the same `detect` answers with every
/// monitor one minute and none the next, and a sleeping monitor answers
/// nothing at all. Finding none is therefore not a verdict, so the search is
/// retried on its own shorter interval instead of waiting out a full refresh.
const REDETECT: Duration = Duration::from_secs(30);
/// The step a wheel notch asks for, in percent of the monitor's own range.
const STEP_PERCENT: u8 = 5;

pub const NAME: &str = "brightness";

/// What each monitor has been asked to become, by connector name. Empty means
/// nothing is owed. It is a module singleton because there is exactly one
/// helper process and one brightness provider in it; threading it through the
/// command dispatch would put a slow monitor's business in every provider's
/// signature.
static PENDING: OnceLock<Mutex<HashMap<String, i32>>> = OnceLock::new();

fn pending() -> &'static Mutex<HashMap<String, i32>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_pending() -> std::sync::MutexGuard<'static, HashMap<String, i32>> {
    match pending().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: brightness: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id))?;
    Ok(())
}

/// Records a step and returns at once. The wheel is answered by the panel's
/// next reading, not by this call — which is the only honest thing a control
/// over a one-second conversation can do.
pub fn action(verb: &str, options: &Payload) -> Result<(), String> {
    let direction: i32 = match verb {
        "brighter" => 1,
        "dimmer" => -1,
        _ => return Err(format!("'{NAME}' does not serve the verb '{verb}'")),
    };

    let output = options
        .get("output")
        .and_then(Value::as_str)
        .filter(|output| !output.is_empty())
        .ok_or_else(|| format!("'{NAME}' needs the output to step"))?;

    *lock_pending().entry(output.to_owned()).or_insert(0) += direction;
    Ok(())
}

/// The monitors that answer DDC at all. Detection is the expensive call, so it
/// happens once: a monitor plugged in later is picked up on the next refresh.
fn detect() -> Vec<DdcDisplay> {
    run_bounded_with("ddcutil", &["detect", "--brief"], DDC_TIMEOUT)
        .map(|listing| brightness::parse_detect(&listing))
        .unwrap_or_default()
}

fn read(display: &DdcDisplay) -> Option<u8> {
    let number = display.number.to_string();
    run_bounded_with(
        "ddcutil",
        &["--display", &number, "getvcp", "10", "--brief"],
        DDC_TIMEOUT,
    )
    .and_then(|reading| brightness::parse_brightness(&reading))
}

fn write(display: &DdcDisplay, value: u8) -> bool {
    let number = display.number.to_string();
    let level = value.to_string();
    run_bounded_with(
        "ddcutil",
        &["--display", &number, "setvcp", "10", &level],
        DDC_TIMEOUT,
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

fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let mut displays = detect();
    let mut levels: HashMap<String, Option<u8>> = displays
        .iter()
        .map(|display| (display.connector.clone(), None))
        .collect();
    // Say which monitors have a brightness before saying what it is: the panel
    // shows "unknown" for a second rather than nothing at all.
    publish(runtime, id, &levels);

    for display in &displays {
        levels.insert(display.connector.clone(), read(display));
        publish(runtime, id, &levels);
    }

    let mut refreshed = Instant::now();
    loop {
        // One target per monitor, however many notches produced it.
        let steps: Vec<(String, i32)> = lock_pending().drain().collect();
        for (connector, steps) in steps {
            let Some(display) = displays
                .iter()
                .find(|display| display.connector == connector)
            else {
                continue;
            };
            let Some(current) = levels.get(&connector).copied().flatten() else {
                // Stepping from a value nobody has read would be guessing.
                eprintln!(
                    "celestina-provider-adapter: brightness: {connector} has not answered yet"
                );
                continue;
            };

            let wanted = brightness::stepped(current, steps, STEP_PERCENT);
            if wanted != current && write(display, wanted) {
                // What the monitor settled on, not what it was asked for.
                levels.insert(connector.clone(), read(display));
                publish(runtime, id, &levels);
            }
        }

        let due = if displays.is_empty() {
            REDETECT
        } else {
            REFRESH
        };
        if refreshed.elapsed() >= due {
            displays = detect();
            levels = displays
                .iter()
                .map(|display| (display.connector.clone(), read(display)))
                .collect();
            publish(runtime, id, &levels);
            refreshed = Instant::now();
        }

        thread::sleep(APPLY_DELAY);
    }
}
