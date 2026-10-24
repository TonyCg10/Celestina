//! The agent's bus thread (`polkit-bus`) and its reply handles.
//!
//! Order matters everywhere here, so there is exactly one reader: a blocking
//! `MessageIterator` over the dedicated connection, created straight after
//! the connection and before any call whose answers or signals matter (the
//! iterator's queue only exists once it does, spike Step 3 fact 6). It is read
//! by a thread of its own (`polkit-read`) from the start, **because zbus lets
//! a full iterator queue stop the socket reader**: while the bus thread waits
//! for the answer to one of its own calls, nobody else would be taking
//! messages out, and any local user may unicast signals to the agent's name
//! on the system bus. The reading thread keeps only what the bus thread can
//! act on (a call to the agent's two members, a driver broadcast about
//! polkitd's name) and drops everything else at once, so a flood of anything
//! else costs one header check each and holds nothing; what it keeps goes to
//! the bus thread in arrival order through a bounded queue. For each message
//! the header decides first:
//!
//! 1. a signal counts only as [`crate::bus_signal::name_owner_change`]
//!    accepts it (the bus driver's broadcast), and only for polkitd's name;
//! 2. a method call counts only on [`AGENT_PATH`] and [`AGENT_INTERFACE`]
//!    with one of the agent's two members; anything else is dropped
//!    unanswered (nothing else is served, design N-13);
//! 3. its sender must be the cached unique owner of [`POLKIT_NAME`], or it
//!    is answered `AccessDenied` and nothing else happens, before the body is
//!    read and without logging an argument (N-12);
//! 4. only then is the body deserialised, validated by the pure domain and
//!    admitted.
//!
//! The thread may block in its own bus calls (each bounded by
//! [`super::CALL_TIMEOUT`], which is long because the registration may have
//! to start polkitd) and in its own replies; it never waits for the person. Whatever way it ends, its guard closes the connection, so a panic
//! makes the unique name vanish and polkitd cancel the agent's sessions.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use celestina_shell::polkit::{
    Admission, CancelOutcome, Cookie, Ending, Identity, RegistrationEvent, RegistrationState,
    Reply, Request, MAX_PASSWD_BYTES,
};
use zbus::blocking::{fdo::DBusProxy, Connection, MessageIterator};
use zbus::message::{Message, Type};
use zbus::names::BusName;
use zbus::zvariant::Value;
use zbus::MatchRule;

use super::session::{self, Environment};
use super::{
    reason, PolkitConfig, PolkitEvent, Registered, Shared, SystemBus, AGENT_INTERFACE, AGENT_PATH,
    AUTHORITY_INTERFACE, AUTHORITY_PATH, CALL_TIMEOUT, CONNECT_TIMEOUT, ERROR_ACCESS_DENIED,
    ERROR_CANCELLED, ERROR_FAILED, POLKIT_NAME,
};
use crate::bus_signal::{self, BUS_DRIVER};

/// polkitd's request to authenticate.
const BEGIN: &str = "BeginAuthentication";
/// polkitd's cancellation of one.
const CANCEL: &str = "CancelAuthentication";
/// The most identities of one request that are looked at; polkitd sends a
/// handful.
const MAX_IDENTITIES: usize = 64;
/// Where the account names come from (design §4.6).
const PASSWD: &str = "/etc/passwd";

/// The body of `BeginAuthentication(action_id, message, icon_name, details,
/// cookie, identities)`, borrowed from the message so the cookie is not
/// copied into a `String` on the way to [`Cookie`].
type BeginBody<'a> = (
    &'a str,
    &'a str,
    &'a str,
    HashMap<&'a str, &'a str>,
    &'a str,
    Vec<(&'a str, HashMap<&'a str, Value<'a>>)>,
);

// ---- replies ----------------------------------------------------------------------------

/// Sends `reply` to `call` on `connection`. Failures are logged by kind
/// only: the connection may be closed, or polkitd gone.
fn send(connection: &Connection, call: &Message, reply: Reply) {
    let header = call.header();
    let sent = match reply {
        Reply::Normal => connection.reply(&header, &()),
        Reply::Cancelled => connection.reply_error(&header, ERROR_CANCELLED, &("dismissed",)),
        Reply::Failed => {
            connection.reply_error(&header, ERROR_FAILED, &("the agent could not do this",))
        }
        Reply::AccessDenied => connection.reply_error(
            &header,
            ERROR_ACCESS_DENIED,
            &("only polkitd may call the agent",),
        ),
    };
    if let Err(err) = sent {
        tracing::debug!(error = %err, "polkit: a reply could not be sent");
    }
}

/// The one reply owed to an admitted `BeginAuthentication` call: a clone of
/// the connection and the call itself (whose header the reply needs).
///
/// It answers exactly once. [`ReplyHandle::reply`] and
/// [`ReplyHandle::abandon`] take it by value, so a second answer does not
/// compile; dropped unanswered, it replies `Cancelled` (design N-23). It has
/// no `Clone`, and its `Debug` shows the call's serial only.
pub struct ReplyHandle {
    connection: Connection,
    call: Message,
    answered: bool,
}

impl std::fmt::Debug for ReplyHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReplyHandle")
            .field("serial", &self.call.primary_header().serial_num())
            .field("answered", &self.answered)
            .finish()
    }
}

impl ReplyHandle {
    pub(super) fn new(connection: Connection, call: Message) -> Self {
        Self {
            connection,
            call,
            answered: false,
        }
    }

    /// Answers the call with `reply`.
    pub fn reply(mut self, reply: Reply) {
        self.answered = true;
        send(&self.connection, &self.call, reply);
    }

    /// Consumes the handle without answering: the caller is gone (polkitd's
    /// owner changed, F-8) or the connection is.
    pub fn abandon(mut self) {
        self.answered = true;
    }
}

impl Drop for ReplyHandle {
    fn drop(&mut self) {
        if !self.answered {
            self.answered = true;
            send(&self.connection, &self.call, Reply::Cancelled);
        }
    }
}

// ---- the thread ---------------------------------------------------------------------------

/// Closes the connection and tears the table down when the bus thread ends,
/// by any path: a clean stop (then the table is already empty), the bus
/// going away, or a panic (F-23).
struct BusGuard<'a> {
    shared: &'a Shared,
    /// The reading thread, joined once the connection is closed (which ends
    /// its iterator) and the queue it fills is gone.
    reader: Option<JoinHandle<()>>,
}

impl Drop for BusGuard<'_> {
    fn drop(&mut self) {
        let panicked = thread::panicking();
        let (taken, connection, stopped, explained) = {
            let mut core = self.shared.lock();
            let stopped = core.stopping;
            let taken = if stopped { Vec::new() } else { core.close() };
            core.registered = None;
            // A setup that failed already said why; that reason stays.
            let explained = matches!(core.registration, RegistrationState::Unavailable(_));
            (taken, core.connection.clone(), stopped, explained)
        };
        if let Some(connection) = connection {
            let _ = connection.close();
        }
        if let Some(reader) = self.reader.take() {
            if reader.join().is_err() {
                tracing::error!("polkit: the reading thread failed");
            }
        }
        if stopped {
            return;
        }
        let ending = if panicked {
            tracing::error!("polkit: the bus thread failed; its connection is closed");
            Ending::WorkerPanic
        } else {
            tracing::warn!("polkit: the bus connection ended");
            Ending::OwnerChanged
        };
        for (id, handle) in taken {
            // The connection is closed: there is no answer to send. polkitd
            // cancels the sessions of an agent whose name vanished.
            if let Some(handle) = handle {
                handle.abandon();
            }
            self.shared.publish(PolkitEvent::Ended { id, ending });
        }
        if !explained {
            self.shared
                .final_registration(RegistrationState::Unavailable(reason(
                    "the agent's bus connection ended",
                )));
        }
    }
}

/// The bus thread's body.
pub(super) fn run(config: PolkitConfig, shared: &Shared) {
    let mut guard = BusGuard {
        shared,
        reader: None,
    };
    // Every line of this thread names its bus.
    let _span = tracing::info_span!("polkit_bus", bus = %config.bus.label()).entered();
    // The agent this one replaces is still being taken down: it unregisters
    // before this one registers, or polkitd refuses the second (the loop
    // does not wait for this; only this thread does).
    config.after.wait();
    if shared.lock().stopping {
        return;
    }
    #[cfg(test)]
    let environment = config.seams.environment();
    #[cfg(not(test))]
    let environment = Environment::read();
    let environment = match environment {
        Ok(environment) => environment,
        Err(why) => {
            shared.set_registration(RegistrationState::Unavailable(reason(&why)));
            return;
        }
    };
    let connection = match connect(&config.bus) {
        Ok(connection) => connection,
        Err(why) => {
            shared.set_registration(RegistrationState::Unavailable(reason(&why)));
            return;
        }
    };
    // Before anything is asked of the bus: the answers and signals that
    // follow are only queued for an iterator that exists.
    let messages = MessageIterator::from(&connection);
    {
        let mut core = shared.lock();
        if core.stopping {
            drop(core);
            let _ = connection.close();
            return;
        }
        core.connection = Some(connection.clone());
    }
    // Taking messages out starts now, before the first call of `setup`.
    let (queue, inbox) = mpsc::sync_channel(READ_AHEAD);
    match thread::Builder::new()
        .name("polkit-read".to_owned())
        .spawn(move || read(messages, &queue))
    {
        Ok(reader) => guard.reader = Some(reader),
        Err(err) => {
            let why = format!("cannot start the reading thread: {err}");
            shared.set_registration(RegistrationState::Unavailable(reason(&why)));
            return;
        }
    }
    let mut agent = Agent {
        shared,
        connection,
        environment,
        owner: None,
        registered_with: None,
        session_id: None,
        #[cfg(test)]
        hook: config.seams.hook.clone(),
    };
    if let Err(why) = agent.setup() {
        // Said once, by the event's `log` in the loop.
        shared.set_registration(RegistrationState::Unavailable(reason(&why)));
        return;
    }
    // The queue ends when the reading thread does: the connection closed.
    for message in inbox {
        agent.handle(&message);
    }
}

/// How many messages the reading thread may hold for the bus thread. The
/// ones kept are polkitd's, or the bus driver's about polkitd, so this fills
/// only when the bus thread is slow to answer polkitd itself.
const READ_AHEAD: usize = 64;

/// Whether the bus thread has any use for `message`, decided on its header
/// (and, for a signal, on the sender and destination before the body).
fn worth_reading(message: &Message) -> bool {
    let header = message.header();
    match header.message_type() {
        Type::MethodCall => {
            header.path().is_some_and(|p| p.as_str() == AGENT_PATH)
                && header
                    .interface()
                    .is_some_and(|i| i.as_str() == AGENT_INTERFACE)
                && matches!(header.member().map(|m| m.as_str()), Some(BEGIN | CANCEL))
        }
        Type::Signal => {
            bus_signal::name_owner_change(message).is_some_and(|c| c.name == POLKIT_NAME)
        }
        // The answers to this thread's own calls are taken by the calls.
        Type::MethodReturn | Type::Error => false,
    }
}

/// The reading thread's body: the one reader of the message stream, in arrival
/// order, until the connection closes.
fn read(messages: MessageIterator, queue: &SyncSender<Message>) {
    for message in messages {
        match message {
            Ok(message) => {
                if worth_reading(&message) && queue.send(message).is_err() {
                    // The bus thread is gone.
                    return;
                }
            }
            // A closed connection ends the stream with an error after the
            // messages already read (spike contradiction 1).
            Err(err) => {
                tracing::debug!(error = %err, "polkit: the message stream ended");
                return;
            }
        }
    }
}

/// Connects to `bus` on a helper thread, waiting at most
/// [`CONNECT_TIMEOUT`]: the handshake has no timeout of its own.
fn connect(bus: &SystemBus) -> Result<Connection, String> {
    let builder = bus
        .builder()
        .map_err(|err| format!("cannot reach the system bus: {err}"))?;
    let (done, result) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("polkit-connect".to_owned())
        .spawn(move || {
            let _ = done.send(builder.method_timeout(CALL_TIMEOUT).build());
        })
        .map_err(|err| format!("cannot start the connection helper: {err}"))?;
    match result.recv_timeout(CONNECT_TIMEOUT) {
        Ok(Ok(connection)) => Ok(connection),
        Ok(Err(err)) => Err(format!("cannot reach the system bus: {err}")),
        Err(_) => Err(format!(
            "the system bus did not answer within {} ms",
            CONNECT_TIMEOUT.as_millis()
        )),
    }
}

/// `UnregisterAuthenticationAgent(subject, object_path)`, on a clean stop.
pub(super) fn unregister(connection: &Connection, session_id: &str) {
    let answer = connection.call_method(
        Some(POLKIT_NAME),
        AUTHORITY_PATH,
        Some(AUTHORITY_INTERFACE),
        "UnregisterAuthenticationAgent",
        &(subject(session_id), AGENT_PATH),
    );
    match answer {
        Ok(_) => tracing::info!("polkit: the agent unregistered"),
        Err(err) => tracing::warn!(error = %err, "polkit: unregistering failed"),
    }
}

/// `unix-session` with `session_id`: the only subject this agent registers
/// for (design N-14).
fn subject(session_id: &str) -> (&'static str, HashMap<&'static str, Value<'_>>) {
    let mut details = HashMap::new();
    details.insert("session-id", Value::from(session_id));
    ("unix-session", details)
}

/// The bus thread's own state.
struct Agent<'a> {
    shared: &'a Shared,
    connection: Connection,
    environment: Environment,
    /// The unique name holding [`POLKIT_NAME`], as the bus driver last said.
    owner: Option<String>,
    /// The unique name that accepted the registration.
    registered_with: Option<String>,
    session_id: Option<String>,
    #[cfg(test)]
    hook: Option<super::testing::Hook>,
}

impl Agent<'_> {
    /// The match, then the owner, then the session, then the registration.
    fn setup(&mut self) -> Result<(), String> {
        let dbus =
            DBusProxy::new(&self.connection).map_err(|err| format!("cannot ask the bus: {err}"))?;
        let rule = owner_rule().map_err(|err| format!("cannot describe the match: {err}"))?;
        // Before the owner is asked for, so a restart in between is seen.
        dbus.add_match_rule(rule)
            .map_err(|err| format!("cannot follow polkitd's name: {err}"))?;
        self.owner = match dbus.get_name_owner(
            BusName::from_static_str(POLKIT_NAME).map_err(|err| format!("bad name: {err}"))?,
        ) {
            Ok(unique) => Some(unique.as_str().to_owned()),
            Err(zbus::fdo::Error::NameHasNoOwner(_)) => None,
            Err(err) => return Err(format!("cannot ask who holds polkitd's name: {err}")),
        };
        let session_id = session::resolve(
            &self.connection,
            self.environment.own_uid,
            self.environment.session_hint.as_deref(),
        )
        .map_err(|err| crate::error_chain(&err))?;
        self.session_id = Some(session_id);
        self.transition(RegistrationEvent::SwitchedOn);
        Ok(())
    }

    fn handle(&mut self, message: &Message) {
        match message.header().message_type() {
            Type::MethodCall => self.method_call(message),
            Type::Signal => {
                if let Some(change) = bus_signal::name_owner_change(message) {
                    if change.name == POLKIT_NAME {
                        self.owner_changed(change.new_owner);
                    }
                }
            }
            // The answers to this thread's own calls also pass through the
            // stream; they were taken by the calls themselves.
            Type::MethodReturn | Type::Error => {}
        }
    }

    fn method_call(&mut self, message: &Message) {
        let header = message.header();
        let ours = header.path().is_some_and(|p| p.as_str() == AGENT_PATH)
            && header
                .interface()
                .is_some_and(|i| i.as_str() == AGENT_INTERFACE);
        let member = match header.member().map(|m| m.as_str()) {
            Some(BEGIN) if ours => BEGIN,
            Some(CANCEL) if ours => CANCEL,
            _ => {
                // Nothing else is served on this connection (N-13).
                tracing::trace!("polkit: a call the agent does not serve is ignored");
                return;
            }
        };
        let from_owner = match (header.sender(), &self.owner) {
            (Some(sender), Some(owner)) => sender.as_str() == owner,
            _ => false,
        };
        if !from_owner {
            // The header alone decided: the body is never read (N-12).
            tracing::warn!(
                "polkit: a call to the agent from a peer that is not polkitd is refused"
            );
            send(&self.connection, message, Reply::AccessDenied);
            return;
        }
        #[cfg(test)]
        if let Some(hook) = &self.hook {
            hook(member);
        }
        if member == BEGIN {
            self.begin(message);
        } else {
            self.cancel(message);
        }
    }

    fn begin(&mut self, message: &Message) {
        let body = message.body();
        let Ok((action_id, text, _icon, _details, cookie, offered)) =
            body.deserialize::<BeginBody<'_>>()
        else {
            tracing::warn!("polkit: a BeginAuthentication with a malformed body is refused");
            send(&self.connection, message, Reply::Failed);
            return;
        };
        let identities: Vec<Identity> = offered
            .iter()
            .take(MAX_IDENTITIES)
            .map(|(kind, details)| identity(kind, details))
            .collect();
        let passwd = match self.environment.passwd() {
            Ok(passwd) => passwd,
            Err(why) => {
                tracing::warn!("polkit: the account cannot be named: {why}");
                send(&self.connection, message, Reply::Failed);
                return;
            }
        };
        // The record is built (validated and sanitised) before the table's
        // lock is taken: the lock covers bookkeeping only.
        let id = super::fresh_id();
        let built = Request::new(
            id,
            cookie,
            action_id,
            text,
            &identities,
            self.environment.own_uid,
            &passwd,
        );
        let now = self.shared.moment(Instant::now());
        let admitted = match built {
            Err(refusal) => Err((id, refusal)),
            Ok(request) => {
                let mut core = self.shared.lock();
                match core.queue.admit(request, now) {
                    Admission::Accepted => {
                        core.replies.insert(
                            id,
                            ReplyHandle::new(self.connection.clone(), message.clone()),
                        );
                        Ok(id)
                    }
                    Admission::Refused(refusal) => Err((id, refusal)),
                }
            }
        };
        match admitted {
            Ok(id) => self.shared.publish(PolkitEvent::Admitted(id)),
            Err((id, refusal)) => {
                tracing::info!(
                    request = id.get(),
                    row = refusal.ending().row(),
                    "polkit: a request was refused before it was shown"
                );
                send(&self.connection, message, refusal.reply());
            }
        }
    }

    fn cancel(&mut self, message: &Message) {
        let body = message.body();
        let Ok((cookie,)) = body.deserialize::<(&str,)>() else {
            send(&self.connection, message, Reply::Failed);
            return;
        };
        let now = self.shared.moment(Instant::now());
        let (ended, handle) = {
            let mut core = self.shared.lock();
            // A cookie that is not even valid cannot be pending (F-22).
            let outcome = Cookie::new(cookie).map_or(CancelOutcome::Unknown, |cookie| {
                core.queue.cancel(&cookie, now)
            });
            match outcome {
                CancelOutcome::Unknown => (None, None),
                CancelOutcome::Waiting(id) => (
                    Some((id, Ending::CancelledByPolkit.reply())),
                    core.replies.remove(&id),
                ),
                CancelOutcome::Active(ended) => (
                    Some((ended.id, ended.reply)),
                    core.replies.remove(&ended.id),
                ),
            }
        };
        if let Some((id, reply)) = ended {
            match (handle, reply) {
                (Some(handle), Some(reply)) => handle.reply(reply),
                (Some(handle), None) => handle.abandon(),
                (None, _) => {}
            }
            self.shared.publish(PolkitEvent::Ended {
                id,
                ending: Ending::CancelledByPolkit,
            });
        }
        // The cancel call itself carries no verdict and is answered normally,
        // known cookie or not (F-21, F-22).
        send(&self.connection, message, Reply::Normal);
    }

    /// polkitd's name changed hands, as the bus driver broadcast it (F-8).
    fn owner_changed(&mut self, new_owner: String) {
        let new = (!new_owner.is_empty()).then_some(new_owner);
        if new == self.owner {
            return;
        }
        self.owner.clone_from(&new);
        if new.is_some() && new == self.registered_with {
            // polkitd was started by the registration itself and has just
            // announced its name: it is the owner the agent registered with.
            return;
        }
        // Every request belonged to the old owner: torn down, unanswered.
        let taken = {
            let mut core = self.shared.lock();
            let ids = core.queue.drain();
            core.registered = None;
            ids.into_iter()
                .map(|id| (id, core.replies.remove(&id)))
                .collect::<Vec<_>>()
        };
        for (id, handle) in taken {
            if let Some(handle) = handle {
                handle.abandon();
            }
            self.shared.publish(PolkitEvent::Ended {
                id,
                ending: Ending::OwnerChanged,
            });
        }
        self.registered_with = None;
        self.transition(if new.is_some() {
            RegistrationEvent::OwnerAppeared
        } else {
            RegistrationEvent::OwnerLost
        });
    }

    /// Moves the registration's state by `event` and registers when the pure
    /// machine says so (once, never after a refusal).
    fn transition(&mut self, event: RegistrationEvent) {
        let state = self.shared.lock().registration.clone();
        let transition = state.on(event);
        self.shared.set_registration(transition.state);
        if transition.register {
            let event = self.register();
            let next = self.shared.lock().registration.clone().on(event);
            self.shared.set_registration(next.state);
        }
    }

    /// `RegisterAuthenticationAgent(subject, locale, object_path)`, once.
    fn register(&mut self) -> RegistrationEvent {
        let Some(session_id) = self.session_id.clone() else {
            return RegistrationEvent::Unavailable("no session".to_owned());
        };
        if self.shared.lock().stopping {
            return RegistrationEvent::SwitchedOff;
        }
        let answer = self.connection.call_method(
            Some(POLKIT_NAME),
            AUTHORITY_PATH,
            Some(AUTHORITY_INTERFACE),
            "RegisterAuthenticationAgent",
            &(
                subject(&session_id),
                self.environment.locale.as_str(),
                AGENT_PATH,
            ),
        );
        match answer {
            Ok(reply) => {
                self.registered_with = reply
                    .header()
                    .sender()
                    .map(|sender| sender.as_str().to_owned());
                self.shared.lock().registered = Some(Registered { session_id });
                tracing::info!("polkit: registered as the session's agent");
                RegistrationEvent::Accepted
            }
            Err(zbus::Error::MethodError(name, _, _)) if absent(name.as_str()) => {
                tracing::info!("polkit: polkitd is not running; the agent waits for it");
                RegistrationEvent::PolkitdAbsent
            }
            Err(zbus::Error::MethodError(name, text, _))
                if already_exists(name.as_str(), text.as_deref()) =>
            {
                // Logged once: the state machine never registers again from
                // here (design decision 1).
                tracing::warn!(
                    error = %reason(name.as_str()),
                    "polkit: another agent holds this session; the agent stays off"
                );
                RegistrationEvent::Held
            }
            // Any other refusal (a session polkit does not take for this
            // caller, no permission) is a fault of the registration, not
            // another agent, and is said as such.
            Err(zbus::Error::MethodError(name, text, _)) => {
                RegistrationEvent::Unavailable(reason(&format!(
                    "polkitd refused the registration: {} {}",
                    name.as_str(),
                    text.as_deref().unwrap_or("")
                )))
            }
            Err(err) => RegistrationEvent::Unavailable(reason(&format!(
                "the registration call failed: {err}"
            ))),
        }
    }
}

/// Whether a registration error is polkit's "an agent already exists for the
/// subject": the only answer that means another agent holds the session.
fn already_exists(name: &str, text: Option<&str>) -> bool {
    name == ERROR_FAILED && text.is_some_and(|text| text.contains("already exists"))
}

/// Whether a registration error means nobody serves polkitd's name.
fn absent(name: &str) -> bool {
    matches!(
        name,
        "org.freedesktop.DBus.Error.ServiceUnknown" | "org.freedesktop.DBus.Error.NameHasNoOwner"
    )
}

/// The match for polkitd's owner changes: the driver's signal, for that
/// name only.
fn owner_rule() -> zbus::Result<MatchRule<'static>> {
    Ok(MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(BUS_DRIVER)?
        .interface(BUS_DRIVER)?
        .member("NameOwnerChanged")?
        .arg(0, POLKIT_NAME)?
        .build())
}

/// One identity polkitd offered.
fn identity(kind: &str, details: &HashMap<&str, Value<'_>>) -> Identity {
    match (kind, details.get("uid"), details.get("gid")) {
        ("unix-user", Some(Value::U32(uid)), _) => Identity::User(*uid),
        ("unix-group", _, Some(Value::U32(gid))) => Identity::Group(*gid),
        _ => Identity::Other,
    }
}

/// The text of `/etc/passwd`, bounded.
pub(super) fn read_passwd() -> Result<String, String> {
    match celestina_core::atomic_file::read_bounded(Path::new(PASSWD), MAX_PASSWD_BYTES as u64) {
        Ok(Some(bytes)) => String::from_utf8(bytes).map_err(|_| format!("{PASSWD} is not UTF-8")),
        Ok(None) => Err(format!("{PASSWD} does not exist")),
        Err(err) => Err(err.to_string()),
    }
}
