//! The pins, and the one way a certificate becomes a fingerprint here.
//!
//! The store itself is `magnetita-net`'s [`TrustStore`]: the same file, the
//! same colon-separated hex the KDE Connect wire pins, so a phone paired on
//! either wire is one entry. This module adds what the link needs on top:
//! a lookup by fingerprint alone, because on an incoming QUIC connection the
//! certificate is known before the hello names a device.

use magnetita_net::trust::{TrustStore, TrustedPeer};
use magnetita_proto::pair::Fingerprint;
use rustls::pki_types::CertificateDer;

/// The protocol's fingerprint of a DER certificate.
pub fn fingerprint_of(der: &CertificateDer<'_>) -> Fingerprint {
    magnetita_proto::pair::fingerprint(der.as_ref())
}

/// The trust store's text form of a fingerprint: colon-separated hex.
pub fn fingerprint_text(fp: &Fingerprint) -> String {
    fp.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// The device id a certificate names: the first eight bytes of its
/// fingerprint in lowercase hex. A phone calls itself this in its hello, and
/// the desktop pins a phone under no other id, so one certificate cannot
/// claim another device's id.
pub fn device_id_of(fp: &Fingerprint) -> String {
    fp[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// The store, seen from the link.
pub struct Trust<'a>(pub &'a TrustStore);

impl Trust<'_> {
    /// The pinned peer presenting this certificate, if any.
    pub fn peer_by_fingerprint(&self, fp: &Fingerprint) -> Option<TrustedPeer> {
        let text = fingerprint_text(fp);
        self.0.peers().find(|p| p.fingerprint == text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_net::cert::fingerprint_der;

    #[test]
    fn text_form_matches_the_kde_connect_wire() {
        let fp = [0xab; 32];
        let text = fingerprint_text(&fp);
        assert_eq!(text.len(), 95);
        assert!(text.starts_with("ab:ab:"));
        let der = CertificateDer::from(vec![1u8, 2, 3]);
        assert_eq!(
            fingerprint_text(&fingerprint_of(&der)),
            fingerprint_der(&der)
        );
    }

    #[test]
    fn the_device_id_is_the_fingerprint_prefix() {
        let mut fp = [0u8; 32];
        fp[..8].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef, 0x00, 0x01, 0x02, 0x0a]);
        fp[8] = 0xff;
        assert_eq!(device_id_of(&fp), "deadbeef0001020a");
    }

    #[test]
    fn lookup_by_fingerprint_finds_the_pin() {
        let mut store = TrustStore::in_memory();
        let fp = [7u8; 32];
        store
            .pin(TrustedPeer {
                device_id: "phone".into(),
                device_name: "S25U".into(),
                fingerprint: fingerprint_text(&fp),
            })
            .unwrap();
        assert_eq!(
            Trust(&store).peer_by_fingerprint(&fp).unwrap().device_id,
            "phone"
        );
        assert!(Trust(&store).peer_by_fingerprint(&[8u8; 32]).is_none());
    }
}
