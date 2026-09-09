//! A phone's identity, pins and sessions, as the peer and the app use them.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use magnetita_link::endpoint::Expect;
use magnetita_link::trust::fingerprint_text;
use magnetita_link::{
    fingerprint_of, DeviceCert, Endpoint, EndpointConfig, LinkError, Session, TrustStore,
    TrustedPeer,
};
use magnetita_proto::daily::battery::BatteryStatus;
use magnetita_proto::pair::{kind as pair_kind, Fingerprint, Pinned, QrPairing, QrPayload};
use magnetita_proto::{capability, CapabilityVersion, DeviceKind, Envelope, Hello};

/// How long one of the QR's addresses gets before the next is tried.
const ADDRESS_ATTEMPT: Duration = Duration::from_secs(3);

/// What this build of the phone offers.
fn capabilities() -> Vec<CapabilityVersion> {
    vec![
        CapabilityVersion {
            capability: capability::BATTERY,
            version: 1,
        },
        CapabilityVersion {
            capability: capability::FIND,
            version: 1,
        },
    ]
}

/// One phone: its identity, its pins and its endpoint. The device id is
/// derived from the certificate, so identity and pin are one fact and no
/// second file can drift.
pub struct Phone {
    pub device_id: String,
    pub name: String,
    pub fingerprint: Fingerprint,
    trust: Mutex<TrustStore>,
    endpoint: Endpoint,
}

/// A session with a desktop, and what came out of pairing for it.
pub struct PhoneSession {
    pub session: Session,
    pub desktop: Hello,
}

impl Phone {
    /// Loads or creates the identity under `dir` and binds an endpoint on
    /// an ephemeral port. Must run inside a tokio runtime.
    pub fn open(dir: &Path, name: &str) -> Result<Self, LinkError> {
        std::fs::create_dir_all(dir)?;
        let cert = DeviceCert::ensure(dir, name)?;
        let fingerprint = fingerprint_of(&cert.chain()?[0]);
        let device_id = fingerprint[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let trust = TrustStore::load(&dir.join("trust.json"))?;
        let hello = Hello {
            device_id: device_id.clone(),
            device_name: name.into(),
            device_kind: DeviceKind::Phone,
            capabilities: capabilities(),
        };
        let endpoint = Endpoint::bind(
            EndpointConfig { cert, hello },
            "0.0.0.0:0".parse().expect("literal"),
        )?;
        Ok(Self {
            device_id,
            name: name.into(),
            fingerprint,
            trust: Mutex::new(trust),
            endpoint,
        })
    }

    /// The desktops this phone has pinned.
    pub fn pinned(&self) -> Vec<TrustedPeer> {
        self.trust
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .peers()
            .collect()
    }

    /// Forgets a pinned desktop.
    pub fn forget(&self, device_id: &str) -> Result<(), LinkError> {
        Ok(self
            .trust
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .forget(device_id)?)
    }

    /// Scans the QR text, dials the desktop it names with its certificate
    /// pinned from the QR, proves the secret, pins the desktop, and keeps
    /// the session open.
    pub async fn pair(&self, uri: &str) -> Result<(Pinned, PhoneSession), LinkError> {
        let payload = QrPayload::parse_uri(uri)?;
        let mut last = LinkError::Connection("the QR names no address".into());
        for address in &payload.addresses {
            let Ok(target) = address.parse::<SocketAddr>() else {
                continue;
            };
            let attempt = tokio::time::timeout(
                ADDRESS_ATTEMPT,
                self.endpoint
                    .connect(target, Expect::Fingerprint(payload.fingerprint)),
            )
            .await
            .unwrap_or(Err(LinkError::HandshakeTimeout));
            match attempt {
                Ok((session, hello)) => {
                    let mut pairing =
                        QrPairing::phone(&payload, self.fingerprint, session.peer_fingerprint())?;
                    session
                        .send_message(
                            capability::PAIRING,
                            pair_kind::QR_PROOF,
                            pairing.proof_to_send()?,
                        )
                        .await?;
                    let reply =
                        tokio::time::timeout(magnetita_link::HANDSHAKE_BUDGET, session.recv())
                            .await
                            .map_err(|_| LinkError::HandshakeTimeout)??;
                    if reply.capability != capability::PAIRING || reply.kind != pair_kind::QR_REPLY
                    {
                        return Err(LinkError::Protocol(
                            magnetita_proto::DecodeError::Malformed("expected the pairing reply"),
                        ));
                    }
                    let pinned = pairing.accept_reply(&reply.body)?;
                    self.trust
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .pin(TrustedPeer {
                            device_id: hello.device_id.clone(),
                            device_name: hello.device_name.clone(),
                            fingerprint: fingerprint_text(&pinned.peer_fingerprint),
                        })?;
                    return Ok((
                        pinned,
                        PhoneSession {
                            session,
                            desktop: hello,
                        },
                    ));
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Dials a pinned desktop.
    pub async fn connect(&self, address: SocketAddr) -> Result<PhoneSession, LinkError> {
        let snapshot = {
            let trust = self.trust.lock().unwrap_or_else(|e| e.into_inner());
            let mut s = TrustStore::in_memory();
            for p in trust.peers() {
                let _ = s.pin(p);
            }
            s
        };
        let (session, desktop) = self
            .endpoint
            .connect(address, Expect::Trusted(&snapshot))
            .await?;
        Ok(PhoneSession { session, desktop })
    }
}

impl PhoneSession {
    /// Reports a battery level.
    pub async fn report_battery(&self, level: u8, charging: bool) -> Result<(), LinkError> {
        self.session
            .send_message(
                capability::BATTERY,
                BatteryStatus::KIND,
                BatteryStatus {
                    level,
                    charging,
                    low: level <= 15,
                }
                .encode(),
            )
            .await?;
        Ok(())
    }

    /// Waits for the next envelope, up to `timeout`; `None` on timeout.
    pub async fn next(&self, timeout: Duration) -> Result<Option<Envelope>, LinkError> {
        match tokio::time::timeout(timeout, self.session.recv()).await {
            Ok(r) => r.map(Some),
            Err(_) => Ok(None),
        }
    }

    pub fn close(&self, reason: &str) {
        self.session.close(reason);
    }
}

/// One line per envelope, for a shell or a log.
pub fn describe(env: &Envelope) -> String {
    match (env.capability, env.kind) {
        (capability::FIND, 1) => "find: ring".into(),
        (capability::FIND, 2) => "find: stop".into(),
        (capability::BATTERY, 2) => "battery: requested".into(),
        (cap, kind) => format!("capability {cap} kind {kind}, {} bytes", env.body.len()),
    }
}
