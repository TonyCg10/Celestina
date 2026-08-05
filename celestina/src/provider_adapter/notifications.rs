//! The session's notification server, when the session does not already have
//! one.
//!
//! This is the bus half of [`celestina_shell_core::notifications`]: it turns a
//! `Notify` method call into an [`Incoming`], hands it to the state machine and
//! reports back what the machine decided. Every rule about identity, expiry,
//! bounds and caps lives there; nothing is decided here.
//!
//! The name is claimed **only when it is free**. This session runs Noctalia's
//! server today, and a shell that took `org.freedesktop.Notifications` from a
//! running server would silently break every notification the person still
//! depends on. So the request carries `DoNotQueue` and nothing else: no
//! replacement, no queueing to inherit the name later, and a reply that is not
//! `PrimaryOwner` means this provider simply does not exist this session. The
//! same shape `TrayWatcherService` uses for the tray.
//!
//! The object is served before the name is requested, so a client that sees the
//! name always finds something behind it.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell_core::notifications::{
    capabilities, CloseReason, Incoming, Notification, Notifications, Urgency, MAX_HISTORY,
};
use celestina_shell_core::runtime::ProviderRuntime;
use celestina_shell_core::snapshot::{Payload, ProviderId};
use serde_json::{json, Value};
use zbus::blocking::Connection;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::zvariant::OwnedValue;

use super::tools::lock_runtime;

pub const NAME: &str = "notifications";

const BUS_NAME: &str = "org.freedesktop.Notifications";
const OBJECT_PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";

/// How often deadlines are checked. A toast leaving a quarter of a second late
/// is imperceptible; a thread waking more often than this is not free.
const TICK: Duration = Duration::from_millis(250);
/// How many ended notifications the panel is handed. History is capped in the
/// core; this is how much of it crosses the wire at once.
const PUBLISHED_HISTORY: usize = 20;
/// The action rows that cross with them. The host bounds list length itself;
/// this keeps the helper from ever reaching that bound and having its frame
/// refused, which would take the whole aggregate down with it.
const MAX_PUBLISHED_ACTIONS: usize = 32;

/// The one state machine this helper serves, shared between the bus thread and
/// the tick that expires what it holds.
static STATE: OnceLock<Mutex<Notifications>> = OnceLock::new();

fn state() -> &'static Mutex<Notifications> {
    STATE.get_or_init(|| Mutex::new(Notifications::new()))
}

fn lock_state() -> std::sync::MutexGuard<'static, Notifications> {
    match state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Reads the `urgency` hint. Anything that is not a byte the specification
/// defines leaves the urgency where the core's default puts it.
fn urgency_of(hints: &HashMap<String, OwnedValue>) -> Urgency {
    hints
        .get("urgency")
        .and_then(|value| u8::try_from(value).ok())
        .map_or(Urgency::Normal, Urgency::from_hint)
}

/// Reads the `image-path` hint as text. Raw pixel hints — `image-data`,
/// `icon_data` — are deliberately ignored: this shell does not decode bytes a
/// stranger sent it, and the core would refuse them anyway.
fn image_of(hints: &HashMap<String, OwnedValue>) -> Option<String> {
    hints
        .get("image-path")
        .or_else(|| hints.get("image_path"))
        .and_then(|value| <&str>::try_from(value).ok())
        .map(ToOwned::to_owned)
}

/// One notification as a **flat** row.
///
/// Deliberately without its actions. The host accepts a provider payload of
/// scalars plus, for a list field, one bounded array of flat rows — one level
/// of structure, so a row that nested its own list would carry the unbounded
/// depth that rule exists to forbid. This row used to carry an `actions` array
/// and the host rejected the whole frame because of it; the actions now travel
/// beside the notifications in their own flat list.
fn entry_json(entry: &Notification) -> Value {
    json!({
        "id": entry.id,
        "app": entry.app_name,
        "summary": entry.summary,
        "body": entry.body,
        "urgency": match entry.urgency {
            Urgency::Low => "low",
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        },
        "read": entry.read,
        "actionCount": entry.actions.len(),
    })
}

/// Every action of every listed notification, each naming the notification it
/// belongs to. Flat rows, one bounded list, joined by the surface.
fn actions_json<'a>(entries: impl Iterator<Item = &'a Notification>) -> Vec<Value> {
    entries
        .flat_map(|entry| {
            entry.actions.iter().map(move |action| {
                json!({
                    "notification": entry.id,
                    "key": action.key,
                    "label": action.label,
                })
            })
        })
        .take(MAX_PUBLISHED_ACTIONS)
        .collect()
}

/// Publishes what the panel may show. The toast list is what the core says may
/// interrupt right now, which is not the same as everything live: while quiet,
/// only critical notifications are in it.
fn publish(runtime: &Mutex<ProviderRuntime>, id: &ProviderId) {
    let held = lock_state();
    let mut payload = Payload::new();
    payload.insert(
        "toasts".to_owned(),
        Value::Array(held.toasts().into_iter().map(entry_json).collect()),
    );
    payload.insert(
        "history".to_owned(),
        Value::Array(
            held.history()
                .iter()
                .take(PUBLISHED_HISTORY)
                .map(entry_json)
                .collect(),
        ),
    );
    payload.insert("unread".to_owned(), Value::from(held.unread()));
    payload.insert("quiet".to_owned(), Value::from(held.is_quiet()));
    payload.insert(
        "historyTruncated".to_owned(),
        Value::from(held.history().len() > PUBLISHED_HISTORY),
    );
    payload.insert("historyCap".to_owned(), Value::from(MAX_HISTORY));
    // The actions of everything published above, in one flat sibling list.
    // They cannot live inside their notification: the host takes one level of
    // structure, and a row carrying its own list is a frame it refuses — which
    // on a live session emptied the entire bar.
    payload.insert(
        "actions".to_owned(),
        Value::Array(actions_json(
            held.toasts()
                .into_iter()
                .chain(held.history().iter().take(PUBLISHED_HISTORY)),
        )),
    );
    drop(held);

    if let Err(error) = lock_runtime(runtime).publish(id, payload) {
        eprintln!("celestina-provider-adapter: notifications: {error}");
    }
}

/// The served interface. It holds no notification state of its own: every
/// method is a translation into the core and back.
struct Server {
    runtime: Arc<Mutex<ProviderRuntime>>,
    id: ProviderId,
}

// The specification fixes these signatures: every method is a bus entry point,
// so it takes `&self` whether or not it reads state, and `Notify` receives its
// hint map by value because that is what zvariant deserializes into. Neither is
// debt to pay down; both are the contract this type exists to satisfy.
#[allow(
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::too_many_arguments,
    reason = "the freedesktop notification interface fixes these signatures"
)]
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    /// The specification's `Notify`. Returns the id the notification is known
    /// by, which is `replaces_id` when that named something still live.
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let incoming = Incoming {
            app_name: app_name.to_owned(),
            replaces_id,
            app_icon: app_icon.to_owned(),
            summary: summary.to_owned(),
            body: body.to_owned(),
            actions,
            urgency: urgency_of(&hints),
            image: image_of(&hints),
            expire_timeout,
        };

        let id = lock_state().post(&incoming, now_ms());
        publish(&self.runtime, &self.id);
        id
    }

    /// The specification's `CloseNotification`. A producer withdrawing its own
    /// notification is a `Requested` close, which is not the person having
    /// dealt with it.
    fn close_notification(&self, id: u32) {
        let closed = lock_state().close(id, CloseReason::Requested);
        if let Some(closed) = closed {
            announce_closed(closed.id, closed.reason);
            publish(&self.runtime, &self.id);
        }
    }

    fn get_capabilities(&self) -> Vec<String> {
        capabilities()
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect()
    }

    /// name, vendor, version, specification version.
    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Celestina".to_owned(),
            "celestina".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            "1.2".to_owned(),
        )
    }
}

/// The bus, once it exists. Signals are emitted through the connection rather
/// than through the interface's own emitter, so the tick thread can report an
/// expiry without holding an object-server reference.
static BUS: OnceLock<Connection> = OnceLock::new();

fn announce_closed(id: u32, reason: CloseReason) {
    let Some(connection) = BUS.get() else {
        return;
    };
    let _ = connection.emit_signal(
        Option::<&str>::None,
        OBJECT_PATH,
        INTERFACE,
        "NotificationClosed",
        &(id, reason as u32),
    );
}

/// Tells the producer that a person pressed one of its buttons. Only ever
/// called for a key the core confirmed that notification offers.
fn announce_action(id: u32, key: &str) {
    let Some(connection) = BUS.get() else {
        return;
    };
    let _ = connection.emit_signal(
        Option::<&str>::None,
        OBJECT_PATH,
        INTERFACE,
        "ActionInvoked",
        &(id, key),
    );
}

/// This helper's own monotonic clock. Deadlines are compared against it and
/// never against the wall clock, which a session may move.
static STARTED: OnceLock<Instant> = OnceLock::new();

fn now_ms() -> u64 {
    let started = STARTED.get_or_init(Instant::now);
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// What asking for the session's notification name ended in.
enum Claim {
    /// This shell is the session's notification server.
    Owned,
    /// Somebody else already is. The normal state during the Noctalia
    /// handover, and not a failure.
    Taken,
    /// The bus itself could not be reached or served.
    Failed(zbus::Error),
}

/// Claims the name only if nothing owns it, and serves the object first.
fn serve(runtime: &Arc<Mutex<ProviderRuntime>>, id: &ProviderId) -> Claim {
    let built = zbus::blocking::connection::Builder::session().and_then(|builder| {
        builder
            .serve_at(
                OBJECT_PATH,
                Server {
                    runtime: Arc::clone(runtime),
                    id: id.clone(),
                },
            )?
            .build()
    });
    let connection = match built {
        Ok(connection) => connection,
        Err(error) => return Claim::Failed(error),
    };

    // No ReplaceExisting and no queueing: this shell either is the session's
    // notification server or is not one at all. A name somebody else holds
    // comes back as an error from zbus and as a plain reply from the bus; both
    // mean the same thing here, and neither is a reason to try harder.
    match connection.request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(RequestNameReply::PrimaryOwner) => {
            let _ = BUS.set(connection);
            Claim::Owned
        }
        Ok(_) | Err(zbus::Error::NameTaken) => Claim::Taken,
        Err(error) => Claim::Failed(error),
    }
}

pub fn spawn(runtime: &Arc<Mutex<ProviderRuntime>>) -> io::Result<()> {
    let Ok(id) = ProviderId::new(NAME) else {
        eprintln!("celestina-provider-adapter: notifications: unusable provider name");
        return Ok(());
    };

    lock_runtime(runtime).register(id.clone());
    let runtime = Arc::clone(runtime);
    thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || run(&runtime, &id))?;
    Ok(())
}

/// Commands the panel sends about what it is showing.
///
/// Dismissing is the person having dealt with something, which is why it is a
/// different reason from a timeout and from a producer's own withdrawal.
pub fn action(
    verb: &str,
    options: &Payload,
    runtime: &Mutex<ProviderRuntime>,
    id: &ProviderId,
) -> Result<(), String> {
    let notification_id = || -> Result<u32, String> {
        options
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|raw| u32::try_from(raw).ok())
            .filter(|id| *id != 0)
            .ok_or_else(|| format!("'{NAME}' needs the notification id to {verb}"))
    };

    match verb {
        "dismiss" => {
            let target = notification_id()?;
            let closed = lock_state().close(target, CloseReason::Dismissed);
            let closed = closed.ok_or_else(|| format!("no notification {target} is showing"))?;
            announce_closed(closed.id, closed.reason);
        }
        "invoke" => {
            let target = notification_id()?;
            let key = options
                .get("action")
                .and_then(Value::as_str)
                .filter(|key| !key.is_empty())
                .ok_or_else(|| format!("'{NAME}' needs the action to invoke"))?;
            // A key the producer never offered is never sent on its behalf.
            if !lock_state().accepts_action(target, key) {
                return Err(format!("notification {target} offers no action '{key}'"));
            }
            announce_action(target, key);
            let closed = lock_state().close(target, CloseReason::Dismissed);
            if let Some(closed) = closed {
                announce_closed(closed.id, closed.reason);
            }
        }
        "mark-read" => lock_state().mark_read(),
        "clear-history" => lock_state().clear_history(),
        "quiet-on" | "quiet-off" | "quiet-toggle" => {
            let mut held = lock_state();
            let quiet = match verb {
                "quiet-on" => true,
                "quiet-off" => false,
                _ => !held.is_quiet(),
            };
            held.set_quiet(quiet);
            drop(held);
            // Silencing a session is a choice, not a reading: a person who
            // silenced it did not mean "until the next restart".
            if let Err(error) = super::settings::remember(|settings| settings.quiet = quiet) {
                eprintln!("celestina-provider-adapter: {NAME}: {error}");
            }
        }
        _ => return Err(format!("'{NAME}' does not serve the verb '{verb}'")),
    }

    publish(runtime, id);
    Ok(())
}

fn run(runtime: &Arc<Mutex<ProviderRuntime>>, id: &ProviderId) {
    match serve(runtime, id) {
        Claim::Owned => {}
        Claim::Taken => {
            eprintln!(
                "celestina-provider-adapter: notifications: another server owns {BUS_NAME}; \
                 this shell is not serving notifications this session"
            );
            lock_runtime(runtime).withdraw(id);
            return;
        }
        Claim::Failed(error) => {
            eprintln!("celestina-provider-adapter: notifications: {error}");
            lock_runtime(runtime).withdraw(id);
            return;
        }
    }

    // The session starts silenced if that is what the person chose; nothing
    // else here survives a restart on purpose.
    lock_state().set_quiet(super::settings::current().quiet);

    publish(runtime, id);
    loop {
        let expired = lock_state().expire(now_ms());
        for closed in &expired {
            announce_closed(closed.id, closed.reason);
        }
        if !expired.is_empty() {
            publish(runtime, id);
        }
        thread::sleep(TICK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value as ZValue;

    fn hint(key: &str, value: ZValue<'static>) -> HashMap<String, OwnedValue> {
        let mut hints = HashMap::new();
        hints.insert(key.to_owned(), OwnedValue::try_from(value).expect("a hint"));
        hints
    }

    #[test]
    fn an_urgency_hint_is_read_and_anything_else_is_normal() {
        assert_eq!(urgency_of(&hint("urgency", 0u8.into())), Urgency::Low);
        assert_eq!(urgency_of(&hint("urgency", 2u8.into())), Urgency::Critical);
        assert_eq!(urgency_of(&hint("urgency", 7u8.into())), Urgency::Normal);
        // A hint of the wrong type is not a reason to shout.
        assert_eq!(
            urgency_of(&hint("urgency", ZValue::from("critical"))),
            Urgency::Normal
        );
        assert_eq!(urgency_of(&HashMap::new()), Urgency::Normal);
    }

    #[test]
    fn only_a_named_image_path_is_read_and_pixels_are_ignored() {
        assert_eq!(
            image_of(&hint("image-path", ZValue::from("/usr/share/icons/a.png"))),
            Some("/usr/share/icons/a.png".to_owned())
        );
        // Raw pixel hints are never carried: this shell decodes nothing a
        // stranger sent it.
        assert_eq!(
            image_of(&hint("image-data", ZValue::from(vec![1u8, 2, 3]))),
            None
        );
        assert_eq!(image_of(&HashMap::new()), None);
    }
}
