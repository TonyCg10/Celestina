//! How the network list reads: ordering, signal bars, security keys.

use std::cmp::Reverse;

use crate::model::{Network, NetworkState, Security};

/// Connected first, then the known ones, then the strongest, then by name.
// The plan fixes this signature; a slice would do as well.
#[allow(clippy::ptr_arg)]
pub fn order_networks(networks: &mut Vec<Network>) {
    networks.sort_by(|a, b| key(a).cmp(&key(b)));
}

fn key(n: &Network) -> (bool, bool, Reverse<u8>, &str) {
    (
        n.state != NetworkState::Connected,
        !n.known,
        Reverse(n.signal.unwrap_or(0)),
        n.name.as_str(),
    )
}

/// The 0..=4 bars a signal strength draws as.
#[must_use]
pub fn signal_bars(signal: u8) -> u8 {
    match signal {
        0..=4 => 0,
        5..=29 => 1,
        30..=54 => 2,
        55..=79 => 3,
        _ => 4,
    }
}

/// The token QML translates into the security label.
#[must_use]
pub fn security_label_key(s: Security) -> &'static str {
    match s {
        Security::Open => "open",
        Security::Psk => "psk",
        Security::Enterprise => "enterprise",
    }
}
