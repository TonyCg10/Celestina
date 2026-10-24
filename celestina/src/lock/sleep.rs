//! The `lock-sleep` worker: the logind delay inhibitor and the
//! `PrepareForSleep` signal (CEL-2-H design §4.4, §4.9, §4.10; spike Q10 and
//! Q17).
//!
//! The worker owns a connection to the system bus of its own (the caffeine
//! worker's is a different one) and does two things for the policy:
//!
//! * It takes and releases `Inhibit("sleep", …, "delay")` when the policy
//!   says so. The file descriptor logind returns *is* the lock: closing it is
//!   the release, so the worker keeps it in a field and the shell's exit, the
//!   worker's end and a panic all release it by `Drop`.
//! * It reports `PrepareForSleep` as an [`Event`].
//!
//! The policy never asks for an inhibitor while a sleep is being prepared,
//! because logind refuses one then (`OperationInProgress`, Q17). A request
//! that raced a sleep anyway gets that answer: it is not a bus failure, it is
//! reported as [`InhibitorFailure::SleepInProgress`] and logged quietly.
//!
//! **Orders are state, wake-ups are not.** The policy's `hold` and `release`
//! set a desired state in one atomic word (a sequence number and the wish)
//! and then poke the worker; the poke channel holds one token and a poke that
//! finds it full is dropped, because the worker reads the atomic and not the
//! channel. A burst of bus signals can therefore never push an order out
//! (finding I3). The signal stream is polled by hand with a waker that pokes
//! the same channel (spike Q10: dropping connection clones never ends a
//! blocking iterator, a hand-polled stream is ended by the stop path poking
//! it, in well under a millisecond even with a hung bus). At rest the worker
//! blocks in `recv`: no periodic wake-up. Only a worker without a bus wakes,
//! every [`RECONNECT`], to try again.
//!
//! **What the worker believes about the bus** (finding C2):
//!
//! * A `PrepareForSleep` counts only when its sender is the current owner of
//!   `org.freedesktop.login1` (`GetNameOwner`, asked whenever a candidate
//!   arrives; a unicast signal from another process of the user reaches the
//!   stream whatever the match rule says).
//! * The `PreparingForSleep` property is the truth the signals only report.
//!   It is read and published after subscribing, after every reconnect and
//!   after every order, so a lost signal cannot leave the policy preparing
//!   for good and the policy never has to guess.
//! * Every (re)connection is announced with [`Event::SleepBusBack`], so an
//!   inhibitor that failed while the bus was away is asked for again
//!   (finding I4).
#![forbid(unsafe_code)]

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use celestina_shell::lock::{Event, InhibitorFailure};
use rustix::io::{fcntl_setfd, FdFlags};
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection, MessageIterator};
use zbus::export::futures_core::Stream;
use zbus::message::Type as MessageType;
use zbus::names::BusName;
use zbus::{MatchRule, MessageStream};

use crate::energy::logind::{connect, LogindBus, LOGIND_NAME, LOGIND_PATH, MANAGER_INTERFACE};
use crate::providers::Worker;

/// What the inhibitor blocks: sleeping, delayed not forbidden.
pub const INHIBIT_WHAT: &str = "sleep";
/// Who the inhibitor is reported as.
pub const INHIBIT_WHO: &str = "Celestina";
/// Why, as `systemd-inhibit --list` shows it.
pub const INHIBIT_WHY: &str = "The session is locked before the machine sleeps";
/// `delay`: logind waits for it (at most `InhibitDelayMaxUSec`) and never
/// keeps the machine awake, so a lock that does not come cannot wedge sleep.
pub const INHIBIT_MODE: &str = "delay";
/// logind's error for a delay inhibitor requested during a pending sleep.
pub const OPERATION_IN_PROGRESS: &str = "org.freedesktop.login1.OperationInProgress";
/// The signal.
pub const PREPARE_FOR_SLEEP: &str = "PrepareForSleep";
/// The property that tells whether a sleep is being prepared.
pub const PREPARING_PROPERTY: &str = "PreparingForSleep";
/// The interface of the property getter.
pub const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
/// How often a worker without a bus tries again.
pub const RECONNECT: Duration = Duration::from_secs(5);
/// How often the property is read again while the last value published to the
/// policy is `true`: a lost `false` cannot leave the policy preparing for good
/// (finding I-C). The timer exists only in that state.
pub const PREPARING_RECHECK: Duration = Duration::from_secs(2);
/// The longest wait between two tries at a property read that keeps failing.
pub const RETRY_CAP: Duration = Duration::from_secs(8);
/// How long stopping the worker waits for it.
pub const JOIN_WAIT: Duration = Duration::from_secs(2);

/// Why the worker could not be told.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// The worker has stopped.
    #[error("the sleep worker has stopped")]
    Stopped,
}

/// The policy's wish about the inhibitor, as one word: the number of orders
/// given so far in the high bits and whether the last one was `hold` in the
/// lowest. A wish is never lost and never queued behind anything.
#[derive(Debug, Default)]
struct Orders(AtomicU64);

impl Orders {
    fn give(&self, hold: bool) {
        // The sequence counts orders; it wraps after 2^63 of them.
        let mut current = self.0.load(Ordering::Acquire);
        loop {
            let next = ((current >> 1).wrapping_add(1) << 1) | u64::from(hold);
            match self
                .0
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return,
                Err(seen) => current = seen,
            }
        }
    }

    /// (sequence, wish).
    fn read(&self) -> (u64, bool) {
        let word = self.0.load(Ordering::Acquire);
        (word >> 1, word & 1 == 1)
    }
}

/// A running sleep worker and the way into it.
#[derive(Debug)]
pub struct Sleep {
    tx: SyncSender<()>,
    orders: Arc<Orders>,
    stop: Arc<AtomicBool>,
    worker: Worker,
}

impl Sleep {
    /// Asks for the delay inhibitor. Never fails for want of room.
    ///
    /// # Errors
    ///
    /// When the worker has stopped.
    pub fn hold(&self) -> Result<(), RequestError> {
        self.orders.give(true);
        self.poke()
    }

    /// Asks for the delay inhibitor to be let go. Never fails for want of
    /// room.
    ///
    /// # Errors
    ///
    /// When the worker has stopped.
    pub fn release(&self) -> Result<(), RequestError> {
        self.orders.give(false);
        self.poke()
    }

    fn poke(&self) -> Result<(), RequestError> {
        poke(&self.tx)
    }

    /// Stops the worker, which closes the inhibitor with it, and waits for
    /// it, at most [`JOIN_WAIT`].
    pub fn stop(self) {
        let Sleep {
            tx, stop, worker, ..
        } = self;
        stop.store(true, Ordering::Release);
        // A full channel is awake already; a gone worker needs no wake-up.
        let _ = tx.try_send(());
        drop(tx);
        worker.join_within(JOIN_WAIT);
    }
}

/// Wakes the worker. A token already waiting is a wake-up pending: the worker
/// reads the order word, not the channel, so a full channel is success.
fn poke(tx: &SyncSender<()>) -> Result<(), RequestError> {
    match tx.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => Ok(()),
        Err(TrySendError::Disconnected(())) => Err(RequestError::Stopped),
    }
}

/// Starts the worker. `publish` receives every [`Event`] and returns `false`
/// once the loop is gone. Nothing is asked of the bus until the worker has
/// connected; the connection and its subscription are made on the worker's
/// own thread.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(
    bus: LogindBus,
    publish: impl Fn(Event) -> bool + Send + 'static,
) -> io::Result<Sleep> {
    let (tx, rx) = mpsc::sync_channel(1);
    let orders = Arc::new(Orders::default());
    let stop = Arc::new(AtomicBool::new(false));
    let worker = {
        let flag = Arc::clone(&stop);
        let waker_tx = tx.clone();
        let wishes = Arc::clone(&orders);
        Worker::spawn("lock-sleep", &stop, move || {
            run(&bus, &publish, &rx, &wishes, waker_tx, &flag);
        })?
    };
    Ok(Sleep {
        tx,
        orders,
        stop,
        worker,
    })
}

/// A waker that posts a wake-up into the worker's own queue.
struct QueueWaker(SyncSender<()>);

impl Wake for QueueWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // A full queue is awake already and drains the stream afterwards.
        let _ = self.0.try_send(());
    }
}

/// The connection and the stream of `PrepareForSleep` signals on it.
struct Session {
    connection: Connection,
    stream: MessageStream,
}

fn subscribe(connection: &Connection) -> zbus::Result<MessageStream> {
    // The stream exists before the rule, so nothing the rule matches is
    // missed between them.
    let stream = MessageIterator::from(connection).into_inner();
    let dbus = DBusProxy::new(connection)?;
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(LOGIND_NAME)?
        .path(LOGIND_PATH)?
        .interface(MANAGER_INTERFACE)?
        .member(PREPARE_FOR_SLEEP)?
        .build();
    dbus.add_match_rule(rule)?;
    Ok(stream)
}

fn open(bus: &LogindBus) -> Result<Session, String> {
    let connection = connect(bus).map_err(|err| crate::error_chain(&err))?;
    let stream = subscribe(&connection).map_err(|err| crate::error_chain(&err))?;
    Ok(Session { connection, stream })
}

/// Whether `error` is logind refusing a delay inhibitor during a pending
/// sleep.
fn sleep_in_progress(error: &zbus::Error) -> bool {
    match error {
        zbus::Error::MethodError(name, _, _) => name.as_str() == OPERATION_IN_PROGRESS,
        _ => false,
    }
}

/// The `Inhibit` call.
fn inhibit(connection: &Connection) -> Result<OwnedFd, InhibitorFailure> {
    let reply = connection
        .call_method(
            Some(LOGIND_NAME),
            LOGIND_PATH,
            Some(MANAGER_INTERFACE),
            "Inhibit",
            &(INHIBIT_WHAT, INHIBIT_WHO, INHIBIT_WHY, INHIBIT_MODE),
        )
        .map_err(|error| {
            if sleep_in_progress(&error) {
                InhibitorFailure::SleepInProgress
            } else {
                tracing::warn!(error = %crate::error_chain(&error), "lock: the sleep inhibitor was not granted");
                InhibitorFailure::Bus
            }
        })?;
    // The reply message holds the descriptor exactly as `recvmsg` delivered
    // it: **without** close-on-exec (zbus passes no `MSG_CMSG_CLOEXEC`). The
    // stream the worker subscribed with receives every message, this reply
    // included, and keeps it until the next `drain`, which runs after the
    // synchronous property read that follows an order. A `spawn` of the lock
    // inside that window would inherit the inhibitor (round 5, finding I-2).
    // The descriptor is therefore borrowed from the message, marked
    // close-on-exec in place (the message and its copies share it), and only
    // then cloned: the clone is close-on-exec too.
    //
    // **Residual X3, narrowed and not removed (round 6, I-B).** The window
    // is now only the span between zbus's `recvmsg` (which delivers the
    // descriptor inheritable) and the `fcntl` in `own_cloexec`. A child
    // spawned by another subsystem of the shell inside that span (a launched
    // application) can still inherit a copy. The lock and its verifier cure
    // themselves (each closes every descriptor it was not handed, first
    // thing it does), so for them the leak lasts microseconds. The same
    // pattern exists in `energy/logind.rs` (the caffeine inhibitor, CEL-2-B),
    // to be handled in its own unit.
    let body = reply.body();
    let borrowed: zbus::zvariant::Fd<'_> = body.deserialize().map_err(|error| {
        tracing::warn!(%error, "lock: logind answered Inhibit with an unexpected value");
        InhibitorFailure::Bus
    })?;
    own_cloexec(borrowed.as_fd())
}

/// A close-on-exec duplicate of `fd`, whose original is made close-on-exec
/// first.
fn own_cloexec(fd: BorrowedFd<'_>) -> Result<OwnedFd, InhibitorFailure> {
    fcntl_setfd(fd, FdFlags::CLOEXEC)
        .map_err(io::Error::from)
        .and_then(|()| fd.try_clone_to_owned())
        .map_err(|error| {
            tracing::warn!(%error, "lock: cannot secure the sleep inhibitor's descriptor");
            InhibitorFailure::Bus
        })
}

/// The sleep a `PrepareForSleep` signal announces (`true`) or ends, if
/// `message` is that signal. The sender is checked apart ([`from_logind`]).
fn prepare_for_sleep(message: &zbus::message::Message) -> Option<bool> {
    let header = message.header();
    if header.message_type() != MessageType::Signal
        || header.member().map(|m| m.as_str()) != Some(PREPARE_FOR_SLEEP)
        || header.interface().map(|i| i.as_str()) != Some(MANAGER_INTERFACE)
        || header.path().map(|p| p.as_str()) != Some(LOGIND_PATH)
    {
        return None;
    }
    message.body().deserialize::<bool>().ok()
}

/// The unique name that owns logind's well-known name now, if any. The bus
/// fills a message's sender in, so a process cannot claim another's; the owner
/// is asked afresh because logind may have restarted.
fn logind_owner(connection: &Connection) -> Option<String> {
    let dbus = DBusProxy::new(connection).ok()?;
    let name = BusName::try_from(LOGIND_NAME).ok()?;
    dbus.get_name_owner(name)
        .ok()
        .map(|owner| owner.as_str().to_owned())
}

/// logind's own account of whether a sleep is being prepared, or why it could
/// not be read. The caller decides what to log ([`ReadStreak`]).
fn preparing_property(connection: &Connection) -> Result<bool, String> {
    let reply = connection
        .call_method(
            Some(LOGIND_NAME),
            LOGIND_PATH,
            Some(PROPERTIES_INTERFACE),
            "Get",
            &(MANAGER_INTERFACE, PREPARING_PROPERTY),
        )
        .map_err(|error| crate::error_chain(&error))?;
    let value: zbus::zvariant::OwnedValue = reply
        .body()
        .deserialize()
        .map_err(|error| error.to_string())?;
    bool::try_from(value).map_err(|error| error.to_string())
}

/// How many reads of the property failed in a row (round 4, m-3 residual).
///
/// A read that keeps failing with the bus alive (logind wedged, or refusing
/// the call) is retried, because dropping it would lose a `true`; but it is
/// logged once per streak and retried ever more slowly, [`PREPARING_RECHECK`]
/// then twice and four times that, so a logind that stays broken does not fill
/// the journal or the bus with a request every two seconds.
#[derive(Debug, Default, PartialEq, Eq)]
struct ReadStreak(u32);

impl ReadStreak {
    /// Notes a failure. `true` for the first of a streak: the only one logged
    /// above debug level.
    fn failed(&mut self) -> bool {
        self.0 = self.0.saturating_add(1);
        self.0 == 1
    }

    /// Notes a success. `true` if it ended a streak.
    fn recovered(&mut self) -> bool {
        std::mem::take(&mut self.0) > 0
    }

    /// How long to wait before the next try: the base period after the first
    /// failure, doubling to [`RETRY_CAP`].
    fn retry_after(&self) -> Duration {
        let doublings = self.0.saturating_sub(1).min(2);
        (PREPARING_RECHECK * 2u32.pow(doublings)).min(RETRY_CAP)
    }
}

/// What the worker holds between messages.
struct State {
    session: Option<Session>,
    /// The inhibitor: closing the descriptor is the release.
    inhibitor: Option<OwnedFd>,
    /// The property is to be read and published once the bus is drained.
    resync: bool,
    /// The last order acted on.
    seen: u64,
    /// The last `PrepareForSleep` value published to the policy. While it is
    /// `true` the property is read again every [`PREPARING_RECHECK`].
    last_preparing: bool,
    /// The reads of the property that failed in a row.
    reads: ReadStreak,
    /// Not before this instant is a failed read tried again, whatever wakes
    /// the worker meanwhile (the reply to the failed call itself wakes it).
    /// A new reason to look ends the wait: an order, a candidate signal, a
    /// new connection.
    retry_at: Option<Instant>,
}

impl State {
    /// The time left before a failed read may be tried again.
    fn retry_in(&self) -> Option<Duration> {
        self.retry_at
            .map(|at| at.saturating_duration_since(Instant::now()))
    }

    /// Something new happened that is worth reading the property for: do not
    /// wait out the backoff.
    fn look_now(&mut self) {
        self.resync = true;
        self.retry_at = None;
    }
}

impl State {
    /// Connects if there is no session. `Ok(true)` when a session was just
    /// made (and announced); `Err(())` when the loop is gone.
    fn connect(
        &mut self,
        bus: &LogindBus,
        publish: &(dyn Fn(Event) -> bool + Send),
    ) -> Result<bool, ()> {
        if self.session.is_some() {
            return Ok(false);
        }
        match open(bus) {
            Ok(session) => {
                self.session = Some(session);
                // A new connection is a new account of things.
                self.look_now();
                self.reads = ReadStreak::default();
                if publish(Event::SleepBusBack) {
                    Ok(true)
                } else {
                    Err(())
                }
            }
            Err(why) => {
                tracing::warn!("lock: no PrepareForSleep subscription: {why}");
                Ok(false)
            }
        }
    }
}

fn run(
    bus: &LogindBus,
    publish: &(dyn Fn(Event) -> bool + Send),
    rx: &Receiver<()>,
    orders: &Orders,
    waker_tx: SyncSender<()>,
    stop: &AtomicBool,
) {
    let waker = Waker::from(Arc::new(QueueWaker(waker_tx)));
    let mut cx = Context::from_waker(&waker);
    let mut state = State {
        session: None,
        inhibitor: None,
        resync: false,
        seen: 0,
        last_preparing: false,
        reads: ReadStreak::default(),
        retry_at: None,
    };
    if state.connect(bus, publish).is_err() {
        return;
    }
    while !stop.load(Ordering::Acquire) {
        // First whatever the bus has and the policy wished, then the truth,
        // then sleep until something changes.
        if !pump(&mut state, &mut cx, orders, bus, publish) {
            return;
        }
        if state.session.is_none() {
            // Degraded: no bus. Try again at the next order or tick.
            match rx.recv_timeout(RECONNECT) {
                Ok(()) => {}
                Err(RecvTimeoutError::Timeout) => {
                    if state.connect(bus, publish).is_err() {
                        return;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else if let Some(bound) = wake_bound(state.last_preparing, state.resync, state.retry_in())
        {
            // The policy believes a sleep is being prepared, or a read of the
            // property failed and is still pending: look at the truth now and
            // then, so a lost `false` cannot leave it so for good (finding
            // I-C) and a lost `true` cannot leave the machine sleeping
            // uncovered (finding m-3). Only in these states does the worker
            // wake by itself.
            match rx.recv_timeout(bound) {
                Ok(()) => {}
                Err(RecvTimeoutError::Timeout) => state.resync = true,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        } else if rx.recv().is_err() {
            return;
        }
    }
}

/// One pass over everything that may have changed. `false` when the worker
/// must end (the loop is gone).
fn pump(
    state: &mut State,
    cx: &mut Context<'_>,
    orders: &Orders,
    bus: &LogindBus,
    publish: &(dyn Fn(Event) -> bool + Send),
) -> bool {
    // The stream first: zbus stops reading the socket while its queue is
    // full, and a blocking call (the owner lookup, `Inhibit`) would then wait
    // for a reply that cannot be read until the queue is emptied.
    if let Some(open_session) = state.session.as_mut() {
        let (heard, ended) = drain(open_session, cx);
        // Any pass with candidates ends by reading the property, whether or
        // not they could be vouched for (finding I-C).
        if needs_resync(&heard) {
            state.resync = true;
            state.retry_at = None;
        }
        match announce(open_session, &heard, publish) {
            Err(()) => return false,
            Ok(Some(last)) => state.last_preparing = last,
            Ok(None) => {}
        }
        if ended {
            tracing::warn!("lock: the system bus connection ended");
            state.session = None;
        }
    }
    if !apply_order(state, orders, bus, publish) {
        return false;
    }
    if let Some(open_session) = state.session.as_ref() {
        let due = state.retry_in().is_none_or(|left| left.is_zero());
        match resync_step(
            &mut state.resync,
            &mut state.last_preparing,
            &mut state.reads,
            due,
            || preparing_property(&open_session.connection),
        ) {
            Read::Published(preparing) => {
                state.retry_at = None;
                return publish(Event::PrepareForSleep(preparing));
            }
            Read::Failed => state.retry_at = Some(Instant::now() + state.reads.retry_after()),
            Read::Idle => {}
        }
    }
    true
}

/// What a pass made of the pending read of the property.
#[derive(Debug, PartialEq, Eq)]
enum Read {
    /// Nothing was owed, or a failed read is not due yet.
    Idle,
    /// The read failed and stays owed.
    Failed,
    /// logind's answer, to be published.
    Published(bool),
}

/// Reads the property when a read is pending and due. The pending mark is
/// cleared only by a read that succeeded: if the owner lookup failed and so
/// does this read, nothing was learned, and dropping the mark would lose a
/// `true` for good (finding m-3); the worker then wakes by itself
/// ([`wake_bound`]) and reads again, less often the longer it fails.
///
/// `due` is false while a failed read is backing off. The reply to the failed
/// call wakes the worker like any message of its stream does, so without it
/// the worker read again at once, and again: a failing logind was asked for
/// the property thousands of times a second (round 4).
fn resync_step(
    resync: &mut bool,
    last_preparing: &mut bool,
    reads: &mut ReadStreak,
    due: bool,
    read: impl FnOnce() -> Result<bool, String>,
) -> Read {
    if !*resync || !due {
        return Read::Idle;
    }
    match read() {
        Ok(preparing) => {
            if reads.recovered() {
                tracing::info!("lock: PreparingForSleep can be read again");
            }
            *resync = false;
            *last_preparing = preparing;
            Read::Published(preparing)
        }
        Err(why) => {
            if reads.failed() {
                tracing::warn!(%why, "lock: cannot read PreparingForSleep; trying again, less often");
            } else {
                tracing::debug!(%why, "lock: still cannot read PreparingForSleep");
            }
            Read::Failed
        }
    }
}

/// How long the worker may sleep with nothing to do: `None` is until the
/// policy sends a wish. It wakes by itself only while a sleep is believed to
/// be in preparation or a read of the property is still pending; a read that
/// keeps failing waits out `retry_in`, the time left of its backoff
/// ([`ReadStreak`]).
fn wake_bound(last_preparing: bool, resync: bool, retry_in: Option<Duration>) -> Option<Duration> {
    if resync {
        if let Some(left) = retry_in {
            return Some(left);
        }
    }
    (last_preparing || resync).then_some(PREPARING_RECHECK)
}

/// Acts on the policy's latest wish if it changed since the last look.
/// `false` when the loop is gone.
fn apply_order(
    state: &mut State,
    orders: &Orders,
    bus: &LogindBus,
    publish: &(dyn Fn(Event) -> bool + Send),
) -> bool {
    let (sequence, hold) = orders.read();
    if sequence == state.seen {
        return true;
    }
    state.seen = sequence;
    // An order is a moment to look at the truth, whichever way it goes.
    state.look_now();
    if !hold {
        if state.inhibitor.take().is_some() {
            tracing::info!("lock: the sleep inhibitor was released");
        }
        return true;
    }
    if state.inhibitor.is_some() {
        return true;
    }
    if state.connect(bus, publish).is_err() {
        return false;
    }
    let Some(open_session) = state.session.as_ref() else {
        return publish(Event::InhibitorFailed(InhibitorFailure::Bus));
    };
    match inhibit(&open_session.connection) {
        Ok(fd) => {
            state.inhibitor = Some(fd);
            tracing::info!("lock: the sleep inhibitor is held");
            true
        }
        Err(failure) => publish(Event::InhibitorFailed(failure)),
    }
}

/// A `PrepareForSleep` heard on the stream and who sent it.
struct Heard {
    preparing: bool,
    sender: Option<String>,
}

/// The most announcements kept from one pass.
const MAX_HEARD: usize = 64;
/// The most of them kept from one sender.
const MAX_PER_SENDER: usize = 8;

/// Keeps `one`, bounded: a sender keeps its latest [`MAX_PER_SENDER`] and the
/// pass its latest [`MAX_HEARD`], so a flood from one process (a unicast
/// signal reaches the stream whatever the match rule says) cannot push out
/// what logind sent, which is only known to be genuine later (finding M-a).
///
/// The per-sender cap protects against one connection, not against several:
/// an attacker of the same user (residual X2) with eight or more connections
/// fills the pass's cap of [`MAX_HEARD`] and can still push logind's
/// announcement out of it. The net that holds then is [`needs_resync`]: any
/// pass with candidates ends by reading the property, so what was dropped is
/// settled by logind's own account (finding I-C), not by this cap.
fn remember(heard: &mut Vec<Heard>, one: Heard) {
    let sender = one.sender.clone();
    heard.push(one);
    let mine = heard.iter().filter(|h| h.sender == sender).count();
    if mine > MAX_PER_SENDER {
        if let Some(at) = heard.iter().position(|h| h.sender == sender) {
            heard.remove(at);
        }
    }
    if heard.len() > MAX_HEARD {
        heard.remove(0);
    }
}

/// Whether the property is to be read after this pass: always when there
/// were candidates. Whatever the pass could not vouch for (the owner lookup
/// failed, a forged sender, a dropped excess) is then settled by logind's own
/// account rather than lost.
fn needs_resync(heard: &[Heard]) -> bool {
    !heard.is_empty()
}

/// The values of the announcements sent by `owner`, in order.
fn genuine_values(owner: Option<&str>, heard: &[Heard]) -> Vec<bool> {
    heard
        .iter()
        .filter(
            |one| matches!((owner, &one.sender), (Some(owner), Some(sender)) if owner == sender),
        )
        .map(|one| one.preparing)
        .collect()
}

/// Everything ready on the session's stream, without blocking and without any
/// call; and whether the stream has ended.
fn drain(session: &mut Session, cx: &mut Context<'_>) -> (Vec<Heard>, bool) {
    let mut heard = Vec::new();
    loop {
        match Pin::new(&mut session.stream).poll_next(cx) {
            Poll::Ready(Some(Ok(message))) => {
                if let Some(preparing) = prepare_for_sleep(&message) {
                    remember(
                        &mut heard,
                        Heard {
                            preparing,
                            sender: message.header().sender().map(ToString::to_string),
                        },
                    );
                }
            }
            Poll::Ready(Some(Err(error))) => {
                tracing::debug!(%error, "lock: the message stream failed");
            }
            Poll::Ready(None) => return (heard, true),
            Poll::Pending => return (heard, false),
        }
    }
}

/// Publishes what logind itself announced and drops the rest. The last value
/// published, if any; `Err(())` when the loop is gone.
fn announce(
    session: &Session,
    heard: &[Heard],
    publish: &(dyn Fn(Event) -> bool + Send),
) -> Result<Option<bool>, ()> {
    if heard.is_empty() {
        return Ok(None);
    }
    let owner = logind_owner(&session.connection);
    if owner.is_none() {
        tracing::warn!("lock: logind's owner is unknown: the announcements wait for the property");
    }
    let values = genuine_values(owner.as_deref(), heard);
    if values.len() < heard.len() {
        tracing::warn!("lock: a PrepareForSleep not sent by logind was ignored");
    }
    let mut last = None;
    for value in values {
        if !publish(Event::PrepareForSleep(value)) {
            return Err(());
        }
        last = Some(value);
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(preparing: bool, sender: &str) -> Heard {
        Heard {
            preparing,
            sender: Some(sender.to_owned()),
        }
    }

    #[test]
    fn a_flood_of_forged_signals_cannot_push_out_the_one_from_logind() {
        // Finding M-a: the cap used to be applied before the sender was
        // known, so enough forgeries pushed logind's announcement out.
        let mut batch = Vec::new();
        for _ in 0..1000 {
            remember(&mut batch, heard(true, ":1.99"));
        }
        remember(&mut batch, heard(false, ":1.5"));
        for _ in 0..1000 {
            remember(&mut batch, heard(true, ":1.99"));
        }
        assert!(batch.len() <= MAX_HEARD, "bounded: {}", batch.len());
        let genuine = genuine_values(Some(":1.5"), &batch);
        assert_eq!(genuine, [false], "logind's announcement survived");
    }

    #[test]
    fn a_failed_read_keeps_the_resync_pending_and_the_worker_wakes_to_retry() {
        // Finding m-3: the owner lookup failed and so did the property read;
        // the `true` was never learned. The mark used to be cleared before
        // the read, and with `last_preparing` still false the worker slept on
        // `recv()` forever: the inhibitor was held until logind's delay ran
        // out and the machine slept uncovered.
        let (mut resync, mut last, mut streak) = (true, false, ReadStreak::default());
        assert_eq!(
            resync_step(
                &mut resync,
                &mut last,
                &mut streak,
                true,
                || Err("x".into())
            ),
            Read::Failed
        );
        assert!(resync, "nothing was learned, so the read is still owed");
        assert_eq!(
            wake_bound(last, resync, None),
            Some(PREPARING_RECHECK),
            "and the worker does not sleep until the next wish"
        );
        // The retry succeeds: published, remembered, and the mark cleared.
        assert_eq!(
            resync_step(&mut resync, &mut last, &mut streak, true, || Ok(true)),
            Read::Published(true)
        );
        assert!(!resync && last);
        assert_eq!(streak, ReadStreak::default(), "the streak ended");
        // Nothing pending and nothing preparing: no wake of its own.
        assert_eq!(wake_bound(false, false, None), None);
        // No read is attempted without the mark.
        let (mut resync, mut last, mut streak) = (false, false, ReadStreak::default());
        assert_eq!(
            resync_step(&mut resync, &mut last, &mut streak, true, || panic!(
                "no read without a mark"
            )),
            Read::Idle
        );
    }

    #[test]
    fn a_failed_read_is_not_tried_again_before_it_is_due() {
        // Round 4: the reply to the failed call woke the worker, which read
        // again at once, thousands of times a second.
        let (mut resync, mut last, mut streak) = (true, false, ReadStreak::default());
        assert_eq!(
            resync_step(&mut resync, &mut last, &mut streak, false, || panic!(
                "not due"
            )),
            Read::Idle
        );
        assert!(resync, "and still owed");
        // While it backs off the worker sleeps exactly what is left.
        assert_eq!(
            wake_bound(false, true, Some(Duration::from_millis(1300))),
            Some(Duration::from_millis(1300))
        );
        // A backoff without a read owed is nothing to wait for.
        assert_eq!(wake_bound(false, false, Some(Duration::from_secs(5))), None);
    }

    #[test]
    fn a_persistent_failure_is_logged_once_and_retried_ever_more_slowly() {
        // Round 4, m-3 residual: logind failing for good with the bus alive
        // was read again at every wake-up with a `warn` each time.
        let mut streak = ReadStreak::default();
        let mut logged = Vec::new();
        let mut waits = Vec::new();
        for _ in 0..6 {
            logged.push(streak.failed());
            waits.push(streak.retry_after().as_secs());
        }
        assert_eq!(logged, [true, false, false, false, false, false]);
        assert_eq!(waits, [2, 4, 8, 8, 8, 8], "2, 4, 8 and then 8 s");
        // A success ends the streak: the next failure is news again and the
        // wait starts over.
        assert!(streak.recovered(), "there was a streak to end");
        assert!(!streak.recovered(), "and only one");
        assert!(streak.failed(), "a new streak is logged again");
        assert_eq!(streak.retry_after(), PREPARING_RECHECK);
        // Healthy and preparing: the plain recheck.
        assert_eq!(wake_bound(true, false, None), Some(PREPARING_RECHECK));
    }

    #[test]
    fn a_batch_that_cannot_be_vouched_for_asks_for_the_property() {
        // Finding I-C: no owner (the lookup timed out), a forged sender, or
        // simply a batch with candidates: the property is read afterwards.
        let batch = [heard(false, ":1.5")];
        assert!(
            genuine_values(None, &batch).is_empty(),
            "no owner, no trust"
        );
        assert!(needs_resync(&batch), "and the truth is read instead");
        let forged = [heard(true, ":1.99")];
        assert!(genuine_values(Some(":1.5"), &forged).is_empty());
        assert!(needs_resync(&forged));
        assert!(!needs_resync(&[]), "an empty pass reads nothing");
    }

    #[test]
    fn the_last_wish_survives_any_number_of_unread_orders_and_pokes() {
        // Finding I3: with a queue of eight a ninth request failed. Here
        // nothing reads, and every order is accepted and the last one wins.
        let (tx, rx) = mpsc::sync_channel::<()>(1);
        let orders = Orders::default();
        assert_eq!(orders.read(), (0, false));
        for round in 0..1000u64 {
            let hold = round % 2 == 0;
            orders.give(hold);
            assert_eq!(poke(&tx), Ok(()), "round {round}");
            assert_eq!(orders.read(), (round + 1, hold));
        }
        // Bus wake-ups landing in the same channel change nothing either.
        assert_eq!(poke(&tx), Ok(()));
        assert_eq!(rx.try_recv(), Ok(()));
        assert!(rx.try_recv().is_err(), "one token stands for all the pokes");
        drop(rx);
        assert_eq!(poke(&tx), Err(RequestError::Stopped));
    }

    /// Whether any descriptor of this process that is **not** close-on-exec
    /// is the read or write end of the pipe with this inode.
    fn inheritable_copies_of(pipe_inode: &str) -> Vec<String> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir("/proc/self/fd")
            .expect("fd dir")
            .flatten()
        {
            let number = entry.file_name().to_string_lossy().into_owned();
            let Ok(target) = std::fs::read_link(entry.path()) else {
                continue;
            };
            if target.to_string_lossy() != format!("pipe:[{pipe_inode}]") {
                continue;
            }
            let info =
                std::fs::read_to_string(format!("/proc/self/fdinfo/{number}")).unwrap_or_default();
            let flags = info
                .lines()
                .find_map(|l| l.strip_prefix("flags:"))
                .and_then(|v| u32::from_str_radix(v.trim(), 8).ok())
                .unwrap_or(0);
            // O_CLOEXEC is 0o2000000.
            if flags & 0o200_0000 == 0 {
                found.push(number);
            }
        }
        found
    }

    /// The hard deadline of the descriptor test below: every wait in it is
    /// either bounded by this or unblocked by the socket shutdown that
    /// follows it.
    const INHIBITOR_TEST_DEADLINE: Duration = Duration::from_secs(20);

    #[test]
    fn the_inhibitor_reply_leaves_no_inheritable_descriptor_in_the_stream() {
        // Finding I-2 (round 5): `inhibit` used to deserialize the reply as an
        // `OwnedFd`, which clones the descriptor with close-on-exec but leaves
        // the original, as `recvmsg` delivered it (no close-on-exec), in the
        // message the worker's stream keeps until the next `drain`.
        //
        // A private peer-to-peer connection stands in for logind: the same
        // message path, no daemon.
        //
        // Round 10: the body runs in its own thread under a hard deadline. A
        // blocking call with no reply used to hang the whole suite; on the
        // deadline both socket ends are shut down, which unblocks every
        // thread still waiting, and the test fails with a clear message.
        use std::os::unix::net::UnixStream;

        let (to_server, to_client) = UnixStream::pair().expect("socketpair");
        let kill_server = to_server.try_clone().expect("clone server end");
        let kill_client = to_client.try_clone().expect("clone client end");
        let (done_tx, done_rx) = mpsc::channel();
        let body = std::thread::spawn(move || {
            inhibitor_reply_check(to_server, to_client);
            let _ = done_tx.send(());
        });
        let outcome = done_rx.recv_timeout(INHIBITOR_TEST_DEADLINE);
        if outcome.is_err() {
            let _ = kill_server.shutdown(std::net::Shutdown::Both);
            let _ = kill_client.shutdown(std::net::Shutdown::Both);
        }
        // A panic in the body disconnects the channel: join reports it.
        let joined = body.join();
        match (outcome, joined) {
            (Ok(()), Ok(())) => {}
            (Err(RecvTimeoutError::Timeout), _) => {
                panic!("the inhibitor-reply test did not finish within {INHIBITOR_TEST_DEADLINE:?}")
            }
            (_, Err(panic)) => std::panic::resume_unwind(panic),
            (Err(error), Ok(())) => panic!("the test body ended without a result: {error:?}"),
        }
    }

    fn inhibitor_reply_check(
        to_server: std::os::unix::net::UnixStream,
        to_client: std::os::unix::net::UnixStream,
    ) {
        use std::os::fd::AsRawFd as _;
        use zbus::blocking::connection::Builder;

        let guid = zbus::Guid::generate();
        // The two handshakes block on each other, so they run concurrently.
        let server = std::thread::spawn(move || {
            Builder::async_io_unix_stream(to_server)
                .server(guid)
                .expect("server")
                .p2p()
                .build()
                .expect("server connection")
        });
        let client = Builder::async_io_unix_stream(to_client)
            .p2p()
            .build()
            .expect("client connection");
        let server = server.join().expect("server joined");

        // The "inhibitor": a pipe whose inode identifies every copy of it.
        let (reader, writer) = std::io::pipe().expect("pipe");
        let inode = std::fs::read_link(format!("/proc/self/fd/{}", writer.as_raw_fd()))
            .expect("link")
            .to_string_lossy()
            .trim_start_matches("pipe:[")
            .trim_end_matches(']')
            .to_owned();

        // The worker's stream exists before the call, as in `subscribe`.
        let stream = MessageIterator::from(&client).into_inner();
        // The server's receiver also exists before the call: a connection
        // broadcasts only to the receivers that exist when a message
        // arrives, so creating it inside the answering thread lost the call
        // whenever that thread started late, and both ends then waited
        // forever (round 10).
        let mut calls = MessageIterator::from(&server);
        let answering = std::thread::spawn(move || {
            while let Some(Ok(call)) = calls.next() {
                if call.header().member().map(|m| m.as_str()) == Some("Inhibit") {
                    server
                        .reply(&call.header(), &zbus::zvariant::Fd::from(writer.as_fd()))
                        .expect("reply");
                    return (server, writer);
                }
            }
            panic!("no Inhibit call arrived");
        });

        let held = inhibit(&client).expect("the inhibitor is granted");
        let leaks = inheritable_copies_of(&inode);
        assert!(
            leaks.is_empty(),
            "descriptors {leaks:?} of the inhibitor survive an exec while the reply sits in the stream"
        );
        // The descriptor handed to the policy is close-on-exec as well.
        assert!(rustix::io::fcntl_getfd(&held)
            .expect("flags")
            .contains(FdFlags::CLOEXEC));
        let _server_side = answering.join().expect("answering joined");
        drop((stream, held, reader));
    }
}
