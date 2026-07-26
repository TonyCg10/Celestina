//! Posting the phone's notifications to the session's own notification server.
//!
//! The phone's `kdeconnect.notification` becomes an `org.freedesktop.Notifications`
//! notification — the same daemon every desktop app uses — so a phone alert looks
//! native and needs no bespoke UI. The server hands back an id; the daemon keeps
//! the phone-id→server-id map so a later update *replaces* and a cancel
//! *withdraws* the right one. Best-effort: no notification server just means no
//! mirror, never an error.

use std::collections::HashMap;

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
