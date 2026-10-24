// Adapted from `celestina-qt/src/provider_adapter/notifications.rs` at
// c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413 (the bus half; the halted original
// is read-only and was not edited). Changed: the state machine is
// `celestina_shell::notifications` behind one mutex owned by this worker
// instead of a process-wide static; the JSON frame is replaced by a typed
// `NotificationsSnapshot` sent into the loop; the clock is the pausable one,
// so hovering the banner stack holds expiry; the panel's string verbs are the
// typed `NotificationsCommand` over a bounded queue; a taken name is never
// asked for again; the owner of a taken name is described; signals are
// declared on the interface; the `#[allow]` on the interface is gone.

//! The session's notification server, when the session does not already have
//! one (design §5.5, §5.6).
//!
//! This is the bus half of [`celestina_shell::notifications`]: it turns a
//! `Notify` call into an [`Incoming`], hands it to the state machine and
//! reports back what the machine decided. Every rule about identity, expiry,
//! bounds and caps lives there; nothing is decided here.
//!
//! The name is claimed **only when it is free**. A shell that took
//! `org.freedesktop.Notifications` from a running server would silently break
//! every notification the person still depends on. So the object is served
//! first and the name is then requested with `DoNotQueue` alone: no
//! replacement, no queueing to inherit it later, and no second request. Any
//! answer but `PrimaryOwner` means this provider serves nothing this session;
//! it says who does ([`NotificationsEvent::Owner`]) and idles until shutdown.
//!
//! One worker ([`spawn`]) owns the connection, the machine and a
//! [`PausableClock`]. It takes [`NotificationsCommand`]s from a queue of
//! [`COMMAND_QUEUE`], expires deadlines every [`TICK`], emits the
//! specification's signals, and publishes a whole [`NotificationsSnapshot`]
//! whenever the state changed: at once after a batch of commands, and at the
//! next tick after a bus call, so a burst of `Notify` calls is one snapshot.

use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::bounded::bounded;
use celestina_shell::notifications::{
    capabilities, CloseReason, Closed, Incoming, Notification, Notifications, PausableClock,
    Urgency, MAX_ICON_CHARS,
};
use celestina_wayland::Sender;
use zbus::blocking::{connection::Builder, fdo::DBusProxy, Connection};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

use crate::message::Message;
use crate::providers::Worker;

/// The session's notification name.
pub const BUS_NAME: &str = "org.freedesktop.Notifications";
/// Where the server is served.
pub const OBJECT_PATH: &str = "/org/freedesktop/Notifications";
/// The specification's interface.
pub const INTERFACE: &str = "org.freedesktop.Notifications";
/// How many commands may wait for the worker. A person cannot click faster
/// than the worker drains; a full queue is a stuck worker, and the command
/// is refused rather than queued behind it.
pub const COMMAND_QUEUE: usize = 32;
/// How often deadlines are checked. A banner leaving a quarter of a second
/// late is imperceptible; a thread waking more often is not free.
pub const TICK: Duration = Duration::from_millis(250);
/// The longest unique name an owner description carries.
const MAX_UNIQUE_NAME_CHARS: usize = 64;
/// The longest process name an owner description carries; the kernel keeps
/// 15 bytes of it.
const MAX_COMM_CHARS: usize = 16;
/// How many bytes of `/proc/<pid>/comm` are read.
const MAX_COMM_BYTES: u64 = 64;

/// What the loop is told about notifications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationsEvent {
    /// Who holds the notifications name: `None` when this shell does, else a
    /// description of the holder (`":1.42 (noctalia)"`, or only the unique
    /// name when its process cannot be read). Sent once, after the claim.
    Owner(Option<String>),
    /// The whole state, sent whenever it changed.
    Snapshot(NotificationsSnapshot),
}

/// Everything the banners and the Notifications page draw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotificationsSnapshot {
    /// What may interrupt now, newest first. While quiet, only critical ones.
    pub toasts: Vec<Notification>,
    /// Every live notification, newest last.
    pub live: Vec<Notification>,
    /// The page's share of the history, newest first.
    pub history: Vec<Notification>,
    /// Whether toasts are withheld.
    pub quiet: bool,
    /// How many the person has not seen.
    pub unread: usize,
    /// The wall-clock time, in Unix milliseconds, at which the worker's
    /// clock read zero: a notification's `posted_ms` plus this is when it
    /// arrived (off by the time the stack spent paused by hovering).
    /// Zero when unknown.
    pub epoch_unix_ms: u64,
}

/// What the Island asks of the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationsCommand {
    /// The person dismissed one notification.
    Close(u32),
    /// The person pressed one of its actions: the key the producer offered.
    Invoke(u32, String),
    /// The person cleared the stack.
    DismissAll,
    /// Do not disturb on or off.
    SetQuiet(bool),
    /// Forget what has ended.
    ClearHistory,
    /// The pointer entered (`true`) or left the banner stack; expiry is held
    /// while it rests there.
    Hover(bool),
    /// The person looked at the page.
    MarkRead,
    /// The shell's own notice (the polkit prompt could not be shown, or is
    /// cooling down): posted like a `Notify` call, so it is a banner that
    /// takes no keyboard. Boxed: it would size every command.
    Post(Box<Incoming>),
}

/// The handle the loop sends commands through.
#[derive(Clone, Debug)]
pub struct NotificationsProvider {
    commands: SyncSender<NotificationsCommand>,
}

impl NotificationsProvider {
    /// A handle and the queue it feeds, [`COMMAND_QUEUE`] deep: what the
    /// worker serves, and what a test reads the commands from.
    #[must_use]
    pub fn channel() -> (NotificationsProvider, Receiver<NotificationsCommand>) {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        (NotificationsProvider { commands }, queue)
    }

    /// Queues `command` without waiting.
    ///
    /// # Errors
    ///
    /// Returns the command when the queue is full or the worker serves
    /// nothing (another program holds the name, or it stopped).
    pub fn send(&self, command: NotificationsCommand) -> Result<(), NotificationsCommand> {
        self.commands.try_send(command).map_err(|err| match err {
            TrySendError::Full(command) | TrySendError::Disconnected(command) => command,
        })
    }
}

/// Which bus the server is served on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bus {
    /// The session bus: production.
    Session,
    /// A bus at this address: tests, on a private bus.
    Address(String),
}

impl Bus {
    fn builder(&self) -> zbus::Result<Builder<'static>> {
        match self {
            Self::Session => Builder::session(),
            Self::Address(address) => Builder::address(address.as_str()),
        }
    }
}

/// Starts the notification server on the session bus, publishing into the
/// loop.
///
/// # Errors
///
/// The failure to start the worker thread.
pub fn spawn(
    sender: Sender<Message>,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(NotificationsProvider, Worker)> {
    spawn_on(
        Bus::Session,
        move |event| sender.send(Message::Notifications(event)).is_ok(),
        shutdown,
    )
}

/// [`spawn`] on `bus`, publishing through `publish`, which returns `false`
/// once nobody listens (the worker then stops).
///
/// # Errors
///
/// The failure to start the worker thread.
pub fn spawn_on(
    bus: Bus,
    publish: impl FnMut(NotificationsEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(NotificationsProvider, Worker)> {
    let (provider, queue) = NotificationsProvider::channel();
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("notifications", shutdown, move || {
        run(&bus, publish, queue, &flag);
    })?;
    Ok((provider, worker))
}

/// What the server owes the bus after a change.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Signal {
    Closed(Closed),
    Action { id: u32, key: String },
}

/// The machine, its clock and whether a snapshot is owed. Every method takes
/// the real milliseconds since the worker started, so it runs without a
/// clock in tests.
#[derive(Debug)]
struct Core {
    machine: Notifications,
    clock: PausableClock,
    dirty: bool,
    /// See [`NotificationsSnapshot::epoch_unix_ms`].
    epoch_unix_ms: u64,
}

impl Core {
    /// A core that owes its first, empty snapshot.
    fn new() -> Self {
        Self {
            machine: Notifications::new(),
            clock: PausableClock::new(),
            dirty: true,
            epoch_unix_ms: unix_now_ms(),
        }
    }

    fn post(&mut self, incoming: &Incoming, real_ms: u64) -> u32 {
        let now = self.clock.now_ms(real_ms);
        self.dirty = true;
        self.machine.post(incoming, now)
    }

    fn close(&mut self, id: u32, reason: CloseReason) -> Option<Closed> {
        let closed = self.machine.close(id, reason);
        self.dirty |= closed.is_some();
        closed
    }

    fn command(&mut self, command: NotificationsCommand, real_ms: u64) -> Vec<Signal> {
        match command {
            NotificationsCommand::Close(id) => self
                .close(id, CloseReason::Dismissed)
                .map(Signal::Closed)
                .into_iter()
                .collect(),
            NotificationsCommand::Invoke(id, key) => {
                // A key the producer never offered is never sent on its
                // behalf.
                if !self.machine.accepts_action(id, &key) {
                    tracing::debug!(id, "an action the notification does not offer");
                    return Vec::new();
                }
                let mut signals = vec![Signal::Action { id, key }];
                signals.extend(self.close(id, CloseReason::Dismissed).map(Signal::Closed));
                signals
            }
            NotificationsCommand::DismissAll => {
                let closed = self.machine.dismiss_all();
                self.dirty |= !closed.is_empty();
                closed.into_iter().map(Signal::Closed).collect()
            }
            NotificationsCommand::SetQuiet(quiet) => {
                self.dirty |= self.machine.is_quiet() != quiet;
                self.machine.set_quiet(quiet);
                Vec::new()
            }
            NotificationsCommand::ClearHistory => {
                self.dirty |= !self.machine.history().is_empty();
                self.machine.clear_history();
                Vec::new()
            }
            NotificationsCommand::Hover(hovered) => {
                self.clock.set_paused(real_ms, hovered);
                Vec::new()
            }
            NotificationsCommand::MarkRead => {
                self.dirty |= self.machine.live().iter().any(|entry| !entry.read);
                self.machine.mark_read();
                Vec::new()
            }
            NotificationsCommand::Post(notice) => {
                self.post(&notice, real_ms);
                Vec::new()
            }
        }
    }

    fn tick(&mut self, real_ms: u64) -> Vec<Signal> {
        let now = self.clock.now_ms(real_ms);
        let expired = self.machine.expire(now);
        self.dirty |= !expired.is_empty();
        expired.into_iter().map(Signal::Closed).collect()
    }

    /// The snapshot owed since the last one, if any.
    fn take_snapshot(&mut self) -> Option<NotificationsSnapshot> {
        let epoch_unix_ms = self.epoch_unix_ms;
        std::mem::take(&mut self.dirty).then(|| NotificationsSnapshot {
            epoch_unix_ms,
            ..snapshot(&self.machine)
        })
    }
}

/// The machine's state as the loop receives it.
pub(crate) fn snapshot(machine: &Notifications) -> NotificationsSnapshot {
    NotificationsSnapshot {
        toasts: machine.toasts().into_iter().rev().cloned().collect(),
        live: machine.live().to_vec(),
        history: machine.history_page().to_vec(),
        quiet: machine.is_quiet(),
        unread: machine.unread(),
        epoch_unix_ms: 0,
    }
}

/// The wall clock in Unix milliseconds (0 before the epoch).
fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The core, whatever a panicking holder left behind: every operation on it
/// leaves it consistent, so a poisoned lock is still usable.
fn lock(core: &Mutex<Core>) -> MutexGuard<'_, Core> {
    match core.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Milliseconds since `started`, the worker's own monotonic clock.
fn real_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Reads the `urgency` hint. Anything that is not a byte leaves the urgency
/// normal.
fn urgency_of(hints: &HashMap<String, OwnedValue>) -> Urgency {
    hints
        .get("urgency")
        .and_then(|value| u8::try_from(value).ok())
        .map_or(Urgency::Normal, Urgency::from_hint)
}

/// Reads the `image-path` hint (or its older `image_path` spelling) as text.
/// Raw pixel hints (`image-data`, `icon_data`) are ignored: this shell
/// decodes nothing a stranger sent it. A path too long for the machine to
/// accept is not copied at all.
fn image_of(hints: &HashMap<String, OwnedValue>) -> Option<String> {
    hints
        .get("image-path")
        .or_else(|| hints.get("image_path"))
        .and_then(|value| <&str>::try_from(value).ok())
        // A char is at most four bytes, so a longer text has more chars than
        // the machine keeps; it would refuse it.
        .filter(|path| path.len() <= MAX_ICON_CHARS * 4)
        .map(ToOwned::to_owned)
}

/// Describes the holder of the name: its unique name, and its process name
/// when one was read.
fn owner_description(unique: &str, comm: Option<&str>) -> String {
    let unique = bounded(unique, MAX_UNIQUE_NAME_CHARS);
    let comm = comm
        .map(|comm| {
            comm.chars()
                .filter(|c| !c.is_control())
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .filter(|comm| !comm.is_empty());
    match comm {
        Some(comm) => format!("{unique} ({})", bounded(&comm, MAX_COMM_CHARS)),
        None => unique,
    }
}

/// The process name of `pid`, bounded, when it can be read.
fn read_comm(pid: u32) -> Option<String> {
    let file = std::fs::File::open(format!("/proc/{pid}/comm")).ok()?;
    let mut comm = String::new();
    file.take(MAX_COMM_BYTES).read_to_string(&mut comm).ok()?;
    Some(comm)
}

/// Asks the bus who holds the name. `?` when the bus will not say (the
/// holder left between the refusal and the question).
fn describe_holder(connection: &Connection) -> String {
    let unique = DBusProxy::new(connection).and_then(|proxy| {
        let unique = proxy.get_name_owner(BusName::from_static_str(BUS_NAME)?)?;
        let pid = BusName::try_from(unique.as_str())
            .ok()
            .and_then(|owner| proxy.get_connection_unix_process_id(owner).ok());
        Ok((unique, pid))
    });
    match unique {
        Ok((unique, pid)) => {
            let comm = pid.and_then(read_comm);
            owner_description(unique.as_str(), comm.as_deref())
        }
        Err(err) => {
            tracing::debug!(%err, "asking who owns {BUS_NAME}");
            "?".to_owned()
        }
    }
}

/// The served interface. It holds no state of its own: every method is a
/// translation into the core and back.
struct Server {
    core: Arc<Mutex<Core>>,
    started: Instant,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    /// The specification's `Notify`. Returns the id the notification is
    /// known by, which is `replaces_id` when that named something still
    /// live. The texts zbus decoded are moved into the machine, never copied
    /// whole: it keeps a bounded prefix of each.
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let incoming = Incoming {
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            urgency: urgency_of(&hints),
            image: image_of(&hints),
            expire_timeout,
        };
        lock(&self.core).post(&incoming, real_ms(self.started))
    }

    /// The specification's `CloseNotification`. A producer withdrawing its
    /// own notification is `Requested`, which is not the person having dealt
    /// with it. An id that is not live is answered without a signal.
    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let closed = lock(&self.core).close(id, CloseReason::Requested);
        if let Some(closed) = closed {
            if let Err(err) =
                Self::notification_closed(&emitter, closed.id, closed.reason as u32).await
            {
                tracing::debug!(%err, "emitting NotificationClosed");
            }
        }
    }

    /// What the machine admits it can do.
    fn get_capabilities(&self) -> Vec<String> {
        capabilities()
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect()
    }

    /// Name, vendor, version, specification version.
    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Celestina".to_owned(),
            "celestina".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            "1.2".to_owned(),
        )
    }

    /// A notification ended, with the specification's reason.
    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    /// A person pressed one of a notification's actions.
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

/// What asking for the name ended in.
enum Claim {
    /// This shell is the session's notification server.
    Owned(Connection),
    /// Somebody else is; the description of the holder.
    Taken(String),
    /// The bus could not be reached or served.
    Failed(zbus::Error),
}

/// Serves the object, then asks for the name once, only if it is free. A
/// connection that does not end up owning the name is dropped on return,
/// taking the served object with it: nothing is served under its unique
/// name either.
fn claim(bus: &Bus, core: &Arc<Mutex<Core>>, started: Instant) -> Claim {
    let server = Server {
        core: Arc::clone(core),
        started,
    };
    let built = bus
        .builder()
        .and_then(|builder| builder.serve_at(OBJECT_PATH, server))
        .and_then(Builder::build);
    let connection = match built {
        Ok(connection) => connection,
        Err(err) => return Claim::Failed(err),
    };
    // No ReplaceExisting and no queueing. A name somebody else holds comes
    // back as a plain reply from the bus or as an error from zbus; both mean
    // the same, and neither is a reason to ask again.
    match connection.request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(RequestNameReply::PrimaryOwner) => Claim::Owned(connection),
        Ok(_) | Err(zbus::Error::NameTaken) => Claim::Taken(describe_holder(&connection)),
        Err(err) => Claim::Failed(err),
    }
}

fn emit(connection: &Connection, signals: &[Signal]) {
    for signal in signals {
        let sent = match signal {
            Signal::Closed(closed) => connection.emit_signal(
                Option::<&str>::None,
                OBJECT_PATH,
                INTERFACE,
                "NotificationClosed",
                &(closed.id, closed.reason as u32),
            ),
            Signal::Action { id, key } => connection.emit_signal(
                Option::<&str>::None,
                OBJECT_PATH,
                INTERFACE,
                "ActionInvoked",
                &(*id, key.as_str()),
            ),
        };
        if let Err(err) = sent {
            tracing::debug!(%err, "emitting a notification signal");
        }
    }
}

fn run(
    bus: &Bus,
    mut publish: impl FnMut(NotificationsEvent) -> bool,
    queue: Receiver<NotificationsCommand>,
    shutdown: &AtomicBool,
) {
    let started = Instant::now();
    let core = Arc::new(Mutex::new(Core::new()));
    let connection = match claim(bus, &core, started) {
        Claim::Owned(connection) => connection,
        Claim::Taken(holder) => {
            tracing::info!("{holder} serves {BUS_NAME}; this shell serves no notifications");
            // Commands to a server that serves nothing fail at the sender.
            drop(queue);
            if publish(NotificationsEvent::Owner(Some(holder))) {
                while !shutdown.load(Ordering::Acquire) {
                    thread::sleep(TICK);
                }
            }
            return;
        }
        Claim::Failed(err) => {
            tracing::warn!(%err, "the notification server cannot use the bus");
            return;
        }
    };
    if publish(NotificationsEvent::Owner(None)) {
        serve(&connection, &core, started, &mut publish, &queue, shutdown);
    }
    if let Err(err) = connection.release_name(BUS_NAME) {
        tracing::debug!(%err, "releasing {BUS_NAME}");
    }
}

fn serve(
    connection: &Connection,
    core: &Mutex<Core>,
    started: Instant,
    publish: &mut impl FnMut(NotificationsEvent) -> bool,
    queue: &Receiver<NotificationsCommand>,
    shutdown: &AtomicBool,
) {
    let mut queue_open = true;
    let mut next_tick = Instant::now() + TICK;
    while !shutdown.load(Ordering::Acquire) {
        let wait = next_tick.saturating_duration_since(Instant::now());
        let received = if queue_open {
            match queue.recv_timeout(wait) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => {
                    // The loop dropped its handle; time still passes.
                    queue_open = false;
                    None
                }
            }
        } else {
            thread::sleep(wait);
            None
        };
        if let Some(command) = received {
            let mut held = lock(core);
            let mut signals = held.command(command, real_ms(started));
            // What else is already queued joins the same snapshot; bounded,
            // so a flood cannot keep the worker from its tick.
            for more in queue.try_iter().take(COMMAND_QUEUE) {
                signals.extend(held.command(more, real_ms(started)));
            }
            drop(held);
            emit(connection, &signals);
        }
        if Instant::now() >= next_tick {
            let signals = lock(core).tick(real_ms(started));
            emit(connection, &signals);
            next_tick = Instant::now() + TICK;
        }
        let owed = lock(core).take_snapshot();
        if let Some(snapshot) = owed {
            if !publish(NotificationsEvent::Snapshot(snapshot)) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::notifications::{MAX_ACTIONS, MAX_BODY_CHARS, PAGE_HISTORY};
    use zbus::zvariant::Value;

    fn hint(key: &str, value: Value<'static>) -> HashMap<String, OwnedValue> {
        let mut hints = HashMap::new();
        match OwnedValue::try_from(value) {
            Ok(value) => hints.insert(key.to_owned(), value),
            Err(err) => panic!("a hint: {err}"),
        };
        hints
    }

    fn incoming(summary: &str, urgency: Urgency, expire_timeout: i32) -> Incoming {
        Incoming {
            app_name: "Test".to_owned(),
            summary: summary.to_owned(),
            urgency,
            expire_timeout,
            ..Incoming::default()
        }
    }

    fn with_action(summary: &str, key: &str) -> Incoming {
        Incoming {
            actions: vec![key.to_owned(), "Label".to_owned()],
            ..incoming(summary, Urgency::Normal, 0)
        }
    }

    fn closed(id: u32, reason: CloseReason) -> Signal {
        Signal::Closed(Closed { id, reason })
    }

    #[test]
    fn an_urgency_hint_is_read_and_anything_else_is_normal() {
        assert_eq!(urgency_of(&hint("urgency", 0u8.into())), Urgency::Low);
        assert_eq!(urgency_of(&hint("urgency", 2u8.into())), Urgency::Critical);
        assert_eq!(urgency_of(&hint("urgency", 7u8.into())), Urgency::Normal);
        assert_eq!(
            urgency_of(&hint("urgency", Value::from("critical"))),
            Urgency::Normal
        );
        assert_eq!(urgency_of(&HashMap::new()), Urgency::Normal);
    }

    #[test]
    fn only_an_image_path_is_read_as_text_and_pixels_are_ignored() {
        assert_eq!(
            image_of(&hint("image-path", Value::from("/usr/share/a.png"))),
            Some("/usr/share/a.png".to_owned())
        );
        assert_eq!(
            image_of(&hint("image_path", Value::from("dialog-information"))),
            Some("dialog-information".to_owned())
        );
        assert_eq!(
            image_of(&hint("image-data", Value::from(vec![1u8, 2, 3]))),
            None
        );
        assert_eq!(image_of(&hint("image-path", Value::from(7u32))), None);
        // Too long for the machine to keep: never copied.
        let long = "a".repeat(MAX_ICON_CHARS * 4 + 1);
        assert_eq!(image_of(&hint("image-path", Value::from(long))), None);
    }

    #[test]
    fn an_owner_is_described_by_unique_name_and_process() {
        assert_eq!(
            owner_description(":1.42", Some("noctalia\n")),
            ":1.42 (noctalia)"
        );
        assert_eq!(owner_description(":1.42", None), ":1.42");
        assert_eq!(owner_description(":1.42", Some(" \n")), ":1.42");
        assert_eq!(
            owner_description(":1.7", Some("a\u{1b}[31mb")),
            ":1.7 (a[31mb)"
        );
        let long = "x".repeat(200);
        let described = owner_description(&long, Some(&long));
        assert!(described.chars().count() <= MAX_UNIQUE_NAME_CHARS + MAX_COMM_CHARS + 3);
    }

    #[test]
    fn the_snapshot_carries_toasts_newest_first_and_quiet_keeps_critical() {
        let mut machine = Notifications::new();
        let low = machine.post(&incoming("low", Urgency::Low, -1), 0);
        let critical = machine.post(&incoming("critical", Urgency::Critical, -1), 1);
        let ids = |list: &[Notification]| list.iter().map(|n| n.id).collect::<Vec<_>>();

        let shown = snapshot(&machine);
        assert_eq!(ids(&shown.toasts), vec![critical, low]);
        assert_eq!(ids(&shown.live), vec![low, critical]);
        assert_eq!(shown.unread, 2);
        assert!(!shown.quiet);

        machine.set_quiet(true);
        let quiet = snapshot(&machine);
        assert_eq!(ids(&quiet.toasts), vec![critical]);
        assert_eq!(quiet.live.len(), 2);
        assert!(quiet.quiet);

        for n in 0..(PAGE_HISTORY + 5) {
            let id = machine.post(&incoming(&format!("n{n}"), Urgency::Normal, 0), 2);
            machine.close(id, CloseReason::Dismissed);
        }
        assert_eq!(snapshot(&machine).history.len(), PAGE_HISTORY);
    }

    #[test]
    fn the_first_snapshot_carries_the_wall_clock_epoch() {
        let mut core = Core::new();
        let first = core.take_snapshot();
        assert!(first.is_some_and(|s| s.epoch_unix_ms > 0));
    }

    #[test]
    fn a_snapshot_is_owed_only_after_a_change() {
        let mut core = Core::new();
        assert!(core.take_snapshot().is_some(), "the first, empty one");
        assert!(core.take_snapshot().is_none());
        assert!(core.tick(10).is_empty());
        assert!(core.take_snapshot().is_none());
        assert!(core
            .command(NotificationsCommand::Hover(true), 10)
            .is_empty());
        assert!(core.take_snapshot().is_none());
        core.post(&incoming("a", Urgency::Normal, -1), 20);
        assert!(core.take_snapshot().is_some());
        core.command(NotificationsCommand::SetQuiet(false), 30);
        assert!(core.take_snapshot().is_none(), "already not quiet");
    }

    #[test]
    fn a_notice_the_shell_posts_itself_is_shown_like_any_other() {
        let mut core = Core::new();
        let _ = core.take_snapshot();
        let notice = Incoming {
            app_name: "Shell".to_owned(),
            summary: "Own notice".to_owned(),
            urgency: Urgency::Critical,
            expire_timeout: 0,
            ..Incoming::default()
        };
        assert!(core
            .command(NotificationsCommand::Post(Box::new(notice)), 5)
            .is_empty());
        let snapshot = core.take_snapshot();
        assert!(
            snapshot.is_some_and(|s| s.toasts.len() == 1 && s.toasts[0].summary == "Own notice")
        );
    }

    #[test]
    fn hovering_holds_expiry_and_leaving_releases_it() {
        let mut core = Core::new();
        let id = core.post(&incoming("a", Urgency::Normal, 1_000), 0);
        core.command(NotificationsCommand::Hover(true), 500);
        assert!(core.tick(5_000).is_empty(), "held while hovered");
        core.command(NotificationsCommand::Hover(false), 5_000);
        assert!(core.tick(5_400).is_empty(), "900 ms of logical time");
        assert_eq!(core.tick(5_500), vec![closed(id, CloseReason::Expired)]);
    }

    #[test]
    fn invoking_an_offered_action_announces_it_then_closes_as_dismissed() {
        let mut core = Core::new();
        let id = core.post(&with_action("a", "open"), 0);
        assert!(core
            .command(NotificationsCommand::Invoke(id, "other".to_owned()), 1)
            .is_empty());
        assert_eq!(
            core.command(NotificationsCommand::Invoke(id, "open".to_owned()), 2),
            vec![
                Signal::Action {
                    id,
                    key: "open".to_owned()
                },
                closed(id, CloseReason::Dismissed),
            ]
        );
        assert!(core.machine.live().is_empty());
    }

    #[test]
    fn closing_and_dismissing_all_are_dismissals() {
        let mut core = Core::new();
        let a = core.post(&incoming("a", Urgency::Normal, 0), 0);
        let b = core.post(&incoming("b", Urgency::Critical, 0), 0);
        let c = core.post(&incoming("c", Urgency::Low, 0), 0);
        assert_eq!(
            core.command(NotificationsCommand::Close(a), 1),
            vec![closed(a, CloseReason::Dismissed)]
        );
        assert!(core.command(NotificationsCommand::Close(a), 1).is_empty());
        assert_eq!(
            core.command(NotificationsCommand::DismissAll, 2),
            vec![
                closed(b, CloseReason::Dismissed),
                closed(c, CloseReason::Dismissed)
            ]
        );
        core.command(NotificationsCommand::MarkRead, 3);
        let _ = core.take_snapshot();
        core.command(NotificationsCommand::ClearHistory, 4);
        let cleared = core.take_snapshot();
        assert!(cleared.is_some_and(|s| s.history.is_empty() && s.unread == 0));
    }

    #[test]
    fn hostile_notify_arguments_are_bounded_by_the_machine() {
        let mut core = Core::new();
        let incoming = Incoming {
            body: "x".repeat(50_000),
            actions: (0..1_000).map(|i| format!("a{i}")).collect(),
            image: image_of(&hint("image-path", Value::from("../../etc/passwd"))),
            ..incoming("s", Urgency::Normal, 0)
        };
        core.post(&incoming, 0);
        let posted = &core.machine.live()[0];
        assert!(posted.body.chars().count() <= MAX_BODY_CHARS);
        assert!(posted.actions.len() <= MAX_ACTIONS);
        assert_eq!(posted.image, None);
    }

    #[test]
    fn a_full_command_queue_refuses_and_returns_the_command() {
        let (commands, queue) = mpsc::sync_channel(COMMAND_QUEUE);
        let provider = NotificationsProvider { commands };
        for _ in 0..COMMAND_QUEUE {
            assert_eq!(provider.send(NotificationsCommand::MarkRead), Ok(()));
        }
        assert_eq!(
            provider.send(NotificationsCommand::Close(7)),
            Err(NotificationsCommand::Close(7))
        );
        drop(queue);
        assert_eq!(
            provider.send(NotificationsCommand::DismissAll),
            Err(NotificationsCommand::DismissAll)
        );
    }
}
