//! The one provider that leaves the machine.
//!
//! [`celestina_shell_core::weather`] owns every rule: what the request may
//! carry, what an answer has to look like to be believed, how long a reading
//! counts, and how long to leave a service alone after it refuses. This module
//! only performs the request — through `curl`, the way every other provider
//! here uses a tool the session already has, rather than linking a TLS stack
//! into the shell for one small GET.
//!
//! Nothing is asked at all until the person has set a location. No location
//! means no weather and no request: this shell does not look up where somebody
//! is.

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use celestina_shell_core::weather::{self, Cached, Next};
use serde_json::Value;

use super::tools::{lock_runtime, run_bounded_with};

pub const NAME: &str = "weather";

/// How often the decision is revisited. The decision itself is almost always
/// "keep"; this is only how promptly a stale reading is noticed.
const TICK: Duration = Duration::from_secs(30);
/// A network request over somebody's connection. Generous, and bounded so a
/// hanging service never becomes a hanging thread.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: weather: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id))?;
    Ok(())
}

/// Asks once. `--fail` so an HTTP error is a failure rather than an error page
/// parsed as weather, and `--silent` so progress never reaches the frames.
fn fetch(url: &str) -> Option<Vec<u8>> {
    run_bounded_with(
        "curl",
        &[
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "15",
            url,
        ],
        REQUEST_TIMEOUT,
    )
    .map(String::into_bytes)
}

fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId, cached: Option<Cached>, now_ms: u64) {
    // A reading that stopped being current is absent, not stale: the widget
    // leaves rather than carrying a temperature that is no longer true.
    let Some(cached) = cached.filter(|cached| weather::still_worth_showing(*cached, now_ms)) else {
        lock_runtime(runtime).withdraw(id);
        return;
    };

    let place = super::settings::current().weather;
    let mut payload = Payload::new();
    payload.insert("celsius".to_owned(), Value::from(cached.reading.celsius));
    payload.insert("code".to_owned(), Value::from(cached.reading.code));
    payload.insert("daylight".to_owned(), Value::from(cached.reading.daylight));
    if let Some(place) = place {
        payload.insert("label".to_owned(), Value::from(place.label));
    }

    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: weather: {error}");
    }
}

fn run(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let started = Instant::now();
    let mut cached: Option<Cached> = None;
    let mut last_failure_ms: Option<u64> = None;

    loop {
        let now_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let place = super::settings::current().weather;

        match place {
            // No location is no weather, and no request either.
            None => {
                cached = None;
                lock_runtime(runtime).withdraw(id);
            }
            Some(place) => match weather::next_step(cached, last_failure_ms, now_ms) {
                Next::Keep | Next::Wait => publish(runtime, id, cached, now_ms),
                Next::Ask => {
                    match fetch(&weather::request_url(&place))
                        .as_deref()
                        .map(weather::read)
                    {
                        Some(Some(reading)) => {
                            cached = Some(Cached {
                                reading,
                                taken_ms: now_ms,
                            });
                            last_failure_ms = None;
                        }
                        // A refusal and an unreadable answer are the same thing
                        // here: no new reading, and a pause before asking again.
                        _ => last_failure_ms = Some(now_ms),
                    }
                    publish(runtime, id, cached, now_ms);
                }
            },
        }

        thread::sleep(TICK);
    }
}
