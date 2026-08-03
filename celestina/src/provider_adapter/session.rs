//! How the session is online, what is connected to it, and how it is running.
//!
//! Three readings that change rarely and come from four short-lived tools, so
//! they share one slow poll rather than three fast ones. Each is its own
//! provider, so one unreadable tool takes only its own widget away.

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

fn publish_network(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let route = run_bounded("ip", &["route", "show", "default"]).unwrap_or_default();
    let devices = run_bounded(
        "nmcli",
        &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"],
    );

    match devices
        .map(|listing| network::parse_devices(&listing))
        .and_then(|devices| {
            network::active_link(
                &devices,
                network::parse_default_route_device(&route).as_deref(),
            )
        }) {
        Some(link) => {
            let mut payload = Payload::new();
            payload.insert("kind".to_owned(), Value::from(link.kind));
            payload.insert("connection".to_owned(), Value::from(link.connection));
            if let Err(error) = lock_runtime(runtime).publish(id, payload) {
                eprintln!("celestina-provider-adapter: network: {error}");
            }
        }
        // Nothing is carrying the session: the widget says so by leaving,
        // which is the one thing it can say truthfully.
        None => lock_runtime(runtime).withdraw(id),
    }
}

fn publish_bluetooth(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let connected = run_bounded("bluetoothctl", &["devices", "Connected"])
        .map(|listing| bluetooth::parse_connected(&listing))
        .unwrap_or_default();

    if connected.is_empty() {
        // A powered adapter with nothing on it is not news, and R5 is where
        // turning it on and off belongs.
        lock_runtime(runtime).withdraw(id);
        return;
    }

    let mut payload = Payload::new();
    payload.insert(
        "count".to_owned(),
        Value::from(u32::try_from(connected.len()).unwrap_or(u32::MAX)),
    );
    payload.insert("first".to_owned(), Value::from(connected[0].clone()));
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
    loop {
        publish_network(runtime, net);
        publish_bluetooth(runtime, bt);
        publish_power(runtime, profile);
        thread::sleep(INTERVAL);
    }
}
