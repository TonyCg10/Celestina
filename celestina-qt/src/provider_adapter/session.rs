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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::command::Outcome;
use celestina_shell_core::connectivity::{self, Action, Expected};
use celestina_shell_core::inventory::Answer;
use celestina_shell_core::pending::{Awaiting, Pending, Settled};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use celestina_shell_core::{bluetooth, inventory, network, power};
use serde_json::Value;

use super::tools::{lock_runtime, probe_bounded, probe_bounded_with_cancel, run_bounded};

const INTERVAL: Duration = Duration::from_secs(5);

/// What an action's own tool is given before it is killed.
///
/// Longer than the reading deadline because these are conversations, not
/// queries: `bluetoothctl connect` negotiates with a device across a radio and
/// `nmcli connection up` waits on an association. Still bounded, still killed
/// and reaped, and still not on the Qt thread.
const ACTION_TIMEOUT: Duration = Duration::from_secs(10);

pub const NETWORK: &str = "network";
pub const BLUETOOTH: &str = "bluetooth";
pub const POWER: &str = "power";

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>, shutdown: &Arc<AtomicBool>) -> io::Result<()> {
    // Recorded before the thread starts, so an action's child is killed and
    // reaped as soon as the helper is stopping.
    let _ = STOPPING.set(Arc::clone(shutdown));

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

/// What the command worker and the polling thread both need, and the one lock
/// they meet under.
///
/// The poll writes the inventories and reads the ledger; the worker reads the
/// inventories to validate an identity and writes the ledger. Neither ever
/// holds this across a process, so a ten-second `bluetoothctl` cannot block a
/// poll from settling something else.
#[derive(Default)]
struct Connectivity {
    /// The last conclusive rows each provider published. The only identities a
    /// request may name.
    networks: Vec<network::KnownNetwork>,
    devices: Vec<bluetooth::KnownDevice>,
    waiting: Pending<Expected>,
    /// Set by `refresh`, cleared by the poll that honours it.
    look_again: bool,
}

/// The shared state, plus the condition the poll waits on so `refresh` can
/// wake it without shortening the interval for anyone else.
struct Shared {
    state: Mutex<Connectivity>,
    woken: Condvar,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
/// The helper's shutdown flag, so an action's child is killed and reaped when
/// the process is stopping rather than outliving it.
static STOPPING: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn shared() -> &'static Shared {
    SHARED.get_or_init(|| Shared {
        state: Mutex::new(Connectivity::default()),
        woken: Condvar::new(),
    })
}

/// A poisoned lock still owns usable state; recovering it keeps the panel fed
/// rather than taking the helper down with the thread that panicked.
fn lock_state() -> std::sync::MutexGuard<'static, Connectivity> {
    match shared().state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn stopping() -> bool {
    STOPPING
        .get()
        .is_some_and(|flag| flag.load(Ordering::Acquire))
}

/// The monotonic stamp the ledger's deadlines are measured against.
///
/// Its own clock rather than the helper's, because nothing here is compared
/// with a snapshot's timestamps — only with itself.
fn now_ms() -> u64 {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    u64::try_from(STARTED.get_or_init(Instant::now).elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Carries out one validated action.
///
/// Every argument is a separate word handed to `execve`; nothing is a shell
/// line and nothing is concatenated. `nmcli`'s `uuid` keyword and the
/// leading-dash refusal in the domain are what keep an identity from being read
/// as an option.
fn action_command(action: &Action) -> Option<(&'static str, Vec<&str>)> {
    match action {
        Action::Refresh => None,
        Action::ActivateSaved { uuid } => Some(("nmcli", vec!["connection", "up", "uuid", uuid])),
        Action::SetPowered(powered) => Some((
            "bluetoothctl",
            vec!["power", if *powered { "on" } else { "off" }],
        )),
        Action::ConnectKnown { address } => Some(("bluetoothctl", vec!["connect", address])),
        Action::DisconnectKnown { address } => Some(("bluetoothctl", vec!["disconnect", address])),
    }
}

fn carry_out(action: &Action) -> Result<(), String> {
    // Refresh changes nothing on the machine: its later poll is the operation.
    let Some((program, args)) = action_command(action) else {
        return Ok(());
    };
    let answer = run_action(program, &args);

    match answer {
        Answer::Text(_) => Ok(()),
        Answer::Missing => Err("the tool this action needs is not installed".to_owned()),
        // The tool's own output is not repeated: it is another program's text
        // and this reason crosses the protocol.
        Answer::Unreadable => Err("the tool refused the request or did not finish".to_owned()),
    }
}

fn run_action(program: &str, args: &[&str]) -> Answer {
    match STOPPING.get() {
        Some(flag) => probe_bounded_with_cancel(program, args, ACTION_TIMEOUT, Some(flag)),
        None => probe_bounded_with_cancel(program, args, ACTION_TIMEOUT, None),
    }
}

/// Records a settled request for the writer to report.
fn report(runtime: &Mutex<ProviderRuntime>, settled: &Settled) {
    let outcome = if settled.ended.is_confirmed() {
        Outcome::confirmed(settled.id.clone())
    } else {
        Outcome::failed(
            settled.id.clone(),
            settled
                .ended
                .reason()
                .unwrap_or("the request did not take effect"),
        )
    };
    lock_runtime(runtime).settle(outcome);
}

/// Serves one connectivity verb.
///
/// The order is the contract: validate the identity against the last confirmed
/// inventory, record what the request is waiting to see, and only then run
/// anything. A request that cannot be recorded is never carried out, so nothing
/// changes the machine without something waiting to check that it did.
pub fn action(
    provider: &ProviderId,
    verb: &str,
    options: &Payload,
    request_id: &str,
    runtime: &Mutex<ProviderRuntime>,
) -> Result<(), String> {
    let mut state = lock_state();
    let parsed = if provider.as_str() == NETWORK {
        connectivity::read_network_action(verb, options, &state.networks)
    } else {
        connectivity::read_bluetooth_action(verb, options, &state.devices)
    }?;

    // Two refreshes cannot be in flight: the second would be answered by the
    // same observation as the first, which is not an answer to it.
    if parsed == Action::Refresh
        && state
            .waiting
            .awaits_matching(provider, |expected| *expected == Expected::Observation)
    {
        return Err("this provider is already waiting on a request".to_owned());
    }

    let superseded = state
        .waiting
        .reserve_matching(
            Awaiting {
                id: request_id.to_owned(),
                provider: provider.clone(),
                expected: parsed.expects(),
                // Replaced when the worker arms this reservation after
                // writing `accepted`; time spent in the tool is not charged
                // against the confirmation window.
                deadline_ms: 0,
            },
            connectivity::same_target,
        )
        .map_err(|refused| refused.reason().to_owned())?;
    drop(state);

    if let Some(superseded) = superseded {
        report(runtime, &superseded);
    }
    if let Err(reason) = carry_out(&parsed) {
        // The tool never ran, or refused. Nothing will ever confirm this, and
        // the worker is about to report the failure against this same id — so
        // the entry is dropped rather than answering twice when it expires.
        lock_state().waiting.forget(provider, request_id);
        return Err(reason);
    }
    Ok(())
}

/// Makes a successfully carried request observable after `accepted` was sent.
pub fn arm(provider: &ProviderId, request_id: &str) {
    if provider.as_str() != NETWORK && provider.as_str() != BLUETOOTH {
        return;
    }
    let mut state = lock_state();
    let deadline_ms = now_ms().saturating_add(connectivity::CONFIRMATION_WINDOW_MS);
    if !state.waiting.arm(provider, request_id, deadline_ms) {
        return;
    }
    state.look_again = true;
    drop(state);
    shared().woken.notify_all();
}

/// Removes a reservation whose `accepted` frame could not be written.
pub fn discard(provider: &ProviderId, request_id: &str) {
    if provider.as_str() == NETWORK || provider.as_str() == BLUETOOTH {
        let _ = lock_state().waiting.forget(provider, request_id);
    }
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

/// The saved Wi-Fi networks, annotated with the scan results the session
/// already had.
///
/// `--rescan no` is load-bearing: it reports what `NetworkManager` last saw and
/// starts no scan of its own, so a poll every five seconds neither drives the
/// radio nor interrupts a connection. Nothing here is a write.
///
/// `IN-USE` is asked for because it is the one field that lets a saved profile
/// be related to a real SSID in a single bounded run — `connection show` has no
/// SSID field at all, and asking per profile would be one process per row.
fn observe_known_networks() -> inventory::Reading<network::KnownNetwork> {
    let saved = probe_bounded(
        "nmcli",
        &["-t", "-f", "UUID,NAME,TYPE,DEVICE", "connection", "show"],
    );
    let visible = probe_bounded(
        "nmcli",
        &[
            "-t",
            "-f",
            "IN-USE,SSID,SIGNAL",
            "device",
            "wifi",
            "list",
            "--rescan",
            "no",
        ],
    );

    network::read_known_networks(&saved, &visible)
}

/// One poll of the Bluetooth listings.
///
/// `bluetoothctl show` runs once, and each device listing runs at most once —
/// so the summary and the inventory describe the same devices rather than two
/// answers taken moments apart. Read-only: no discovery, pairing, trust,
/// connect or disconnect is started here.
fn observe_bluetooth() -> Option<bluetooth::Observation> {
    let show = probe_bounded("bluetoothctl", &["show"]);
    // An adapter that is off has no device listing worth spawning a command
    // for. The domain still owns what that means; this only avoids the spawn.
    let powered = show
        .text()
        .and_then(bluetooth::parse_powered)
        .unwrap_or(false);
    let (paired, connected) = if powered {
        (
            probe_bounded("bluetoothctl", &["devices", "Paired"]),
            probe_bounded("bluetoothctl", &["devices", "Connected"]),
        )
    } else {
        (Answer::Unreadable, Answer::Unreadable)
    };

    bluetooth::observe(&show, &paired, &connected)
}

fn publish_network(
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    tracker: &mut network::LinkTracker,
    known: &mut inventory::Held<network::KnownNetwork>,
) {
    // The inventory is read whether or not there is a link: a session with no
    // default route still has saved networks, and that is precisely when a
    // person wants to see them.
    let observed = observe_network();
    let networks = known.observe(observe_known_networks());
    // The last confirmed link outlives any number of probes that failed to read
    // it, and is retired only by a poll that positively saw no default route.
    // Not being able to look is not the same as looking and finding nothing.
    let link = tracker.observe(observed);

    // The rows this poll confirmed become the only identities a request may
    // name, and the evidence every waiting request is judged against.
    let settled = {
        let mut state = lock_state();
        state.networks = networks.rows().unwrap_or_default().to_vec();
        let rows = state.networks.clone();
        state
            .waiting
            .settle(id, |expected| connectivity::judge_network(expected, &rows))
    };
    for one in &settled {
        report(runtime, one);
    }

    let Some(payload) = network::payload(link, networks) else {
        // No link, and no inventory either. Nothing here is true enough to
        // publish, and the widget says so by leaving.
        lock_runtime(runtime).withdraw(id);
        return;
    };

    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: network: {error}");
    }
}

fn publish_bluetooth(
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
    known: &mut inventory::Held<bluetooth::KnownDevice>,
) {
    let Some(observation) = observe_bluetooth() else {
        // Nobody could read the adapter, or it is on and its connected listing
        // did not answer. The widget leaves rather than claiming a state it
        // does not have.
        lock_runtime(runtime).withdraw(id);
        return;
    };

    let devices = known.observe(observation.devices.clone());

    let settled = {
        let mut state = lock_state();
        state.devices = devices.rows().unwrap_or_default().to_vec();
        let rows = state.devices.clone();
        state.waiting.settle(id, |expected| {
            connectivity::judge_bluetooth(expected, observation.adapter, &rows)
        })
    };
    for one in &settled {
        report(runtime, one);
    }

    if let Err(error) = lock_runtime(runtime).publish(id, bluetooth::payload(&observation, devices))
    {
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
    // One holder each, for the same reason: a poll that could not read a list
    // publishes the last one that was read rather than an empty one.
    let mut networks = inventory::Held::new();
    let mut devices = inventory::Held::new();

    while !stopping() {
        publish_network(runtime, net, &mut link, &mut networks);
        publish_bluetooth(runtime, bt, &mut devices);
        publish_power(runtime, profile);

        // A request that nothing confirmed or contradicted still gets an
        // answer, so a menu entry never stays pending for ever.
        let expired = lock_state().waiting.expire(now_ms());
        for one in &expired {
            report(runtime, one);
        }

        wait_for_next_poll();
    }

    // Whatever was still waiting is answered rather than dying silently with
    // the process. A new helper has run none of these, so none of them may
    // survive it either.
    let cancelled = lock_state().waiting.cancel_all();
    for one in &cancelled {
        report(runtime, one);
    }
}

/// Sleeps until the next poll, or until a `refresh` asks for one sooner.
///
/// The interval is unchanged: this waits the same five seconds and is woken
/// early only by an explicit request. One thread owns the wait, so two
/// refreshes cannot produce two polls — the second is coalesced into the one
/// the first is already about to run.
fn wait_for_next_poll() {
    let shared = shared();
    let mut state = lock_state();
    if std::mem::take(&mut state.look_again) {
        // Something asked while this poll was running. Go round again now
        // rather than waiting out an interval that has already been overtaken.
        return;
    }

    let (mut state, _timeout) = shared
        .woken
        .wait_timeout(state, INTERVAL)
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.look_again = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    use celestina_shell_core::connectivity::{ACTIVATE_SAVED, ID_OPTION, REFRESH};

    /// The connectivity state is process-wide, so these run one at a time.
    static SESSION_TEST: Mutex<()> = Mutex::new(());

    fn fresh() -> std::sync::MutexGuard<'static, ()> {
        let guard = SESSION_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = lock_state();
        *state = Connectivity::default();
        guard
    }

    fn runtime_with(providers: &[&str]) -> Mutex<ProviderRuntime> {
        let mut runtime = ProviderRuntime::new(1);
        for name in providers {
            if let Ok(id) = ProviderId::new(name) {
                runtime.register(id);
            }
        }
        Mutex::new(runtime)
    }

    fn network_id() -> ProviderId {
        ProviderId::new(NETWORK).expect("a valid provider name")
    }

    fn options(pairs: &[(&str, Value)]) -> Payload {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }

    /// `refresh` changes nothing on the machine, so it is the one verb that
    /// can be exercised end to end without touching connectivity.
    #[test]
    fn a_refresh_asks_the_poll_to_look_again_and_waits_for_it() {
        let _serialized = fresh();
        let runtime = runtime_with(&[NETWORK]);

        assert_eq!(
            action(&network_id(), REFRESH, &Payload::new(), "1", &runtime),
            Ok(())
        );
        let state = lock_state();
        // It is reserved but cannot be observed before the command worker has
        // actually written `accepted`.
        assert!(!state.look_again);
        assert!(state.waiting.awaits(&network_id()));
        assert_eq!(state.waiting.len(), 1);
        drop(state);

        // Nothing has been confirmed yet: acceptance is not arrival.
        assert!(lock_runtime(&runtime).take_outcomes().is_empty());
        arm(&network_id(), "1");
        assert!(lock_state().look_again);
    }

    /// Two refreshes cannot overlap. The second would be answered by the same
    /// observation as the first, which is not an answer to it.
    #[test]
    fn a_second_refresh_is_refused_while_the_first_is_still_waiting() {
        let _serialized = fresh();
        let runtime = runtime_with(&[NETWORK]);

        action(&network_id(), REFRESH, &Payload::new(), "1", &runtime).expect("accepted");
        let refusal = action(&network_id(), REFRESH, &Payload::new(), "2", &runtime)
            .expect_err("the second is refused");

        assert!(refusal.contains("already waiting"));
        assert_eq!(lock_state().waiting.len(), 1);
    }

    /// The observation settles it, and only then.
    #[test]
    fn the_next_observation_confirms_a_waiting_refresh() {
        let _serialized = fresh();
        let runtime = runtime_with(&[NETWORK]);
        action(&network_id(), REFRESH, &Payload::new(), "1", &runtime).expect("accepted");
        arm(&network_id(), "1");

        let settled = lock_state().waiting.settle(&network_id(), |expected| {
            connectivity::judge_network(expected, &[])
        });
        for one in &settled {
            report(&runtime, one);
        }

        let outcomes = lock_runtime(&runtime).take_outcomes();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].id, "1");
        assert!(outcomes[0].confirmed);
        assert!(lock_state().waiting.is_empty());
    }

    /// An identity that is not in the last confirmed inventory is refused
    /// before any program is chosen, so nothing is recorded and nothing runs.
    #[test]
    fn an_unknown_identity_is_refused_without_running_anything() {
        let _serialized = fresh();
        let runtime = runtime_with(&[NETWORK]);

        let refusal = action(
            &network_id(),
            ACTIVATE_SAVED,
            &options(&[(ID_OPTION, Value::from("9f1c-9"))]),
            "1",
            &runtime,
        )
        .expect_err("refused");

        assert!(refusal.contains("last confirmed inventory"));
        // Nothing is waiting, so nothing will be confirmed or expire later.
        assert!(lock_state().waiting.is_empty());
        assert!(!lock_state().look_again);
    }

    /// A helper that stops answers everything it was holding, and a new one
    /// inherits none of it.
    #[test]
    fn shutdown_answers_every_request_still_waiting() {
        let _serialized = fresh();
        let runtime = runtime_with(&[NETWORK]);
        action(&network_id(), REFRESH, &Payload::new(), "1", &runtime).expect("accepted");
        arm(&network_id(), "1");

        let cancelled = lock_state().waiting.cancel_all();
        for one in &cancelled {
            report(&runtime, one);
        }

        let outcomes = lock_runtime(&runtime).take_outcomes();
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].confirmed);
        assert_eq!(
            outcomes[0].reason.as_deref(),
            Some("the helper stopped before the request took effect")
        );
        assert!(lock_state().waiting.is_empty());
    }

    /// A request nothing ever confirms still gets an answer.
    #[test]
    fn a_request_that_never_takes_effect_expires_with_a_failure() {
        let _serialized = fresh();
        let runtime = runtime_with(&[NETWORK]);
        action(&network_id(), REFRESH, &Payload::new(), "1", &runtime).expect("accepted");
        arm(&network_id(), "1");

        let expired = lock_state()
            .waiting
            .expire(now_ms().saturating_add(connectivity::CONFIRMATION_WINDOW_MS + 1));
        for one in &expired {
            report(&runtime, one);
        }

        let outcomes = lock_runtime(&runtime).take_outcomes();
        assert_eq!(outcomes.len(), 1);
        assert!(!outcomes[0].confirmed);
        assert_eq!(
            outcomes[0].reason.as_deref(),
            Some("the request was accepted but never took effect")
        );
    }

    #[test]
    fn every_action_has_one_exact_argument_vector() {
        let uuid = "9f1c-1".to_owned();
        let address = "AA:BB:CC:DD:EE:01".to_owned();

        assert_eq!(action_command(&Action::Refresh), None);
        assert_eq!(
            action_command(&Action::ActivateSaved { uuid }),
            Some(("nmcli", vec!["connection", "up", "uuid", "9f1c-1"]))
        );
        assert_eq!(
            action_command(&Action::SetPowered(false)),
            Some(("bluetoothctl", vec!["power", "off"]))
        );
        assert_eq!(
            action_command(&Action::ConnectKnown {
                address: address.clone(),
            }),
            Some(("bluetoothctl", vec!["connect", "AA:BB:CC:DD:EE:01"]))
        );
        assert_eq!(
            action_command(&Action::DisconnectKnown { address }),
            Some(("bluetoothctl", vec!["disconnect", "AA:BB:CC:DD:EE:01"]))
        );
    }
}
