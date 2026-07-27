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
//! identities, TLS wraps it, they trust each other through a [`Pairing`]
//! handshake, and from then on every message is a [`NetworkPacket`] whose `body`
//! a plugin owns. This crate grows one piece at a time as Magnetita earns each:
//! the envelope, the identity, the pairing state machine, and then the plugin
//! bodies.

pub mod battery;
pub mod clipboard;
pub mod event;
pub mod findmyphone;
pub mod identity;
pub mod mpris;
pub mod notification;
pub mod packet;
pub mod pair;
pub mod ping;
pub mod session;
pub mod sftp;
pub mod share;

pub use battery::{read_battery, Battery, TYPE_BATTERY, TYPE_BATTERY_REQUEST};
pub use clipboard::{read_clipboard, TYPE_CLIPBOARD, TYPE_CLIPBOARD_CONNECT};
pub use event::{ConnectionEvent, LostReason};
pub use findmyphone::TYPE_FINDMYPHONE_REQUEST;
pub use identity::{DeviceType, Identity, DEFAULT_PORT, PROTOCOL_VERSION, TYPE_IDENTITY};
pub use mpris::{
    read_mpris, read_mpris_request, MprisRequest, MprisUpdate, PlayerState, TYPE_MPRIS,
    TYPE_MPRIS_REQUEST,
};
pub use notification::{read_notification, Notification, TYPE_NOTIFICATION};
pub use packet::NetworkPacket;
pub use pair::{pair_packet, PairAction, PairState, Pairing, TIMEOUT_SECS, TYPE_PAIR};
pub use ping::{ping_packet, TYPE_PING};
pub use session::{Outgoing, Reaction, Session};
pub use sftp::{read_sftp, request_packet, SftpMount, SftpReply, TYPE_SFTP, TYPE_SFTP_REQUEST};
pub use share::{read_share, share_request_packet, IncomingFile, TYPE_SHARE_REQUEST};
