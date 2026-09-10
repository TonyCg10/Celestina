//! The phone's notifications on the desktop, over the own wire, and the
//! desktop's presses going back.
//!
//! The phone keys every notification; the desktop's notification server
//! numbers what it shows. The bridge keeps both maps, bounded per device,
//! so a later post replaces, a dismissal on either side closes the other,
//! and a button pressed on the desktop reaches the phone by the index it
//! sent. The server is a trait so the loopback tests record instead of
//! posting, and a daemon without a session bus simply shows nothing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use magnetita_proto::daily::notifications::NotificationPosted;
use zbus::blocking::{Connection, Proxy};

use crate::devices::Command;
use crate::lock::LockOk;
use crate::Daemon;

/// How many of one device's notifications the bridge tracks; past this a
/// new one is shown but not tracked, so a peer cannot grow the maps.
const MAX_TRACKED_PER_DEVICE: usize = 128;

/// Where a notification is shown.
pub(crate) trait NotificationServer: Send + Sync {
    /// Shows or replaces (`replaces` non-zero) and returns the server's id.
    fn post(
        &self,
        app: &str,
        replaces: u32,
        summary: &str,
        body: &str,
        buttons: &[String],
        replyable: bool,
    ) -> Option<u32>;

    fn close(&self, id: u32);
}

/// The session's `org.freedesktop.Notifications`.
pub(crate) struct DbusServer(pub(crate) Connection);

impl NotificationServer for DbusServer {
    fn post(
        &self,
        app: &str,
        replaces: u32,
        summary: &str,
        body: &str,
        buttons: &[String],
        replyable: bool,
    ) -> Option<u32> {
        crate::notify::post_with(&self.0, app, replaces, summary, body, buttons, replyable)
    }

    fn close(&self, id: u32) {
        crate::notify::close(&self.0, id);
    }
}

/// No bus: nothing is shown, nothing is tracked.
pub(crate) struct NoServer;

impl NotificationServer for NoServer {
    fn post(&self, _: &str, _: u32, _: &str, _: &str, _: &[String], _: bool) -> Option<u32> {
        None
    }

    fn close(&self, _: u32) {}
}

#[derive(Default)]
pub(crate) struct Bridge {
    /// (device id, phone key) → server id.
    shown: Mutex<HashMap<(String, String), u32>>,
    /// server id → (device id, phone key), for the server's signals.
    origin: Mutex<HashMap<u32, (String, String)>>,
}

impl Bridge {
    /// Shows or replaces one phone notification; the line for the UI log.
    pub(crate) fn posted(
        &self,
        server: &dyn NotificationServer,
        device_id: &str,
        device_name: &str,
        note: &NotificationPosted,
    ) -> Option<String> {
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
        let id_key = (device_id.to_owned(), note.key.clone());
        let (replaces, tracked) = {
            let shown = self.shown.lock_ok();
            (
                shown.get(&id_key).copied().unwrap_or(0),
                shown.keys().filter(|(d, _)| d == device_id).count(),
            )
        };
        let buttons: Vec<String> = note.actions.iter().map(|a| a.label.clone()).collect();
        let server_id = server.post(
            app,
            replaces,
            &summary,
            &note.body,
            &buttons,
            note.replyable,
        )?;
        if replaces != 0 || tracked < MAX_TRACKED_PER_DEVICE {
            if replaces != 0 && replaces != server_id {
                self.origin.lock_ok().remove(&replaces);
            }
            self.shown.lock_ok().insert(id_key.clone(), server_id);
            self.origin.lock_ok().insert(server_id, id_key);
        }
        Some(format!("\u{1f514} {app}: {summary}"))
    }

    /// The phone withdrew one: close it here.
    pub(crate) fn dismissed(&self, server: &dyn NotificationServer, device_id: &str, key: &str) {
        let id_key = (device_id.to_owned(), key.to_owned());
        if let Some(server_id) = self.shown.lock_ok().remove(&id_key) {
            self.origin.lock_ok().remove(&server_id);
            server.close(server_id);
        }
    }

    /// The server's signal named `server_id`: whose notification was it?
    /// Forgets the entry, since the server is done with it, when `closing`.
    pub(crate) fn origin(&self, server_id: u32, closing: bool) -> Option<(String, String)> {
        let found = if closing {
            self.origin.lock_ok().remove(&server_id)
        } else {
            self.origin.lock_ok().get(&server_id).cloned()
        };
        if closing {
            if let Some(id_key) = &found {
                self.shown.lock_ok().remove(id_key);
            }
        }
        found
    }

    /// A session that ends takes its keys with it.
    pub(crate) fn forget_device(&self, device_id: &str) {
        let mut shown = self.shown.lock_ok();
        let mut origin = self.origin.lock_ok();
        shown.retain(|(d, _), server_id| {
            let keep = d != device_id;
            if !keep {
                origin.remove(server_id);
            }
            keep
        });
    }

    /// What one of the server's signals means for the phone, if anything:
    /// `ActionInvoked` with the index we offered, `NotificationClosed` by the
    /// person (reason 2), or KDE's `NotificationReplied`.
    pub(crate) fn command_for(&self, signal: ServerSignal) -> Option<(String, Command)> {
        match signal {
            ServerSignal::Action(server_id, action_key) => {
                let action: u16 = action_key.parse().ok()?;
                let (device, key) = self.origin(server_id, false)?;
                Some((device, Command::NotificationAction { key, action }))
            }
            ServerSignal::Replied(server_id, text) => {
                let (device, key) = self.origin(server_id, false)?;
                Some((device, Command::NotificationReply { key, text }))
            }
            ServerSignal::Closed(server_id, reason) => {
                let (device, key) = self.origin(server_id, true)?;
                (reason == 2).then_some((device, Command::NotificationDismiss { key }))
            }
        }
    }
}

/// The three signals the session's notification server emits about what
/// it shows for us.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ServerSignal {
    Action(u32, String),
    Closed(u32, u32),
    Replied(u32, String),
}

/// Listens to the server's signals for the daemon's lifetime and forwards
/// what they mean to the device's command queue. Best-effort: no bus, no
/// listener.
pub(crate) fn spawn_signal_watch(bridge: Arc<Bridge>, daemon: Arc<Daemon>) {
    for (signal, parse) in [
        (
            "ActionInvoked",
            parse_action as fn(&zbus::Message) -> Option<ServerSignal>,
        ),
        ("NotificationClosed", parse_closed),
        ("NotificationReplied", parse_replied),
    ] {
        let bridge = Arc::clone(&bridge);
        let daemon = Arc::clone(&daemon);
        thread::spawn(move || {
            let Ok(connection) = Connection::session() else {
                return;
            };
            let Ok(proxy) = Proxy::new(
                &connection,
                "org.freedesktop.Notifications",
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
            ) else {
                return;
            };
            let Ok(signals) = proxy.receive_signal(signal) else {
                return;
            };
            for message in signals {
                let Some(meaning) = parse(&message) else {
                    continue;
                };
                if let Some((device, command)) = bridge.command_for(meaning) {
                    let sender = daemon.commands.lock_ok().get(&device).cloned();
                    if let Some(sender) = sender {
                        let _ = sender.try_send(command);
                    }
                }
            }
        });
    }
}

fn parse_action(message: &zbus::Message) -> Option<ServerSignal> {
    let (id, key): (u32, String) = message.body().deserialize().ok()?;
    Some(ServerSignal::Action(id, key))
}

fn parse_closed(message: &zbus::Message) -> Option<ServerSignal> {
    let (id, reason): (u32, u32) = message.body().deserialize().ok()?;
    Some(ServerSignal::Closed(id, reason))
}

fn parse_replied(message: &zbus::Message) -> Option<ServerSignal> {
    let (id, text): (u32, String) = message.body().deserialize().ok()?;
    Some(ServerSignal::Replied(id, text))
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;

    /// One recorded post.
    #[derive(Clone, Debug)]
    pub(crate) struct Posted {
        pub(crate) id: u32,
        pub(crate) app: String,
        pub(crate) replaces: u32,
        pub(crate) summary: String,
        pub(crate) body: String,
        pub(crate) buttons: Vec<String>,
        pub(crate) replyable: bool,
    }

    /// Records what would be shown; ids count up from 1.
    #[derive(Default)]
    pub(crate) struct Recorder {
        pub(crate) posted: Mutex<Vec<Posted>>,
        pub(crate) closed: Mutex<Vec<u32>>,
    }

    impl NotificationServer for Recorder {
        fn post(
            &self,
            app: &str,
            replaces: u32,
            summary: &str,
            body: &str,
            buttons: &[String],
            replyable: bool,
        ) -> Option<u32> {
            let mut posted = self.posted.lock_ok();
            let id = if replaces != 0 {
                replaces
            } else {
                posted.len() as u32 + 1
            };
            posted.push(Posted {
                id,
                app: app.into(),
                replaces,
                summary: summary.into(),
                body: body.into(),
                buttons: buttons.to_vec(),
                replyable,
            });
            Some(id)
        }

        fn close(&self, id: u32) {
            self.closed.lock_ok().push(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Recorder;
    use super::*;
    use magnetita_proto::daily::notifications::Action;

    fn note(key: &str, title: &str) -> NotificationPosted {
        NotificationPosted {
            key: key.into(),
            app_name: "Messages".into(),
            title: title.into(),
            body: "hello".into(),
            timestamp_ms: 1,
            replyable: true,
            actions: vec![Action {
                label: "Mark read".into(),
            }],
            icon: None,
        }
    }

    #[test]
    fn a_post_replaces_a_dismissal_closes_and_signals_map_back_to_the_phone() {
        let server = Recorder::default();
        let bridge = Bridge::default();
        assert_eq!(
            bridge.posted(&server, "dev", "Phone", &note("k1", "Ana")),
            Some("\u{1f514} Messages: Ana".into())
        );
        bridge.posted(&server, "dev", "Phone", &note("k1", "Ana again"));
        let posted = server.posted.lock_ok().clone();
        assert_eq!(posted.len(), 2);
        assert_eq!(posted[1].replaces, 1, "the second post replaces the first");
        assert_eq!(posted[1].buttons, vec!["Mark read".to_owned()]);
        assert!(posted[1].replyable);

        assert!(matches!(
            bridge.command_for(ServerSignal::Action(1, "0".into())),
            Some((device, Command::NotificationAction { key, action: 0 })) if device == "dev" && key == "k1"
        ));
        assert!(matches!(
            bridge.command_for(ServerSignal::Replied(1, "on my way".into())),
            Some((device, Command::NotificationReply { key, text })) if device == "dev" && key == "k1" && text == "on my way"
        ));
        bridge.dismissed(&server, "dev", "k1");
        assert_eq!(server.closed.lock_ok().as_slice(), [1]);
        bridge.dismissed(&server, "dev", "k1");
        assert_eq!(
            server.closed.lock_ok().len(),
            1,
            "closing twice does nothing"
        );

        bridge.posted(&server, "dev", "Phone", &note("k2", "Bo"));
        let k2 = server.posted.lock_ok().last().unwrap().id;
        assert!(
            bridge.command_for(ServerSignal::Closed(k2, 1)).is_none(),
            "expiry is not a dismissal"
        );
        bridge.posted(&server, "dev", "Phone", &note("k3", "Cy"));
        let k3 = server.posted.lock_ok().last().unwrap().id;
        assert!(matches!(
            bridge.command_for(ServerSignal::Closed(k3, 2)),
            Some((device, Command::NotificationDismiss { key })) if device == "dev" && key == "k3"
        ));
        assert!(
            bridge.command_for(ServerSignal::Closed(k3, 2)).is_none(),
            "forgotten once closed"
        );
    }

    #[test]
    fn a_device_is_tracked_up_to_the_bound_and_forgotten_whole() {
        let server = Recorder::default();
        let bridge = Bridge::default();
        for i in 0..(MAX_TRACKED_PER_DEVICE + 5) {
            bridge.posted(&server, "dev", "Phone", &note(&format!("k{i}"), "t"));
        }
        assert_eq!(bridge.shown.lock_ok().len(), MAX_TRACKED_PER_DEVICE);
        bridge.forget_device("dev");
        assert!(bridge.shown.lock_ok().is_empty());
        assert!(bridge.origin.lock_ok().is_empty());
    }
}
