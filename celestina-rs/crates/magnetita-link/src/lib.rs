#![forbid(unsafe_code)]

//! The link — `magnetita-proto` on the wire.
//!
//! QUIC with mutual TLS on self-signed certificates: each device holds one
//! certificate for life (the same [`DeviceCert`] the KDE Connect wire uses),
//! and after pairing the only thing that authenticates a connection is the
//! peer certificate's SHA-256 fingerprint, pinned in the [`TrustStore`].
//! The TLS layer here therefore accepts *any* certificate; what it never does
//! is hand a connection to the caller before that certificate has been
//! checked against the pins, or against the one fingerprint a pairing
//! expects. That check is [`Endpoint::accept`] and [`Endpoint::connect`],
//! and there is no way around them.
//!
//! One [`Endpoint`] is both server and client on one UDP socket, so either
//! side may dial. A [`Session`] is one connection: a control stream of
//! length-framed envelopes, transfer streams for bulk bytes, datagrams for
//! pointer motion, and the hello exchange that opens every session. The
//! `MAG-S1` handshake discipline carries over: one absolute deadline covers
//! the whole of accept, TLS and hello.
//!
//! What is *not* here: discovery, the daemon's device registry, the
//! reconnection loop itself. [`Backoff`] is the pure schedule that loop
//! follows; running it is the daemon's, so that it stays one owned thread.

pub mod backoff;
pub mod discovery;
pub mod endpoint;
pub mod error;
pub mod session;
pub mod tls;
pub mod trust;

pub use backoff::Backoff;
pub use discovery::{Peer, PORT, SERVICE_TYPE};
pub use endpoint::{Endpoint, EndpointConfig, Incoming};
pub use error::LinkError;
pub use magnetita_net::cert::DeviceCert;
pub use magnetita_net::trust::{TrustCheck, TrustStore, TrustedPeer};
pub use quinn::{RecvStream, SendStream, VarInt};
pub use session::{Session, Transfers, HANDSHAKE_BUDGET};
pub use trust::{fingerprint_of, fingerprint_text, Trust};
