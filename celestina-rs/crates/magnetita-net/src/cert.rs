//! The device certificate — our identity on the own link's TLS, and the thing
//! the phone pins.
//!
//! There is no certificate authority. Each device makes one self-signed
//! certificate and keeps it forever; pairing by QR carries each side's
//! fingerprint to the other, and from then on a link is trusted only if the
//! certificate matches the pinned one, so the certificate *is* the device's
//! identity; its SHA-256 [`fingerprint`] is what the trust store pins.
//!
//! So this is generated once and never casually regenerated — throwing it away
//! is unpairing from every device at once. [`DeviceCert::ensure`] makes it on
//! first run and loads the same one every run after, next to the rest of the
//! app's data.
//!
//! [`fingerprint`]: DeviceCert::fingerprint

use std::fs;
use std::io::{self, BufReader};
use std::path::Path;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// The file names under the cert directory. They are the names the daemon has
/// always used, so an existing identity is found where it was left.
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
    /// if absent. `device_id` becomes the certificate's Common Name. A missing
    /// directory is created
    /// `0700`, and both files are written owner-only through
    /// [`celestina_core::atomic_file::replace_private`], whose sibling is
    /// `0600` from its creation: the private key is never readable by another
    /// local user, not even for the moment between its write and its rename,
    /// and an interrupted write leaves the previous key, or none, never a
    /// truncated one.
    pub fn ensure(dir: &Path, device_id: &str) -> io::Result<DeviceCert> {
        let cert_path = dir.join(CERT_FILE);
        let key_path = dir.join(KEY_FILE);
        if cert_path.exists() && key_path.exists() {
            return Ok(DeviceCert {
                cert_pem: fs::read_to_string(&cert_path)?,
                key_pem: fs::read_to_string(&key_path)?,
            });
        }
        let fresh = DeviceCert::generate(device_id);
        for (path, pem) in [(&key_path, &fresh.key_pem), (&cert_path, &fresh.cert_pem)] {
            celestina_core::atomic_file::replace_private(path, pem.as_bytes())
                .map(drop)
                .map_err(io::Error::other)?;
        }
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
        // Only new certificates carry this name: pins are by fingerprint, so
        // an identity made under the older name stays valid.
        dn.push(DnType::OrganizationName, "Celestina");
        dn.push(DnType::OrganizationalUnitName, "Magnetita");
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

#[cfg(test)]
mod tests {
    use super::DeviceCert;

    #[test]
    fn a_new_certificate_names_the_suite_not_kde_connect() {
        let der = DeviceCert::generate("celestina-test").chain().unwrap()[0].to_vec();
        let has = |needle: &[u8]| der.windows(needle.len()).any(|w| w == needle);
        assert!(has(b"Magnetita") && has(b"Celestina"));
        assert!(!has(b"KDE") && !has(b"Kde connect"));
    }

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

    #[cfg(unix)]
    #[test]
    fn the_private_key_is_never_readable_by_another_local_user() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("mag-cert-mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        DeviceCert::ensure(&dir, "celestina-abc").unwrap();

        let key_mode = std::fs::metadata(dir.join(super::KEY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(key_mode & 0o777, 0o600, "the key must be owner-only");
        let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(dir_mode & 0o777, 0o700, "the directory must be owner-only");
        // The publication is a rename, so no temporary survives it.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
