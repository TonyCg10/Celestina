#![forbid(unsafe_code)]

//! What the own link and the daemon share below the wire: the device
//! certificate (self-signed, generated once, pinned by fingerprint), the
//! trust store of pinned peers, and the limiter that bounds concurrent bulk
//! transfers. The QUIC link itself is `magnetita-link`.

pub mod cert;
pub mod payload;
pub mod trust;

pub use cert::{fingerprint_der, DeviceCert};
pub use payload::{PayloadLimiter, PayloadPermit, MAX_PAYLOAD_SIZE};
pub use trust::{TrustCheck, TrustStore, TrustedPeer};
