//! How the session is online, what is connected to it, and how it is running.
//!
//! Three readings that change rarely and come from a handful of short-lived
//! tools, so they share one slow poll rather than three fast ones. Each is its
//! own provider, so one unreadable tool takes only its own widget away.
//!
//! "Unreadable" is the word to keep hold of here. These tools are fast until
//! they are not — `nmcli` answers in four milliseconds and occasionally in
//! three seconds — and a poll that missed its deadline saw nothing rather than
//! seeing that there is nothing. What each reading does about that is its own
//! decision: the link is held for a bounded run of polls, and the adapter is
//! not reported at all.

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use celestina_shell_core::{bluetooth, network, power};
use serde_json::Value;

use super::tools::{lock_runtime, run_bounded};

const INTERVAL: Duration = Duration::from_secs(5);

pub const NETWORK: &str = "network";
pub const BLUETOOTH: &str = "bluetooth";
pub const POWER: &str = "power";

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let (Ok(net), Ok(bt), Ok(profile)) = (
        ProviderId::new(NETWORK),
        ProviderId::new(BLUETOOTH),
        ProviderId::new(POWER),
    ) else {
        eprintln!("celestina-provider-adapter: session: unusable provider name");
        return Ok(());
    };

    let mut state = lock_runtime(runtime);
    state.register(net.clone());
    state.register(bt.clone());
    state.register(profile.clone());
    drop(state);

    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name("session".to_owned())
        .spawn(move || run(&runtime, &net, &bt, &profile))?;
    Ok(())
}

/// Asks the daemon for the next profile it offers. A click is a request here
/// too: the panel never paints the profile it asked for, only the one the
/// daemon reports next — which is why a successful switch republishes
/// immediately rather than leaving the panel to show the old profile for up
/// to [`INTERVAL`] until the next poll catches up.
pub fn cycle_power_profile(
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    let listing = run_bounded("powerprofilesctl", &["list"])
        .ok_or_else(|| "power-profiles-daemon did not answer".to_owned())?;
    let active = power::parse_active(&listing)
        .ok_or_else(|| "power-profiles-daemon reports no active profile".to_owned())?;
    let next = power::next_profile(&active, &power::parse_profiles(&listing))
        .ok_or_else(|| format!("there is nothing to switch to from '{active}'"))?;

    run_bounded("powerprofilesctl", &["set", &next])
        .ok_or_else(|| format!("the daemon refused the profile '{next}'"))?;

    publish_power(runtime, id);
    Ok(())
}

/// What this poll of the routing table and `NetworkManager` managed to see.
///
/// The route is decisive when it is absent and otherwise names the device that
/// `nmcli` must describe. A failed route read is inconclusive; a failed device
/// read is inconclusive only when a route actually made it necessary.
fn observe_network() -> network::Observation {
    // The routing table first, and on its own. It is the only one of the two
    // that knows whether anything carries the session, and two of the three
    // outcomes are settled by its answer alone — so requiring `nmcli` to answer
    // before classifying anything is what let a real disconnection be held
    // indefinitely behind a slow device list.
    let route = network::read_route(run_bounded("ip", &["route", "show", "default"]).as_deref());

    // And the second command runs only when the first left something to ask.
    let devices = if network::needs_device_list(&route) {
        run_bounded(
            "nmcli",
            &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"],
        )
    } else {
        None
    };

    network::observe_with(&route, devices.as_deref())
}

fn publish_network(
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    tracker: &mut network::LinkTracker,
) {
    // The last confirmed link outlives any number of probes that failed to read
    // it, and is retired only by a poll that positively saw no default route.
    // Not being able to look is not the same as looking and finding nothing.
    let Some(link) = tracker.observe(observe_network()) else {
        // The routing table says nothing is carrying the session, twice over.
        // The widget says so by leaving, which is the one thing it can say
        // truthfully.
        lock_runtime(runtime).withdraw(id);
        return;
    };

    let mut payload = Payload::new();
    payload.insert("kind".to_owned(), Value::from(link.kind.clone()));
    payload.insert(
        "connection".to_owned(),
        Value::from(link.connection.clone()),
    );
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: network: {error}");
    }
}

fn publish_bluetooth(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    // The adapter is asked about first and separately: an adapter that is off
    // has no device list worth spawning a second command for, and an adapter
    // that never answered must not be reported as having none.
    let show = run_bounded("bluetoothctl", &["show"]);
    let devices = if show
        .as_deref()
        .and_then(bluetooth::parse_powered)
        .unwrap_or(false)
    {
        run_bounded("bluetoothctl", &["devices", "Connected"])
    } else {
        None
    };

    let Some(reading) = bluetooth::reading(show.as_deref(), devices.as_deref()) else {
        // Nobody could read the adapter. The widget leaves rather than claiming
        // a state it does not have.
        lock_runtime(runtime).withdraw(id);
        return;
    };

    let mut payload = Payload::new();
    payload.insert(
        "adapter".to_owned(),
        Value::from(reading.adapter.as_token()),
    );
    payload.insert(
        "count".to_owned(),
        Value::from(u32::try_from(reading.connected.len()).unwrap_or(u32::MAX)),
    );
    if let Some(first) = reading.connected.first() {
        payload.insert("first".to_owned(), Value::from(first.clone()));
    }
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: bluetooth: {error}");
    }
}

fn publish_power(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let Some(listing) = run_bounded("powerprofilesctl", &["list"]) else {
        lock_runtime(runtime).withdraw(id);
        return;
    };
    let Some(active) = power::parse_active(&listing) else {
        lock_runtime(runtime).withdraw(id);
        return;
    };

    let mut payload = Payload::new();
    payload.insert("active".to_owned(), Value::from(active));
    payload.insert(
        "count".to_owned(),
        Value::from(power::parse_profiles(&listing).len()),
    );
    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: power: {error}");
    }
}

fn run(runtime: &Mutex<ProviderRuntime>, net: &ProviderId, bt: &ProviderId, profile: &ProviderId) {
    // One tracker for one poll loop. Nothing else polls `nmcli`, and this
    // thread runs its three readings in order, so no two of these commands can
    // be in flight at once and a slow one delays the next poll rather than
    // overlapping it.
    let mut link = network::LinkTracker::new();
    loop {
        publish_network(runtime, net, &mut link);
        publish_bluetooth(runtime, bt);
        publish_power(runtime, profile);
        thread::sleep(INTERVAL);
    }
}
