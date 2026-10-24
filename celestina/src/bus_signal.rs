//! Recognising the bus driver's `NameOwnerChanged` broadcast, shared by every
//! worker that watches names come and go (the tray, the polkit agent).
//!
//! A connection's message stream also carries signals a peer addresses to
//! this process, and a peer can forge the driver's signal; acting on one
//! would drop other watchers' state or redirect commands to the forger. So a
//! change counts only when the driver itself sent it and only as a
//! broadcast (no destination).

use zbus::message::{Message, Type};

/// The bus driver: the only sender of `NameOwnerChanged` that is trusted.
pub const BUS_DRIVER: &str = "org.freedesktop.DBus";

/// A name changed owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameOwnerChange {
    /// The name.
    pub name: String,
    /// Its new owner, or empty when it was released.
    pub new_owner: String,
}

/// `message` as a [`NameOwnerChange`], or `None` when it is anything else, a
/// forgery by a peer, or a copy the driver addressed to one connection.
#[must_use]
pub fn name_owner_change(message: &Message) -> Option<NameOwnerChange> {
    let header = message.header();
    if header.message_type() != Type::Signal
        || header.interface()?.as_str() != BUS_DRIVER
        || header.member()?.as_str() != "NameOwnerChanged"
    {
        return None;
    }
    let from_driver = header.sender().is_some_and(|s| s.as_str() == BUS_DRIVER);
    if !from_driver || header.destination().is_some() {
        tracing::debug!("a NameOwnerChanged not broadcast by the bus is ignored");
        return None;
    }
    let (name, _old, new_owner): (String, String, String) = message.body().deserialize().ok()?;
    Some(NameOwnerChange { name, new_owner })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(sender: &str, destination: Option<&str>) -> Message {
        let mut builder = Message::signal(
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
        )
        .expect("a builder")
        .sender(sender)
        .expect("a sender");
        if let Some(destination) = destination {
            builder = builder.destination(destination).expect("a destination");
        }
        builder
            .build(&("org.kde.foo", ":1.4", ""))
            .expect("a message")
    }

    #[test]
    fn accepts_a_driver_broadcast() {
        assert_eq!(
            name_owner_change(&signal(BUS_DRIVER, None)),
            Some(NameOwnerChange {
                name: "org.kde.foo".into(),
                new_owner: String::new()
            })
        );
    }

    #[test]
    fn ignores_a_unicast_forgery() {
        assert_eq!(name_owner_change(&signal(BUS_DRIVER, Some(":1.7"))), None);
    }

    #[test]
    fn ignores_a_signal_from_a_peer() {
        assert_eq!(name_owner_change(&signal(":1.666", None)), None);
        assert_eq!(name_owner_change(&signal(":1.666", Some(":1.7"))), None);
    }
}
