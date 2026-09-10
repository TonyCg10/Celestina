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
use std::io::{self, BufReader, Write};
use std::path::Path;

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
        create_private_dir(dir)?;
        let fresh = DeviceCert::generate(device_id);
        write_private(&key_path, &fresh.key_pem)?;
        celestina_core::atomic_file::replace(&cert_path, fresh.cert_pem.as_bytes())?;
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

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)
}

/// Publish the private key atomically and owner-only.
///
/// The mode is part of the *creation*, not a repair afterwards: a plain write
/// followed by `set_permissions` leaves a window in which `privateKey.pem` is
/// world-readable, and this is the one file in the suite whose disclosure is
/// total. The atomic sibling-then-rename shape is the suite's, but
/// [`celestina_core::atomic_file::replace`] cannot be reused here because its
/// temporary is created at the process umask, which is exactly the window this
/// closes. The rename means an interrupted write leaves the previous key —
/// or no key — never a truncated PEM the daemon can never start from again.
fn write_private(path: &Path, pem: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temporary = parent.join(format!(".{KEY_FILE}.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temporary);
    let result = (|| {
        let mut file = private_file(&temporary)?;
        file.write_all(pem.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn private_file(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::DeviceCert;

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
