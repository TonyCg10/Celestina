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

#[test]
fn the_server_answers_magnetitas_flow_and_never_takes_a_taken_name() {
    let Some(bus) = PrivateBus::start() else {
        // Recorded rather than silently passed: without dbus-daemon this
        // machine cannot run the only check that needs two processes.
        eprintln!("skipped: no dbus-daemon to start a private session bus");
        return;
    };

    let mut helper = Helper::start(&bus.address);
    // The provider appears once the helper owns the name; on a private bus
    // nothing else does.
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
            let toasts = notifications(frame)?.get("toasts")?.as_array()?;
            let entry = toasts
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_u64) == Some(u64::from(id)))?;
            Some(entry.clone())
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
            let toasts = payload.get("toasts")?.as_array()?;
            if toasts
                .iter()
                .any(|entry| entry.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
            {
                return None;
            }
            let history = payload.get("history")?.as_array()?;
            history
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
                .cloned()
        })
        .expect("a withdrawn notification is remembered, not lost");
    assert_eq!(
        remembered.get("summary").and_then(Value::as_str),
        Some("Pixel")
    );

    // The property this session depends on: a second shell finds the name
    // taken and serves nothing, rather than displacing the first.
    //
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
    let after = notify(&proxy, 0, "Still served by the first shell");
    assert_ne!(after, 0);
}
