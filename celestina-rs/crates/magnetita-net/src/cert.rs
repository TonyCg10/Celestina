//! The device certificate — our identity on the TLS wire, and the thing the
//! phone pins.
//!
//! KDE Connect does not use a certificate authority. Each device makes one
//! self-signed certificate, keeps it forever, and the *first* time two devices
//! pair they remember each other's certificate — trust-on-first-use. From then
//! on a link is trusted only if the certificate matches the pinned one, so the
//! certificate *is* the device's identity; its SHA-256 [`fingerprint`] is what
//! the trust store pins. The short code humans compare is a separate symmetric
//! hash of both public keys and the active pairing timestamp.
//!
//! So this is generated once and never casually regenerated — throwing it away
//! is unpairing from every device at once. [`DeviceCert::ensure`] makes it on
//! first run and loads the same one every run after, next to the rest of the
//! app's data.
//!
//! [`fingerprint`]: DeviceCert::fingerprint

use std::fs;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// The file names under the cert directory. Matching KDE Connect's own names
/// keeps the on-disk layout familiar to anyone who has looked at its config.
const CERT_FILE: &str = "certificate.pem";
const KEY_FILE: &str = "privateKey.pem";

/// One device's long-lived self-signed certificate and its private key, held as
/// PEM so it round-trips to disk unchanged and parses to DER on demand for
/// rustls.
#[derive(Clone)]
pub struct DeviceCert {
    cert_pem: String,
    key_pem: String,
}

impl DeviceCert {
    /// Load the certificate at `dir`, or generate and persist a fresh one there
    /// if absent. `device_id` becomes the certificate's Common Name, the way
    /// KDE Connect binds the id to the key. The directory is created if missing;
    /// the private key is written owner-only.
    pub fn ensure(dir: &Path, device_id: &str) -> io::Result<DeviceCert> {
        let cert_path = dir.join(CERT_FILE);
        let key_path = dir.join(KEY_FILE);
        if cert_path.exists() && key_path.exists() {
            return Ok(DeviceCert {
                cert_pem: fs::read_to_string(&cert_path)?,
                key_pem: fs::read_to_string(&key_path)?,
            });
        }
        fs::create_dir_all(dir)?;
        let fresh = DeviceCert::generate(device_id);
        write_private(&key_path, &fresh.key_pem)?;
        fs::write(&cert_path, &fresh.cert_pem)?;
        Ok(fresh)
    }

    /// A fresh self-signed EC certificate in memory, not touching disk — the
    /// building block of [`ensure`](DeviceCert::ensure), and what tests use so
    /// they never write to a real home.
    pub fn generate(device_id: &str) -> DeviceCert {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};

        let mut params =
            CertificateParams::new(Vec::new()).expect("no subject-alt-names is always valid");
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, device_id);
        dn.push(DnType::OrganizationName, "KDE");
        dn.push(DnType::OrganizationalUnitName, "Kde connect");
        params.distinguished_name = dn;

        let key_pair = KeyPair::generate().expect("ring generates a P-256 key");
        let cert = params
            .self_signed(&key_pair)
            .expect("self-signing our own params never fails");
        DeviceCert {
            cert_pem: cert.pem(),
            key_pem: key_pair.serialize_pem(),
        }
    }

    /// The certificate chain to present in a TLS handshake — for a self-signed
    /// device certificate that is just the one certificate.
    pub fn chain(&self) -> io::Result<Vec<CertificateDer<'static>>> {
        rustls_pemfile::certs(&mut BufReader::new(self.cert_pem.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// The private key that proves we own the certificate.
    pub fn private_key(&self) -> io::Result<PrivateKeyDer<'static>> {
        rustls_pemfile::private_key(&mut BufReader::new(self.key_pem.as_bytes()))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no private key in PEM"))
    }

    /// The certificate's SHA-256 fingerprint, lowercase hex with colon-separated
    /// bytes — the stable value the trust store pins a peer by.
    pub fn fingerprint(&self) -> io::Result<String> {
        let chain = self.chain()?;
        let leaf = chain
            .first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty certificate"))?;
        Ok(fingerprint_der(leaf))
    }
}

/// SHA-256 of a DER certificate as lowercase `aa:bb:…` hex — used for our own
/// certificate and, by the trust store, for a peer's. The hash is ring's, the
/// provider rustls already links, so no extra crypto crate.
pub fn fingerprint_der(der: &CertificateDer<'_>) -> String {
    let sum = ring::digest::digest(&ring::digest::SHA256, der.as_ref());
    let sum = sum.as_ref();
    let mut out = String::with_capacity(sum.len() * 3);
    for (i, byte) in sum.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// KDE Connect's human-comparable code for one active pairing exchange.
///
/// Both peers sort the two RFC 5280 SubjectPublicKeyInfo encodings in the same
/// descending byte order, hash them, and for protocol v8 append the request's
/// decimal Unix timestamp. A restored v8 session has no active timestamp, so it
/// truthfully has no new code to display.
pub fn verification_key(
    ours: &CertificateDer<'_>,
    peer: &CertificateDer<'_>,
    timestamp: Option<i64>,
    protocol_version: i32,
) -> io::Result<Option<String>> {
    let mut a = public_key_der(ours)?;
    let mut b = public_key_der(peer)?;
    Ok(verification_key_from_spki(
        &mut a,
        &mut b,
        timestamp,
        protocol_version,
    ))
}

fn verification_key_from_spki(
    a: &mut Vec<u8>,
    b: &mut Vec<u8>,
    timestamp: Option<i64>,
    protocol_version: i32,
) -> Option<String> {
    if a < b {
        std::mem::swap(a, b);
    }

    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    hash.update(a);
    hash.update(b);
    if protocol_version >= 8 {
        let timestamp = timestamp?;
        hash.update(timestamp.to_string().as_bytes());
    }
    let digest = hash.finish();
    let code = digest.as_ref()[..4]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect();
    Some(code)
}

fn public_key_der(certificate: &CertificateDer<'_>) -> io::Result<Vec<u8>> {
    let parsed = rustls::server::ParsedCertificate::try_from(certificate)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(parsed.subject_public_key_info().as_ref().to_vec())
}

/// Writes a private key owner-readable-only where the platform allows it.
fn write_private(path: &PathBuf, pem: &str) -> io::Result<()> {
    fs::write(path, pem)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{verification_key, verification_key_from_spki, DeviceCert};
    use rustls::pki_types::CertificateDer;

    #[test]
    fn a_generated_cert_parses_to_a_chain_and_key() {
        let dc = DeviceCert::generate("celestina-test");
        assert_eq!(dc.chain().unwrap().len(), 1);
        assert!(dc.private_key().is_ok());
    }

    #[test]
    fn the_fingerprint_is_colon_hex_sha256() {
        let dc = DeviceCert::generate("celestina-test");
        let fp = dc.fingerprint().unwrap();
        // 32 bytes → 32 hex pairs joined by 31 colons = 95 chars.
        assert_eq!(fp.len(), 95);
        assert_eq!(fp.matches(':').count(), 31);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit() || c == ':'));
    }

    #[test]
    fn two_generated_certs_differ() {
        let a = DeviceCert::generate("same-id").fingerprint().unwrap();
        let b = DeviceCert::generate("same-id").fingerprint().unwrap();
        assert_ne!(a, b, "each generation is a fresh key");
    }

    #[test]
    fn ensure_persists_and_then_reloads_the_same_cert() {
        let dir = std::env::temp_dir().join(format!("mag-cert-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let first = DeviceCert::ensure(&dir, "celestina-abc").unwrap();
        let again = DeviceCert::ensure(&dir, "celestina-abc").unwrap();
        assert_eq!(
            first.fingerprint().unwrap(),
            again.fingerprint().unwrap(),
            "a second ensure loads the persisted cert, not a new one"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_pairing_code_is_symmetric_and_timestamp_bound() {
        let a = DeviceCert::generate("a").chain().unwrap().remove(0);
        let b = DeviceCert::generate("b").chain().unwrap().remove(0);
        let ab = verification_key(&a, &b, Some(1_700_000_000), 8)
            .unwrap()
            .unwrap();
        let ba = verification_key(&b, &a, Some(1_700_000_000), 8)
            .unwrap()
            .unwrap();
        let later = verification_key(&a, &b, Some(1_700_000_001), 8)
            .unwrap()
            .unwrap();
        assert_eq!(ab, ba);
        assert_ne!(ab, later);
        assert_eq!(ab.len(), 8);
        assert!(ab.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn a_restored_v8_link_does_not_invent_a_pairing_code() {
        let a = DeviceCert::generate("a").chain().unwrap().remove(0);
        let b = DeviceCert::generate("b").chain().unwrap().remove(0);
        assert_eq!(verification_key(&a, &b, None, 8).unwrap(), None);
        assert!(verification_key(&a, &b, None, 7).unwrap().is_some());
    }

    #[test]
    fn the_pairing_hash_matches_a_fixed_protocol_vector() {
        let mut a = b"key-a".to_vec();
        let mut b = b"key-b".to_vec();
        assert_eq!(
            verification_key_from_spki(&mut a, &mut b, Some(1_700_000_000), 8).as_deref(),
            Some("7C6FA008")
        );
    }

    #[test]
    fn malformed_certificate_der_is_rejected() {
        let malformed = CertificateDer::from(vec![0x30, 0x01, 0xff]);
        let valid = DeviceCert::generate("valid").chain().unwrap().remove(0);
        assert_eq!(
            verification_key(&malformed, &valid, Some(1_700_000_000), 8)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}
