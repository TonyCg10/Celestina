//! The polkit agent's bus side (CEL-2-I; design
//! `docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md`,
//! sections 4.2, 4.3 and 4.5).
//!
//! polkitd calls the authentication agent registered for the person's
//! session on the **system** bus. This module is that agent's half that talks
//! to the bus: it registers, takes polkitd's `BeginAuthentication` and
//! `CancelAuthentication` calls in arrival order, admits them into the pure
//! [`Queue`] of `celestina_shell::polkit`, and answers each call exactly once.
//! It decides nothing about a password: only polkit's own helper reports a
//! success to polkitd, and the only normal reply to `BeginAuthentication`
//! follows [`Ending::Authenticated`] (the pure domain's invariant).
//!
//! Two threads, started by [`spawn`]:
//!
//! - **polkit-bus** ([`agent`]) owns a dedicated connection that exports
//!   nothing (no object server, no interface macro, no well-known name) and
//!   reads **one blocking** `MessageIterator` in arrival order. It looks at a
//!   message's header before its body: a method call that is not on the
//!   agent's path and interface is dropped; one whose sender is not the
//!   cached unique owner of `org.freedesktop.PolicyKit1` is answered
//!   `AccessDenied` without its body being read. The owner cache is seeded by
//!   `GetNameOwner` after the `NameOwnerChanged` match is installed, and kept
//!   only by the bus driver's broadcasts ([`crate::bus_signal`]). It never
//!   waits for the person, so a cancellation or an owner change is seen while
//!   a prompt is up.
//! - **polkit-reply** sends the replies the loop decides (the loop never does
//!   bus IO, design N-22), and on stop cancels every request, unregisters and
//!   closes the connection, which is what ends the bus thread's iterator.
//!
//! The loop holds a [`PolkitAgent`]. The request table (the queue, and the
//! [`ReplyHandle`] of every admitted call) is shared behind one mutex that
//! is never held across IO; the loop promotes and ends requests through the
//! agent, and the events it receives ([`PolkitEvent`]) carry request ids,
//! endings and the registration state only. **A cookie never leaves this
//! module and the request record**: it is not in [`PolkitEvent`], in the
//! loop's `Message`, or in any channel.
//!
//! Every way out closes the connection: a clean stop (after unregistering),
//! a panic of either thread (a drop guard), and the bus going away. polkitd
//! then removes the agent and cancels its sessions (design fact 4); a
//! request whose reply can no longer be sent is never left hanging.

mod agent;
pub mod helper;
pub mod session;
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use celestina_shell::display_text::{sanitise_display, Limit};
use celestina_shell::polkit::{
    Ended, Ending, Moment, Promotion, Queue, RegistrationState, Reply, Request, RequestId,
};
use zbus::blocking::Connection;

use crate::providers::{Reaper, Worker};

pub use agent::ReplyHandle;

/// polkitd's well-known name on the system bus.
pub const POLKIT_NAME: &str = "org.freedesktop.PolicyKit1";
/// Where polkitd serves its authority.
pub const AUTHORITY_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
/// polkitd's authority interface.
pub const AUTHORITY_INTERFACE: &str = "org.freedesktop.PolicyKit1.Authority";
/// The path the agent registers. Nothing is served there: the bus thread
/// reads the calls addressed to it from its message stream.
pub const AGENT_PATH: &str = "/org/celestina/PolkitAgent1";
/// The interface polkitd calls the agent on.
pub const AGENT_INTERFACE: &str = "org.freedesktop.PolicyKit1.AuthenticationAgent";
/// The error polkitd records as "dismissed" (spike U-2).
pub const ERROR_CANCELLED: &str = "org.freedesktop.PolicyKit1.Error.Cancelled";
/// The error for an agent-side fault (design Q1).
pub const ERROR_FAILED: &str = "org.freedesktop.PolicyKit1.Error.Failed";
/// The bus's own refusal, for a caller that is not polkitd (F-1).
pub const ERROR_ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";
/// The longest one bus call of the agent may take.
///
/// Long on purpose, for the registration's sake: with polkitd not running,
/// `RegisterAuthenticationAgent` is what starts it (bus activation), which
/// can take many seconds, and a registration that timed out is neither known
/// to have happened nor safe to repeat (a second one would be refused as
/// "already exists"). A timeout is therefore a fault (`Unavailable`), never
/// "polkitd is absent" with a retry. Only the bus thread waits for it, and
/// closing the connection (the stop's first act) ends a call in flight, so
/// nothing else is held up.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(20);
/// The longest connecting to the bus may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// How often the reply thread looks at the shutdown flag.
pub const POLL: Duration = Duration::from_millis(100);
/// The most bytes of a reason (often a bus error's text) that is published.
pub const MAX_REASON: usize = 240;

/// Which bus the agent registers on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SystemBus {
    /// The system bus: production. zbus reads `DBUS_SYSTEM_BUS_ADDRESS`, and
    /// `smoke.sh` points it at a dead socket on purpose (design §6.2 A).
    #[default]
    System,
    /// A bus at this address: tests, on a private bus (the pattern of
    /// `energy::logind::LogindBus`).
    Address(String),
}

impl SystemBus {
    /// Which bus this is, for the journal: every line the bus thread writes
    /// carries it (a test bus has an address of its own, so a line can be
    /// told from the lines of another agent running at the same time).
    fn label(&self) -> &str {
        match self {
            Self::System => "system",
            Self::Address(address) => address,
        }
    }

    fn builder(&self) -> zbus::Result<zbus::blocking::connection::Builder<'static>> {
        match self {
            Self::System => zbus::blocking::connection::Builder::system(),
            Self::Address(address) => {
                zbus::blocking::connection::Builder::address(address.as_str())
            }
        }
    }
}

/// How the agent is built.
#[derive(Default)]
pub struct PolkitConfig {
    /// Where polkitd and logind are looked for.
    pub bus: SystemBus,
    /// The teardown of the agent this one replaces, if it is still running:
    /// the bus thread waits for it before it connects, so an old agent that
    /// is still unregistering never overlaps the new one's registration
    /// (polkitd would refuse the second as "already exists"). The loop never
    /// waits for it. The agent's own [`PolkitAgent::retire`] waits for it
    /// too, which makes a chain of quick replacements transitive.
    pub after: Reaper,
    /// What the tests replace: the account, the session hint, the locale,
    /// `/etc/passwd` and a hook in the bus thread.
    #[cfg(test)]
    pub(crate) seams: testing::Seams,
}

/// What the loop is told by the agent. Ids, endings and the registration
/// state: never a cookie, never text a caller sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolkitEvent {
    /// polkitd's request `id` was admitted and waits: call
    /// [`PolkitAgent::promote`].
    Admitted(RequestId),
    /// Request `id` ended on the bus side (polkitd cancelled it, polkitd's
    /// owner changed, the agent stopped or its worker failed). Its reply, if
    /// any, has been decided; close its prompt and end its helper.
    Ended {
        /// The request.
        id: RequestId,
        /// How it ended.
        ending: Ending,
    },
    /// Where the registration with polkitd stands.
    Registration(RegistrationState),
}

impl PolkitEvent {
    /// Writes the event to the journal. Nothing here holds a secret, a cookie
    /// or a caller's text.
    pub fn log(&self) {
        match self {
            Self::Admitted(id) => tracing::debug!(request = id.get(), "polkit: request admitted"),
            Self::Ended { id, ending } => {
                tracing::info!(
                    request = id.get(),
                    row = ending.row(),
                    "polkit: request ended"
                );
            }
            Self::Registration(RegistrationState::Unavailable(why)) => {
                tracing::warn!("polkit: the agent is unavailable: {why}");
            }
            Self::Registration(state) => tracing::info!("polkit: registration {state:?}"),
        }
    }
}

/// What a card shows of the active request: polkitd's message and action id
/// after the sanitiser, and the account being asked for (design N-18). Owned
/// copies; the request record keeps the cookie.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    /// The request.
    pub id: RequestId,
    /// polkit's message, sanitised and bounded.
    pub message: String,
    /// The action id, on its own line.
    pub action_id: String,
    /// The account being asked for.
    pub account: String,
}

/// The last request id given out, for the whole process.
///
/// Ids are never reused, not even by a later agent (the duty switched off and
/// on again): an event of an abandoned helper thread or of an agent that was
/// switched off carries an id that no request of the next agent has, so it
/// can end nothing of theirs.
static LAST_REQUEST: AtomicU64 = AtomicU64::new(0);

/// A request id nobody had before.
fn fresh_id() -> RequestId {
    RequestId::new(LAST_REQUEST.fetch_add(1, Ordering::Relaxed) + 1)
}

/// What the reply thread is asked to do.
enum Outgoing {
    /// Send this reply (decided by the loop).
    Reply(ReplyHandle, Reply),
    /// Cancel everything, unregister and close the connection.
    Stop,
}

/// The agent's registration while it holds one: what to unregister.
struct Registered {
    session_id: String,
}

/// The request table and the registration, shared by the two threads and
/// the loop. Every operation on it is pure bookkeeping: no IO happens while
/// the lock is held.
struct Core {
    queue: Queue,
    replies: HashMap<RequestId, ReplyHandle>,
    registration: RegistrationState,
    registered: Option<Registered>,
    /// The connection, once the bus thread made it.
    connection: Option<Connection>,
    /// A stop was asked for (or the agent ended): nothing more is admitted.
    stopping: bool,
}

impl Core {
    fn new() -> Self {
        let mut queue = Queue::new();
        // The agent exists only while the duty is on.
        queue.set_duty(true);
        Self {
            queue,
            replies: HashMap::new(),
            registration: RegistrationState::Off,
            registered: None,
            connection: None,
            stopping: false,
        }
    }

    /// Ends the agent's side of the table: no more admissions, every request
    /// taken out with its reply handle. Returns them for the caller to answer
    /// outside the lock.
    fn close(&mut self) -> Vec<(RequestId, Option<ReplyHandle>)> {
        self.stopping = true;
        self.queue.set_duty(false);
        let ids = self.queue.drain();
        ids.into_iter()
            .map(|id| (id, self.replies.remove(&id)))
            .collect()
    }
}

type Publish = Box<dyn Fn(PolkitEvent) -> bool + Send + Sync>;

/// What both threads and the loop share.
struct Shared {
    origin: Instant,
    core: Mutex<Core>,
    publish: Publish,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Core> {
        // The table is plain bookkeeping: a panic on another thread leaves it
        // usable, and the drop guards rely on reaching it after one.
        match self.core.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn moment(&self, at: Instant) -> Moment {
        at.saturating_duration_since(self.origin)
    }

    fn publish(&self, event: PolkitEvent) {
        if !(self.publish)(event) {
            tracing::debug!("polkit: nobody listens to the agent's events");
        }
    }

    /// Records a new registration state and publishes it when it changed.
    /// Ignored once the agent is stopping: a bus call cut short by the stop
    /// must not overwrite the state the stop leaves.
    fn set_registration(&self, state: RegistrationState) {
        self.update_registration(state, false);
    }

    /// The state the agent ends in (the stop's `Off`, or why the bus thread
    /// ended), recorded even while stopping.
    fn final_registration(&self, state: RegistrationState) {
        self.update_registration(state, true);
    }

    fn update_registration(&self, state: RegistrationState, last: bool) {
        let changed = {
            let mut core = self.lock();
            if core.stopping && !last {
                return;
            }
            let changed = core.registration != state;
            core.registration = state.clone();
            changed
        };
        if changed {
            self.publish(PolkitEvent::Registration(state));
        }
    }

    /// The connection, taken for closing.
    fn connection(&self) -> Option<Connection> {
        self.lock().connection.clone()
    }
}

/// The loop's handle on the agent: promotes and ends requests, and stops the
/// agent when joined or dropped.
pub struct PolkitAgent {
    shared: Arc<Shared>,
    outbox: mpsc::Sender<Outgoing>,
    bus: Option<Worker>,
    replier: Option<Worker>,
    /// The teardown this agent was started behind ([`PolkitConfig::after`]):
    /// its own reaper waits for it too, so the chain of replacements is
    /// transitive (see [`PolkitAgent::retire`]).
    after: Reaper,
}

impl std::fmt::Debug for PolkitAgent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PolkitAgent")
            .field("bus", &self.bus)
            .field("replier", &self.replier)
            .finish()
    }
}

/// How long [`Drop`] waits for the two threads (the shell's exit; the loop
/// uses [`PolkitAgent::retire`] instead).
pub const STOP_WAIT: Duration = Duration::from_secs(2);
/// How long the reaper of [`PolkitAgent::retire`] waits for each thread:
/// room for the unregistration to be answered or to give up.
pub const REAP_WAIT: Duration = Duration::from_secs(5);

impl PolkitAgent {
    /// Whether the session is locked, read by the loop synchronously before
    /// it maps a prompt; nothing is admitted or shown while it is (N-26).
    pub fn set_locked(&self, locked: bool) {
        self.shared.lock().queue.set_locked(locked);
    }

    /// What the front of the queue does at `now` ([`Queue::promote`]). A
    /// request refused at promotion is answered here; call again until
    /// [`Promotion::Idle`] or [`Promotion::Later`].
    pub fn promote(&self, now: Instant) -> Promotion {
        let moment = self.shared.moment(now);
        let (promotion, refused) = {
            let mut core = self.shared.lock();
            let promotion = core.queue.promote(moment);
            let refused = match promotion {
                Promotion::Refused { id, refusal } => {
                    core.replies.remove(&id).map(|h| (h, refusal.reply()))
                }
                _ => None,
            };
            (promotion, refused)
        };
        if let Some((handle, reply)) = refused {
            self.send(handle, Some(reply));
        }
        promotion
    }

    /// The text of request `id` while it is the active one.
    #[must_use]
    pub fn prompt(&self, id: RequestId) -> Option<Prompt> {
        let core = self.shared.lock();
        let request = core.queue.active().filter(|request| request.id() == id)?;
        let display = request.display();
        Some(Prompt {
            id,
            message: display.message.to_owned(),
            action_id: display.action_id.to_owned(),
            account: display.account.to_owned(),
        })
    }

    /// What the helper conversation of request `id` is opened with while it
    /// is the active one: its account and a fresh copy of its cookie, made
    /// under the lock and handed to the helper's thread, so the cookie
    /// reaches neither [`PolkitEvent`] nor the loop's `Message` (design N-2)
    /// and the lock is never held across the socket's IO.
    #[must_use]
    pub fn helper_input(&self, id: RequestId) -> Option<helper::HelperInput> {
        let core = self.shared.lock();
        let request = core.queue.active().filter(|request| request.id() == id)?;
        helper::HelperInput::from_request(request)
    }

    /// Ends request `id`, if it is the active one ([`Queue::end_active`]),
    /// and sends its reply from the reply thread. `None` when it is not
    /// active: polkitd may have cancelled it first (it was answered then), or
    /// the caller is talking about a request that is already gone, which must
    /// never end the one that took its place.
    pub fn end_active(
        &self,
        id: RequestId,
        ending: Ending,
        attempted: bool,
        now: Instant,
    ) -> Option<Ended> {
        let moment = self.shared.moment(now);
        let (ended, handle) = {
            let mut core = self.shared.lock();
            if core.queue.active().map(Request::id) != Some(id) {
                return None;
            }
            let ended = core.queue.end_active(ending, attempted, moment)?;
            let handle = core.replies.remove(&ended.id);
            (ended, handle)
        };
        if let Some(handle) = handle {
            self.send(handle, ended.reply);
        }
        Some(ended)
    }

    /// Hands a reply to the reply thread, so the loop does no bus IO.
    fn send(&self, handle: ReplyHandle, reply: Option<Reply>) {
        let Some(reply) = reply else {
            handle.abandon();
            return;
        };
        if let Err(mpsc::SendError(Outgoing::Reply(handle, _))) =
            self.outbox.send(Outgoing::Reply(handle, reply))
        {
            // The reply thread is gone, and with it the connection: there is
            // nobody left to answer.
            handle.abandon();
        }
    }

    /// Stops the agent: every request is answered `Cancelled`, the agent is
    /// unregistered and its connection closed, on the reply thread; then both
    /// threads are joined, each within `wait`. Returns whether both stopped
    /// in time.
    pub fn join_within(mut self, wait: Duration) -> bool {
        self.stop_and_join(wait)
    }

    /// Hands the stop to a reaper thread and returns at once: the agent is
    /// stopped, unregistered and joined there, within [`REAP_WAIT`] per
    /// thread. This is how the loop gets rid of an agent (N-22): it never
    /// waits for the unregistration, which polkitd may be slow to answer. The
    /// agent that replaces it is started with the reaper in
    /// [`PolkitConfig::after`], so the two never overlap on polkitd.
    ///
    /// The reaper is over only when the teardown this agent was itself
    /// started behind is over too. Without that, an agent switched off before
    /// its predecessor finished unregistering would end its own reaper after
    /// [`REAP_WAIT`] (its bus thread still waiting for the predecessor), and
    /// the next agent, waiting for this one only, could register while the
    /// first still holds the session: polkitd refuses it as "already exists".
    pub fn retire(self) -> Reaper {
        self.retire_within(REAP_WAIT)
    }

    /// [`PolkitAgent::retire`] with the bound on each thread given.
    pub(crate) fn retire_within(self, wait: Duration) -> Reaper {
        let after = self.after.clone();
        Reaper::spawn("polkit", move || {
            if !self.join_within(wait) {
                tracing::warn!("polkit: the agent did not stop in time; it is left behind");
            }
            after.wait();
        })
    }

    fn stop_and_join(&mut self, wait: Duration) -> bool {
        // A closed outbox stops the reply thread too.
        let _ = self.outbox.send(Outgoing::Stop);
        let mut stopped = true;
        if let Some(replier) = self.replier.take() {
            stopped &= replier.join_within(wait);
        }
        if let Some(bus) = self.bus.take() {
            if !stopped {
                // The reply thread did not get to closing; do it here so the
                // bus thread's iterator ends.
                if let Some(connection) = self.shared.connection() {
                    let _ = connection.close();
                }
            }
            stopped &= bus.join_within(wait);
        }
        stopped
    }
}

impl Drop for PolkitAgent {
    fn drop(&mut self) {
        if self.bus.is_some() || self.replier.is_some() {
            self.stop_and_join(STOP_WAIT);
        }
    }
}

/// Starts the agent with `config`, publishing [`PolkitEvent`]s through
/// `publish`, which returns `false` once nobody listens. `shutdown` is the
/// agent's own flag (joining raises it): a duty switched off must not stop
/// the shell. The loop's agent comes from `crate::polkit::duty::Gated`,
/// which gates `publish`.
///
/// # Errors
///
/// The failure to start a thread.
pub fn spawn(
    config: PolkitConfig,
    publish: impl Fn(PolkitEvent) -> bool + Send + Sync + 'static,
    shutdown: &Arc<AtomicBool>,
) -> io::Result<PolkitAgent> {
    let shared = Arc::new(Shared {
        origin: Instant::now(),
        core: Mutex::new(Core::new()),
        publish: Box::new(publish),
    });
    let after = config.after.clone();
    let (outbox, inbox) = mpsc::channel();
    let replier = {
        let shared = Arc::clone(&shared);
        let flag = Arc::clone(shutdown);
        Worker::spawn("polkit-reply", shutdown, move || {
            replier(&shared, &inbox, &flag);
        })?
    };
    let bus = {
        let shared = Arc::clone(&shared);
        Worker::spawn("polkit-bus", shutdown, move || agent::run(config, &shared))
    };
    let bus = match bus {
        Ok(bus) => bus,
        Err(err) => {
            let _ = outbox.send(Outgoing::Stop);
            replier.join();
            return Err(err);
        }
    };
    Ok(PolkitAgent {
        shared,
        outbox,
        bus: Some(bus),
        replier: Some(replier),
        after,
    })
}

/// Closes the connection when dropped, the panic backstop of both threads.
struct CloseOnDrop<'a>(&'a Shared);

impl Drop for CloseOnDrop<'_> {
    fn drop(&mut self) {
        if let Some(connection) = self.0.connection() {
            let _ = connection.close();
        }
    }
}

/// The reply thread: sends the replies the loop decided until it is asked to
/// stop (or the shutdown flag rises, or the agent's handle is gone), then
/// stops the agent.
fn replier(shared: &Shared, inbox: &Receiver<Outgoing>, shutdown: &AtomicBool) {
    let _close = CloseOnDrop(shared);
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        match inbox.recv_timeout(POLL) {
            Ok(Outgoing::Reply(handle, reply)) => handle.reply(reply),
            Ok(Outgoing::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
    stop(shared, inbox);
}

/// The clean stop (F-9): the replies already decided go out, every other
/// request is answered `Cancelled`, the agent unregisters, and the
/// connection is closed, which ends the bus thread's iterator.
fn stop(shared: &Shared, inbox: &Receiver<Outgoing>) {
    while let Ok(outgoing) = inbox.try_recv() {
        if let Outgoing::Reply(handle, reply) = outgoing {
            handle.reply(reply);
        }
    }
    let (taken, registered, connection) = {
        // When the bus thread ended first it tore the table down already;
        // this finds it empty then.
        let mut core = shared.lock();
        let taken = core.close();
        let registered = core.registered.take();
        (taken, registered, core.connection.clone())
    };
    let mut ids = Vec::with_capacity(taken.len());
    for (id, handle) in taken {
        if let Some(handle) = handle {
            handle.reply(Reply::Cancelled);
        }
        ids.push(id);
    }
    if let (Some(registered), Some(connection)) = (registered, connection.as_ref()) {
        agent::unregister(connection, &registered.session_id);
    }
    if let Some(connection) = connection {
        let _ = connection.close();
    }
    for id in ids {
        shared.publish(PolkitEvent::Ended {
            id,
            ending: Ending::ShellExit,
        });
    }
    shared.final_registration(RegistrationState::Off);
}

/// `text`, cleaned and bounded for publishing: a reason reaches the journal
/// and the duty's state, and part of it is polkitd's own error text, so no
/// control or direction character survives (the card's sanitiser).
fn reason(text: &str) -> String {
    sanitise_display(text, Limit::new(MAX_REASON, MAX_REASON))
}
