#![forbid(unsafe_code)]

//! Magnetita's own protocol — the wire both ends share, pure and offline.
//!
//! ADR 0001 replaces the KDE Connect wire with a private one, and this crate
//! is its single implementation: the desktop daemon links it, and the Android
//! application links the very same crate through UniFFI. There is no second
//! copy of any rule here in Kotlin, so interoperability is by construction.
//!
//! Nothing in this crate does I/O, keeps time or spawns a thread. It turns
//! bytes into typed messages and typed messages into bytes, and it refuses
//! bytes it does not like with a typed [`DecodeError`]. The transport
//! (`magnetita-link`) hands it one [`Envelope`] at a time; what the envelope
//! carries is one message of one capability, encoded by the capability's own
//! module.
//!
//! **Every peer-chosen length is bounded before it is copied.** A string, a
//! byte string, a list or a map from the network is read only after its
//! declared length has been checked against the limit in [`bound`]; the
//! `MAG-S1` lesson — validate where the value becomes typed, never above —
//! is a property of this decoder, not a patch on it.
//!
//! The wire is CBOR with integer map keys. Unknown keys are skipped, so a
//! newer peer can add fields; unknown capabilities are declined in the
//! [`Hello`], so a newer peer can add features; and the envelope carries a
//! protocol version so a wire change that neither of those covers is refused
//! outright.

pub mod bound;
pub mod envelope;
pub mod error;
pub mod hello;
pub mod pair;

pub use envelope::{Envelope, PROTOCOL_VERSION};
pub use error::DecodeError;
pub use hello::{capability, negotiate, CapabilityVersion, DeviceKind, Hello};
pub use pair::{CodePairing, Fingerprint, PairError, Pinned, QrPairing, QrPayload};
