//! The notification server against a real bus and a real client.
//!
//! The bus is a private `dbus-daemon` started here from a configuration of the
//! test's own (a unix socket in a temporary directory, no service directories,
//! so nothing is ever activated), and it is killed through the `Child` handle
//! this test owns. Nothing here can reach the session bus or its
//! `org.freedesktop.Notifications` name. Without `dbus-daemon` the test says
//! it was skipped and passes.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use celestina::providers::notifications::{
    spawn_on, Bus, NotificationsCommand, NotificationsEvent, NotificationsSnapshot, BUS_NAME,
    INTERFACE, OBJECT_PATH,
};
use celestina_shell::notifications::{capabilities, MAX_ACTIONS, MAX_BODY_CHARS};
use zbus::blocking::{connection::Builder, Connection, MessageIterator};
use zbus::message::Type as MessageType;
use zbus::zvariant::Value;
use zbus::MatchRule;

/// Long enough for a bus round trip plus a tick, still bounded.
const DEADLINE: Duration = Duration::from_secs(5);

/// A `dbus-daemon` of the test's own, killed by its own handle when dropped.
struct PrivateBus {
    child: Child,
    dir: PathBuf,
    address: String,
}

impl PrivateBus {
    fn start() -> Option<Self> {
        let dir = std::env::temp_dir().join(format!(
            "celestina-notifications-bus-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        let config = dir.join("bus.conf");
        let socket = dir.join("bus");
        let text = format!(
            r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
            socket.display()
        );
        std::fs::write(&config, text).ok()?;
        let spawned = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .arg("--nofork")
            .arg("--nopidfile")
            .arg("--print-address=1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = spawned else {
            let _ = std::fs::remove_dir_all(&dir);
            return None;
        };
        let mut address = String::new();
        let read = child
            .stdout
            .take()
            .map(|out| BufReader::new(out).read_line(&mut address));
        if !matches!(read, Some(Ok(n)) if n > 0) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&dir);
            return None;
        }
        Some(Self {
            child,
            dir,
            address: address.trim().to_owned(),
        })
    }

    fn connect(&self) -> Connection {
        match Builder::address(self.address.as_str()).and_then(Builder::build) {
            Ok(connection) => connection,
            Err(err) => panic!("cannot connect to the private bus: {err}"),
        }
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A signal the server emitted, as the client saw it.
#[derive(Debug, PartialEq, Eq)]
enum Signal {
    Closed(u32, u32),
    Action(u32, String),
}

/// Forwards the server's signals into a channel, so the test waits on them
/// with a deadline. The thread ends when the bus goes away.
fn signals(connection: &Connection) -> Receiver<Signal> {
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .interface(INTERFACE)
        .and_then(|b| b.path(OBJECT_PATH))
        .map(zbus::match_rule::Builder::build);
    let iterator =
        match rule.and_then(|rule| MessageIterator::for_match_rule(rule, connection, Some(64))) {
            Ok(iterator) => iterator,
            Err(err) => panic!("cannot subscribe to the server's signals: {err}"),
        };
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for message in iterator {
            let Ok(message) = message else { break };
            let header = message.header();
            let signal = match header.member().map(|m| m.as_str()) {
                Some("NotificationClosed") => message
                    .body()
                    .deserialize::<(u32, u32)>()
                    .ok()
                    .map(|(id, reason)| Signal::Closed(id, reason)),
                Some("ActionInvoked") => message
                    .body()
                    .deserialize::<(u32, String)>()
                    .ok()
                    .map(|(id, key)| Signal::Action(id, key)),
                _ => None,
            };
            if let Some(signal) = signal {
                if tx.send(signal).is_err() {
                    break;
                }
            }
        }
    });
    rx
}

fn next_signal(rx: &Receiver<Signal>) -> Signal {
    match rx.recv_timeout(DEADLINE) {
        Ok(signal) => signal,
        Err(err) => panic!("no signal within {DEADLINE:?}: {err}"),
    }
}

/// Waits for a snapshot that satisfies `wanted`.
fn snapshot_where(
    rx: &Receiver<NotificationsEvent>,
    wanted: impl Fn(&NotificationsSnapshot) -> bool,
) -> NotificationsSnapshot {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(NotificationsEvent::Snapshot(snapshot)) if wanted(&snapshot) => return snapshot,
            Ok(_) => {}
            Err(err) => panic!("no matching snapshot within {DEADLINE:?}: {err}"),
        }
    }
}

fn owner_event(rx: &Receiver<NotificationsEvent>) -> Option<String> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(NotificationsEvent::Owner(owner)) => return owner,
            Ok(_) => {}
            Err(err) => panic!("no owner event within {DEADLINE:?}: {err}"),
        }
    }
}

fn notify(
    connection: &Connection,
    summary: &str,
    body: &str,
    actions: &[String],
    hints: &HashMap<&str, Value<'_>>,
    expire_timeout: i32,
) -> u32 {
    let reply = connection.call_method(
        Some(BUS_NAME),
        OBJECT_PATH,
        Some(INTERFACE),
        "Notify",
        &(
            "Test",
            0u32,
            "",
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        ),
    );
    match reply.and_then(|reply| reply.body().deserialize::<u32>()) {
        Ok(id) => id,
        Err(err) => panic!("Notify failed: {err}"),
    }
}

#[test]
fn the_server_bounds_answers_and_steps_aside_on_a_private_bus() {
    let Some(bus) = PrivateBus::start() else {
        println!("skipped: dbus-daemon is absent");
        return;
    };
    let shutdown = Arc::new(AtomicBool::new(false));
    let (events, first_events) = mpsc::channel();
    let (provider, worker) = match spawn_on(
        Bus::Address(bus.address.clone()),
        move |event| events.send(event).is_ok(),
        &shutdown,
    ) {
        Ok(started) => started,
        Err(err) => panic!("the provider does not start: {err}"),
    };
    assert_eq!(
        owner_event(&first_events),
        None,
        "the first server owns the name"
    );

    let client = bus.connect();
    let closed = signals(&client);

    // Hostile input is bounded before anything holds it.
    let body = "x".repeat(50_000);
    let actions: Vec<String> = (0..1_000).map(|i| format!("a{i}")).collect();
    let mut hints = HashMap::new();
    hints.insert("image-path", Value::from("../../etc/passwd"));
    hints.insert("urgency", Value::from(1u8));
    let id = notify(&client, "Summary", &body, &actions, &hints, 0);
    let snapshot = snapshot_where(&first_events, |s| s.live.iter().any(|n| n.id == id));
    let posted = snapshot.live.iter().find(|n| n.id == id).cloned();
    let Some(posted) = posted else {
        panic!("the posted notification is missing");
    };
    assert!(posted.body.chars().count() <= MAX_BODY_CHARS);
    assert!(posted.actions.len() <= MAX_ACTIONS);
    assert_eq!(posted.image, None);
    assert_eq!(snapshot.toasts.first().map(|n| n.id), Some(id));

    // The capabilities are the core's.
    let reply = client.call_method(
        Some(BUS_NAME),
        OBJECT_PATH,
        Some(INTERFACE),
        "GetCapabilities",
        &(),
    );
    let listed = reply.and_then(|reply| reply.body().deserialize::<Vec<String>>());
    assert_eq!(
        listed.ok(),
        Some(capabilities().iter().map(|c| (*c).to_owned()).collect())
    );

    // A producer withdrawing its notification is reason 3.
    let reply = client.call_method(
        Some(BUS_NAME),
        OBJECT_PATH,
        Some(INTERFACE),
        "CloseNotification",
        &(id,),
    );
    assert!(reply.is_ok(), "CloseNotification answers");
    assert_eq!(next_signal(&closed), Signal::Closed(id, 3));

    // Invoking an offered action: ActionInvoked, then closed as dismissed.
    let offered = vec!["open".to_owned(), "Open".to_owned()];
    let id = notify(&client, "Act", "", &offered, &HashMap::new(), 0);
    snapshot_where(&first_events, |s| s.live.iter().any(|n| n.id == id));
    assert!(provider
        .send(NotificationsCommand::Invoke(id, "nope".to_owned()))
        .is_ok());
    assert!(provider
        .send(NotificationsCommand::Invoke(id, "open".to_owned()))
        .is_ok());
    assert_eq!(next_signal(&closed), Signal::Action(id, "open".to_owned()));
    assert_eq!(next_signal(&closed), Signal::Closed(id, 2));

    // An explicit short timeout expires as reason 1.
    let id = notify(&client, "Short", "", &[], &HashMap::new(), 300);
    assert_eq!(next_signal(&closed), Signal::Closed(id, 1));

    // Hovering holds the stack still; leaving lets it expire.
    assert!(provider.send(NotificationsCommand::Hover(true)).is_ok());
    let id = notify(&client, "Held", "", &[], &HashMap::new(), 300);
    assert!(matches!(
        closed.recv_timeout(Duration::from_millis(900)),
        Err(RecvTimeoutError::Timeout)
    ));
    assert!(provider.send(NotificationsCommand::Hover(false)).is_ok());
    assert_eq!(next_signal(&closed), Signal::Closed(id, 1));

    // A second server on the same bus steps aside and serves nothing.
    let second_shutdown = Arc::new(AtomicBool::new(false));
    let (events, second_events) = mpsc::channel();
    let (second, second_worker) = match spawn_on(
        Bus::Address(bus.address.clone()),
        move |event| events.send(event).is_ok(),
        &second_shutdown,
    ) {
        Ok(started) => started,
        Err(err) => panic!("the second provider does not start: {err}"),
    };
    let owner = owner_event(&second_events);
    let Some(owner) = owner else {
        panic!("the second server claims to own the name");
    };
    assert!(owner.starts_with(":1."), "{owner}");
    let id = notify(&client, "Still first", "", &[], &HashMap::new(), 0);
    snapshot_where(&first_events, |s| s.live.iter().any(|n| n.id == id));
    assert!(matches!(
        second_events.recv_timeout(Duration::from_millis(600)),
        Err(RecvTimeoutError::Timeout)
    ));
    assert!(second.send(NotificationsCommand::MarkRead).is_err());

    second_worker.join();
    worker.join();
}
