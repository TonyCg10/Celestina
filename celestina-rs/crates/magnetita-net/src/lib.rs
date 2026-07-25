#![forbid(unsafe_code)]

//! Magnetita's live transport — the sockets, TLS and threads that turn the pure
//! [`magnetita-core`] protocol into a real link to the phone.
//!
//! Where `magnetita-core` is the offline brain (packets, identity, pairing,
//! session), this crate is the body: it hears the phone's UDP announce, opens
//! the TCP link, wraps it in TLS with a **pinned, self-signed peer certificate**
//! (trust-on-first-use, not a CA chain), re-exchanges identities over the
//! encrypted channel, and then feeds each arrived [`NetworkPacket`] to a
//! [`Session`] and sends back what the session decides.
//!
//! It stays deliberately small: blocking `std::net` sockets on a thread, no
//! async runtime. CP0 talks to one phone, and a `tokio` reactor is weight we
//! have not earned. The KDE Connect v8 handshake it implements is exact — the
//! device that *hears* the announce opens the link and becomes the **TLS
//! server**; the announcer becomes the **TLS client**; from protocol 8 on, the
//! identities are re-sent *encrypted* once the channel is up.
//!
//! [`magnetita-core`]: magnetita_core
//! [`NetworkPacket`]: magnetita_core::NetworkPacket
//! [`Session`]: magnetita_core::Session

pub mod cert;
pub mod discovery;
pub mod link;
pub mod tls;
pub mod trust;

pub use cert::{fingerprint_der, DeviceCert};
pub use discovery::{parse_announcement, Announcement, Discovery};
pub use link::{Link, LinkError};
pub use tls::{peer_leaf_fingerprint, TlsConfigs};
pub use trust::{TrustCheck, TrustStore, TrustedPeer};

/// The transport's version, surfaced so the app and logs can report it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
