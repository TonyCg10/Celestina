//! The one way to end a session lock.
//!
//! [`UnlockCapability`] is minted by the lock runtime exactly once, when the
//! compositor's `locked` arrives, and handed to the product inside
//! `LockEvent::Locked`. It is neither `Clone` nor `Copy`; [`unlock`] consumes
//! it. The toolkit has no other function that sends `unlock_and_destroy`, and
//! the underlying `SessionLock` never leaves this crate, so a consumer that
//! was not given the capability cannot unlock, and one that was given it can
//! unlock once (design §4.13, T-U1).
//!
//! Dropping the capability without calling [`unlock`] does nothing: the lock
//! stays, as it does when the process dies (the compositor keeps the session
//! locked for a dead client).
//!
//! [`unlock`]: UnlockCapability::unlock
#![forbid(unsafe_code)]

use smithay_client_toolkit::session_lock::SessionLock;
use wayland_client::{backend::WaylandError, Connection};

/// Why the unlock could not be confirmed. The request was sent; whether the
/// compositor processed it is unknown (the connection broke while waiting).
#[derive(Debug, thiserror::Error)]
pub enum UnlockError {
    #[error("the compositor did not answer the round trip that confirms the unlock")]
    Sync(#[source] WaylandError),
}

impl UnlockError {
    /// Whether the round trip failed because the compositor killed this
    /// client with a protocol error (the compositor is alive and the session
    /// stays locked), as opposed to a connection that broke (the compositor
    /// is gone).
    #[must_use]
    pub fn is_protocol_error(&self) -> bool {
        match self {
            Self::Sync(err) => matches!(err, WaylandError::Protocol(_)),
        }
    }
}

/// The right to end this session lock, once.
#[must_use = "the lock ends only when `unlock` is called"]
pub struct UnlockCapability {
    lock: SessionLock,
    conn: Connection,
}

impl std::fmt::Debug for UnlockCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UnlockCapability")
    }
}

impl UnlockCapability {
    /// Minted by the lock runtime for the `locked` event, and only there.
    pub(super) fn issue(lock: SessionLock, conn: Connection) -> Self {
        Self { lock, conn }
    }

    /// Sends `unlock_and_destroy` and waits for the compositor to process it
    /// with a synchronous round trip, so the caller may report the session
    /// unlocked when this returns `Ok`. Blocks for as long as that round trip
    /// takes; called from inside the loop, as sctk's own example does.
    pub fn unlock(self) -> Result<(), UnlockError> {
        self.lock.unlock();
        self.conn.roundtrip().map(|_| ()).map_err(UnlockError::Sync)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wayland_client::backend::protocol::ProtocolError;

    #[test]
    fn a_protocol_error_is_told_from_a_broken_connection() {
        let protocol = UnlockError::Sync(WaylandError::Protocol(ProtocolError {
            code: 1,
            object_id: 3,
            object_interface: "ext_session_lock_v1".into(),
            message: "invalid".into(),
        }));
        assert!(protocol.is_protocol_error());
        let broken = UnlockError::Sync(WaylandError::Io(std::io::Error::from(
            std::io::ErrorKind::BrokenPipe,
        )));
        assert!(!broken.is_protocol_error());
    }
}
