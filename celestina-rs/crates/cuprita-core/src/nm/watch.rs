//! The change watcher: one subscription to NetworkManager's signals, filtered
//! to the ones that change a snapshot and coalesced into one call per burst.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type as MessageType;
use zbus::MatchRule;

use super::{map_error, IFACE_PROPERTIES, NM, NM_PATH};
use crate::error::NetworkError;

/// How long the watcher waits for a burst of signals to settle.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// The longest a change waits from its first signal, however long the burst.
const DEBOUNCE_MAX: Duration = Duration::from_millis(1500);

/// Keeps the watcher alive. Dropping it stops the calls to `on_change` at
/// once; the watcher's two threads end with the next NetworkManager signal
/// (the subscription blocks until one arrives).
pub struct WatchHandle {
    stopped: Arc<AtomicBool>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

/// Whether a NetworkManager signal can change what a snapshot shows: the
/// manager's and the devices' properties (Wi-Fi switch, device and active
/// states, the access-point list), devices and access points coming and
/// going, active connections changing state, and saved profiles added or
/// removed. Access points' own property changes (strength every scan) are
/// left out; their signal bars refresh with the next change that counts.
pub(super) fn relevant(path: &str, interface: &str, member: &str) -> bool {
    match member {
        "DeviceAdded" | "DeviceRemoved" | "AccessPointAdded" | "AccessPointRemoved"
        | "NewConnection" | "ConnectionRemoved" => true,
        "PropertiesChanged" if interface == IFACE_PROPERTIES => {
            path == NM_PATH
                || path.starts_with("/org/freedesktop/NetworkManager/Devices/")
                || path.starts_with("/org/freedesktop/NetworkManager/ActiveConnection/")
        }
        "StateChanged" => path.starts_with("/org/freedesktop/NetworkManager/ActiveConnection/"),
        _ => false,
    }
}

/// How long to keep waiting for a burst that started at `first`, at `now`:
/// `None` once it has waited long enough.
fn next_wait(first: Instant, now: Instant) -> Option<Duration> {
    let left = DEBOUNCE_MAX.checked_sub(now.saturating_duration_since(first))?;
    (!left.is_zero()).then(|| left.min(DEBOUNCE))
}

/// Calls `on_change` from a thread of its own whenever NetworkManager reports
/// a change that matters (see `relevant`), once per burst: signals arriving
/// within 300 ms of each other are coalesced into one call, made at most
/// 1.5 s after the burst's first signal.
pub fn watch(on_change: impl Fn() + Send + 'static) -> Result<WatchHandle, NetworkError> {
    let connection = Connection::system().map_err(map_error)?;
    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(NM)
        .map_err(map_error)?
        .build();
    let messages = MessageIterator::for_match_rule(rule, &connection, None).map_err(map_error)?;

    let stopped = Arc::new(AtomicBool::new(false));
    let (tick, ticks) = mpsc::channel::<()>();

    std::thread::Builder::new()
        .name("cuprita-nm-signals".to_owned())
        .spawn(move || {
            // Holds the connection for as long as the iterator runs.
            let _connection = connection;
            for message in messages {
                let Ok(message) = message else { continue };
                let header = message.header();
                let path = header.path().map(|p| p.as_str()).unwrap_or_default();
                let interface = header.interface().map(|i| i.as_str()).unwrap_or_default();
                let member = header.member().map(|m| m.as_str()).unwrap_or_default();
                // The debouncer is gone: the handle was dropped.
                if relevant(path, interface, member) && tick.send(()).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| NetworkError::Failed(e.to_string()))?;

    let flag = Arc::clone(&stopped);
    std::thread::Builder::new()
        .name("cuprita-nm-debounce".to_owned())
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
        .map_err(|e| NetworkError::Failed(e.to_string()))?;

    Ok(WatchHandle { stopped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nm::IFACE_WIRELESS;

    #[test]
    fn only_snapshot_changing_signals_count() {
        assert!(relevant(NM_PATH, IFACE_PROPERTIES, "PropertiesChanged"));
        assert!(relevant(
            "/org/freedesktop/NetworkManager/Devices/22",
            IFACE_PROPERTIES,
            "PropertiesChanged"
        ));
        assert!(relevant(
            "/org/freedesktop/NetworkManager/Devices/22",
            IFACE_WIRELESS,
            "AccessPointAdded"
        ));
        assert!(!relevant(
            "/org/freedesktop/NetworkManager/AccessPoint/3054",
            IFACE_PROPERTIES,
            "PropertiesChanged"
        ));
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
