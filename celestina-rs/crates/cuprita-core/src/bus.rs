//! What the two system-bus clients (`nm`, `bluez`) share: how a D-Bus failure
//! reads as a section error, and the signal watcher that coalesces a burst of
//! a service's signals into one call. Each client keeps its own names, paths
//! and filter; only the bus mechanics live here.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type as MessageType;
use zbus::{DBusError, MatchRule, Message};

/// A D-Bus failure, before it becomes one section's error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fault {
    /// Policy or polkit refused.
    Denied,
    /// The service is not running or the bus cannot be reached.
    Unavailable,
    /// Anything else, with the service's detail (or the error name).
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
/// refusal `…PermissionDenied`, the bus itself `…AccessDenied`.
pub(crate) fn fault_of_name(name: &str, detail: Option<String>) -> Fault {
    if name.ends_with("PermissionDenied") || name.ends_with("AccessDenied") {
        Fault::Denied
    } else if name.ends_with("ServiceUnknown") || name.ends_with("NameHasNoOwner") {
        Fault::Unavailable
    } else {
        Fault::Failed(detail.unwrap_or_else(|| name.to_owned()))
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
/// the same four shapes (see `error.rs`).
macro_rules! section_from_fault {
    ($error:ty) => {
        impl From<Fault> for $error {
            fn from(fault: Fault) -> Self {
                match fault {
                    Fault::Denied => Self::Denied,
                    Fault::Unavailable => Self::Unavailable,
                    Fault::Failed(detail) => Self::Failed(detail),
                }
            }
        }
    };
}

section_from_fault!(crate::error::NetworkError);
section_from_fault!(crate::error::BluetoothError);

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
            Fault::Failed("no such profile".to_owned())
        );
        assert_eq!(
            fdo_fault(zbus::fdo::Error::AccessDenied("polkit".to_owned())),
            Fault::Denied
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
