//! Posting the phone's notifications to the session's own notification server.
//!
//! The phone's `kdeconnect.notification` becomes an `org.freedesktop.Notifications`
//! notification — the same daemon every desktop app uses — so a phone alert looks
//! native and needs no bespoke UI. The server hands back an id; the daemon keeps
//! the phone-id→server-id map so a later update *replaces* and a cancel
//! *withdraws* the right one. Best-effort: no notification server just means no
//! mirror, never an error.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use magnetita_core::Notification;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

const SERVICE: &str = "org.freedesktop.Notifications";
const OBJECT: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";

/// Post a notification (replacing `replaces_id` if non-zero); returns the
/// server's id so it can later be replaced or closed.
pub fn post(
    connection: &Connection,
    app_name: &str,
    replaces_id: u32,
    summary: &str,
    body: &str,
) -> Option<u32> {
    let proxy = Proxy::new(connection, SERVICE, OBJECT, INTERFACE).ok()?;
    let actions: Vec<&str> = Vec::new();
    let hints: HashMap<&str, Value> = HashMap::new();
    // Notify(app_name, replaces_id, app_icon, summary, body, actions, hints,
    //        expire_timeout) -> id. -1 timeout leaves it to the server's default.
    let id: u32 = proxy
        .call(
            "Notify",
            &(
                app_name,
                replaces_id,
                "phone",
                summary,
                body,
                actions,
                hints,
                -1i32,
            ),
        )
        .ok()?;
    Some(id)
}

/// Withdraw a notification by the server id [`post`] returned.
pub fn close(connection: &Connection, id: u32) {
    if let Ok(proxy) = Proxy::new(connection, SERVICE, OBJECT, INTERFACE) {
        let _: Result<(), zbus::Error> = proxy.call("CloseNotification", &(id,));
    }
}

/// How many of one device's notifications may be tracked for replacement at
/// once. A phone with a hundred live notifications is already unusual; past
/// this the mirror shows new ones without tracking them, rather than letting a
/// peer decide how much memory the daemon holds.
const MAX_TRACKED_PER_DEVICE: usize = 128;

/// The phone-id→server-id map behind replace and withdraw.
///
/// Both halves of every key come from the peer, so this owns the bound and the
/// per-device cleanup rather than leaving them to whoever happens to insert.
#[derive(Default)]
pub struct Mirror {
    /// `<device-id>\0<phone-notification-id>` → freedesktop server id.
    tracked: Mutex<HashMap<String, u32>>,
}

impl Mirror {
    /// Show, replace or withdraw one phone notification. Returns the line to
    /// record for the person when something was shown.
    pub fn apply(
        &self,
        connection: &Connection,
        device_id: &str,
        device_name: &str,
        note: &Notification,
    ) -> Option<String> {
        let tracking_key = key(device_id, &note.id);
        if note.is_cancel {
            if let Some(server_id) = self.lock().remove(&tracking_key) {
                close(connection, server_id);
            }
            return None;
        }
        let app = if note.app_name.is_empty() {
            device_name
        } else {
            &note.app_name
        };
        let summary = if note.title.is_empty() {
            app.to_owned()
        } else {
            note.title.clone()
        };
        let (replaces, tracked) = {
            let map = self.lock();
            let prefix = key(device_id, "");
            (
                map.get(&tracking_key).copied().unwrap_or(0),
                map.keys().filter(|held| held.starts_with(&prefix)).count(),
            )
        };
        // A new id past the bound is still shown; it is simply not tracked, so
        // the peer cannot grow the map without limit.
        let trackable = replaces != 0 || tracked < MAX_TRACKED_PER_DEVICE;
        let server_id = post(connection, app, replaces, &summary, &note.text)?;
        if trackable {
            self.lock().insert(tracking_key, server_id);
        }
        Some(format!("🔔 {app}: {summary}"))
    }

    /// Drop every mapping of one device. A session that ends takes the keys it
    /// was given with it.
    pub fn forget_device(&self, device_id: &str) {
        let prefix = key(device_id, "");
        self.lock().retain(|held, _| !held.starts_with(&prefix));
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, u32>> {
        match self.tracked.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// The key one device's notification is tracked under. The NUL cannot occur in
/// either part, so no pair of device and notification ids can collide.
fn key(device_id: &str, notification_id: &str) -> String {
    format!("{device_id}\u{0}{notification_id}")
}
