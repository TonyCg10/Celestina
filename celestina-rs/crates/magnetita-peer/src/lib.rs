#![forbid(unsafe_code)]

//! A phone with no screen: the own protocol's headless peer.
//!
//! It links the same `magnetita-proto` and `magnetita-link` the Android
//! application will, holds its own certificate and trust file under a
//! directory of its own, and does from a shell what the phone does from its
//! UI: scan a QR (paste its text), pair, connect to a pinned desktop, report
//! a battery, answer a ring. The daemon's tests and the author's LAN checks
//! use it so no phone has to be in hand.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use magnetita_link::discovery::{parse_peers, Peer as Advertised, SERVICE_TYPE};
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

/// One headless phone: its identity, its pins and its endpoint.
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub fingerprint: Fingerprint,
    trust: TrustStore,
    endpoint: Endpoint,
}

impl Peer {
    /// Loads or creates the identity under `dir` and binds an endpoint on
    /// an ephemeral port. The device id is derived from the certificate, so
    /// it is as stable as the pin and needs no second file.
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
            capabilities: vec![
                CapabilityVersion {
                    capability: capability::BATTERY,
                    version: 1,
                },
                CapabilityVersion {
                    capability: capability::FIND,
                    version: 1,
                },
            ],
        };
        let endpoint = Endpoint::bind(
            EndpointConfig { cert, hello },
            "0.0.0.0:0".parse().expect("literal"),
        )?;
        Ok(Self {
            device_id,
            name: name.into(),
            fingerprint,
            trust,
            endpoint,
        })
    }

    /// The desktops this peer has pinned.
    pub fn pinned(&self) -> Vec<TrustedPeer> {
        self.trust.peers().collect()
    }

    /// Scans the QR text, dials the desktop it names with its certificate
    /// pinned from the QR, proves the secret, and pins the desktop.
    pub async fn pair(&mut self, uri: &str) -> Result<(Pinned, Hello, Session), LinkError> {
        let payload = QrPayload::parse_uri(uri)?;
        let mut last = LinkError::Connection("the QR names no address".into());
        for address in &payload.addresses {
            let Ok(target) = address.parse::<SocketAddr>() else {
                continue;
            };
            // A QR may list an address this network cannot reach; give each a
            // short try rather than the whole handshake budget.
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
                    self.trust.pin(TrustedPeer {
                        device_id: hello.device_id.clone(),
                        device_name: hello.device_name.clone(),
                        fingerprint: fingerprint_text(&pinned.peer_fingerprint),
                    })?;
                    return Ok((pinned, hello, session));
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Dials a pinned desktop.
    pub async fn connect(&self, address: SocketAddr) -> Result<(Session, Hello), LinkError> {
        self.endpoint
            .connect(address, Expect::Trusted(&self.trust))
            .await
    }

    /// Reports a battery level.
    pub async fn report_battery(
        session: &Session,
        level: u8,
        charging: bool,
    ) -> Result<(), LinkError> {
        session
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

    /// Waits for the next envelope, up to `timeout`.
    pub async fn next(session: &Session, timeout: Duration) -> Result<Option<Envelope>, LinkError> {
        match tokio::time::timeout(timeout, session.recv()).await {
            Ok(r) => r.map(Some),
            Err(_) => Ok(None),
        }
    }

    /// The Magnetita desktops Avahi sees right now.
    pub fn browse() -> Vec<Advertised> {
        let output = Command::new("avahi-browse")
            .args(["-rpt", SERVICE_TYPE])
            .output();
        match output {
            Ok(o) => parse_peers(&String::from_utf8_lossy(&o.stdout)),
            Err(_) => Vec::new(),
        }
    }

    pub fn dir_default() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("magnetita-peer")
    }
}

/// One line per envelope, for a shell to read.
pub fn describe(env: &Envelope) -> String {
    match (env.capability, env.kind) {
        (capability::FIND, 1) => "find: ring".into(),
        (capability::FIND, 2) => "find: stop".into(),
        (capability::BATTERY, 2) => "battery: requested".into(),
        (cap, kind) => format!("capability {cap} kind {kind}, {} bytes", env.body.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_proto::daily::find::FindRing;

    fn desktop(dir: &Path) -> (Endpoint, Fingerprint, DeviceCert) {
        let cert = DeviceCert::ensure(dir, "desktop").unwrap();
        let fp = fingerprint_of(&cert.chain().unwrap()[0]);
        let hello = Hello {
            device_id: "desktop".into(),
            device_name: "Celestina".into(),
            device_kind: DeviceKind::Desktop,
            capabilities: vec![],
        };
        (
            Endpoint::bind(
                EndpointConfig {
                    cert: cert.clone(),
                    hello,
                },
                "127.0.0.1:0".parse().unwrap(),
            )
            .unwrap(),
            fp,
            cert,
        )
    }

    #[tokio::test]
    async fn the_peer_pairs_persists_the_pin_and_reports_a_battery() {
        let tmp = std::env::temp_dir().join(format!("magnetita-peer-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (desktop, desktop_fp, _cert) = desktop(&tmp.join("desktop"));
        let addr = desktop.local_addr().unwrap();
        let secret = [0x42u8; 32];
        let uri = QrPayload {
            device_id: "desktop".into(),
            fingerprint: desktop_fp,
            secret,
            addresses: vec!["127.0.0.1:1".into(), addr.to_string()],
        }
        .to_uri();

        let mut peer = Peer::open(&tmp.join("phone"), "headless").unwrap();
        let peer_fp = peer.fingerprint;
        let desktop_side = async {
            let incoming = desktop.accept().await.unwrap().unwrap();
            let fp = incoming.peer_fingerprint();
            let (session, hello) = desktop
                .admit(incoming, Expect::Fingerprint(fp))
                .await
                .unwrap();
            assert_eq!(hello.device_name, "headless");
            let mut pairing = QrPairing::desktop(secret, desktop_fp, fp);
            let proof = session.recv().await.unwrap();
            let (reply, pinned) = pairing.accept_proof(&proof.body).unwrap();
            session
                .send_message(capability::PAIRING, pair_kind::QR_REPLY, reply)
                .await
                .unwrap();
            let battery = session.recv().await.unwrap();
            assert_eq!(BatteryStatus::decode(&battery.body).unwrap().level, 33);
            session
                .send_message(capability::FIND, FindRing::KIND, FindRing.encode())
                .await
                .unwrap();
            // Hold the session until the phone has read the ring.
            tokio::time::sleep(Duration::from_millis(200)).await;
            pinned
        };
        let phone_side = async {
            let (pinned, hello, session) = peer.pair(&uri).await.unwrap();
            assert_eq!(hello.device_id, "desktop");
            Peer::report_battery(&session, 33, false).await.unwrap();
            let ring = Peer::next(&session, Duration::from_secs(5))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(describe(&ring), "find: ring");
            pinned
        };
        let (by_desktop, by_phone) = tokio::join!(desktop_side, phone_side);
        assert_eq!(by_desktop.peer_fingerprint, peer_fp);
        assert_eq!(by_phone.peer_fingerprint, desktop_fp);

        // The pin survived to disk under the peer's own directory.
        let reopened = Peer::open(&tmp.join("phone"), "headless").unwrap();
        assert_eq!(
            reopened.device_id, peer.device_id,
            "the id is derived from the certificate"
        );
        assert_eq!(reopened.pinned().len(), 1);
        assert_eq!(reopened.pinned()[0].device_id, "desktop");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
