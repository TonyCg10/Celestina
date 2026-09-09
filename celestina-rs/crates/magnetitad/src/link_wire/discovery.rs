//! Where the own-protocol peers are: this daemon advertises itself on
//! `_magnetita._udp` and browses for phones doing the same.
//!
//! Both are Avahi subprocesses, the way [`mirror_discovery`](crate::mirror_discovery)
//! reaches the ADB advertisements: `avahi-browse -rpt` is bounded and
//! terminating, and `avahi-publish` is one long-lived child owned by pid for
//! the daemon's lifetime and terminated by its process group, never by name.
//! The parsing is `magnetita-link`'s, shared with the peer.

use std::process::{Child, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use magnetita_link::discovery::{parse_peers, Peer, SERVICE_TYPE};
use rustix::process::Pid;

use crate::subprocess;

/// How long a browse may take before it is abandoned.
const BROWSE_BUDGET: Duration = Duration::from_secs(4);

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

/// Asks Avahi which Magnetita peers are advertising right now.
pub(crate) fn browse(stopping: &AtomicBool) -> Vec<Peer> {
    let deadline = Instant::now() + BROWSE_BUDGET;
    let Some(output) = subprocess::command_output_from(
        "avahi-browse",
        &["-rpt", SERVICE_TYPE],
        deadline,
        stopping,
    ) else {
        return Vec::new();
    };
    parse_peers(&String::from_utf8_lossy(&output))
}
