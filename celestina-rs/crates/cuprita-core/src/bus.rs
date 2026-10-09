//! What the two system-bus clients (`nm`, `bluez`) share: how a D-Bus failure
//! reads as a section error, and the signal watcher that coalesces a burst of
//! a service's signals into one call, and how a proxy to one of their objects
//! is built. Each client keeps its own names, paths and filter; only the bus
//! mechanics live here.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type as MessageType;
use zbus::proxy::CacheProperties;
use zbus::{DBusError, MatchRule, Message};

/// A D-Bus failure, before it becomes one section's error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fault {
    /// Policy or polkit refused.
    Denied,
    /// The service is not running, the bus cannot be reached, or the service
    /// is not ready for the call (BlueZ's adapter is off, NetworkManager has
    /// no device that can carry the connection).
    Unavailable,
    /// The object the call named does not exist (any more).
    NotFound,
    /// The service is already busy with an operation of the same kind.
    Busy,
    /// Anything else, with the service's detail (or the error name). Only
    /// logged: the notice says a short Spanish sentence instead.
    Failed(String),
}

/// The error name a service replied with and its detail, when the failure is
/// a method error at all (not a broken connection).
pub(crate) fn error_name(error: &zbus::Error) -> Option<(String, Option<String>)> {
    match error {
        zbus::Error::MethodError(name, detail, _) => {
            Some((name.as_str().to_owned(), detail.clone()))
        }
        zbus::Error::FDO(inner) => match inner.as_ref() {
            zbus::fdo::Error::ZBus(inner) => error_name(inner),
            other => Some((
                other.name().as_str().to_owned(),
                other.description().map(str::to_owned),
            )),
        },
        _ => None,
    }
}

/// An error name and its detail as a fault. NetworkManager names a polkit
/// refusal `…PermissionDenied`, the bus itself `…AccessDenied`; a vanished
/// object is NetworkManager's `UnknownConnection` or `UnknownDevice`, BlueZ's
/// `DoesNotExist` or the bus's `UnknownObject`; BlueZ says `NotReady` while
/// the adapter is off and NetworkManager `ConnectionNotAvailable` when no
/// device can carry the profile; both say `InProgress` (BlueZ also `Busy`)
/// for a second call while one runs.
pub(crate) fn fault_of_name(name: &str, detail: Option<String>) -> Fault {
    let last = name.rsplit('.').next().unwrap_or(name);
    match last {
        "PermissionDenied" | "AccessDenied" | "NotAuthorized" => Fault::Denied,
        "ServiceUnknown" | "NameHasNoOwner" | "NotReady" | "ConnectionNotAvailable" => {
            Fault::Unavailable
        }
        "UnknownConnection" | "UnknownDevice" | "DoesNotExist" | "UnknownObject" => Fault::NotFound,
        "InProgress" | "Busy" => Fault::Busy,
        _ => Fault::Failed(match detail {
            Some(detail) => format!("{name}: {detail}"),
            None => name.to_owned(),
        }),
    }
}

pub(crate) fn fault(error: zbus::Error) -> Fault {
    if let Some((name, detail)) = error_name(&error) {
        return fault_of_name(&name, detail);
    }
    match error {
        zbus::Error::InputOutput(_) | zbus::Error::Address(_) => Fault::Unavailable,
        other => Fault::Failed(other.to_string()),
    }
}

pub(crate) fn fdo_fault(error: zbus::fdo::Error) -> Fault {
    fault(zbus::Error::FDO(Box::new(error)))
}

/// A D-Bus failure as one section's error.
pub(crate) fn map_error<E: From<Fault>>(error: zbus::Error) -> E {
    fault(error).into()
}

/// A property write's failure as one section's error.
pub(crate) fn map_fdo<E: From<Fault>>(error: zbus::fdo::Error) -> E {
    fdo_fault(error).into()
}

/// A proxy to `destination`'s object at `path`, for `interface`, that reads
/// every property afresh: the clients re-read on each snapshot and a cached
/// value would hide a change between two signals.
pub(crate) fn proxy<'a, E: From<Fault>>(
    connection: &Connection,
    destination: &'static str,
    path: &'a str,
    interface: &'a str,
) -> Result<Proxy<'a>, E> {
    zbus::blocking::proxy::Builder::new(connection)
        .destination(destination)
        .and_then(|b| b.path(path))
        .and_then(|b| b.interface(interface))
        .map(|b| b.cache_properties(CacheProperties::No))
        .and_then(|b| b.build())
        .map_err(map_error)
}

/// A system-bus connection whose every method call gives up after
/// `timeout`, so a hung service or an unanswered prompt frees the worker.
/// zbus sets this per connection (`connection::Builder::method_timeout`).
pub(crate) fn system_connection<E: From<Fault>>(timeout: Duration) -> Result<Connection, E> {
    zbus::blocking::connection::Builder::system()
        .map(|b| b.method_timeout(timeout))
        .and_then(|b| b.build())
        .map_err(map_error)
}

/// How long the watcher waits for a burst of signals to settle.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// The longest a change waits from its first signal, however long the burst.
const DEBOUNCE_MAX: Duration = Duration::from_millis(1500);

/// Keeps a watcher alive. Dropping it stops the calls to `on_change` at once;
/// the watcher's two threads end with the service's next signal (the
/// subscription blocks until one arrives).
pub struct WatchHandle {
    stopped: Arc<AtomicBool>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

/// How long to keep waiting for a burst that started at `first`, at `now`:
/// `None` once it has waited long enough.
fn next_wait(first: Instant, now: Instant) -> Option<Duration> {
    let left = DEBOUNCE_MAX.checked_sub(now.saturating_duration_since(first))?;
    (!left.is_zero()).then(|| left.min(DEBOUNCE))
}

/// Calls `on_change` from a thread of its own whenever `sender` emits a
/// signal on the system bus that `relevant` keeps, once per burst: signals
/// arriving within 300 ms of each other are coalesced into one call, made at
/// most 1.5 s after the burst's first signal. `name` labels the two threads.
pub(crate) fn watch_signals(
    sender: &'static str,
    name: &str,
    relevant: impl Fn(&Message) -> bool + Send + 'static,
    on_change: impl Fn() + Send + 'static,
) -> Result<WatchHandle, Fault> {
    let connection = Connection::system().map_err(fault)?;
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(sender)
        .map_err(fault)?
        .build();
    let messages = MessageIterator::for_match_rule(rule, &connection, None).map_err(fault)?;

    let stopped = Arc::new(AtomicBool::new(false));
    let (tick, ticks) = mpsc::channel::<()>();

    std::thread::Builder::new()
        .name(format!("cuprita-{name}-signals"))
        .spawn(move || {
            // Holds the connection for as long as the iterator runs.
            let _connection = connection;
            for message in messages {
                let Ok(message) = message else { continue };
                // The debouncer is gone: the handle was dropped.
                if relevant(&message) && tick.send(()).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| Fault::Failed(e.to_string()))?;

    let flag = Arc::clone(&stopped);
    std::thread::Builder::new()
        .name(format!("cuprita-{name}-debounce"))
        .spawn(move || {
            while ticks.recv().is_ok() {
                let first = Instant::now();
                // Wait for the burst to settle, but never past the cap.
                while let Some(wait) = next_wait(first, Instant::now()) {
                    match ticks.recv_timeout(wait) {
                        Ok(()) => {}
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                on_change();
            }
        })
        .map_err(|e| Fault::Failed(e.to_string()))?;

    Ok(WatchHandle { stopped })
}

/// A signal's path, interface and member, empty where the header has none.
pub(crate) fn signal_header(message: &Message) -> (String, String, String) {
    let header = message.header();
    (
        header
            .path()
            .map(|p| p.as_str().to_owned())
            .unwrap_or_default(),
        header
            .interface()
            .map(|i| i.as_str().to_owned())
            .unwrap_or_default(),
        header
            .member()
            .map(|m| m.as_str().to_owned())
            .unwrap_or_default(),
    )
}

/// Turns a fault into one section's error; each section's error enum has
/// the same shapes (see `error.rs`). The service's own detail is English and
/// technical: it goes to the log, and the notice says a Spanish sentence.
macro_rules! section_from_fault {
    ($error:ty, $label:literal) => {
        impl From<Fault> for $error {
            fn from(fault: Fault) -> Self {
                match fault {
                    Fault::Denied => Self::Denied,
                    Fault::Unavailable => Self::Unavailable,
                    Fault::NotFound => Self::NotFound(String::new()),
                    Fault::Busy => Self::Busy,
                    Fault::Failed(detail) => {
                        eprintln!("cuprita: {}: {detail}", $label);
                        Self::service_failed()
                    }
                }
            }
        }
    };
}

section_from_fault!(crate::error::NetworkError, "NetworkManager");
section_from_fault!(crate::error::BluetoothError, "BlueZ");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_names_map_to_faults() {
        assert_eq!(
            fault_of_name("org.freedesktop.NetworkManager.PermissionDenied", None),
            Fault::Denied
        );
        assert_eq!(
            fault_of_name("org.freedesktop.DBus.Error.ServiceUnknown", None),
            Fault::Unavailable
        );
        assert_eq!(
            fault_of_name(
                "org.freedesktop.NetworkManager.UnknownConnection",
                Some("no such profile".to_owned())
            ),
            Fault::NotFound
        );
        assert_eq!(
            fault_of_name("org.freedesktop.NetworkManager.UnknownDevice", None),
            Fault::NotFound
        );
        assert_eq!(
            fault_of_name("org.bluez.Error.DoesNotExist", None),
            Fault::NotFound
        );
        assert_eq!(
            fault_of_name("org.bluez.Error.NotReady", None),
            Fault::Unavailable
        );
        assert_eq!(
            fault_of_name(
                "org.freedesktop.NetworkManager.ConnectionNotAvailable",
                None
            ),
            Fault::Unavailable
        );
        assert_eq!(
            fault_of_name("org.bluez.Error.InProgress", None),
            Fault::Busy
        );
        assert_eq!(fault_of_name("org.bluez.Error.Busy", None), Fault::Busy);
        assert_eq!(
            fault_of_name(
                "org.bluez.Error.Failed",
                Some("Input/output error".to_owned())
            ),
            Fault::Failed("org.bluez.Error.Failed: Input/output error".to_owned())
        );
        assert_eq!(
            fdo_fault(zbus::fdo::Error::AccessDenied("polkit".to_owned())),
            Fault::Denied
        );
    }

    #[test]
    fn a_service_detail_never_reaches_the_notice() {
        let fault = fault_of_name(
            "org.freedesktop.NetworkManager.Failed",
            Some("Connection activation failed".to_owned()),
        );
        let message = crate::error::NetworkError::from(fault).message_es();
        assert!(!message.contains("org.freedesktop"), "{message}");
        assert!(!message.contains("activation"), "{message}");
        assert_eq!(
            crate::error::BluetoothError::from(Fault::Busy).message_es(),
            crate::error::BluetoothError::Busy.message_es()
        );
        assert_eq!(
            crate::error::NetworkError::from(Fault::NotFound).message_es(),
            "Eso ya no existe"
        );
    }

    #[test]
    fn the_debounce_is_capped() {
        let first = Instant::now();
        assert_eq!(next_wait(first, first), Some(DEBOUNCE));
        assert_eq!(
            next_wait(first, first + Duration::from_millis(1400)),
            Some(Duration::from_millis(100))
        );
        assert_eq!(next_wait(first, first + DEBOUNCE_MAX), None);
        assert_eq!(next_wait(first, first + Duration::from_secs(5)), None);
    }
}
