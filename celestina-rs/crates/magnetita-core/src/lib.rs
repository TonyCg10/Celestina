#![forbid(unsafe_code)]

//! Magnetita's shared shapes with no sockets, no TLS and no threads: what a
//! clipboard text worth syncing is, a phone notification, a player's state
//! and the transport actions, the mirror's state machine and its options.
//! The wire itself is `magnetita-proto` and the link `magnetita-link`; this
//! crate is what the daemon, the desktop app and the shell agree on.

pub mod clipboard;
pub mod mirror;
pub mod mirror_options;
pub mod mpris;
pub mod notification;
pub mod text;

pub use mirror::{
    valid_service_name, AdbService, MirrorAction, MirrorEndpoint, MirrorError, MirrorEvent,
    MirrorLink, MirrorState, SERVICE_CONNECT, SERVICE_PAIRING,
};
pub use mirror_options::{MirrorAudio, MirrorOptions, MirrorQuality, MirrorRate, MirrorResolution};
pub use mpris::{
    playback_progress, IncomingAlbumArt, MediaAction, MprisRequest, PlaybackProgress, PlayerState,
};
pub use notification::Notification;
