//! logind: the caffeine inhibitor and the power verbs (T5; decisions D2, D5,
//! D6).
//!
//! One worker owns a blocking `zbus` connection to the system bus and serves
//! two kinds of request from a bounded queue ([`QUEUE`]):
//!
//! - **Caffeine.** `Inhibit("idle:sleep", "Celestina", WHY, "block")` on
//!   `org.freedesktop.login1.Manager`. logind keeps the lock exactly as long
//!   as the returned file descriptor is open, so the descriptor *is* the
//!   state: the worker holds it, and closing it is the release. It lives in
//!   the worker's own stack frame, so it is closed on release, when the
//!   worker stops (shutdown flag or every [`LogindHandle`] dropped), and on
//!   the unwind of a panic, all by `Drop`. Holding twice takes one lock.
//! - **Power.** Reboot, power off and suspend are `Manager` calls with
//!   `interactive = false` (a shell cannot answer a polkit prompt, so a
//!   session that is not allowed must fail visibly rather than hang), each
//!   after its `CanXxx` probe: `yes` and `challenge` go ahead, `na` is
//!   "unavailable" and nothing is called. Suspend is refused while no locker
//!   exists (D2: a session that suspends unlocked wakes unlocked). Log out is
//!   not logind's: the compositor ends the session (D5), so it is refused
//!   here and the wiring routes it to niri.
//!
//! Every bus call carries [`CALL_TIMEOUT`] and connecting is bounded by
//! [`CONNECT_TIMEOUT`]; the loop waits on its queue in [`POLL`] slices and
//! looks at the shutdown flag between the steps of a request, so
//! [`Worker::join`] is bounded. A bus or a service that is not there
//! degrades to "unavailable"; nothing here panics. Text from the bus is
//! hostile input and is bounded ([`MAX_REASON`]) before it is published.

use std::io;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use celestina_shell::session::PowerAction;
use zbus::blocking::{connection::Builder, Connection};

use crate::providers::worker::Worker;

/// logind's well-known name.
pub const LOGIND_NAME: &str = "org.freedesktop.login1";
/// Where logind serves its manager.
pub const LOGIND_PATH: &str = "/org/freedesktop/login1";
/// The manager interface.
pub const MANAGER_INTERFACE: &str = "org.freedesktop.login1.Manager";

/// What the caffeine inhibitor blocks: idling and sleeping.
pub const INHIBIT_WHAT: &str = "idle:sleep";
/// Who the inhibitor is reported as.
pub const INHIBIT_WHO: &str = "Celestina";
/// Why, as `systemd-inhibit --list` shows it.
pub const INHIBIT_WHY: &str = "The session was asked to stay awake";
/// `block`, not `delay`: a delay only postpones sleep, and what caffeine
/// promises is that the session does not idle or suspend while it is on.
pub const INHIBIT_MODE: &str = "block";

/// The requests that may wait for the worker. Caffeine and the power verbs
/// are rare, deliberate gestures; past this a binding is repeating itself,
/// and refusing is more honest than performing a stale request late.
pub const QUEUE: usize = 4;
/// The longest one bus call may take. A request makes at most two (the probe
/// and the call), so the worker answers within twice this.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(2);
/// The longest connecting to the bus may take; a connection still not made
/// by then is left to its helper thread and reported unavailable.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// How often the idle worker looks at the shutdown flag.
pub const POLL: Duration = Duration::from_millis(100);
/// The most bytes of a reason (often a bus error's text) that is published.
pub const MAX_REASON: usize = 240;

/// The reason suspend is refused while no locker exists (D2).
pub const NO_LOCKER: &str = "no locker yet";
/// The reason log out is refused here (D5).
pub const LOG_OUT_ELSEWHERE: &str = "log out goes through the compositor";
/// The reason given for a request cut short by the shell stopping.
const STOPPING: &str = "the shell is stopping";

/// Which bus logind is looked for on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LogindBus {
    /// The system bus: production.
    #[default]
    System,
    /// A bus at this address: tests, on a private bus.
    Address(String),
}

impl LogindBus {
    fn builder(&self) -> zbus::Result<Builder<'static>> {
        match self {
            Self::System => Builder::system(),
            Self::Address(address) => Builder::address(address.as_str()),
        }
    }
}

/// How the worker is built.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogindConfig {
    /// Where logind is looked for.
    pub bus: LogindBus,
    /// Whether a session locker exists. While it is `false` (the default,
    /// until CEL-2-H) suspend is refused with [`NO_LOCKER`].
    pub locker_available: bool,
}

/// What caffeine is left in after a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caffeine {
    /// The inhibitor is held.
    Held,
    /// No inhibitor is held.
    Released,
    /// No inhibitor is held because logind could not be asked or refused;
    /// the reason is bounded.
    Unavailable(String),
}

/// How a power request ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// logind accepted the call.
    Done,
    /// This shell does not perform it (no locker, log out, or logind says
    /// this session may not).
    Refused(String),
    /// logind is not there, or says this machine cannot do it.
    Unavailable(String),
    /// logind was asked and the call failed (denied, timed out, malformed).
    Failed(String),
}

/// What the worker publishes, one per request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogindEvent {
    /// The state caffeine is now in.
    Caffeine(Caffeine),
    /// How a power request ended.
    Power(PowerAction, Outcome),
}

/// Why a request was not queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// [`QUEUE`] requests are waiting already.
    #[error("the logind queue is full")]
    Full,
    /// The worker has stopped.
    #[error("the logind worker has stopped")]
    Stopped,
}

/// A failure to ask logind, with its source.
#[derive(Debug, thiserror::Error)]
pub enum LogindError {
    /// The bus could not be reached.
    #[error("cannot reach the bus")]
    Connect(#[source] Box<zbus::Error>),
    /// The bus did not finish the connection within [`CONNECT_TIMEOUT`].
    #[error("the bus did not answer within {} ms", CONNECT_TIMEOUT.as_millis())]
    ConnectTimeout,
    /// No thread could be started to bound the connection.
    #[error("cannot start the connection helper")]
    ConnectHelper(#[source] io::Error),
    /// A manager call failed.
    #[error("logind's {method} failed")]
    Call {
        method: &'static str,
        #[source]
        source: Box<zbus::Error>,
    },
    /// A manager call answered with something other than its signature.
    #[error("logind's {method} answered with an unexpected value")]
    Reply {
        method: &'static str,
        #[source]
        source: Box<zbus::Error>,
    },
}

impl LogindError {
    /// Whether this means logind (or the bus) is not there at all, rather
    /// than that it answered no.
    fn absent(&self) -> bool {
        match self {
            Self::Connect(_) | Self::ConnectTimeout | Self::ConnectHelper(_) => true,
            Self::Call { source, .. } => service_missing(source) || connection_lost(source),
            Self::Reply { .. } => false,
        }
    }

    /// Whether the connection should be dropped and made again next time.
    fn connection_broken(&self) -> bool {
        match self {
            Self::Call { source, .. } => connection_lost(source),
            _ => false,
        }
    }

    fn reason(&self) -> String {
        bounded(&crate::error_chain(self))
    }
}

/// Whether the bus answered that nobody serves logind's name.
fn service_missing(err: &zbus::Error) -> bool {
    const MISSING: [&str; 2] = [
        "org.freedesktop.DBus.Error.ServiceUnknown",
        "org.freedesktop.DBus.Error.NameHasNoOwner",
    ];
    match err {
        zbus::Error::MethodError(name, _, _) => MISSING.contains(&name.as_str()),
        zbus::Error::FDO(fdo) => matches!(
            **fdo,
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
        ),
        _ => false,
    }
}

/// Whether the connection itself failed (not a timeout, which leaves it
/// usable).
fn connection_lost(err: &zbus::Error) -> bool {
    match err {
        zbus::Error::InputOutput(io) => io.kind() != io::ErrorKind::TimedOut,
        zbus::Error::Connection(..) => true,
        _ => false,
    }
}

/// `text`, cut to [`MAX_REASON`] bytes on a character boundary.
fn bounded(text: &str) -> String {
    if text.len() <= MAX_REASON {
        return text.to_owned();
    }
    let mut end = MAX_REASON;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut cut = text[..end].to_owned();
    cut.push('…');
    cut
}

#[derive(Debug)]
enum Request {
    Caffeine(bool),
    Power(PowerAction),
}

/// The way into the worker: queues a request, never waits for it.
#[derive(Clone, Debug)]
pub struct LogindHandle {
    requests: SyncSender<Request>,
}

impl LogindHandle {
    fn channel() -> (LogindHandle, Receiver<Request>) {
        let (requests, queue) = mpsc::sync_channel(QUEUE);
        (LogindHandle { requests }, queue)
    }

    /// Asks for the caffeine inhibitor to be held (`true`) or released.
    ///
    /// # Errors
    ///
    /// [`RequestError`] when the queue is full or the worker has stopped.
    pub fn caffeine(&self, held: bool) -> Result<(), RequestError> {
        self.send(Request::Caffeine(held))
    }

    /// Asks for `action`.
    ///
    /// # Errors
    ///
    /// [`RequestError`] when the queue is full or the worker has stopped.
    pub fn power(&self, action: PowerAction) -> Result<(), RequestError> {
        self.send(Request::Power(action))
    }

    fn send(&self, request: Request) -> Result<(), RequestError> {
        self.requests.try_send(request).map_err(|err| match err {
            TrySendError::Full(_) => RequestError::Full,
            TrySendError::Disconnected(_) => RequestError::Stopped,
        })
    }
}

/// Starts the worker with `config`, publishing one [`LogindEvent`] per
/// request through `publish`, which returns `false` once nobody listens (the
/// worker then stops). The worker also stops when `shutdown` is raised or
/// every [`LogindHandle`] is dropped; either way it releases the inhibitor.
///
/// Nothing is asked of the bus until the first request.
///
/// # Errors
///
/// Returns the failure to start the thread.
pub fn spawn(
    config: LogindConfig,
    publish: impl FnMut(LogindEvent) -> bool + Send + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<(LogindHandle, Worker)> {
    let (handle, queue) = LogindHandle::channel();
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("logind", shutdown, move || {
        run(config, publish, &queue, &flag);
    })?;
    Ok((handle, worker))
}

fn run(
    config: LogindConfig,
    mut publish: impl FnMut(LogindEvent) -> bool,
    queue: &Receiver<Request>,
    shutdown: &AtomicBool,
) {
    // Owns the inhibitor: whatever way this function is left, the descriptor
    // is closed when `logind` goes out of scope.
    let mut logind = Logind::new(config);
    while !shutdown.load(Ordering::Acquire) {
        let event = match queue.recv_timeout(POLL) {
            Ok(Request::Caffeine(held)) => LogindEvent::Caffeine(logind.caffeine(held, shutdown)),
            Ok(Request::Power(action)) => {
                LogindEvent::Power(action, logind.power(action, shutdown))
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if !publish(event) {
            break;
        }
    }
    if logind.inhibitor.is_some() {
        tracing::info!("logind: the worker stops and releases the caffeine inhibitor");
    }
}

/// The worker's state: the connection, made on first use and again after it
/// broke, and the inhibitor descriptor.
struct Logind {
    config: LogindConfig,
    connection: Option<Connection>,
    inhibitor: Option<OwnedFd>,
}

impl Logind {
    fn new(config: LogindConfig) -> Self {
        Self {
            config,
            connection: None,
            inhibitor: None,
        }
    }

    fn caffeine(&mut self, held: bool, shutdown: &AtomicBool) -> Caffeine {
        if !held {
            // Closing the descriptor is the release.
            if self.inhibitor.take().is_some() {
                tracing::info!("logind: caffeine released");
            }
            return Caffeine::Released;
        }
        if self.inhibitor.is_some() {
            return Caffeine::Held;
        }
        if shutdown.load(Ordering::Acquire) {
            return Caffeine::Unavailable(STOPPING.to_owned());
        }
        match self.inhibit() {
            Ok(fd) => {
                self.inhibitor = Some(fd);
                tracing::info!("logind: caffeine held");
                Caffeine::Held
            }
            Err(err) => {
                tracing::warn!(error = %crate::error_chain(&err), "logind: caffeine unavailable");
                Caffeine::Unavailable(err.reason())
            }
        }
    }

    fn inhibit(&mut self) -> Result<OwnedFd, LogindError> {
        const METHOD: &str = "Inhibit";
        let reply = self.call(
            METHOD,
            &(INHIBIT_WHAT, INHIBIT_WHO, INHIBIT_WHY, INHIBIT_MODE),
        )?;
        let fd: zbus::zvariant::OwnedFd =
            reply
                .body()
                .deserialize()
                .map_err(|source| LogindError::Reply {
                    method: METHOD,
                    source: Box::new(source),
                })?;
        Ok(fd.into())
    }

    fn power(&mut self, action: PowerAction, shutdown: &AtomicBool) -> Outcome {
        let (probe, method, verb) = match action {
            PowerAction::LogOut => return Outcome::Refused(LOG_OUT_ELSEWHERE.to_owned()),
            PowerAction::Suspend if !self.config.locker_available => {
                return Outcome::Refused(NO_LOCKER.to_owned());
            }
            PowerAction::Suspend => ("CanSuspend", "Suspend", "suspend"),
            PowerAction::Reboot => ("CanReboot", "Reboot", "reboot"),
            PowerAction::PowerOff => ("CanPowerOff", "PowerOff", "power off"),
        };
        if shutdown.load(Ordering::Acquire) {
            return Outcome::Failed(STOPPING.to_owned());
        }
        let answer = match self.probe(probe) {
            Ok(answer) => answer,
            Err(err) => return failure(&err),
        };
        match answer.as_str() {
            "yes" | "challenge" => {}
            "na" => return Outcome::Unavailable(format!("this machine cannot {verb}")),
            "no" => {
                return Outcome::Refused(format!("logind does not allow this session to {verb}"))
            }
            other => {
                return Outcome::Unavailable(bounded(&format!(
                    "logind answered {probe} with '{other}'"
                )))
            }
        }
        if shutdown.load(Ordering::Acquire) {
            return Outcome::Failed(STOPPING.to_owned());
        }
        // `false`: never ask to authenticate interactively (see the module).
        match self.call(method, &(false,)) {
            Ok(_) => {
                tracing::info!("logind: {method} accepted");
                Outcome::Done
            }
            Err(err) => failure(&err),
        }
    }

    fn probe(&mut self, method: &'static str) -> Result<String, LogindError> {
        let reply = self.call(method, &())?;
        reply
            .body()
            .deserialize::<String>()
            .map(|answer| bounded(&answer))
            .map_err(|source| LogindError::Reply {
                method,
                source: Box::new(source),
            })
    }

    fn call<B>(&mut self, method: &'static str, body: &B) -> Result<zbus::Message, LogindError>
    where
        B: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
    {
        let connection = self.connection()?;
        let reply = connection
            .call_method(
                Some(LOGIND_NAME),
                LOGIND_PATH,
                Some(MANAGER_INTERFACE),
                method,
                body,
            )
            .map_err(|source| LogindError::Call {
                method,
                source: Box::new(source),
            });
        if let Err(err) = &reply {
            if err.connection_broken() {
                self.connection = None;
            }
        }
        reply
    }

    fn connection(&mut self) -> Result<Connection, LogindError> {
        if let Some(connection) = &self.connection {
            return Ok(connection.clone());
        }
        let connection = connect(&self.config.bus)?;
        self.connection = Some(connection.clone());
        Ok(connection)
    }
}

/// A failed power request as published: "unavailable" when logind or the bus
/// is not there, "failed" when it answered with an error.
fn failure(err: &LogindError) -> Outcome {
    tracing::warn!(error = %crate::error_chain(err), "logind: a power request did not go through");
    if err.absent() {
        Outcome::Unavailable(err.reason())
    } else {
        Outcome::Failed(err.reason())
    }
}

/// Connects to `bus` on a helper thread, waiting at most
/// [`CONNECT_TIMEOUT`]: the handshake has no timeout of its own, and a hung
/// bus must not hold the worker past its join. A helper still connecting
/// after that is left behind and ends with its connection attempt.
fn connect(bus: &LogindBus) -> Result<Connection, LogindError> {
    let builder = bus
        .builder()
        .map_err(|err| LogindError::Connect(Box::new(err)))?;
    let (done, result) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("logind-connect".to_owned())
        .spawn(move || {
            let connection = builder.method_timeout(CALL_TIMEOUT).build();
            let _ = done.send(connection);
        })
        .map_err(LogindError::ConnectHelper)?;
    match result.recv_timeout(CONNECT_TIMEOUT) {
        Ok(connection) => connection.map_err(|err| LogindError::Connect(Box::new(err))),
        Err(_) => Err(LogindError::ConnectTimeout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{PipeReader, Read};
    use std::sync::Mutex;
    use std::time::Instant;

    use zbus::zvariant;

    use crate::tray::test_bus::PrivateBus;

    const WAIT: Duration = Duration::from_secs(5);

    /// What the fake logind was asked, in order.
    type Calls = Arc<Mutex<Vec<String>>>;
    /// The read ends of the pipes handed out by `Inhibit`.
    type Readers = Arc<Mutex<Vec<PipeReader>>>;

    /// A stand-in `org.freedesktop.login1.Manager`: records every call and
    /// its arguments, answers the probes with `can`, fails the power calls
    /// when `deny`, and hands out the write end of a fresh pipe for
    /// `Inhibit`, keeping the read end so a test sees when the client closes
    /// its copy.
    struct FakeManager {
        calls: Calls,
        readers: Readers,
        can: &'static str,
        deny: bool,
    }

    impl FakeManager {
        fn record(&self, call: String) {
            self.calls.lock().expect("calls lock").push(call);
        }

        fn act(&self, call: String) -> zbus::fdo::Result<()> {
            self.record(call);
            if self.deny {
                Err(zbus::fdo::Error::AccessDenied(
                    "not on this seat".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }

    #[zbus::interface(name = "org.freedesktop.login1.Manager")]
    impl FakeManager {
        fn inhibit(
            &self,
            what: &str,
            who: &str,
            why: &str,
            mode: &str,
        ) -> zbus::fdo::Result<zvariant::OwnedFd> {
            self.record(format!("Inhibit {what}|{who}|{why}|{mode}"));
            let (reader, writer) =
                std::io::pipe().map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
            self.readers.lock().expect("readers lock").push(reader);
            Ok(zvariant::OwnedFd::from(OwnedFd::from(writer)))
        }

        fn can_reboot(&self) -> String {
            self.record("CanReboot".to_owned());
            self.can.to_owned()
        }

        fn can_power_off(&self) -> String {
            self.record("CanPowerOff".to_owned());
            self.can.to_owned()
        }

        fn can_suspend(&self) -> String {
            self.record("CanSuspend".to_owned());
            self.can.to_owned()
        }

        fn reboot(&self, interactive: bool) -> zbus::fdo::Result<()> {
            self.act(format!("Reboot {interactive}"))
        }

        fn power_off(&self, interactive: bool) -> zbus::fdo::Result<()> {
            self.act(format!("PowerOff {interactive}"))
        }

        fn suspend(&self, interactive: bool) -> zbus::fdo::Result<()> {
            self.act(format!("Suspend {interactive}"))
        }
    }

    /// How the fake answers.
    struct Fake {
        can: &'static str,
        deny: bool,
    }

    const YES: Option<Fake> = Some(Fake {
        can: "yes",
        deny: false,
    });

    fn answering(can: &'static str) -> Option<Fake> {
        Some(Fake { can, deny: false })
    }

    /// A private bus with (or without) the fake logind, and a worker on it.
    struct Fixture {
        // Field order is drop order: the worker stops before the fake and
        // the bus go away.
        handle: Option<LogindHandle>,
        worker: Option<Worker>,
        events: Receiver<LogindEvent>,
        calls: Calls,
        readers: Readers,
        _server: Option<Connection>,
        _bus: PrivateBus,
    }

    impl Fixture {
        /// `None` (with the bus's notice) where `dbus-daemon` is absent.
        fn start(name: &str, fake: Option<Fake>, locker_available: bool) -> Option<Fixture> {
            let bus = PrivateBus::start(&format!("logind-{name}"))?;
            let calls = Calls::default();
            let readers = Readers::default();
            let server = fake.map(|fake| {
                let manager = FakeManager {
                    calls: Arc::clone(&calls),
                    readers: Arc::clone(&readers),
                    can: fake.can,
                    deny: fake.deny,
                };
                bus.builder()
                    .name(LOGIND_NAME)
                    .expect("the name is valid")
                    .serve_at(LOGIND_PATH, manager)
                    .expect("the path is valid")
                    .build()
                    .expect("the fake logind starts")
            });
            let config = LogindConfig {
                bus: LogindBus::Address(bus.address().to_owned()),
                locker_available,
            };
            let (sent, events) = mpsc::channel();
            // A flag of the test's own: this worker's join stops nothing else.
            let shutdown = Arc::new(AtomicBool::new(false));
            let (handle, worker) = spawn(config, move |event| sent.send(event).is_ok(), &shutdown)
                .expect("the worker starts");
            Some(Fixture {
                handle: Some(handle),
                worker: Some(worker),
                events,
                calls,
                readers,
                _server: server,
                _bus: bus,
            })
        }

        fn handle(&self) -> &LogindHandle {
            self.handle.as_ref().expect("the handle is kept")
        }

        fn next(&self) -> LogindEvent {
            self.events.recv_timeout(WAIT).expect("the worker answers")
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("calls lock").clone()
        }

        fn inhibitors(&self) -> usize {
            self.readers.lock().expect("readers lock").len()
        }

        /// The first inhibitor's read end, as a receiver that fires once
        /// every write end (the client's) is closed.
        fn closed(&self) -> Receiver<()> {
            let mut reader = self.readers.lock().expect("readers lock").remove(0);
            let (done, closed) = mpsc::channel();
            thread::spawn(move || {
                let mut buf = [0u8; 8];
                // Nothing is ever written: the read returns 0 at end of file.
                if matches!(reader.read(&mut buf), Ok(0)) {
                    let _ = done.send(());
                }
            });
            closed
        }
    }

    fn inhibit_call() -> String {
        format!("Inhibit {INHIBIT_WHAT}|{INHIBIT_WHO}|{INHIBIT_WHY}|{INHIBIT_MODE}")
    }

    #[test]
    fn the_inhibitor_is_taken_as_documented_and_closed_on_release() {
        let Some(fixture) = Fixture::start("release", YES, false) else {
            return;
        };
        fixture.handle().caffeine(true).expect("queued");
        assert_eq!(fixture.next(), LogindEvent::Caffeine(Caffeine::Held));
        assert_eq!(fixture.calls(), vec![inhibit_call()]);

        let closed = fixture.closed();
        assert!(
            closed.recv_timeout(Duration::from_millis(300)).is_err(),
            "the descriptor stays open while caffeine is held"
        );

        fixture.handle().caffeine(false).expect("queued");
        assert_eq!(fixture.next(), LogindEvent::Caffeine(Caffeine::Released));
        assert!(closed.recv_timeout(WAIT).is_ok(), "the release closes it");
        assert_eq!(fixture.calls().len(), 1, "releasing calls nothing");
    }

    #[test]
    fn holding_twice_takes_one_inhibitor() {
        let Some(fixture) = Fixture::start("twice", YES, false) else {
            return;
        };
        fixture.handle().caffeine(true).expect("queued");
        fixture.handle().caffeine(true).expect("queued");
        assert_eq!(fixture.next(), LogindEvent::Caffeine(Caffeine::Held));
        assert_eq!(fixture.next(), LogindEvent::Caffeine(Caffeine::Held));
        assert_eq!(fixture.calls(), vec![inhibit_call()]);
        assert_eq!(fixture.inhibitors(), 1);
    }

    #[test]
    fn stopping_the_worker_closes_the_inhibitor_within_a_bounded_join() {
        let Some(mut fixture) = Fixture::start("shutdown", YES, false) else {
            return;
        };
        fixture.handle().caffeine(true).expect("queued");
        assert_eq!(fixture.next(), LogindEvent::Caffeine(Caffeine::Held));
        let closed = fixture.closed();

        let started = Instant::now();
        fixture.worker.take().expect("the worker is kept").join();
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the join is bounded"
        );
        assert!(closed.recv_timeout(WAIT).is_ok(), "shutdown closes it");
    }

    #[test]
    fn dropping_every_handle_stops_the_worker_and_closes_the_inhibitor() {
        let Some(mut fixture) = Fixture::start("dropped", YES, false) else {
            return;
        };
        fixture.handle().caffeine(true).expect("queued");
        assert_eq!(fixture.next(), LogindEvent::Caffeine(Caffeine::Held));
        let closed = fixture.closed();

        drop(fixture.handle.take());
        assert!(closed.recv_timeout(WAIT).is_ok(), "the worker let go");
    }

    #[test]
    fn reboot_and_power_off_are_probed_then_called_without_interaction() {
        let Some(fixture) = Fixture::start("power", YES, false) else {
            return;
        };
        fixture.handle().power(PowerAction::Reboot).expect("queued");
        assert_eq!(
            fixture.next(),
            LogindEvent::Power(PowerAction::Reboot, Outcome::Done)
        );
        fixture
            .handle()
            .power(PowerAction::PowerOff)
            .expect("queued");
        assert_eq!(
            fixture.next(),
            LogindEvent::Power(PowerAction::PowerOff, Outcome::Done)
        );
        assert_eq!(
            fixture.calls(),
            ["CanReboot", "Reboot false", "CanPowerOff", "PowerOff false"]
        );
    }

    #[test]
    fn a_challenge_still_calls_without_interaction() {
        let Some(fixture) = Fixture::start("challenge", answering("challenge"), false) else {
            return;
        };
        fixture.handle().power(PowerAction::Reboot).expect("queued");
        assert_eq!(
            fixture.next(),
            LogindEvent::Power(PowerAction::Reboot, Outcome::Done)
        );
        assert_eq!(fixture.calls(), ["CanReboot", "Reboot false"]);
    }

    #[test]
    fn a_machine_that_cannot_is_unavailable_and_nothing_is_called() {
        let Some(fixture) = Fixture::start("na", answering("na"), false) else {
            return;
        };
        fixture.handle().power(PowerAction::Reboot).expect("queued");
        let event = fixture.next();
        assert!(
            matches!(&event, LogindEvent::Power(PowerAction::Reboot, Outcome::Unavailable(why)) if why.contains("reboot")),
            "{event:?}"
        );
        assert_eq!(fixture.calls(), ["CanReboot"]);
    }

    #[test]
    fn a_probe_of_no_is_refused_and_nothing_is_called() {
        let Some(fixture) = Fixture::start("no", answering("no"), false) else {
            return;
        };
        fixture
            .handle()
            .power(PowerAction::PowerOff)
            .expect("queued");
        let event = fixture.next();
        assert!(
            matches!(
                &event,
                LogindEvent::Power(PowerAction::PowerOff, Outcome::Refused(_))
            ),
            "{event:?}"
        );
        assert_eq!(fixture.calls(), ["CanPowerOff"]);
    }

    #[test]
    fn a_denied_call_fails_with_logind_s_reason() {
        let fake = Some(Fake {
            can: "yes",
            deny: true,
        });
        let Some(fixture) = Fixture::start("denied", fake, false) else {
            return;
        };
        fixture.handle().power(PowerAction::Reboot).expect("queued");
        let event = fixture.next();
        assert!(
            matches!(&event, LogindEvent::Power(PowerAction::Reboot, Outcome::Failed(why)) if why.contains("not on this seat")),
            "{event:?}"
        );
        assert_eq!(fixture.calls(), ["CanReboot", "Reboot false"]);
    }

    #[test]
    fn suspend_is_refused_without_a_locker_and_sent_with_one() {
        let Some(fixture) = Fixture::start("nolocker", YES, false) else {
            return;
        };
        fixture
            .handle()
            .power(PowerAction::Suspend)
            .expect("queued");
        assert_eq!(
            fixture.next(),
            LogindEvent::Power(PowerAction::Suspend, Outcome::Refused(NO_LOCKER.to_owned()))
        );
        assert!(fixture.calls().is_empty(), "nothing reached logind");

        let Some(fixture) = Fixture::start("locker", YES, true) else {
            return;
        };
        fixture
            .handle()
            .power(PowerAction::Suspend)
            .expect("queued");
        assert_eq!(
            fixture.next(),
            LogindEvent::Power(PowerAction::Suspend, Outcome::Done)
        );
        assert_eq!(fixture.calls(), ["CanSuspend", "Suspend false"]);
    }

    #[test]
    fn log_out_is_refused_here_and_never_reaches_logind() {
        let Some(fixture) = Fixture::start("logout", YES, true) else {
            return;
        };
        fixture.handle().power(PowerAction::LogOut).expect("queued");
        assert_eq!(
            fixture.next(),
            LogindEvent::Power(
                PowerAction::LogOut,
                Outcome::Refused(LOG_OUT_ELSEWHERE.to_owned())
            )
        );
        assert!(fixture.calls().is_empty());
    }

    #[test]
    fn a_missing_service_is_unavailable() {
        let Some(fixture) = Fixture::start("missing", None, true) else {
            return;
        };
        fixture.handle().caffeine(true).expect("queued");
        let event = fixture.next();
        assert!(
            matches!(&event, LogindEvent::Caffeine(Caffeine::Unavailable(_))),
            "{event:?}"
        );
        for action in [
            PowerAction::Reboot,
            PowerAction::PowerOff,
            PowerAction::Suspend,
        ] {
            fixture.handle().power(action).expect("queued");
            let event = fixture.next();
            assert!(
                matches!(&event, LogindEvent::Power(a, Outcome::Unavailable(_)) if *a == action),
                "{event:?}"
            );
        }
    }

    #[test]
    fn a_missing_bus_is_unavailable() {
        let absent =
            std::env::temp_dir().join(format!("celestina-logind-nobus-{}/bus", std::process::id()));
        let config = LogindConfig {
            bus: LogindBus::Address(format!("unix:path={}", absent.display())),
            locker_available: false,
        };
        let (sent, events) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (handle, worker) =
            spawn(config, move |event| sent.send(event).is_ok(), &shutdown).expect("starts");
        handle.caffeine(true).expect("queued");
        let event = events.recv_timeout(WAIT);
        assert!(
            matches!(&event, Ok(LogindEvent::Caffeine(Caffeine::Unavailable(_)))),
            "{event:?}"
        );
        handle.power(PowerAction::Reboot).expect("queued");
        let event = events.recv_timeout(WAIT);
        assert!(
            matches!(
                &event,
                Ok(LogindEvent::Power(
                    PowerAction::Reboot,
                    Outcome::Unavailable(_)
                ))
            ),
            "{event:?}"
        );
        worker.join();
    }

    #[test]
    fn the_queue_is_bounded_and_refuses_when_full_or_stopped() {
        let (handle, queue) = LogindHandle::channel();
        for _ in 0..QUEUE {
            handle.caffeine(true).expect("room in the queue");
        }
        assert_eq!(handle.power(PowerAction::Reboot), Err(RequestError::Full));
        drop(queue);
        assert_eq!(handle.caffeine(false), Err(RequestError::Stopped));
    }

    #[test]
    fn a_long_reason_is_bounded_on_a_character_boundary() {
        let cut = bounded(&"\u{e9}".repeat(MAX_REASON));
        assert!(cut.len() <= MAX_REASON + '…'.len_utf8());
        assert!(cut.ends_with('…'));
        assert_eq!(bounded("short"), "short");
    }
}
