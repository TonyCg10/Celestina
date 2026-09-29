//! Who may become a session on the own wire, and under which id.
//!
//! The session loop applies two rules that live here, apart from it:
//!
//! - The pairing window. [`PairingArm::arm`] draws a one-time secret and
//!   returns the QR text; for two minutes an unpinned phone may connect and
//!   try to prove it holds the secret. Connecting does not use the window
//!   up: only a proof that verifies closes it, so a port scanner, a second
//!   phone or a racer on the LAN cannot burn the author's QR. Wrong proofs
//!   never close it either (the secret is 32 random bytes, so they cannot
//!   guess it): an address that has sent [`MAX_FAILED_PROOFS`] of them is
//!   refused for the rest of the window instead (as is every unpinned
//!   attempt once [`MAX_FAILURE_ADDRESSES`] addresses have failed), and the
//!   window keeps its own expiry. At most [`MAX_PAIRING_ATTEMPTS`] unpinned connections wait
//!   for their proof at once, and a phone gives its attempt back once it is
//!   pinned.
//! - The session id. A session runs under the id its certificate is pinned
//!   under, never under the id its hello claims, and a hello that names any
//!   other id is refused ([`session_id`]). A phone pairs only under the id
//!   its certificate gives it ([`magnetita_link::device_id_of`]), which is
//!   how the phone names itself, and never under an id already pinned to
//!   another certificate ([`pairing_id`]). One certificate therefore holds
//!   one id, and no pinned device can take over another's registry entry,
//!   mount, commands or pin.

use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use magnetita_link::trust::fingerprint_text;
use magnetita_link::{device_id_of, Trust, TrustCheck, TrustStore};
use magnetita_proto::pair::{Fingerprint, QrPayload};
use rand_core::{OsRng, RngCore};

use crate::lock::LockOk;

/// How long an armed pairing stays open.
const PAIRING_WINDOW: Duration = Duration::from_secs(120);
/// Wrong proofs one address may send in a window before it is refused for
/// the rest of it. The bound is not against guessing a 32-byte secret; it
/// keeps one host from occupying the pairing attempts for two minutes.
pub(super) const MAX_FAILED_PROOFS: u32 = 3;
/// Addresses one window keeps a wrong-proof count for. A full book refuses
/// every further unpinned attempt until the window ends, so the count stays
/// bounded without forgetting a host that was already refused.
pub(super) const MAX_FAILURE_ADDRESSES: usize = 256;
/// Unpinned connections that may wait for their proof at the same time.
pub(super) const MAX_PAIRING_ATTEMPTS: usize = 4;

struct Armed {
    secret: [u8; 32],
    until: Instant,
    /// Wrong proofs by source address. Only a completed TLS handshake can
    /// send a proof, so an entry is a real host of the LAN; the map holds at
    /// most [`MAX_FAILURE_ADDRESSES`] and lives only as long as the window.
    failures: HashMap<IpAddr, u32>,
}

/// The pairing window, shared between the served interface (which arms it)
/// and the link thread (which reads it and closes it on a verified proof).
#[derive(Clone, Default)]
pub(crate) struct PairingArm(Arc<Mutex<Option<Armed>>>);

impl PairingArm {
    /// Draws a fresh secret and returns the text the QR shows.
    pub(crate) fn arm(
        &self,
        device_id: &str,
        fingerprint: Fingerprint,
        addresses: Vec<String>,
    ) -> String {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        *self.0.lock_ok() = Some(Armed {
            secret,
            until: Instant::now() + PAIRING_WINDOW,
            failures: HashMap::new(),
        });
        QrPayload {
            device_id: device_id.to_owned(),
            fingerprint,
            secret,
            addresses,
        }
        .to_uri()
    }

    /// The secret while a window is open. Reading it leaves the window open.
    pub(super) fn live_secret(&self) -> Option<[u8; 32]> {
        let mut armed = self.0.lock_ok();
        match armed.as_ref() {
            Some(a) if a.until > Instant::now() => Some(a.secret),
            Some(_) => {
                *armed = None;
                None
            }
            None => None,
        }
    }

    /// Closes the window whose `secret` has just verified a proof. False when
    /// that window has already closed, expired or been armed again with
    /// another secret: one QR pins one phone.
    pub(super) fn consume(&self, secret: &[u8; 32]) -> bool {
        let mut armed = self.0.lock_ok();
        let live = armed
            .as_ref()
            .is_some_and(|a| a.until > Instant::now() && a.secret == *secret);
        if live {
            *armed = None;
        }
        live
    }

    /// Counts a proof from `address` that did not verify against `secret`.
    /// The window stays open; the address is refused once it has sent
    /// [`MAX_FAILED_PROOFS`]. A proof against an older window counts against
    /// nothing.
    pub(super) fn failed(&self, secret: &[u8; 32], address: IpAddr) {
        if let Some(a) = self.0.lock_ok().as_mut().filter(|a| a.secret == *secret) {
            let room = a.failures.len() < MAX_FAILURE_ADDRESSES;
            if let Some(count) = a.failures.get_mut(&address) {
                *count += 1;
            } else if room {
                a.failures.insert(address, 1);
            }
        }
    }

    /// Whether the open window refuses unpinned attempts from `address`: it
    /// has used up its wrong proofs, or the book of failing addresses is full.
    pub(super) fn refuses(&self, address: IpAddr) -> bool {
        self.0.lock_ok().as_ref().is_some_and(|a| {
            a.failures.len() >= MAX_FAILURE_ADDRESSES
                || a.failures.get(&address).copied().unwrap_or(0) >= MAX_FAILED_PROOFS
        })
    }

    #[cfg(test)]
    pub(super) fn is_armed(&self) -> bool {
        self.0
            .lock_ok()
            .as_ref()
            .is_some_and(|a| a.until > Instant::now())
    }
}

/// Why a peer may not run, or pair, under the id its hello names.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum IdentityRefusal {
    /// No pin holds the certificate any more: it was forgotten meanwhile.
    NotPinned,
    /// The hello names an id other than the one the certificate is pinned
    /// under.
    Claimed { claimed: String, pinned: String },
    /// A pairing phone names an id its certificate does not give it.
    NotDerived { claimed: String, derived: String },
    /// The id is already pinned to another certificate.
    PinnedElsewhere { id: String },
}

impl fmt::Display for IdentityRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPinned => f.write_str("the certificate is no longer pinned"),
            Self::Claimed { claimed, pinned } => write!(
                f,
                "the hello names {claimed:?}, the certificate is pinned as {pinned:?}"
            ),
            Self::NotDerived { claimed, derived } => write!(
                f,
                "the hello names {claimed:?}, the certificate's id is {derived:?}"
            ),
            Self::PinnedElsewhere { id } => {
                write!(f, "{id:?} is already pinned to another certificate")
            }
        }
    }
}

/// The id the session of a pinned certificate runs under: the id of its pin,
/// provided the hello names the same one.
pub(super) fn session_id(
    trust: &TrustStore,
    fp: &Fingerprint,
    claimed: &str,
) -> Result<String, IdentityRefusal> {
    let pinned = Trust(trust)
        .peer_by_fingerprint(fp)
        .ok_or(IdentityRefusal::NotPinned)?;
    if pinned.device_id != claimed {
        return Err(IdentityRefusal::Claimed {
            claimed: claimed.to_owned(),
            pinned: pinned.device_id,
        });
    }
    Ok(pinned.device_id)
}

/// The id a phone pairing with certificate `fp` is pinned under: the one the
/// certificate gives it, which its hello must name, and which no other
/// certificate may already hold. The same certificate pairing again keeps
/// its id.
pub(super) fn pairing_id(
    trust: &TrustStore,
    fp: &Fingerprint,
    claimed: &str,
) -> Result<String, IdentityRefusal> {
    let derived = device_id_of(fp);
    if claimed != derived {
        return Err(IdentityRefusal::NotDerived {
            claimed: claimed.to_owned(),
            derived,
        });
    }
    match trust.check(&derived, &fingerprint_text(fp)) {
        TrustCheck::Changed => Err(IdentityRefusal::PinnedElsewhere { id: derived }),
        TrustCheck::Trusted | TrustCheck::Unknown => Ok(derived),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_link::TrustedPeer;

    fn store_with(id: &str, fp: &Fingerprint) -> TrustStore {
        let mut store = TrustStore::in_memory();
        store
            .pin(TrustedPeer {
                device_id: id.into(),
                device_name: "S25U".into(),
                fingerprint: fingerprint_text(fp),
            })
            .unwrap();
        store
    }

    #[test]
    fn reading_the_window_leaves_it_open_until_a_verified_proof() {
        let arm = PairingArm::default();
        assert!(arm.live_secret().is_none());
        let uri = arm.arm("desk", [1u8; 32], vec!["10.0.0.1:1760".into()]);
        assert!(uri.starts_with("magnetita://pair?v=1&id=desk&fp=0101"));
        let secret = arm.live_secret().unwrap();
        assert_eq!(arm.live_secret(), Some(secret), "reading does not close it");
        assert!(!arm.consume(&[0u8; 32]), "another secret closes nothing");
        assert!(arm.is_armed());
        assert!(arm.consume(&secret));
        assert!(arm.live_secret().is_none(), "one QR pins one phone");
        assert!(!arm.consume(&secret));
    }

    #[test]
    fn a_window_expires_and_a_new_one_voids_the_old_secret() {
        let arm = PairingArm::default();
        arm.arm("desk", [1u8; 32], Vec::new());
        let old = arm.live_secret().unwrap();
        arm.arm("desk", [1u8; 32], Vec::new());
        assert!(
            !arm.consume(&old),
            "a proof against the old QR pins nothing"
        );
        *arm.0.lock_ok() = Some(Armed {
            secret: [0; 32],
            until: Instant::now() - Duration::from_secs(1),
            failures: HashMap::new(),
        });
        assert!(!arm.consume(&[0; 32]), "an expired window admits nobody");
        assert!(arm.live_secret().is_none());
    }

    #[test]
    fn wrong_proofs_refuse_their_address_and_never_close_the_window() {
        let arm = PairingArm::default();
        let (host, other) = (IpAddr::from([10, 0, 0, 7]), IpAddr::from([10, 0, 0, 8]));
        arm.arm("desk", [1u8; 32], Vec::new());
        let secret = arm.live_secret().unwrap();
        arm.failed(&[9u8; 32], host);
        for _ in 1..MAX_FAILED_PROOFS {
            arm.failed(&secret, host);
        }
        assert!(
            !arm.refuses(host),
            "a failure against another window is not counted"
        );
        for _ in 0..8 {
            arm.failed(&secret, host);
        }
        assert!(arm.refuses(host));
        assert!(!arm.refuses(other));
        assert!(arm.is_armed(), "the window stays open");
        assert!(arm.consume(&secret));
        assert!(!arm.refuses(host), "a new window forgets the old failures");
    }

    #[test]
    fn a_full_failure_book_refuses_every_unpinned_attempt_until_the_window_ends() {
        let arm = PairingArm::default();
        arm.arm("desk", [1u8; 32], Vec::new());
        let secret = arm.live_secret().unwrap();
        let host = |n: usize| IpAddr::from([10, 1, (n / 256) as u8, (n % 256) as u8]);
        for n in 0..MAX_FAILURE_ADDRESSES - 1 {
            arm.failed(&secret, host(n));
        }
        assert!(!arm.refuses(host(100_000)), "the book still has room");
        arm.failed(&secret, host(MAX_FAILURE_ADDRESSES - 1));
        arm.failed(&secret, host(MAX_FAILURE_ADDRESSES));
        assert_eq!(
            arm.0.lock_ok().as_ref().unwrap().failures.len(),
            MAX_FAILURE_ADDRESSES,
            "the book never grows past its bound"
        );
        assert!(arm.refuses(host(100_000)), "a full book refuses newcomers");
        assert!(arm.is_armed(), "the window itself stays open");
        arm.arm("desk", [1u8; 32], Vec::new());
        assert!(!arm.refuses(host(100_000)), "a new window starts empty");
    }

    #[test]
    fn a_session_runs_under_its_pin_and_never_under_a_claimed_id() {
        let fp = [3u8; 32];
        let store = store_with("phone-id", &fp);
        assert_eq!(session_id(&store, &fp, "phone-id"), Ok("phone-id".into()));
        assert_eq!(
            session_id(&store, &fp, "other"),
            Err(IdentityRefusal::Claimed {
                claimed: "other".into(),
                pinned: "phone-id".into()
            })
        );
        assert_eq!(
            session_id(&store, &[4u8; 32], "phone-id"),
            Err(IdentityRefusal::NotPinned)
        );
    }

    #[test]
    fn a_phone_pairs_only_under_its_own_free_id() {
        let fp = [5u8; 32];
        let derived = device_id_of(&fp);
        let empty = TrustStore::in_memory();
        assert_eq!(pairing_id(&empty, &fp, &derived), Ok(derived.clone()));
        assert!(matches!(
            pairing_id(&empty, &fp, "chosen"),
            Err(IdentityRefusal::NotDerived { .. })
        ));
        // The same certificate pairing again keeps its id.
        let own = store_with(&derived, &fp);
        assert_eq!(pairing_id(&own, &fp, &derived), Ok(derived.clone()));
        // The id pinned to another certificate is not taken over.
        let taken = store_with(&derived, &[6u8; 32]);
        assert_eq!(
            pairing_id(&taken, &fp, &derived),
            Err(IdentityRefusal::PinnedElsewhere { id: derived })
        );
    }
}
