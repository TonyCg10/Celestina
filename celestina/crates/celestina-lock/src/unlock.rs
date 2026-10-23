//! The single unlock site (design §4.5, §4.13, §6 items 1 and 2, TM-1, T-U1).
//!
//! This is the only function in the crate that consumes the toolkit's
//! [`UnlockCapability`], and it takes the [`Authenticated`] token with it: no
//! timeout, crash, error, signal, file or event can reach it, because none of
//! them can make the token. The toolkit has no other unlock path, so the lock
//! can end only from here (`tests/guards.rs` scans for any second call site).
#![forbid(unsafe_code)]

use celestina_wayland::lock::{UnlockCapability, UnlockError};

use crate::verify::Authenticated;

/// Ends the session lock after a verifier said yes, and waits for the
/// compositor to process it.
///
/// # Errors
///
/// [`UnlockError`] when the round trip that confirms the unlock failed: the
/// request was sent, whether the compositor acted on it is unknown, so the
/// caller does not report `unlocked`.
pub fn unlock(capability: UnlockCapability, proof: Authenticated) -> Result<(), UnlockError> {
    // The proof is spent: one token, one unlock.
    drop(proof);
    capability.unlock()
}
