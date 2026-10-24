//! Test seams and stand-ins for the agent's bus tests: a fake polkitd and a
//! fake logind on a private `dbus-daemon` (`crate::tray::test_bus`), and a
//! capture of everything the process logs. The fakes read their own message
//! streams by hand, as the agent does; nothing here serves an interface.
//! Compiled only for tests.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator};
use zbus::message::{Message, Type};
use zbus::zvariant::{ObjectPath, StructureBuilder, Value};

use super::session::Environment;
use super::{AGENT_INTERFACE, AGENT_PATH, POLKIT_NAME};
use crate::energy::logind::LOGIND_NAME;
use crate::tray::test_bus::PrivateBus;

/// A function the bus thread calls with the member of every call that passed
/// the sender check: a test makes it panic to stand for a bug.
pub(crate) type Hook = Arc<dyn Fn(&str) + Send + Sync>;

/// What a test replaces in the agent.
#[derive(Clone, Default)]
pub(crate) struct Seams {
    /// The account, the session hint, the locale and `/etc/passwd`; the
    /// process's own when `None`.
    pub(crate) environment: Option<Environment>,
    /// Called on the bus thread after the sender check.
    pub(crate) hook: Option<Hook>,
}

impl Seams {
    pub(crate) fn environment(&self) -> Result<Environment, String> {
        match &self.environment {
            Some(environment) => Ok(environment.clone()),
            None => Environment::read(),
        }
    }
}

/// The uid the tests pretend to be.
pub(crate) const UID: u32 = 4242;
/// The display session the fake logind reports.
pub(crate) const DISPLAY: &str = "c7";

/// The environment of a test agent.
pub(crate) fn environment() -> Environment {
    Environment {
        own_uid: UID,
        session_hint: None,
        locale: "es_ES.UTF-8".to_owned(),
        passwd: Some(format!(
            "root:x:0:0::/root:/bin/sh\ntester:x:{UID}:{UID}:Tester:/home/tester:/bin/sh\n"
        )),
    }
}

/// Waits up to `wait` for `done`.
pub(crate) fn eventually(wait: Duration, mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + wait;
    while Instant::now() < until {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    done()
}

/// A connection to `bus` whose calls give up after `timeout`.
pub(crate) fn client(bus: &PrivateBus, timeout: Duration) -> Connection {
    match bus.builder().method_timeout(timeout).build() {
        Ok(connection) => connection,
        Err(err) => panic!("a client connection: {err}"),
    }
}

/// A peer that owns `name` and answers calls with `handler`, on its own
/// thread, until it is stopped (its connection closed).
pub(crate) struct Service {
    pub(crate) connection: Connection,
    thread: Option<JoinHandle<()>>,
}

impl Service {
    pub(crate) fn start(
        bus: &PrivateBus,
        name: &str,
        mut handler: impl FnMut(&Connection, &Message) + Send + 'static,
    ) -> Service {
        let connection = client(bus, Duration::from_secs(10));
        // The stream first: calls are queued only for a stream that exists.
        let messages = MessageIterator::from(&connection);
        if let Err(err) = connection.request_name(name.to_owned()) {
            panic!("cannot own {name}: {err}");
        }
        let own = connection.clone();
        let thread = thread::spawn(move || {
            for message in messages {
                let Ok(message) = message else { return };
                if message.header().message_type() == Type::MethodCall {
                    handler(&own, &message);
                }
            }
        });
        Service {
            connection,
            thread: Some(thread),
        }
    }

    /// Closes the connection: its unique name vanishes from the bus.
    pub(crate) fn stop(&mut self) {
        let _ = self.connection.clone().close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `header`'s member, or empty.
fn member(message: &Message) -> String {
    message
        .header()
        .member()
        .map(|m| m.as_str().to_owned())
        .unwrap_or_default()
}

/// Answers one call to the fake polkitd and records it.
fn answer(
    connection: &Connection,
    call: &Message,
    record: &Mutex<PolkitdLog>,
    behaviour: Behaviour,
) {
    let header = call.header();
    let member = member(call);
    let mut log = match record.lock() {
        Ok(log) => log,
        Err(poisoned) => poisoned.into_inner(),
    };
    log.members.push(member.clone());
    type Subject<'a> = (&'a str, HashMap<&'a str, Value<'a>>);
    let session = |subject: &Subject<'_>| match subject.1.get("session-id") {
        Some(Value::Str(id)) => id.as_str().to_owned(),
        _ => String::new(),
    };
    let body = call.body();
    let answer = match member.as_str() {
        "RegisterAuthenticationAgent" => match body.deserialize::<(Subject<'_>, &str, &str)>() {
            Ok((subject, locale, path)) => {
                log.registrations
                    .push((session(&subject), locale.to_owned(), path.to_owned()));
                log.agent = header.sender().map(|s| s.as_str().to_owned());
                if let Some((name, text)) = behaviour.register_error {
                    connection.reply_error(&header, name, &(text,))
                } else if behaviour.accept && !(behaviour.strict && log.holder.is_some()) {
                    log.holder = log.agent.clone();
                    connection.reply(&header, &())
                } else {
                    connection.reply_error(
                        &header,
                        "org.freedesktop.PolicyKit1.Error.Failed",
                        &("An authentication agent already exists for the given subject",),
                    )
                }
            }
            Err(_) => connection.reply_error(
                &header,
                "org.freedesktop.DBus.Error.InvalidArgs",
                &("bad body",),
            ),
        },
        "UnregisterAuthenticationAgent" => {
            if let Ok((subject, _path)) = body.deserialize::<(Subject<'_>, &str)>() {
                log.unregistrations.push(session(&subject));
                log.holder = None;
            }
            connection.reply(&header, &())
        }
        _ => connection.reply_error(
            &header,
            "org.freedesktop.DBus.Error.UnknownMethod",
            &("not served",),
        ),
    };
    if let Err(err) = answer {
        eprintln!("the fake polkitd could not answer: {err}");
    }
}

/// What the fake polkitd saw.
#[derive(Clone, Debug, Default)]
pub(crate) struct PolkitdLog {
    /// Every authority member called, in order.
    pub(crate) members: Vec<String>,
    /// `(session id, locale, object path)` of each registration.
    pub(crate) registrations: Vec<(String, String, String)>,
    /// The session ids unregistered.
    pub(crate) unregistrations: Vec<String>,
    /// The unique name that registered.
    pub(crate) agent: Option<String>,
    /// The unique name that holds the registration now: set by an accepted
    /// registration, cleared by an unregistration.
    pub(crate) holder: Option<String>,
}

/// How the fake polkitd behaves.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Behaviour {
    /// Whether a registration succeeds; when not, it is refused the way
    /// polkitd refuses a second agent for a session.
    pub(crate) accept: bool,
    /// Refuses a registration while another holds the session, as polkitd
    /// does, and lets it through once that one unregistered.
    pub(crate) strict: bool,
    /// How long it takes to answer a registration.
    pub(crate) register_delay: Duration,
    /// How long it takes to answer an unregistration.
    pub(crate) unregister_delay: Duration,
    /// Answers a registration with this error (name and text) instead of
    /// accepting it or refusing it as "already exists".
    pub(crate) register_error: Option<(&'static str, &'static str)>,
}

impl Behaviour {
    pub(crate) fn accepting() -> Behaviour {
        Behaviour {
            accept: true,
            strict: false,
            register_delay: Duration::ZERO,
            unregister_delay: Duration::ZERO,
            register_error: None,
        }
    }
}

/// A stand-in polkitd: owns `org.freedesktop.PolicyKit1`, records what the
/// agent asks of its authority, accepts or refuses the registration, and
/// calls the agent.
pub(crate) struct FakePolkitd {
    pub(crate) service: Service,
    pub(crate) log: Arc<Mutex<PolkitdLog>>,
}

impl FakePolkitd {
    /// `accept`: whether a registration succeeds; when not, it is refused the
    /// way polkitd refuses a second agent for a session.
    pub(crate) fn start(bus: &PrivateBus, accept: bool) -> FakePolkitd {
        FakePolkitd::behaving(
            bus,
            Behaviour {
                accept,
                ..Behaviour::accepting()
            },
        )
    }

    /// [`FakePolkitd::start`] with a chosen [`Behaviour`].
    pub(crate) fn behaving(bus: &PrivateBus, behaviour: Behaviour) -> FakePolkitd {
        let log = Arc::new(Mutex::new(PolkitdLog::default()));
        let record = Arc::clone(&log);
        let service = Service::start(bus, POLKIT_NAME, move |connection, call| {
            let delay = match member(call).as_str() {
                "RegisterAuthenticationAgent" => behaviour.register_delay,
                "UnregisterAuthenticationAgent" => behaviour.unregister_delay,
                _ => Duration::ZERO,
            };
            if delay.is_zero() {
                answer(connection, call, &record, behaviour);
                return;
            }
            // A slow answer is a thread of its own, so the calls that arrive
            // meanwhile are served at once, as polkitd serves them.
            let (connection, call, record) =
                (connection.clone(), call.clone(), Arc::clone(&record));
            thread::spawn(move || {
                thread::sleep(delay);
                answer(&connection, &call, &record, behaviour);
            });
        });
        FakePolkitd { service, log }
    }

    pub(crate) fn log(&self) -> PolkitdLog {
        match self.log.lock() {
            Ok(log) => log.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// The agent's unique name, once it registered.
    pub(crate) fn agent(&self, wait: Duration) -> String {
        let mut agent = None;
        eventually(wait, || {
            agent = self.log().agent;
            agent.is_some()
        });
        match agent {
            Some(agent) => agent,
            None => panic!("the agent never registered with the fake polkitd"),
        }
    }

    /// The fake's own unique name.
    pub(crate) fn unique(&self) -> String {
        unique(&self.service.connection)
    }
}

/// `connection`'s unique name.
pub(crate) fn unique(connection: &Connection) -> String {
    match connection.unique_name() {
        Some(name) => name.as_str().to_owned(),
        None => panic!("a bus connection has a unique name"),
    }
}

/// An identity list with the one user `uid`.
pub(crate) fn user(uid: u32) -> Vec<(&'static str, HashMap<&'static str, Value<'static>>)> {
    let mut details = HashMap::new();
    details.insert("uid", Value::from(uid));
    vec![("unix-user", details)]
}

/// Calls `BeginAuthentication` on `agent` from `connection`, on a thread;
/// the receiver gets the call's outcome.
pub(crate) fn begin(
    connection: &Connection,
    agent: &str,
    action_id: &'static str,
    message: &'static str,
    cookie: String,
    identities: Vec<(&'static str, HashMap<&'static str, Value<'static>>)>,
) -> Receiver<zbus::Result<Message>> {
    let connection = connection.clone();
    let agent = agent.to_owned();
    let (done, outcome) = mpsc::channel();
    thread::spawn(move || {
        let details: HashMap<&str, &str> = HashMap::new();
        let answer = connection.call_method(
            Some(agent.as_str()),
            AGENT_PATH,
            Some(AGENT_INTERFACE),
            "BeginAuthentication",
            &(
                action_id,
                message,
                "dialog-password",
                details,
                cookie.as_str(),
                identities,
            ),
        );
        let _ = done.send(answer);
    });
    outcome
}

/// Calls `CancelAuthentication(cookie)` on `agent` from `connection`.
pub(crate) fn cancel(connection: &Connection, agent: &str, cookie: &str) -> zbus::Result<Message> {
    connection.call_method(
        Some(agent),
        AGENT_PATH,
        Some(AGENT_INTERFACE),
        "CancelAuthentication",
        &(cookie,),
    )
}

/// The error name of `outcome`, or `"normal"` for a method return.
pub(crate) fn answer_name(outcome: &zbus::Result<Message>) -> String {
    match outcome {
        Ok(_) => "normal".to_owned(),
        Err(zbus::Error::MethodError(name, _, _)) => name.as_str().to_owned(),
        Err(zbus::Error::InputOutput(err)) if err.kind() == io::ErrorKind::TimedOut => {
            "timeout".to_owned()
        }
        Err(other) => format!("other: {other}"),
    }
}

/// What the fake logind was asked.
pub(crate) type LogindCalls = Arc<Mutex<Vec<String>>>;

/// How the fake logind describes the display session.
#[derive(Clone, Debug)]
pub(crate) struct SessionShape {
    pub(crate) display: &'static str,
    pub(crate) uid: u32,
    pub(crate) class: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) remote: bool,
}

impl Default for SessionShape {
    fn default() -> Self {
        Self {
            display: DISPLAY,
            uid: UID,
            class: "user",
            kind: "wayland",
            remote: false,
        }
    }
}

/// A stand-in logind: `GetUser`, the user's `Display`, the session's
/// properties, and `GetSessionByPID` failing as it does for the shell.
pub(crate) fn fake_logind(bus: &PrivateBus, shape: SessionShape) -> (Service, LogindCalls) {
    let calls: LogindCalls = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&calls);
    let service = Service::start(bus, LOGIND_NAME, move |connection, call| {
        let header = call.header();
        let member = member(call);
        if let Ok(mut calls) = record.lock() {
            calls.push(member.clone());
        }
        let user_path = format!("/org/freedesktop/login1/user/_{}", shape.uid);
        let session_path = format!("/org/freedesktop/login1/session/{}", shape.display);
        let path = |text: &str| match ObjectPath::try_from(text.to_owned()) {
            Ok(path) => path,
            Err(err) => panic!("a path: {err}"),
        };
        let answer = match member.as_str() {
            "GetUser" => connection.reply(&header, &path(&user_path)),
            "GetSessionByPID" => connection.reply_error(
                &header,
                "org.freedesktop.login1.NoSessionForPID",
                &("PID does not belong to any known session",),
            ),
            "Get" => {
                let display = StructureBuilder::new()
                    .add_field(shape.display.to_owned())
                    .add_field(path(&session_path))
                    .build();
                match display {
                    Ok(display) => connection.reply(&header, &Value::from(display)),
                    Err(err) => panic!("a structure: {err}"),
                }
            }
            "GetAll" => {
                let user = match StructureBuilder::new()
                    .add_field(shape.uid)
                    .add_field(path(&user_path))
                    .build()
                {
                    Ok(user) => user,
                    Err(err) => panic!("a structure: {err}"),
                };
                let mut properties: HashMap<&str, Value<'_>> = HashMap::new();
                properties.insert("Id", Value::from(shape.display));
                properties.insert("User", Value::from(user));
                properties.insert("Class", Value::from(shape.class));
                properties.insert("Type", Value::from(shape.kind));
                properties.insert("Remote", Value::from(shape.remote));
                connection.reply(&header, &properties)
            }
            _ => connection.reply_error(
                &header,
                "org.freedesktop.DBus.Error.UnknownMethod",
                &("not served",),
            ),
        };
        if let Err(err) = answer {
            eprintln!("the fake logind could not answer: {err}");
        }
    });
    (service, calls)
}

// ---- the log capture ----------------------------------------------------------------------

static CAPTURED: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static INSTALLED: OnceLock<bool> = OnceLock::new();

struct Capture;

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match CAPTURED.lock() {
            Ok(mut captured) => captured.extend_from_slice(bytes),
            Err(poisoned) => poisoned.into_inner().extend_from_slice(bytes),
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Installs, once per test binary, a global `TRACE` subscriber that keeps
/// every line any thread logs. Returns whether it is in place.
pub(crate) fn capture_logs() -> bool {
    *INSTALLED.get_or_init(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(|| Capture)
            .finish();
        tracing::subscriber::set_global_default(subscriber).is_ok()
    })
}

/// Whether one logged line contains every one of `needles`: a line of the
/// bus thread of one particular test bus carries that bus's address.
pub(crate) fn logged_line(needles: &[&str]) -> bool {
    let captured = match CAPTURED.lock() {
        Ok(captured) => captured.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    String::from_utf8_lossy(&captured)
        .lines()
        .any(|line| needles.iter().all(|needle| line.contains(needle)))
}

/// Whether anything logged so far contains `needle`.
pub(crate) fn logged(needle: &str) -> bool {
    let captured = match CAPTURED.lock() {
        Ok(captured) => captured.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    String::from_utf8_lossy(&captured).contains(needle)
}

// ---- the bus monitor ----------------------------------------------------------------------

/// What a monitor saw sent to the bus driver, in order: the member and the
/// first string of the body.
pub(crate) type DriverCalls = Arc<Mutex<Vec<(String, String)>>>;

/// A connection that became a monitor of the private bus (`BecomeMonitor`):
/// it receives a copy of every message and sends none.
pub(crate) struct Monitor {
    calls: DriverCalls,
    connection: Connection,
    thread: Option<JoinHandle<()>>,
}

impl Monitor {
    /// Starts watching `bus`. Panics where the daemon refuses: a test cannot
    /// say anything then.
    pub(crate) fn start(bus: &PrivateBus) -> Monitor {
        let connection = client(bus, Duration::from_secs(10));
        // The stream first: copies are queued only for a stream that exists.
        let messages = MessageIterator::from(&connection);
        let rules: Vec<&str> = Vec::new();
        if let Err(err) = connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus.Monitoring"),
            "BecomeMonitor",
            &(rules, 0u32),
        ) {
            panic!("the bus does not let a test monitor it: {err}");
        }
        let calls: DriverCalls = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&calls);
        let thread = thread::spawn(move || {
            for message in messages {
                let Ok(message) = message else { return };
                let header = message.header();
                let to_driver = header
                    .destination()
                    .is_some_and(|d| d.as_str() == "org.freedesktop.DBus");
                if header.message_type() != Type::MethodCall || !to_driver {
                    continue;
                }
                let first = message
                    .body()
                    .deserialize::<(String,)>()
                    .map(|(first,)| first)
                    .unwrap_or_default();
                let member = member(&message);
                if let Ok(mut calls) = record.lock() {
                    calls.push((member, first));
                }
            }
        });
        Monitor {
            calls,
            connection,
            thread: Some(thread),
        }
    }

    /// The driver calls seen so far.
    pub(crate) fn calls(&self) -> Vec<(String, String)> {
        match self.calls.lock() {
            Ok(calls) => calls.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.connection.clone().close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
