//! The TLS layer — an encrypted channel to a device we do not yet trust by name.
//!
//! KDE Connect's TLS is *mutual and authority-free*. Both ends present their own
//! self-signed certificate and neither checks it against a certificate authority
//! — there is none. What is checked is real: the handshake signatures are
//! verified with the peer's presented key, so a peer proves it holds the private
//! key for the certificate it shows. We then read that certificate back out and
//! pin it ([`TrustStore`]) — trust-on-first-use. So the verifiers here accept
//! *any* certificate's identity but never fake its cryptography.
//!
//! The connector — the side that heard the announce and dialed — is the TLS
//! **server**; the announcer is the **client**. This module builds both configs
//! from our [`DeviceCert`] so either role works, and [`peer_leaf_fingerprint`]
//! reads the peer's certificate off a finished connection for the trust check.
//!
//! [`TrustStore`]: crate::trust::TrustStore

use std::io;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{
    ring, verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms,
};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, ServerConfig, SignatureScheme,
};

use crate::cert::{fingerprint_der, DeviceCert};

/// The two rustls configs for one device certificate: [`server_config`] for when
/// we dialed the peer (we are the TLS server) and [`client_config`] for when the
/// peer dialed us.
///
/// [`server_config`]: TlsConfigs::server_config
/// [`client_config`]: TlsConfigs::client_config
#[derive(Clone)]
pub struct TlsConfigs {
    server: Arc<ServerConfig>,
    client: Arc<ClientConfig>,
}

impl TlsConfigs {
    /// Build both configs from our device certificate.
    pub fn build(cert: &DeviceCert) -> io::Result<TlsConfigs> {
        let provider = Arc::new(ring::default_provider());
        let algs = provider.signature_verification_algorithms;
        let chain = cert.chain()?;
        let key = cert.private_key()?;

        let server = ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(to_io)?
            .with_client_cert_verifier(Arc::new(AcceptAnyPeer { algs }))
            .with_single_cert(chain.clone(), key.clone_key())
            .map_err(to_io)?;

        let client = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(to_io)?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyPeer { algs }))
            .with_client_auth_cert(chain, key)
            .map_err(to_io)?;

        Ok(TlsConfigs {
            server: Arc::new(server),
            client: Arc::new(client),
        })
    }

    /// The config to use as the TLS server (we dialed the peer).
    pub fn server_config(&self) -> Arc<ServerConfig> {
        self.server.clone()
    }

    /// The config to use as the TLS client (the peer dialed us).
    pub fn client_config(&self) -> Arc<ClientConfig> {
        self.client.clone()
    }
}

/// The peer's leaf-certificate fingerprint from a finished connection's
/// certificates (`conn.peer_certificates()`), for the trust check. `None` if the
/// peer presented nothing — which for KDE Connect's mutual TLS should not happen.
pub fn peer_leaf_fingerprint(peer_certificates: Option<&[CertificateDer<'_>]>) -> Option<String> {
    peer_certificates?.first().map(fingerprint_der)
}

fn to_io(e: rustls::Error) -> io::Error {
    io::Error::other(e)
}

/// Verifies a peer for both roles: it accepts any certificate's *identity*
/// (there is no CA to check against — pinning comes after) while verifying the
/// handshake signatures for real against the presented key, so a peer must
/// actually own the certificate it shows.
#[derive(Debug)]
struct AcceptAnyPeer {
    algs: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for AcceptAnyPeer {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

impl ClientCertVerifier for AcceptAnyPeer {
    /// No CA hints — we do not steer the peer toward any issuer.
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::{peer_leaf_fingerprint, TlsConfigs};
    use crate::cert::DeviceCert;
    use rustls::pki_types::ServerName;
    use rustls::{ClientConnection, ServerConnection};

    /// Drives a rustls client and server to a finished handshake with no sockets,
    /// by handing each side's outgoing TLS records straight to the other.
    fn pump(client: &mut ClientConnection, server: &mut ServerConnection) {
        for _ in 0..16 {
            let mut c2s = Vec::new();
            while client.wants_write() {
                client.write_tls(&mut c2s).unwrap();
            }
            let mut r = c2s.as_slice();
            while !r.is_empty() {
                server.read_tls(&mut r).unwrap();
            }
            server.process_new_packets().unwrap();

            let mut s2c = Vec::new();
            while server.wants_write() {
                server.write_tls(&mut s2c).unwrap();
            }
            let mut r = s2c.as_slice();
            while !r.is_empty() {
                client.read_tls(&mut r).unwrap();
            }
            client.process_new_packets().unwrap();

            if !client.is_handshaking() && !server.is_handshaking() {
                return;
            }
        }
        panic!("handshake did not finish");
    }

    #[test]
    fn a_mutual_handshake_lets_each_side_read_the_others_fingerprint() {
        // 32-hex ids, the shape KDE Connect uses (a UUID without dashes).
        let ours = DeviceCert::generate("aaaabbbbccccddddeeeeffff00001111");
        let theirs = DeviceCert::generate("11110000ffffeeeeddddccccbbbbaaaa");
        let ours_fp = ours.fingerprint().unwrap();
        let theirs_fp = theirs.fingerprint().unwrap();

        // We dialed: we are the server; the peer is the client.
        let server_cfg = TlsConfigs::build(&ours).unwrap().server_config();
        let client_cfg = TlsConfigs::build(&theirs).unwrap().client_config();

        let mut server = ServerConnection::new(server_cfg).unwrap();
        let name = ServerName::try_from("celestina").unwrap();
        let mut client = ClientConnection::new(client_cfg, name).unwrap();

        pump(&mut client, &mut server);
        assert!(!client.is_handshaking() && !server.is_handshaking());

        // Each side, after the handshake, holds the other's certificate — the
        // basis of the pin. Mutual auth means the server sees the client's too.
        assert_eq!(
            peer_leaf_fingerprint(server.peer_certificates()),
            Some(theirs_fp)
        );
        assert_eq!(
            peer_leaf_fingerprint(client.peer_certificates()),
            Some(ours_fp)
        );
    }

    #[test]
    fn both_configs_build_from_one_certificate() {
        let cert = DeviceCert::generate("00000000000000000000000000000001");
        let cfgs = TlsConfigs::build(&cert).unwrap();
        // Cheap smoke: both are usable to open a connection.
        assert!(ServerConnection::new(cfgs.server_config()).is_ok());
        let name = ServerName::try_from("celestina").unwrap();
        assert!(ClientConnection::new(cfgs.client_config(), name).is_ok());
    }
}
