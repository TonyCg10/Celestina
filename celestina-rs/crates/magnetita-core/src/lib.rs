#![forbid(unsafe_code)]

//! Magnetita's protocol core — the KDE Connect wire format, pure and offline.
//!
//! Magnetita is the suite's phone↔desktop bridge (the replacement for Valent),
//! and it speaks the **KDE Connect** protocol so the phone keeps using the
//! Android app it already runs. This crate is the part with no sockets, no TLS
//! and no threads: the packet envelope and the plugin bodies, as plain Rust
//! types that serialize to and parse from the wire. Everything here is unit-
//! testable without a network — the live link, pairing and mounts live in the
//! app, the way `siderita-core` is testable without a toolkit.
//!
//! The protocol is line-delimited JSON over TLS: a device announces itself with
//! an [`Identity`] over UDP, the two ends open a TCP link and re-exchange
//! identities, TLS wraps it, and from then on every message is a
//! [`NetworkPacket`] whose `body` a plugin owns. This crate starts with those
//! two — the envelope and the identity — and grows one plugin body at a time as
//! Magnetita earns each.

pub mod identity;
pub mod packet;

pub use identity::{DeviceType, Identity, DEFAULT_PORT, PROTOCOL_VERSION, TYPE_IDENTITY};
pub use packet::NetworkPacket;
