//! The power worker (CEL-2-F, D8, D10): UPower's display device and
//! power-profiles-daemon, read every five seconds on the system bus and
//! published only when the value changed, so rest costs no frames.
//!
//! One named thread behind the [`Worker`] guard owns one blocking `zbus`
//! connection whose every method call gives up after [`CALL_TIMEOUT`]. It
//! takes [`PowerCommand`]s from a queue of [`COMMAND_QUEUE`] and publishes
//! [`PowerEvent`]s through a sink, the same shape as the wallpaper worker.
//!
//! - Reading is one `Properties.GetAll` per service per poll, sent with
//!   `NoAutoStart`: the worker never asks the bus to start UPower or
//!   power-profiles-daemon (a disabled daemon stays down; waking one is a
//!   side effect, D2/D9). power-profiles-daemon is looked for under
//!   `org.freedesktop.UPower.PowerProfiles` first and `net.hadess.PowerProfiles`
//!   second; the name that answered is kept until it stops answering.
//! - Writing is exactly one call: `org.freedesktop.DBus.Properties.Set` on
//!   `ActiveProfile`, and only after [`validate`] found the profile in the
//!   daemon's own `Profiles` list (a refused request makes no call at all).
//!   The result is the next observation (D5): the worker re-reads at once
//!   and the pending window of [`PendingSet`] ends in a confirmation or a
//!   typed failure.
//! - A service that is absent (no bus, name unowned, first read failed) is
//!   published `Unavailable` once and retried with a backoff of 5 s doubling
//!   to 60 s (D10). A service that appears later is picked up by the retry.
//!   The connection is dropped, and opened again by the next attempt, only
//!   when a call finds the connection itself gone (`PowerError::Lost`: an
//!   I/O error, which includes a call that outlived [`CALL_TIMEOUT`], or a
//!   disconnect), so a restarted bus is recovered. A reply from the bus or a
//!   daemon proves the link alive, whatever it says: `ServiceUnknown` or
//!   `NameHasNoOwner` for an absent UPower, or UPower reporting no battery,
//!   never drop it. A profile change that finds no link opens one and reads
//!   the daemon's name before its one `Set` call.
//! - Everything from the bus is bounded: the types are the shell's bounded
//!   sections, profile names over 32 bytes are dropped, the list is scanned
//!   to 1024 entries, and no error text reaches the journal: a log line
//!   carries a fixed kind token (`PowerError::kind`).
//! - Every wait is at most [`TICK`], so the shutdown flag is seen at that
//!   interval; the longest a call can hold the worker is [`CALL_TIMEOUT`].
//!
//! `PowerEvent` is wrapped by the caller into the loop's message, for
//! example `|event| sender.send(Message::Connectivity(ConnectivityEvent::Power(event))).is_ok()`.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::connectivity::{
    validate, Action, BatterySection, ChargeState, Checked, Degraded, Expected, Failure, Inventory,
    PendingSet, ProfileName, ProfileSection, Refusal, Resolution, Section, Service, Step,
    MAX_PROFILE_BYTES, MAX_SCANNED_ROWS,
};
use zbus::blocking::{connection::Builder, Connection, Proxy};
use zbus::names::InterfaceName;
use zbus::proxy::MethodFlags;
use zbus::zvariant::{OwnedValue, Value};

use crate::providers::Worker;

/// How often a present service is read.
pub const POLL: Duration = Duration::from_secs(5);
/// The first wait after a service was found absent.
pub const RETRY_START: Duration = Duration::from_secs(5);
/// The longest wait between attempts at an absent service.
pub const RETRY_MAX: Duration = Duration::from_secs(60);
/// The longest one method call may hold the worker.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(3);
/// The longest the worker sleeps without looking at the shutdown flag.
pub const TICK: Duration = Duration::from_millis(200);
/// Commands that may wait for the worker; more are refused.
pub const COMMAND_QUEUE: usize = 4;
/// The longest error text kept for the journal.
const MAX_LOG_CHARS: usize = 160;

const UPOWER_NAME: &str = "org.freedesktop.UPower";
const UPOWER_DISPLAY_PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const UPOWER_DEVICE_INTERFACE: &str = "org.freedesktop.UPower.Device";

/// Where power-profiles-daemon may be: bus name (also its interface) and
/// object path, in the order they are tried.
const PPD_TARGETS: [(&str, &str); 2] = [
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
    ),
    ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
];

/// What the worker may be asked, the power subset of
/// [`celestina_shell::connectivity::Action`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PowerCommand {
    /// Read a section now (battery or profiles).
    Refresh(Section),
    /// Ask the daemon for this listed profile.
    SetProfile(ProfileName),
    /// Ask the daemon for the next listed profile.
    CycleProfile,
}

impl PowerCommand {
    /// The power command of `action`, or `None` for an action this worker
    /// does not serve (network, Bluetooth, editors).
    #[must_use]
    pub fn from_action(action: &Action) -> Option<Self> {
        match action {
            Action::Refresh(section @ (Section::Battery | Section::Profiles)) => {
                Some(Self::Refresh(*section))
            }
            Action::SetProfile(name) => Some(Self::SetProfile(name.clone())),
            Action::CycleProfile => Some(Self::CycleProfile),
            _ => None,
        }
    }

    fn action(&self) -> Action {
        match self {
            Self::Refresh(section) => Action::Refresh(*section),
            Self::SetProfile(name) => Action::SetProfile(name.clone()),
            Self::CycleProfile => Action::CycleProfile,
        }
    }
}

/// What the worker publishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PowerEvent {
    /// The display device changed, or was found absent (`Unavailable` also
    /// when UPower runs but reports no battery).
    Battery(Service<BatterySection>),
    /// The profile daemon's state changed, or it was found absent.
    Profiles(Service<ProfileSection>),
    /// A request ended: confirmed by an observation, or failed by kind.
    Resolved(Resolution),
    /// A request was refused before any call; the worker made no call.
    Refused(Refusal),
}

/// Which bus the worker reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bus {
    /// The system bus (`DBUS_SYSTEM_BUS_ADDRESS` or the default socket).
    System,
    /// A bus at this address (tests, on a private daemon).
    Address(String),
}

/// The intervals of the worker, so a test need not wait seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    /// Between reads of a present service.
    pub poll: Duration,
    /// First wait after a service was found absent.
    pub retry_start: Duration,
    /// Cap of the doubling wait.
    pub retry_max: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            poll: POLL,
            retry_start: RETRY_START,
            retry_max: RETRY_MAX,
        }
    }
}

/// The loop's handle on the worker: the bounded command queue.
#[derive(Debug, Clone)]
pub struct PowerProvider {
    commands: SyncSender<PowerCommand>,
}

impl PowerProvider {
    /// Queues a command for the worker.
    ///
    /// # Errors
    ///
    /// The command back, when [`COMMAND_QUEUE`] commands already wait or the
    /// worker has stopped.
    pub fn request(&self, command: PowerCommand) -> Result<(), PowerCommand> {
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(refused) | TrySendError::Disconnected(refused)) => Err(refused),
        }
    }
}

/// Starts the worker on the system bus. `publish` returns whether the event
/// was accepted (false ends the worker).
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    shutdown: &Arc<AtomicBool>,
    publish: impl FnMut(PowerEvent) -> bool + Send + 'static,
) -> io::Result<(PowerProvider, Worker)> {
    spawn_with(Bus::System, Timing::default(), shutdown, publish)
}

/// [`spawn`] with its bus and intervals given.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn_with(
    bus: Bus,
    timing: Timing,
    shutdown: &Arc<AtomicBool>,
    publish: impl FnMut(PowerEvent) -> bool + Send + 'static,
) -> io::Result<(PowerProvider, Worker)> {
    let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("power", shutdown, move || {
        run(&bus, timing, &queue, &flag, publish);
    })?;
    Ok((PowerProvider { commands }, worker))
}

// ---------------------------------------------------------------------------
// Publishing on change, with the backoff of an absent service
// ---------------------------------------------------------------------------

/// One service's published state and its schedule. Pure: the caller passes
/// the clock.
#[derive(Debug)]
struct Watch<T> {
    published: Option<Service<T>>,
    delay: Duration,
    next: Instant,
}

impl<T: Clone + PartialEq> Watch<T> {
    fn new(now: Instant, timing: Timing) -> Self {
        Self {
            published: None,
            delay: timing.retry_start,
            next: now,
        }
    }

    fn due(&self, now: Instant) -> bool {
        now >= self.next
    }

    /// Reads again at the next look.
    fn refresh(&mut self, now: Instant) {
        self.next = now;
    }

    /// Takes one read (`None`: the service did not answer) and schedules the
    /// next. Returns the state to publish when it differs from the last one
    /// published; the first state is always published, an unchanged one
    /// never.
    fn settle(&mut self, now: Instant, read: Option<T>, timing: Timing) -> Option<Service<T>> {
        let state = match read {
            Some(value) => {
                self.delay = timing.retry_start;
                self.next = now + timing.poll;
                Service::Ready(value)
            }
            None => {
                self.next = now + self.delay;
                self.delay = (self.delay * 2).min(timing.retry_max);
                Service::Unavailable
            }
        };
        if self.published.as_ref() == Some(&state) {
            None
        } else {
            self.published = Some(state.clone());
            Some(state)
        }
    }

    fn ready(&self) -> Option<&T> {
        self.published.as_ref().and_then(Service::ready)
    }
}

// ---------------------------------------------------------------------------
// The bus
// ---------------------------------------------------------------------------

/// Why one bus operation failed; detail goes to the journal, the kind to the
/// page.
#[derive(Debug, thiserror::Error)]
enum PowerError {
    #[error("the bus is not reachable: {0}")]
    Connect(String),
    #[error("{0}")]
    Call(String),
    #[error("{what} is missing or has an unexpected type")]
    Shape { what: &'static str },
    #[error("the service is not there")]
    Absent,
    #[error("the bus connection is gone")]
    Lost,
    #[error("the daemon denied the change")]
    Denied,
}

impl PowerError {
    /// A fixed token for the journal. The error's own text can carry what the
    /// bus or the daemon said, so a log line never holds it.
    const fn kind(&self) -> &'static str {
        match self {
            Self::Connect(_) => "connect",
            Self::Call(_) => "call",
            Self::Shape { .. } => "shape",
            Self::Absent => "absent",
            Self::Lost => "lost",
            Self::Denied => "denied",
        }
    }
}

/// A text from the bus cut for a stored error's message: control characters
/// dropped, at most [`MAX_LOG_CHARS`] characters. It is held for the
/// error's `Display` only; no log line prints it (see [`PowerError::kind`]).
fn clip(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_LOG_CHARS)
        .collect()
}

fn call_error(error: &zbus::Error) -> PowerError {
    use zbus::Error as E;
    let name = match error {
        E::MethodError(name, _, _) => Some(name.as_str().to_owned()),
        E::FDO(fdo) => match fdo.as_ref() {
            zbus::fdo::Error::ServiceUnknown(_)
            | zbus::fdo::Error::NameHasNoOwner(_)
            | zbus::fdo::Error::NoReply(_) => return PowerError::Absent,
            zbus::fdo::Error::Disconnected(_) => return PowerError::Lost,
            zbus::fdo::Error::AccessDenied(_) | zbus::fdo::Error::AuthFailed(_) => {
                return PowerError::Denied
            }
            _ => None,
        },
        E::InputOutput(_) => return PowerError::Lost,
        _ => None,
    };
    match name.as_deref() {
        Some(
            "org.freedesktop.DBus.Error.ServiceUnknown"
            | "org.freedesktop.DBus.Error.NameHasNoOwner"
            | "org.freedesktop.DBus.Error.NoReply",
        ) => PowerError::Absent,
        Some("org.freedesktop.DBus.Error.Disconnected") => PowerError::Lost,
        Some(
            "org.freedesktop.DBus.Error.AccessDenied" | "org.freedesktop.DBus.Error.AuthFailed",
        ) => PowerError::Denied,
        _ => PowerError::Call(clip(&error.to_string())),
    }
}

fn connect(bus: &Bus) -> Result<Connection, PowerError> {
    let builder = match bus {
        Bus::System => Builder::system(),
        Bus::Address(address) => Builder::address(address.as_str()),
    };
    builder
        .map(|builder| builder.method_timeout(CALL_TIMEOUT))
        .and_then(Builder::build)
        .map_err(|error| PowerError::Connect(clip(&error.to_string())))
}

type Properties = HashMap<String, OwnedValue>;

const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";

// Every call the worker makes carries `MethodFlags::NoAutoStart`: it never
// asks the bus to start a service (D-Bus activation). The worker only
// observes; waking power-profiles-daemon or UPower from a disabled or absent
// state would be a side effect of merely running the shell (D2, D9). The
// test `reading_never_activates_an_activatable_absent_service` pins it.

fn properties_proxy(
    connection: &Connection,
    name: &'static str,
    path: &'static str,
) -> Result<Proxy<'static>, PowerError> {
    Proxy::new(connection, name, path, PROPERTIES_INTERFACE).map_err(|error| call_error(&error))
}

fn get_all(
    connection: &Connection,
    name: &'static str,
    path: &'static str,
    interface: &'static str,
) -> Result<Properties, PowerError> {
    let proxy = properties_proxy(connection, name, path)?;
    let interface = InterfaceName::try_from(interface)
        .map_err(|error| PowerError::Call(clip(&error.to_string())))?;
    match proxy.call_with_flags::<_, _, Properties>(
        "GetAll",
        MethodFlags::NoAutoStart.into(),
        &(interface,),
    ) {
        Ok(Some(properties)) => Ok(properties),
        Ok(None) => Err(PowerError::Shape { what: "GetAll" }),
        Err(error) => Err(call_error(&error)),
    }
}

/// A property of type `T`; `None` when missing or of another type.
fn field<T: TryFrom<OwnedValue>>(properties: &Properties, key: &str) -> Option<T> {
    properties
        .get(key)
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| T::try_from(value).ok())
}

/// The display device: `Ok(None)` when UPower runs and reports no battery.
fn read_battery(connection: &Connection) -> Result<Option<BatterySection>, PowerError> {
    let properties = get_all(
        connection,
        UPOWER_NAME,
        UPOWER_DISPLAY_PATH,
        UPOWER_DEVICE_INTERFACE,
    )?;
    let present =
        field::<bool>(&properties, "IsPresent").ok_or(PowerError::Shape { what: "IsPresent" })?;
    if !present {
        return Ok(None);
    }
    let percentage =
        field::<f64>(&properties, "Percentage").ok_or(PowerError::Shape { what: "Percentage" })?;
    // A missing State or time reads as unknown, which the section models.
    let state = field::<u32>(&properties, "State").unwrap_or(0);
    let to_empty = field::<i64>(&properties, "TimeToEmpty").unwrap_or(0);
    let to_full = field::<i64>(&properties, "TimeToFull").unwrap_or(0);
    Ok(BatterySection::new(
        true,
        ChargeState::from_upower(state),
        percentage,
        to_empty,
        to_full,
    ))
}

/// The daemon at the target `index` of [`PPD_TARGETS`].
fn read_profiles_at(connection: &Connection, index: usize) -> Result<ProfileSection, PowerError> {
    let (name, path) = PPD_TARGETS.get(index).copied().ok_or(PowerError::Absent)?;
    let properties = get_all(connection, name, path, name)?;
    let active = field::<String>(&properties, "ActiveProfile").ok_or(PowerError::Shape {
        what: "ActiveProfile",
    })?;
    let listed = field::<Vec<HashMap<String, OwnedValue>>>(&properties, "Profiles")
        .ok_or(PowerError::Shape { what: "Profiles" })?;
    let degraded = field::<String>(&properties, "PerformanceDegraded").unwrap_or_default();
    // Names longer than the grammar allows are not worth a copy: dropped
    // here, the section drops the rest of the invalid ones.
    let names: Vec<String> = listed
        .iter()
        .take(MAX_SCANNED_ROWS)
        .filter_map(|entry| field::<String>(entry, "Profile"))
        .filter(|name| name.len() <= MAX_PROFILE_BYTES)
        .collect();
    let active = if active.len() <= MAX_PROFILE_BYTES {
        active
    } else {
        String::new()
    };
    Ok(ProfileSection::new(
        &active,
        names,
        Degraded::from_daemon(&degraded),
    ))
}

/// The live connection and the profile daemon name that answered last.
struct Link {
    connection: Connection,
    ppd: Option<usize>,
}

impl Link {
    fn read_profiles(&mut self) -> Result<ProfileSection, PowerError> {
        let all = [0, 1];
        let remembered = self.ppd.map(|index| [index]);
        let candidates: &[usize] = match &remembered {
            Some(one) => one,
            None => &all,
        };
        let mut last = PowerError::Absent;
        for &index in candidates {
            match read_profiles_at(&self.connection, index) {
                Ok(section) => {
                    self.ppd = Some(index);
                    return Ok(section);
                }
                Err(error) => last = error,
            }
        }
        self.ppd = None;
        Err(last)
    }

    /// `Properties.Set(<daemon interface>, "ActiveProfile", profile)` on the
    /// daemon that answered the last read; the one call the worker writes.
    fn set_profile(&self, profile: &ProfileName) -> Result<(), PowerError> {
        let (name, path) = self
            .ppd
            .and_then(|index| PPD_TARGETS.get(index).copied())
            .ok_or(PowerError::Absent)?;
        let proxy = properties_proxy(&self.connection, name, path)?;
        let interface = InterfaceName::try_from(name)
            .map_err(|error| PowerError::Call(clip(&error.to_string())))?;
        let value = Value::from(profile.as_str());
        proxy
            .call_with_flags::<_, _, ()>(
                "Set",
                MethodFlags::NoAutoStart.into(),
                &(interface, "ActiveProfile", value),
            )
            .map(drop)
            .map_err(|error| call_error(&error))
    }
}

fn failure_of(error: &PowerError) -> Failure {
    match error {
        PowerError::Denied => Failure::Denied,
        PowerError::Absent | PowerError::Lost | PowerError::Connect(_) => Failure::Unreachable,
        PowerError::Call(_) | PowerError::Shape { .. } => Failure::Other,
    }
}

// ---------------------------------------------------------------------------
// The worker
// ---------------------------------------------------------------------------

struct State {
    bus: Bus,
    timing: Timing,
    link: Option<Link>,
    battery: Watch<BatterySection>,
    profiles: Watch<ProfileSection>,
    pending: PendingSet,
}

fn run(
    bus: &Bus,
    timing: Timing,
    queue: &Receiver<PowerCommand>,
    shutdown: &AtomicBool,
    mut publish: impl FnMut(PowerEvent) -> bool,
) {
    let now = Instant::now();
    let mut state = State {
        bus: bus.clone(),
        timing,
        link: None,
        battery: Watch::new(now, timing),
        profiles: Watch::new(now, timing),
        pending: PendingSet::new(),
    };
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let mut events = Vec::new();
        match queue.recv_timeout(TICK) {
            Ok(first) => {
                let batch: Vec<PowerCommand> = std::iter::once(first)
                    .chain(queue.try_iter().take(COMMAND_QUEUE))
                    .collect();
                for command in batch {
                    if shutdown.load(Ordering::Acquire) {
                        return;
                    }
                    // The poll comes before every command, not once per
                    // batch: a command meets the freshest inventory the
                    // worker can have, and a profile change just made has
                    // been re-read (it asked for a read at once), so two
                    // quick cycles advance two steps instead of asking twice
                    // for the same next profile.
                    poll(&mut state, &mut events);
                    handle(&mut state, &command, &mut events);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        poll(&mut state, &mut events);
        expire(&mut state, &mut events);
        for event in events {
            if !publish(event) {
                tracing::debug!("power: the loop is gone; the worker stops");
                return;
            }
        }
    }
}

/// Reads whatever is due and queues the changes.
fn poll(state: &mut State, events: &mut Vec<PowerEvent>) {
    let now = Instant::now();
    let battery_due = state.battery.due(now);
    let profiles_due = state.profiles.due(now);
    if !battery_due && !profiles_due {
        return;
    }
    if state.link.is_none() {
        match connect(&state.bus) {
            Ok(connection) => {
                state.link = Some(Link {
                    connection,
                    ppd: None,
                });
            }
            Err(error) => tracing::debug!(kind = error.kind(), "power: no system bus"),
        }
    }
    // Every read that was due; the link is dropped only when a read finds
    // the connection itself gone. An absent service is a reply from the bus,
    // so it proves the link alive.
    let mut lost = false;
    if battery_due {
        let read = match state
            .link
            .as_ref()
            .map(|link| read_battery(&link.connection))
        {
            Some(Ok(section)) => section,
            Some(Err(error)) => {
                lost |= matches!(error, PowerError::Lost);
                tracing::debug!(kind = error.kind(), "power: UPower is not readable");
                None
            }
            None => None,
        };
        if let Some(state_to_publish) = state.battery.settle(now, read, state.timing) {
            events.push(PowerEvent::Battery(state_to_publish));
        }
    }
    if profiles_due {
        let read = match state.link.as_mut().map(Link::read_profiles) {
            Some(Ok(section)) => Some(section),
            Some(Err(error)) => {
                lost |= matches!(error, PowerError::Lost);
                tracing::debug!(
                    kind = error.kind(),
                    "power: power-profiles-daemon is not readable"
                );
                None
            }
            None => None,
        };
        let resolved = match &read {
            Some(section) => state.pending.observe(
                &Inventory {
                    profiles: Some(section),
                    ..Inventory::default()
                },
                now,
            ),
            None => unreachable_pending(&mut state.pending),
        };
        if let Some(state_to_publish) = state.profiles.settle(now, read, state.timing) {
            events.push(PowerEvent::Profiles(state_to_publish));
        }
        events.extend(resolved.into_iter().map(PowerEvent::Resolved));
    }
    if lost {
        // The connection is gone: reconnect from scratch at the next attempt
        // (the backoff of each `Watch` bounds how often).
        state.link = None;
    }
}

/// The daemon went away with a request outstanding: it fails as unreachable.
/// Only profile requests are pending in this worker and the pending set keys
/// them by subject, so any profile name stands for them.
fn unreachable_pending(pending: &mut PendingSet) -> Vec<Resolution> {
    ProfileName::new("balanced")
        .and_then(|name| pending.fail(&Expected::Profile(name), Failure::Unreachable))
        .into_iter()
        .collect()
}

/// Ends requests whose window passed even when no read is due.
fn expire(state: &mut State, events: &mut Vec<PowerEvent>) {
    let now = Instant::now();
    if state
        .pending
        .next_deadline()
        .is_none_or(|deadline| now < deadline)
    {
        return;
    }
    let inventory = Inventory {
        profiles: state.profiles.ready(),
        ..Inventory::default()
    };
    events.extend(
        state
            .pending
            .observe(&inventory, now)
            .into_iter()
            .map(PowerEvent::Resolved),
    );
}

/// The link with the profile daemon's name known, for a change: a missing
/// connection is opened and a forgotten name is read again first, so a change
/// never fails only because the link was dropped or the name not yet looked
/// up. The read here only finds the name; the inventory the change was
/// validated against is not replaced (the observation after the call is).
fn linked_daemon(state: &mut State) -> Result<&Link, PowerError> {
    if state.link.is_none() {
        state.link = Some(Link {
            connection: connect(&state.bus)?,
            ppd: None,
        });
    }
    let link = state.link.as_mut().ok_or(PowerError::Absent)?;
    if link.ppd.is_none() {
        if let Err(error) = link.read_profiles() {
            if matches!(error, PowerError::Lost) {
                state.link = None;
            }
            return Err(error);
        }
    }
    state.link.as_ref().ok_or(PowerError::Absent)
}

fn handle(state: &mut State, command: &PowerCommand, events: &mut Vec<PowerEvent>) {
    let now = Instant::now();
    let action = command.action();
    let checked = {
        let inventory = Inventory {
            profiles: state.profiles.ready(),
            ..Inventory::default()
        };
        validate(&action, &inventory)
    };
    let checked = match checked {
        Ok(checked) => checked,
        Err(refusal) => {
            tracing::debug!(reason = refusal.key(), "power: request refused");
            events.push(PowerEvent::Refused(refusal));
            return;
        }
    };
    apply(state, &checked, now, events);
}

fn apply(state: &mut State, checked: &Checked, now: Instant, events: &mut Vec<PowerEvent>) {
    match checked.step() {
        Step::Refresh(Section::Battery) => state.battery.refresh(now),
        Step::Refresh(Section::Profiles) => state.profiles.refresh(now),
        Step::SetProfile(profile) => {
            let Some(expected) = checked.expected() else {
                return;
            };
            state.pending.start(expected.clone(), now);
            let result = linked_daemon(state).and_then(|link| link.set_profile(profile));
            match result {
                // The observation after the call is the truth (D5).
                Ok(()) => state.profiles.refresh(now),
                Err(error) => {
                    tracing::debug!(kind = error.kind(), "power: the profile change failed");
                    if matches!(error, PowerError::Lost) {
                        state.link = None;
                    } else if let (PowerError::Absent, Some(link)) = (&error, state.link.as_mut()) {
                        // The daemon is gone from the bus: look its name up
                        // again at the next read.
                        link.ppd = None;
                    }
                    if let Some(resolution) = state.pending.fail(&expected, failure_of(&error)) {
                        events.push(PowerEvent::Resolved(resolution));
                    }
                }
            }
        }
        // `validate` returns only the steps of the actions this worker was
        // given; any other step is not its to perform.
        Step::SetWifiEnabled(_)
        | Step::Join(_)
        | Step::Leave(_)
        | Step::SetBluetoothPowered(_)
        | Step::ConnectDevice(_)
        | Step::DisconnectDevice(_)
        | Step::OpenEditor(_)
        | Step::Refresh(Section::Network | Section::Bluetooth) => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_fake_system::{FakeError, FakeSystem, Ppd, Services};
    use std::sync::mpsc::Receiver;

    const FAST: Timing = Timing {
        poll: Duration::from_millis(40),
        retry_start: Duration::from_millis(40),
        retry_max: Duration::from_millis(160),
    };
    const WAIT: Duration = Duration::from_secs(5);
    const PPD_NAMES: [&str; 3] = celestina_fake_system::PPD_PROFILES;

    struct Running {
        provider: PowerProvider,
        worker: Worker,
        events: Receiver<PowerEvent>,
    }

    fn start_on(bus: Bus, timing: Timing) -> Running {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (tx, events) = mpsc::channel();
        let (provider, worker) =
            spawn_with(bus, timing, &shutdown, move |event| tx.send(event).is_ok())
                .expect("the thread");
        Running {
            provider,
            worker,
            events,
        }
    }

    fn fake(name: &str, services: Services) -> Option<FakeSystem> {
        match FakeSystem::start(name, services) {
            Ok(system) => Some(system),
            Err(FakeError::DaemonMissing) => {
                eprintln!("dbus-daemon is not installed: the power worker test is skipped");
                None
            }
            Err(error) => panic!("{error}"),
        }
    }

    fn next(running: &Running) -> PowerEvent {
        running.events.recv_timeout(WAIT).expect("an event")
    }

    /// The next event satisfying `wanted`, skipping others.
    fn until(running: &Running, wanted: impl Fn(&PowerEvent) -> bool) -> PowerEvent {
        loop {
            let event = next(running);
            if wanted(&event) {
                return event;
            }
        }
    }

    fn stop(running: Running) {
        assert!(running.worker.join_within(Duration::from_secs(2)));
    }

    fn profile(raw: &str) -> ProfileName {
        ProfileName::new(raw).expect("valid profile")
    }

    fn is_battery(event: &PowerEvent) -> bool {
        matches!(event, PowerEvent::Battery(_))
    }

    fn is_profiles(event: &PowerEvent) -> bool {
        matches!(event, PowerEvent::Profiles(_))
    }

    #[test]
    fn a_watch_publishes_the_first_state_and_only_changes_after() {
        let t0 = Instant::now();
        let mut watch: Watch<u8> = Watch::new(t0, FAST);
        assert_eq!(watch.settle(t0, Some(1), FAST), Some(Service::Ready(1)));
        assert_eq!(watch.settle(t0 + FAST.poll, Some(1), FAST), None);
        assert_eq!(
            watch.settle(t0 + FAST.poll * 2, Some(2), FAST),
            Some(Service::Ready(2))
        );
        assert!(!watch.due(t0 + FAST.poll * 2));
        assert!(watch.due(t0 + FAST.poll * 3));
    }

    #[test]
    fn an_absent_service_is_published_once_and_retried_with_doubling_backoff() {
        let t0 = Instant::now();
        let mut watch: Watch<u8> = Watch::new(t0, FAST);
        assert_eq!(watch.settle(t0, None, FAST), Some(Service::Unavailable));
        let mut waits = Vec::new();
        let mut at = t0;
        for _ in 0..5 {
            waits.push(watch.next - at);
            at = watch.next;
            assert_eq!(watch.settle(at, None, FAST), None, "no repeat");
        }
        assert_eq!(
            waits,
            [40, 80, 160, 160, 160].map(Duration::from_millis).to_vec()
        );
        // It appears: published, and the delay starts over.
        assert_eq!(watch.settle(at, Some(7), FAST), Some(Service::Ready(7)));
        assert_eq!(
            watch.settle(at + FAST.poll, None, FAST),
            Some(Service::Unavailable)
        );
        assert_eq!(watch.next - (at + FAST.poll), FAST.retry_start);
    }

    #[test]
    fn the_default_timing_is_the_documented_one() {
        let timing = Timing::default();
        assert_eq!(timing.poll, Duration::from_secs(5));
        assert_eq!(timing.retry_start, Duration::from_secs(5));
        assert_eq!(timing.retry_max, Duration::from_secs(60));
    }

    #[test]
    fn only_power_actions_become_commands() {
        assert_eq!(
            PowerCommand::from_action(&Action::CycleProfile),
            Some(PowerCommand::CycleProfile)
        );
        assert_eq!(
            PowerCommand::from_action(&Action::Refresh(Section::Battery)),
            Some(PowerCommand::Refresh(Section::Battery))
        );
        assert_eq!(
            PowerCommand::from_action(&Action::Refresh(Section::Network)),
            None
        );
        assert_eq!(
            PowerCommand::from_action(&Action::SetWifiEnabled(true)),
            None
        );
    }

    #[test]
    fn the_first_reads_publish_the_battery_and_the_profiles() {
        let Some(system) = fake("pw-reads", Services::all()) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        let PowerEvent::Battery(Service::Ready(battery)) = until(&running, is_battery) else {
            panic!("a battery");
        };
        assert_eq!(battery.percentage(), 64);
        assert_eq!(battery.state(), ChargeState::Discharging);
        assert_eq!(battery.time_to_empty(), Some(Duration::from_secs(7200)));
        assert_eq!(battery.time_to_full(), None);
        let PowerEvent::Profiles(Service::Ready(profiles)) = until(&running, is_profiles) else {
            panic!("profiles");
        };
        assert_eq!(profiles.active(), Some(&profile("balanced")));
        assert_eq!(
            profiles.profiles(),
            [
                profile("power-saver"),
                profile("balanced"),
                profile("performance")
            ]
        );
        assert_eq!(profiles.degraded(), Degraded::No);
        assert_eq!(
            system.calls().all(),
            Vec::<String>::new(),
            "reading makes no call"
        );
        stop(running);
    }

    #[test]
    fn an_unchanged_service_publishes_nothing_more_and_a_change_publishes_once() {
        let Some(system) = fake("pw-change", Services::all()) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        until(&running, is_battery);
        until(&running, is_profiles);
        // Several polls pass with nothing changed: no event at all.
        assert!(running.events.recv_timeout(FAST.poll * 6).is_err());
        system.set_battery(41.0, 1).expect("battery");
        let PowerEvent::Battery(Service::Ready(battery)) = next(&running) else {
            panic!("a battery change");
        };
        assert_eq!(
            (battery.percentage(), battery.state()),
            (41, ChargeState::Charging)
        );
        system.set_active_profile("performance").expect("profile");
        let PowerEvent::Profiles(Service::Ready(profiles)) = next(&running) else {
            panic!("a profile change");
        };
        assert_eq!(profiles.active(), Some(&profile("performance")));
        assert!(running.events.recv_timeout(FAST.poll * 6).is_err());
        stop(running);
    }

    #[test]
    fn setting_a_listed_profile_makes_one_set_call_and_is_confirmed_by_observation() {
        let Some(system) = fake("pw-set", Services::all()) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        until(&running, is_profiles);
        running
            .provider
            .request(PowerCommand::SetProfile(profile("performance")))
            .expect("queued");
        let resolved = until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        assert_eq!(
            resolved,
            PowerEvent::Resolved(Resolution::Confirmed(Expected::Profile(profile(
                "performance"
            ))))
        );
        assert_eq!(system.calls().all(), ["ppd:Set ActiveProfile=performance"]);
        stop(running);
    }

    #[test]
    fn cycling_asks_for_the_next_listed_profile() {
        let Some(system) = fake("pw-cycle", Services::all()) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        until(&running, is_profiles);
        running
            .provider
            .request(PowerCommand::CycleProfile)
            .expect("queued");
        until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        // balanced -> performance, then performance -> power-saver (wraps).
        running
            .provider
            .request(PowerCommand::CycleProfile)
            .expect("queued");
        until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        assert_eq!(
            system.calls().all(),
            [
                "ppd:Set ActiveProfile=performance",
                "ppd:Set ActiveProfile=power-saver"
            ]
        );
        stop(running);
    }

    #[test]
    fn a_profile_the_daemon_does_not_list_is_refused_before_any_call() {
        let Some(system) = fake("pw-refuse", Services::all()) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        until(&running, is_profiles);
        running
            .provider
            .request(PowerCommand::SetProfile(profile("turbo")))
            .expect("queued");
        assert_eq!(
            until(&running, |e| matches!(e, PowerEvent::Refused(_))),
            PowerEvent::Refused(Refusal::NoSuchProfile)
        );
        assert_eq!(system.calls().count("ppd"), 0, "no call was made");
        stop(running);
    }

    #[test]
    fn a_request_without_an_inventory_is_refused_and_makes_no_call() {
        // No daemon at all: nothing was ever read, so nothing is valid.
        let Some(system) = fake("pw-noinv", Services::none()) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        until(&running, is_profiles);
        running
            .provider
            .request(PowerCommand::SetProfile(profile("balanced")))
            .expect("queued");
        assert_eq!(
            until(&running, |e| matches!(e, PowerEvent::Refused(_))),
            PowerEvent::Refused(Refusal::NoInventory(Section::Profiles))
        );
        running
            .provider
            .request(PowerCommand::CycleProfile)
            .expect("queued");
        assert_eq!(
            until(&running, |e| matches!(e, PowerEvent::Refused(_))),
            PowerEvent::Refused(Refusal::NoInventory(Section::Profiles))
        );
        assert_eq!(system.calls().all(), Vec::<String>::new());
        stop(running);
    }

    #[test]
    fn the_hadess_name_is_the_second_choice() {
        let services = Services {
            ppd: Ppd::Hadess,
            ..Services::all()
        };
        let Some(system) = fake("pw-hadess", services) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        let PowerEvent::Profiles(Service::Ready(profiles)) = until(&running, is_profiles) else {
            panic!("profiles under the second name");
        };
        assert_eq!(profiles.active(), Some(&profile("balanced")));
        running
            .provider
            .request(PowerCommand::SetProfile(profile("power-saver")))
            .expect("queued");
        until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        assert_eq!(
            system.calls().all(),
            ["ppd-hadess:Set ActiveProfile=power-saver"]
        );
        stop(running);
    }

    #[test]
    fn the_upower_name_wins_when_both_are_served() {
        let services = Services {
            ppd: Ppd::Both,
            ..Services::all()
        };
        let Some(system) = fake("pw-both", services) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        until(&running, is_profiles);
        running
            .provider
            .request(PowerCommand::SetProfile(profile("performance")))
            .expect("queued");
        until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        assert_eq!(system.calls().all(), ["ppd:Set ActiveProfile=performance"]);
        stop(running);
    }

    #[test]
    fn absent_services_are_published_unavailable_once() {
        let Some(system) = fake("pw-absent", Services::none()) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        let first = until(&running, is_battery);
        assert_eq!(first, PowerEvent::Battery(Service::Unavailable));
        assert_eq!(
            until(&running, is_profiles),
            PowerEvent::Profiles(Service::Unavailable)
        );
        // Retries go on, silently.
        assert!(running.events.recv_timeout(FAST.retry_max * 4).is_err());
        stop(running);
    }

    #[test]
    fn a_missing_bus_is_unavailable_for_both_and_does_not_panic() {
        let missing = Bus::Address("unix:path=/nonexistent/celestina-test/bus".to_owned());
        let running = start_on(missing, FAST);
        assert_eq!(next(&running), PowerEvent::Battery(Service::Unavailable));
        assert_eq!(next(&running), PowerEvent::Profiles(Service::Unavailable));
        assert!(running.events.recv_timeout(FAST.retry_max * 3).is_err());
        stop(running);
    }

    #[test]
    fn a_missing_daemon_is_unavailable_independently_of_the_battery() {
        let services = Services {
            ppd: Ppd::Absent,
            ..Services::all()
        };
        let Some(system) = fake("pw-nopres", services) else {
            return;
        };
        let running = start_on(Bus::Address(system.address().to_owned()), FAST);
        // The fake reports a battery; a present one is Ready, and the
        // missing daemon is Unavailable independently of it.
        assert!(matches!(
            until(&running, is_battery),
            PowerEvent::Battery(Service::Ready(_))
        ));
        assert_eq!(
            until(&running, is_profiles),
            PowerEvent::Profiles(Service::Unavailable)
        );
        stop(running);
    }

    #[test]
    fn a_refresh_reads_again_without_waiting_for_the_poll() {
        let Some(system) = fake("pw-refresh", Services::all()) else {
            return;
        };
        let slow = Timing {
            poll: Duration::from_secs(60),
            ..FAST
        };
        let running = start_on(Bus::Address(system.address().to_owned()), slow);
        until(&running, is_battery);
        until(&running, is_profiles);
        system.set_battery(10.0, 2).expect("battery");
        assert!(running
            .events
            .recv_timeout(Duration::from_millis(300))
            .is_err());
        running
            .provider
            .request(PowerCommand::Refresh(Section::Battery))
            .expect("queued");
        let PowerEvent::Battery(Service::Ready(battery)) = next(&running) else {
            panic!("a refreshed battery");
        };
        assert_eq!(battery.percentage(), 10);
        stop(running);
    }

    #[test]
    fn the_queue_is_bounded_and_refuses_the_rest() {
        let (commands, _held) = mpsc::sync_channel(COMMAND_QUEUE);
        let provider = PowerProvider { commands };
        for _ in 0..COMMAND_QUEUE {
            provider.request(PowerCommand::CycleProfile).expect("room");
        }
        assert_eq!(
            provider.request(PowerCommand::CycleProfile),
            Err(PowerCommand::CycleProfile)
        );
    }

    #[test]
    fn shutdown_is_seen_within_a_bound_even_with_a_long_poll() {
        let Some(system) = fake("pw-stop", Services::all()) else {
            return;
        };
        let slow = Timing {
            poll: Duration::from_secs(60),
            retry_start: Duration::from_secs(60),
            retry_max: Duration::from_secs(60),
        };
        let running = start_on(Bus::Address(system.address().to_owned()), slow);
        until(&running, is_battery);
        let began = Instant::now();
        stop(running);
        assert!(began.elapsed() < Duration::from_secs(2));
    }

    /// The bus address without its `guid=` part: a restarted daemon at the
    /// same socket has a new guid, which a client given the old address
    /// would reject.
    fn plain(address: &str) -> String {
        address.split(',').next().unwrap_or(address).to_owned()
    }

    #[test]
    fn a_bus_that_dies_and_comes_back_is_recovered_by_reconnecting() {
        let Some(first) = fake("pw-restart", Services::all()) else {
            return;
        };
        let address_before = plain(first.address());
        let running = start_on(Bus::Address(address_before.clone()), FAST);
        assert!(matches!(
            until(&running, is_battery),
            PowerEvent::Battery(Service::Ready(_))
        ));
        until(&running, is_profiles);
        // The bus goes away under a live connection.
        drop(first);
        assert_eq!(
            until(&running, is_battery),
            PowerEvent::Battery(Service::Unavailable)
        );
        assert_eq!(
            until(&running, is_profiles),
            PowerEvent::Profiles(Service::Unavailable)
        );
        // A new daemon at the same socket: the worker reconnects by itself.
        let Some(second) = fake("pw-restart", Services::all()) else {
            return;
        };
        assert_eq!(plain(second.address()), address_before);
        assert!(matches!(
            until(&running, is_battery),
            PowerEvent::Battery(Service::Ready(_))
        ));
        let PowerEvent::Profiles(Service::Ready(profiles)) = until(&running, is_profiles) else {
            panic!("profiles after the restart");
        };
        assert_eq!(profiles.active(), Some(&profile("balanced")));
        // And a request works over the new connection.
        running
            .provider
            .request(PowerCommand::SetProfile(profile("performance")))
            .expect("queued");
        until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        assert_eq!(second.calls().all(), ["ppd:Set ActiveProfile=performance"]);
        stop(running);
    }

    #[test]
    fn reading_never_activates_an_activatable_absent_service() {
        if !std::path::Path::new(celestina_fake_system::TOUCH).exists() {
            eprintln!("touch is not installed: the activation test is skipped");
            return;
        }
        let names = [
            celestina_fake_system::UPOWER_NAME,
            celestina_fake_system::PPD_UPOWER_NAME,
            celestina_fake_system::PPD_HADESS_NAME,
        ];
        let system = match FakeSystem::start_activatable("pw-noact", Services::none(), &names) {
            Ok(system) => system,
            Err(FakeError::DaemonMissing) => {
                eprintln!("dbus-daemon is not installed: the power worker test is skipped");
                return;
            }
            Err(error) => panic!("{error}"),
        };
        let running = start_on(Bus::Address(plain(system.address())), FAST);
        assert_eq!(
            until(&running, is_battery),
            PowerEvent::Battery(Service::Unavailable)
        );
        assert_eq!(
            until(&running, is_profiles),
            PowerEvent::Profiles(Service::Unavailable)
        );
        // Several retries over every name, a request that is refused, and a
        // refresh: no read asks the bus to start anything.
        std::thread::sleep(FAST.retry_max * 4);
        running
            .provider
            .request(PowerCommand::Refresh(Section::Battery))
            .expect("queued");
        std::thread::sleep(FAST.retry_max * 2);
        stop(running);

        // The control: the same call WITHOUT the flag does activate, so the
        // empty record above proves the worker's flag, not a blind fake.
        let address = plain(system.address());
        let connection = Builder::address(address.as_str())
            .map(|builder| builder.method_timeout(Duration::from_millis(300)))
            .and_then(Builder::build)
            .expect("a client");
        let proxy = Proxy::new(
            &connection,
            celestina_fake_system::UPOWER_NAME,
            celestina_fake_system::UPOWER_DISPLAY_PATH,
            PROPERTIES_INTERFACE,
        )
        .expect("a proxy");
        let _ = proxy.call_method("GetAll", &("org.freedesktop.UPower.Device",));
        let deadline = Instant::now() + WAIT;
        while system.activated().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            system.activated(),
            [celestina_fake_system::UPOWER_NAME],
            "a plain call activates; the worker's reads did not"
        );
    }

    #[test]
    fn a_change_succeeds_on_a_machine_without_upower() {
        // Review D #1: no UPower (every battery read is `ServiceUnknown`), the
        // profiles read seldom and the battery retried often. A reply from the
        // bus proves the link alive, so the battery retries never drop it, and
        // a change made between two profile reads makes its one `Set` call.
        let services = Services {
            upower: false,
            ..Services::all()
        };
        let Some(system) = fake("pw-nobat", services) else {
            return;
        };
        let timing = Timing {
            poll: Duration::from_secs(60),
            retry_start: Duration::from_millis(100),
            retry_max: Duration::from_millis(100),
        };
        let running = start_on(Bus::Address(plain(system.address())), timing);
        assert!(matches!(
            until(&running, is_profiles),
            PowerEvent::Profiles(Service::Ready(_))
        ));
        // Several battery-only retries pass.
        std::thread::sleep(Duration::from_millis(450));
        running
            .provider
            .request(PowerCommand::SetProfile(profile("performance")))
            .expect("queued");
        assert_eq!(
            until(&running, |e| matches!(e, PowerEvent::Resolved(_))),
            PowerEvent::Resolved(Resolution::Confirmed(Expected::Profile(profile(
                "performance"
            ))))
        );
        assert_eq!(system.calls().all(), ["ppd:Set ActiveProfile=performance"]);
        stop(running);
    }

    #[test]
    fn a_change_with_no_link_opens_one_and_finds_the_daemon_first() {
        // The worker's state as after a lost connection: the inventory still
        // holds the profiles, but there is no link. The change connects,
        // reads the daemon's name and makes its one call.
        let Some(system) = fake("pw-nolink", Services::all()) else {
            return;
        };
        let now = Instant::now();
        let mut state = State {
            bus: Bus::Address(plain(system.address())),
            timing: FAST,
            link: None,
            battery: Watch::new(now, FAST),
            profiles: Watch::new(now, FAST),
            pending: PendingSet::new(),
        };
        let section = ProfileSection::new("balanced", PPD_NAMES, Degraded::No);
        state.profiles.settle(now, Some(section), FAST);
        let mut events = Vec::new();
        handle(
            &mut state,
            &PowerCommand::SetProfile(profile("performance")),
            &mut events,
        );
        assert_eq!(events, Vec::new(), "no failure was published");
        assert!(state.link.as_ref().is_some_and(|link| link.ppd == Some(0)));
        assert_eq!(system.calls().all(), ["ppd:Set ActiveProfile=performance"]);
    }

    #[test]
    fn two_quick_cycles_advance_two_profiles() {
        let Some(system) = fake("pw-twice", Services::all()) else {
            return;
        };
        let slow = Timing {
            poll: Duration::from_secs(60),
            ..FAST
        };
        let running = start_on(Bus::Address(plain(system.address())), slow);
        until(&running, is_profiles);
        // Both are queued before the worker looks: balanced -> performance
        // -> power-saver, not performance twice.
        running
            .provider
            .request(PowerCommand::CycleProfile)
            .expect("queued");
        running
            .provider
            .request(PowerCommand::CycleProfile)
            .expect("queued");
        until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        until(&running, |e| matches!(e, PowerEvent::Resolved(_)));
        assert_eq!(
            system.calls().all(),
            [
                "ppd:Set ActiveProfile=performance",
                "ppd:Set ActiveProfile=power-saver"
            ]
        );
        stop(running);
    }

    #[test]
    fn a_hostile_profile_list_is_bounded_by_the_section() {
        let section = ProfileSection::new(
            "balanced",
            [
                "balanced",
                "BAD name",
                "balanced",
                &"x".repeat(MAX_PROFILE_BYTES + 1),
            ],
            Degraded::No,
        );
        assert_eq!(section.profiles(), [profile("balanced")]);
    }
}
