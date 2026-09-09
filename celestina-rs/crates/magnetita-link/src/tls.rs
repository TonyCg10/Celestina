//! The TLS configurations: mutual certificates, verified after the handshake
//! by pin, never by a chain.
//!
//! rustls wants a verifier at handshake time, so both verifiers here accept
//! any certificate and let [`Endpoint`](crate::Endpoint) decide with the
//! trust store once the peer's certificate is known. The signature checks
//! stay real: a peer must prove it holds the key of the certificate it
//! presents, or the fingerprint pin would be worth nothing.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};

use crate::error::LinkError;
use crate::DeviceCert;

/// The name the client puts in SNI; pinning ignores it, TLS needs one.
pub const SERVER_NAME: &str = "magnetita";

#[derive(Debug)]
struct PinLater(Arc<CryptoProvider>);

impl ServerCertVerifier for PinLater {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

impl ClientCertVerifier for PinLater {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.0.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Both roles' QUIC crypto for one device certificate.
pub struct Configs {
    pub server: quinn::ServerConfig,
    pub client: quinn::ClientConfig,
}

fn transport() -> Arc<quinn::TransportConfig> {
    let mut tp = quinn::TransportConfig::default();
    // Measured in MAG-P0: the phone's uplink black-holed larger datagrams.
    tp.initial_mtu(1200).mtu_discovery_config(None);
    // A static mirror or an idle phone must not look like a dead peer.
    tp.keep_alive_interval(Some(std::time::Duration::from_secs(1)));
    tp.max_idle_timeout(Some(
        std::time::Duration::from_secs(30)
            .try_into()
            .expect("30 s fits"),
    ));
    Arc::new(tp)
}

impl Configs {
    pub fn build(cert: &DeviceCert) -> Result<Self, LinkError> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let chain = cert.chain()?;
        let key = cert.private_key()?;
        let server_tls = rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| LinkError::Connection(e.to_string()))?
            .with_client_cert_verifier(Arc::new(PinLater(provider.clone())))
            .with_single_cert(chain.clone(), key.clone_key())
            .map_err(|e| LinkError::Connection(e.to_string()))?;
        let client_tls = rustls::ClientConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| LinkError::Connection(e.to_string()))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PinLater(provider)))
            .with_client_auth_cert(chain, key)
            .map_err(|e| LinkError::Connection(e.to_string()))?;
        let mut server = quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)
                .map_err(|e| LinkError::Connection(e.to_string()))?,
        ));
        server.transport_config(transport());
        let mut client = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_tls)
                .map_err(|e| LinkError::Connection(e.to_string()))?,
        ));
        client.transport_config(transport());
        Ok(Self { server, client })
    }
}
