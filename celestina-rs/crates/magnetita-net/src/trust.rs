//! The peer trust store — who we have paired with, remembered by certificate.
//!
//! KDE Connect's trust is first-use, not authority: the first time we pair with
//! a phone we write down its certificate's [`fingerprint`] under its device id,
//! and from then on a link claiming to be that phone is believed only if the
//! certificate still matches. A *different* certificate for a known id is the
//! one thing we refuse outright — it is either the phone reinstalled (and the
//! user must deliberately re-pair) or someone standing in the middle, and we do
//! not guess which.
//!
//! So the store answers one question — [`TrustStore::check`] — with three
//! honest outcomes: known-and-matching, never-seen, or known-but-changed. It is
//! pure and file-backed, no sockets; pinning and forgetting are the only writes,
//! and each persists immediately because pairing is a durable promise.
//!
//! [`fingerprint`]: crate::cert::fingerprint_der

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A device we have paired with, and the certificate we pinned for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustedPeer {
    pub device_id: String,
    pub device_name: String,
    /// Colon-separated lowercase hex SHA-256 of the peer's certificate, as
    /// produced by [`fingerprint_der`](crate::cert::fingerprint_der).
    pub fingerprint: String,
}

/// The verdict on a certificate presented for a device id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustCheck {
    /// A pinned peer whose certificate still matches — trusted.
    Trusted,
    /// No pin for this id yet — a first pairing may establish one.
    Unknown,
    /// A pin exists but the certificate differs — refused as a possible
    /// impostor (or a reinstall that must be re-paired on purpose).
    Changed,
}

/// The pinned peers, keyed by device id, optionally backed by a file.
#[derive(Debug, Default)]
pub struct TrustStore {
    peers: BTreeMap<String, PeerRecord>,
    /// Where to persist. `None` keeps the store in memory only (tests).
    path: Option<PathBuf>,
}

/// The on-disk shape: the device id is the map key, so it is not repeated here.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PeerRecord {
    #[serde(rename = "deviceName")]
    device_name: String,
    fingerprint: String,
}

#[derive(Serialize, Deserialize, Default)]
struct StoreFile {
    peers: BTreeMap<String, PeerRecord>,
}

impl TrustStore {
    /// Load the store at `path`, or start empty (still bound to `path`) if the
    /// file is absent. A file that exists but does not parse is an error, not a
    /// silent reset — silently dropping trust would unpair every device.
    pub fn load(path: &Path) -> io::Result<TrustStore> {
        let peers = match fs::read_to_string(path) {
            Ok(text) => {
                serde_json::from_str::<StoreFile>(&text)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    .peers
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e),
        };
        Ok(TrustStore {
            peers,
            path: Some(path.to_path_buf()),
        })
    }

    /// A store that never touches disk — for tests and throwaway sessions.
    pub fn in_memory() -> TrustStore {
        TrustStore::default()
    }

    /// The verdict on a certificate `fingerprint` claimed for `device_id`.
    pub fn check(&self, device_id: &str, fingerprint: &str) -> TrustCheck {
        match self.peers.get(device_id) {
            None => TrustCheck::Unknown,
            Some(rec) if rec.fingerprint.eq_ignore_ascii_case(fingerprint) => TrustCheck::Trusted,
            Some(_) => TrustCheck::Changed,
        }
    }

    /// Whether we have any pin for this device id (regardless of a live cert).
    pub fn is_trusted(&self, device_id: &str) -> bool {
        self.peers.contains_key(device_id)
    }

    /// Pin (or re-pin) a peer and persist. Re-pinning overwrites — the way a
    /// deliberate re-pair after a reinstall replaces the old certificate.
    pub fn pin(&mut self, peer: TrustedPeer) -> io::Result<()> {
        self.peers.insert(
            peer.device_id,
            PeerRecord {
                device_name: peer.device_name,
                fingerprint: peer.fingerprint,
            },
        );
        self.persist()
    }

    /// Forget a peer (unpair) and persist. Forgetting an unknown id is a no-op,
    /// still persisted so the file reflects the intent.
    pub fn forget(&mut self, device_id: &str) -> io::Result<()> {
        self.peers.remove(device_id);
        self.persist()
    }

    /// The pinned peers, in device-id order.
    pub fn peers(&self) -> impl Iterator<Item = TrustedPeer> + '_ {
        self.peers.iter().map(|(id, rec)| TrustedPeer {
            device_id: id.clone(),
            device_name: rec.device_name.clone(),
            fingerprint: rec.fingerprint.clone(),
        })
    }

    /// Writes the store to its file, creating the parent directory. A no-op for
    /// an in-memory store.
    fn persist(&self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = StoreFile {
            peers: self.peers.clone(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::{TrustCheck, TrustStore, TrustedPeer};

    fn peer(id: &str, fp: &str) -> TrustedPeer {
        TrustedPeer {
            device_id: id.to_owned(),
            device_name: "Galaxy".to_owned(),
            fingerprint: fp.to_owned(),
        }
    }

    #[test]
    fn unknown_then_pinned_then_matching_is_trusted() {
        let mut s = TrustStore::in_memory();
        assert_eq!(s.check("phone", "aa:bb"), TrustCheck::Unknown);
        s.pin(peer("phone", "aa:bb")).unwrap();
        assert_eq!(s.check("phone", "aa:bb"), TrustCheck::Trusted);
        assert!(s.is_trusted("phone"));
    }

    #[test]
    fn a_changed_certificate_for_a_known_id_is_refused() {
        let mut s = TrustStore::in_memory();
        s.pin(peer("phone", "aa:bb")).unwrap();
        assert_eq!(s.check("phone", "cc:dd"), TrustCheck::Changed);
    }

    #[test]
    fn the_match_is_case_insensitive() {
        let mut s = TrustStore::in_memory();
        s.pin(peer("phone", "AA:BB:CC")).unwrap();
        assert_eq!(s.check("phone", "aa:bb:cc"), TrustCheck::Trusted);
    }

    #[test]
    fn forgetting_returns_to_unknown() {
        let mut s = TrustStore::in_memory();
        s.pin(peer("phone", "aa:bb")).unwrap();
        s.forget("phone").unwrap();
        assert_eq!(s.check("phone", "aa:bb"), TrustCheck::Unknown);
        assert!(!s.is_trusted("phone"));
    }

    #[test]
    fn a_pin_survives_a_reload_from_disk() {
        let path = std::env::temp_dir().join(format!("mag-trust-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let mut s = TrustStore::load(&path).unwrap();
        s.pin(peer("phone", "aa:bb:cc")).unwrap();

        let reloaded = TrustStore::load(&path).unwrap();
        assert_eq!(reloaded.check("phone", "aa:bb:cc"), TrustCheck::Trusted);
        assert_eq!(reloaded.peers().count(), 1);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_missing_file_loads_empty_but_a_corrupt_one_errors() {
        let missing = std::env::temp_dir().join("mag-trust-does-not-exist-xyz.json");
        let _ = std::fs::remove_file(&missing);
        assert_eq!(TrustStore::load(&missing).unwrap().peers().count(), 0);

        let corrupt =
            std::env::temp_dir().join(format!("mag-trust-bad-{}.json", std::process::id()));
        std::fs::write(&corrupt, "{ not json").unwrap();
        assert!(TrustStore::load(&corrupt).is_err());
        std::fs::remove_file(&corrupt).unwrap();
    }
}
