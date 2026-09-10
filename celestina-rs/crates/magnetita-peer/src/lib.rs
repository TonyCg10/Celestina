#![forbid(unsafe_code)]

//! A phone with no screen: the own protocol's headless peer.
//!
//! Everything a phone is lives in `magnetita-mobile`; this crate adds the
//! two things a shell needs and an application does not — an Avahi browse
//! and a default directory — and the binary.

use std::path::PathBuf;
use std::process::Command;

use magnetita_link::discovery::{parse_peers, Peer as Advertised, SERVICE_TYPE};

pub use magnetita_mobile::phone::{clipboard_text, describe, share_fields, Incoming};
pub use magnetita_mobile::{Phone, PhoneSession};

/// The Magnetita desktops Avahi sees right now.
pub fn browse() -> Vec<Advertised> {
    match Command::new("avahi-browse")
        .args(["-rpt", SERVICE_TYPE])
        .output()
    {
        Ok(o) => parse_peers(&String::from_utf8_lossy(&o.stdout)),
        Err(_) => Vec::new(),
    }
}

/// Where the peer keeps its certificate and pins unless told otherwise.
pub fn dir_default() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("magnetita-peer")
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetita_link::endpoint::Expect;
    use magnetita_link::{fingerprint_of, DeviceCert, Endpoint, EndpointConfig};
    use magnetita_proto::daily::battery::BatteryStatus;
    use magnetita_proto::daily::find::FindRing;
    use magnetita_proto::pair::{kind as pair_kind, Fingerprint, QrPairing, QrPayload};
    use magnetita_proto::{capability, DeviceKind, Hello};
    use std::path::Path;
    use std::time::Duration;

    fn desktop(dir: &Path) -> (Endpoint, Fingerprint) {
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
                EndpointConfig { cert, hello },
                "127.0.0.1:0".parse().unwrap(),
            )
            .unwrap(),
            fp,
        )
    }

    #[tokio::test]
    async fn the_phone_pairs_persists_the_pin_and_reports_a_battery() {
        let tmp = std::env::temp_dir().join(format!("magnetita-peer-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (desktop, desktop_fp) = desktop(&tmp.join("desktop"));
        let addr = desktop.local_addr().unwrap();
        let secret = [0x42u8; 32];
        let uri = QrPayload {
            device_id: "desktop".into(),
            fingerprint: desktop_fp,
            secret,
            addresses: vec!["127.0.0.1:1".into(), addr.to_string()],
        }
        .to_uri();

        let phone = Phone::open(&tmp.join("phone"), "headless").unwrap();
        let phone_fp = phone.fingerprint;
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
            tokio::time::sleep(Duration::from_millis(200)).await;
            pinned
        };
        let phone_side = async {
            let (pinned, session) = phone.pair(&uri).await.unwrap();
            assert_eq!(session.desktop.device_id, "desktop");
            session.report_battery(33, false).await.unwrap();
            let ring = match session.next(Duration::from_secs(5)).await.unwrap().unwrap() {
                Incoming::Envelope(env) => env,
                other => panic!("expected an envelope, got {other:?}"),
            };
            assert_eq!(describe(&ring), "find: ring");
            pinned
        };
        let (by_desktop, by_phone) = tokio::join!(desktop_side, phone_side);
        assert_eq!(by_desktop.peer_fingerprint, phone_fp);
        assert_eq!(by_phone.peer_fingerprint, desktop_fp);

        let reopened = Phone::open(&tmp.join("phone"), "headless").unwrap();
        assert_eq!(
            reopened.device_id, phone.device_id,
            "the id is derived from the certificate"
        );
        assert_eq!(reopened.pinned().len(), 1);
        assert_eq!(reopened.pinned()[0].device_id, "desktop");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
