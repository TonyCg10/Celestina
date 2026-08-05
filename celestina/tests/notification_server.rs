//! The notification server against a real bus and a real producer.
//!
//! Every other test in this suite proves a rule in isolation. This one proves
//! the two things that only show up when a process, a bus and a client are all
//! present at once:
//!
//! - Magnetita's exact call shape is served as it is sent — `Notify` with no
//!   actions, no hints, the `phone` icon and the server's default timeout, then
//!   a replacement by id, then `CloseNotification`
//!   (see `celestina-rs/crates/magnetitad/src/notify.rs`);
//! - a second shell on the same bus **does not** take the name from the first.
//!   That is the property protecting this session's running notification
//!   server, and it cannot be checked without two processes.
//!
//! The bus is private and started here, so nothing in this test can reach the
//! author's session, take its notification name or show anything on screen.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value as ZValue;

const SERVICE: &str = "org.freedesktop.Notifications";
const OBJECT: &str = "/org/freedesktop/Notifications";
/// Generous against process start plus a bus round trip, and still bounded: a
/// test that waits forever is a test that hangs a pipeline.
const DEADLINE: Duration = Duration::from_secs(15);
/// How long a property that must never hold is watched for. The helper
/// publishes several frames a second, so this is many chances to be wrong.
const WATCH: Duration = Duration::from_secs(3);

/// A private session bus, torn down with the test whatever it fails on.
struct PrivateBus {
    daemon: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Option<Self> {
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--print-address", "--nofork", "--nopidfile"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let stdout = daemon.stdout.take()?;
        let mut address = String::new();
        BufReader::new(stdout).read_line(&mut address).ok()?;
        let address = address.trim().to_owned();
        if address.is_empty() {
            let _ = daemon.kill();
            return None;
        }
        Some(Self { daemon, address })
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// One provider helper, with its stdin held open so it does not exit, and its
/// frames readable.
struct Helper {
    process: Child,
    frames: BufReader<ChildStdout>,
}

impl Helper {
    fn start(address: &str) -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_celestina-provider-adapter"))
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the provider helper starts");
        let frames = BufReader::new(process.stdout.take().expect("the helper's frames"));
        Self { process, frames }
    }

    /// Reads frames for `window`, returning the first that satisfies `wanted`.
    /// Unlike [`Helper::wait_for`], running out of time is the expected result:
    /// this is how a property that must *never* hold is checked.
    fn watch<T>(
        &mut self,
        window: Duration,
        mut wanted: impl FnMut(&Value) -> Option<T>,
    ) -> Option<T> {
        let deadline = Instant::now() + window;
        let mut line = String::new();
        while Instant::now() < deadline {
            line.clear();
            if self.frames.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(found) = wanted(&frame) {
                return Some(found);
            }
        }
        None
    }

    /// Reads frames until one satisfies `wanted`, or the deadline passes.
    fn wait_for<T>(&mut self, wanted: impl Fn(&Value) -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + DEADLINE;
        let mut line = String::new();
        while Instant::now() < deadline {
            line.clear();
            if self.frames.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let Ok(frame) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(found) = wanted(&frame) {
                return Some(found);
            }
        }
        None
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// The `notifications` provider's published payload, if the frame carries one.
fn notifications(frame: &Value) -> Option<&Value> {
    frame.get("providers")?.get("notifications")
}

/// Posts exactly what `magnetitad` posts.
fn notify(proxy: &Proxy<'_>, replaces: u32, body: &str) -> u32 {
    let actions: Vec<&str> = Vec::new();
    let hints: HashMap<&str, ZValue> = HashMap::new();
    proxy
        .call(
            "Notify",
            &(
                "Magnetita",
                replaces,
                "phone",
                "Pixel",
                body,
                actions,
                hints,
                -1i32,
            ),
        )
        .expect("the server answers Notify")
}

fn client(address: &str) -> Connection {
    // The client speaks to the private bus by address, never to the session.
    zbus::blocking::connection::Builder::address(address)
        .and_then(zbus::blocking::connection::Builder::build)
        .expect("a client connection to the private bus")
}

/// Posts a notification that offers one real action, which is what a producer
/// with buttons sends and what `actions=[]` never exercises.
fn notify_with_action(proxy: &Proxy<'_>, replaces: u32, body: &str) -> u32 {
    let hints: HashMap<&str, ZValue> = HashMap::new();
    proxy
        .call(
            "Notify",
            &(
                "Magnetita",
                replaces,
                "phone",
                "Pixel",
                body,
                vec!["open", "Abrir"],
                hints,
                -1i32,
            ),
        )
        .expect("the server answers Notify")
}

/// The rows of one published list field.
fn rows<'a>(payload: &'a Value, field: &str) -> &'a [Value] {
    payload
        .get(field)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Magnetita's own shape: no actions, no hints, replacement by id, then close.
#[test]
fn the_server_answers_magnetitas_flow() {
    let Some(bus) = PrivateBus::start() else {
        eprintln!("skipped: no dbus-daemon to start a private session bus");
        return;
    };

    let mut helper = Helper::start(&bus.address);
    assert!(
        helper
            .wait_for(|frame| notifications(frame).map(|_| ()))
            .is_some(),
        "the helper never published a notifications provider"
    );

    let connection = client(&bus.address);
    let proxy = Proxy::new(&connection, SERVICE, OBJECT, SERVICE).expect("the server is there");

    let id = notify(&proxy, 0, "A message arrived");
    assert_ne!(id, 0, "an id is never zero");

    // A replacement keeps the id: one conversation, not a stream.
    let replaced = notify(&proxy, id, "Two messages arrived");
    assert_eq!(replaced, id);

    let live = helper
        .wait_for(|frame| {
            let toasts = rows(notifications(frame)?, "toasts");
            toasts
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
                .cloned()
        })
        .expect("the replacement reaches the panel");
    assert_eq!(live.get("app").and_then(Value::as_str), Some("Magnetita"));
    assert_eq!(live.get("summary").and_then(Value::as_str), Some("Pixel"));
    assert_eq!(
        live.get("body").and_then(Value::as_str),
        Some("Two messages arrived"),
        "the panel shows the replacement, not what it replaced"
    );

    let capabilities: Vec<String> = proxy
        .call("GetCapabilities", &())
        .expect("the server answers GetCapabilities");
    assert!(capabilities.iter().any(|entry| entry == "actions"));
    assert!(
        !capabilities.iter().any(|entry| entry == "body-markup"),
        "claiming markup would invite producers to send it and have it shown raw"
    );

    let (name, vendor, _version, specification): (String, String, String, String) = proxy
        .call("GetServerInformation", &())
        .expect("the server answers GetServerInformation");
    assert_eq!(name, "Celestina");
    assert_eq!(vendor, "celestina");
    assert_eq!(specification, "1.2");

    // A producer withdrawing its own notification: it leaves the panel and is
    // remembered, rather than disappearing.
    let () = proxy
        .call("CloseNotification", &(id,))
        .expect("the server answers CloseNotification");
    let remembered = helper
        .wait_for(|frame| {
            let payload = notifications(frame)?;
            if rows(payload, "toasts")
                .iter()
                .any(|entry| entry.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
            {
                return None;
            }
            rows(payload, "history")
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
                .cloned()
        })
        .expect("a withdrawn notification is remembered, not lost");
    assert_eq!(
        remembered.get("summary").and_then(Value::as_str),
        Some("Pixel")
    );
}

/// A notification that offers a button, published in the shape the host can
/// decode.
///
/// The live failure this covers: actions used to travel *inside* their
/// notification, the C++ decoder refused the whole frame for nesting a list,
/// and every unrelated provider's reading was cleared with it. The actions must
/// still arrive — dropping them would "fix" the rejection by losing the feature.
#[test]
fn an_offered_action_is_published_flat_and_still_names_its_notification() {
    let Some(bus) = PrivateBus::start() else {
        eprintln!("skipped: no dbus-daemon to start a private session bus");
        return;
    };

    let mut helper = Helper::start(&bus.address);
    assert!(
        helper
            .wait_for(|frame| notifications(frame).map(|_| ()))
            .is_some(),
        "the helper never claimed the name"
    );

    let connection = client(&bus.address);
    let proxy = Proxy::new(&connection, SERVICE, OBJECT, SERVICE).expect("the server is there");

    let id = notify_with_action(&proxy, 0, "Un mensaje con bot\u{f3}n");
    assert_ne!(id, 0);

    let payload = helper
        .wait_for(|frame| {
            let payload = notifications(frame)?;
            rows(payload, "actions")
                .iter()
                .any(|action| {
                    action.get("notification").and_then(Value::as_u64) == Some(u64::from(id))
                })
                .then(|| payload.clone())
        })
        .expect("the offered action reaches the panel");

    // The action is carried, and says which notification offers it.
    let action = rows(&payload, "actions")
        .iter()
        .find(|action| action.get("notification").and_then(Value::as_u64) == Some(u64::from(id)))
        .expect("the action names its notification");
    assert_eq!(action.get("key").and_then(Value::as_str), Some("open"));
    assert_eq!(action.get("label").and_then(Value::as_str), Some("Abrir"));

    // The notification itself says how many it offers, without carrying them.
    let toast = rows(&payload, "toasts")
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
        .expect("the notification is live");
    assert_eq!(toast.get("actionCount").and_then(Value::as_u64), Some(1));

    // And nothing published nests a list: that is the rule the host enforces
    // and the one this payload broke on a live session.
    for (field, value) in payload.as_object().expect("a payload") {
        let Some(rows) = value.as_array() else {
            continue;
        };
        for row in rows {
            let row = row
                .as_object()
                .unwrap_or_else(|| panic!("{field} carries something that is not a row"));
            for (key, nested) in row {
                assert!(
                    !nested.is_array() && !nested.is_object(),
                    "{field}.{key} nests a list the host will refuse"
                );
            }
        }
    }
}

/// The property this session depends on: a second shell finds the name taken
/// and serves nothing, rather than displacing the first.
#[test]
fn a_second_shell_never_takes_a_name_that_is_already_owned() {
    let Some(bus) = PrivateBus::start() else {
        eprintln!("skipped: no dbus-daemon to start a private session bus");
        return;
    };

    let mut first = Helper::start(&bus.address);
    assert!(
        first
            .wait_for(|frame| notifications(frame).map(|_| ()))
            .is_some(),
        "the first helper never claimed the name"
    );

    // Watched over a window rather than judged on one frame: a helper that
    // published its other providers first and only then claimed the name would
    // pass a single-frame check for the wrong reason.
    let mut second = Helper::start(&bus.address);
    let mut publishing = false;
    let watched = second.watch(WATCH, |frame| {
        let providers = frame.get("providers")?.as_object()?;
        if !providers.is_empty() {
            publishing = true;
        }
        providers.contains_key("notifications").then_some(())
    });
    assert!(
        publishing,
        "the second helper never published anything, so nothing was observed"
    );
    assert!(
        watched.is_none(),
        "a second shell must not claim a notification name that is already owned"
    );

    // The first one still owns it and still answers.
    let connection = client(&bus.address);
    let proxy = Proxy::new(&connection, SERVICE, OBJECT, SERVICE).expect("the first shell answers");
    assert_ne!(notify(&proxy, 0, "Still served by the first shell"), 0);
}
