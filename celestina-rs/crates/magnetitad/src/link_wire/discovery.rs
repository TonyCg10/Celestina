//! How a phone finds this desktop: the daemon advertises itself on
//! `_magnetita._udp` so the phone can dial it. The phone does not
//! advertise and the desktop never dials, so nothing here browses.
//!
//! The advertisement is one long-lived `avahi-publish` child owned by pid for
//! the daemon's lifetime and terminated by its process group, never by name.

use std::process::{Child, Stdio};

use magnetita_link::discovery::SERVICE_TYPE;
use rustix::process::Pid;

use crate::subprocess;

/// The advertisement this daemon keeps up while it listens: one owned
/// `avahi-publish`, killed by pid when dropped.
pub(crate) struct Advertisement {
    child: Child,
    group: Pid,
}

impl Advertisement {
    /// Publishes `device_id` on the service at `port`, with the human name
    /// as a TXT record for anyone browsing by eye.
    pub(crate) fn publish(device_id: &str, name: &str, port: u16) -> std::io::Result<Self> {
        let port = port.to_string();
        let txt = format!("name={name}");
        let (child, group) = subprocess::spawn_grouped(
            "avahi-publish",
            &["-s", device_id, SERVICE_TYPE, &port, &txt],
            Stdio::null(),
        )?;
        Ok(Self { child, group })
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        subprocess::terminate_group_and_reap(&mut self.child, self.group);
    }
}
