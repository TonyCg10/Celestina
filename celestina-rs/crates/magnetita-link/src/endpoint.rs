//! The endpoint: one socket, both roles, and the only door into a session.
//!
//! Every path to a [`Session`] runs the same gate: the TLS handshake yields
//! the peer's certificate, its fingerprint is looked up in the trust store
//! (or compared with the one fingerprint a pairing expects), the hello is
//! exchanged, and only then does a session exist — all inside one absolute
//! deadline. An unpinned peer that is not pairing is closed before it can
//! send a single envelope.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use magnetita_proto::pair::Fingerprint;
use magnetita_proto::Hello;
use rustls::pki_types::CertificateDer;

use crate::error::LinkError;
use crate::session::{Session, HANDSHAKE_BUDGET};
use crate::tls::{Configs, SERVER_NAME};
use crate::trust::{fingerprint_of, Trust};
use crate::{DeviceCert, TrustStore};

/// What an endpoint is.
pub struct EndpointConfig {
    pub cert: DeviceCert,
    pub hello: Hello,
}

/// Both roles on one UDP socket.
pub struct Endpoint {
    inner: quinn::Endpoint,
    hello: Hello,
}

/// A connection that has passed TLS and the pin check but not yet the hello:
/// the caller decides whether to admit it.
pub struct Incoming {
    conn: quinn::Connection,
    peer_fingerprint: Fingerprint,
    started: Instant,
}

/// Who a peer must be for a connection to proceed.
pub enum Expect<'a> {
    /// Any peer pinned in the store.
    Trusted(&'a TrustStore),
    /// Exactly this certificate, as a pairing in progress requires; the
    /// store is not consulted.
    Fingerprint(Fingerprint),
}

fn peer_certificate(conn: &quinn::Connection) -> Result<Fingerprint, LinkError> {
    let certs = conn
        .peer_identity()
        .and_then(|i| i.downcast::<Vec<CertificateDer<'static>>>().ok())
        .ok_or(LinkError::Untrusted)?;
    let first = certs.first().ok_or(LinkError::Untrusted)?;
    Ok(fingerprint_of(first))
}

fn check(expect: &Expect<'_>, fp: &Fingerprint) -> Result<(), LinkError> {
    let ok = match expect {
        Expect::Trusted(store) => Trust(store).peer_by_fingerprint(fp).is_some(),
        Expect::Fingerprint(want) => want == fp,
    };
    if ok {
        Ok(())
    } else {
        Err(LinkError::Untrusted)
    }
}

impl Endpoint {
    /// Binds `addr` for both roles.
    pub fn bind(config: EndpointConfig, addr: SocketAddr) -> Result<Self, LinkError> {
        let configs = Configs::build(&config.cert)?;
        let mut inner = quinn::Endpoint::server(configs.server, addr)?;
        inner.set_default_client_config(configs.client);
        Ok(Self {
            inner,
            hello: config.hello,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, LinkError> {
        Ok(self.inner.local_addr()?)
    }

    /// Moves to a new socket; live connections migrate to it.
    pub fn rebind(&self, socket: std::net::UdpSocket) -> Result<(), LinkError> {
        Ok(self.inner.rebind(socket)?)
    }

    /// Dials `addr`, runs the gate, exchanges hellos. Returns the session and
    /// the peer's hello.
    pub async fn connect(
        &self,
        addr: SocketAddr,
        expect: Expect<'_>,
    ) -> Result<(Session, Hello), LinkError> {
        let started = Instant::now();
        let connecting = self.inner.connect(addr, SERVER_NAME)?;
        let conn = deadline(started, connecting).await??;
        let fp = peer_certificate(&conn)?;
        if let Err(e) = check(&expect, &fp) {
            conn.close(1u32.into(), b"untrusted");
            return Err(e);
        }
        let (send, recv) = deadline(started, conn.open_bi()).await??;
        let session = Session::new(conn, send, recv, fp);
        let theirs = deadline(started, session.exchange_hello(&self.hello)).await??;
        Ok((session, theirs))
    }

    /// Waits for the next connection to pass TLS. `None` when the endpoint
    /// is closed.
    pub async fn accept(&self) -> Option<Result<Incoming, LinkError>> {
        let incoming = self.inner.accept().await?;
        let started = Instant::now();
        Some(
            async move {
                let connecting = incoming.accept()?;
                let conn = deadline(started, connecting).await??;
                let peer_fingerprint = peer_certificate(&conn)?;
                Ok(Incoming {
                    conn,
                    peer_fingerprint,
                    started,
                })
            }
            .await,
        )
    }

    /// Admits an accepted connection under `expect`, exchanges hellos.
    pub async fn admit(
        &self,
        incoming: Incoming,
        expect: Expect<'_>,
    ) -> Result<(Session, Hello), LinkError> {
        let Incoming {
            conn,
            peer_fingerprint,
            started,
        } = incoming;
        if let Err(e) = check(&expect, &peer_fingerprint) {
            conn.close(1u32.into(), b"untrusted");
            return Err(e);
        }
        let (send, recv) = deadline(started, conn.accept_bi()).await??;
        let session = Session::new(conn, send, recv, peer_fingerprint);
        let theirs = deadline(started, session.exchange_hello(&self.hello)).await??;
        Ok((session, theirs))
    }

    /// Stops accepting and closes every connection.
    pub fn close(&self) {
        self.inner.close(0u32.into(), b"endpoint closed");
    }

    pub async fn wait_idle(&self) {
        self.inner.wait_idle().await;
    }
}

impl Incoming {
    /// The certificate the peer proved it holds.
    pub fn peer_fingerprint(&self) -> Fingerprint {
        self.peer_fingerprint
    }

    pub fn remote_address(&self) -> SocketAddr {
        self.conn.remote_address()
    }

    /// Refuses without admitting.
    pub fn refuse(self) {
        self.conn.close(1u32.into(), b"refused");
    }
}

/// Runs `fut` inside what remains of the handshake budget that began at
/// `started`; every step of one handshake shares the same end.
async fn deadline<T>(
    started: Instant,
    fut: impl std::future::Future<Output = T>,
) -> Result<T, LinkError> {
    let remaining = HANDSHAKE_BUDGET
        .checked_sub(started.elapsed())
        .unwrap_or(Duration::ZERO);
    tokio::time::timeout(remaining, fut)
        .await
        .map_err(|_| LinkError::HandshakeTimeout)
}

#[allow(dead_code)]
fn _assert_send() {
    fn is_send<T: Send>() {}
    is_send::<Arc<Endpoint>>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::fingerprint_text;
    use crate::TrustedPeer;
    use magnetita_proto::pair::{QrPairing, QrPayload};
    use magnetita_proto::{capability, CapabilityVersion, DeviceKind};

    fn hello(name: &str, kind: DeviceKind) -> Hello {
        Hello {
            device_id: name.into(),
            device_name: name.into(),
            device_kind: kind,
            capabilities: vec![CapabilityVersion {
                capability: capability::BATTERY,
                version: 1,
            }],
        }
    }

    fn endpoint(name: &str, kind: DeviceKind) -> (Endpoint, Fingerprint) {
        let cert = DeviceCert::generate(name);
        let fp = fingerprint_of(&cert.chain().unwrap()[0]);
        let ep = Endpoint::bind(
            EndpointConfig {
                cert,
                hello: hello(name, kind),
            },
            "127.0.0.1:0".parse().unwrap(),
        )
        .unwrap();
        (ep, fp)
    }

    fn pinned(fp: &Fingerprint, id: &str) -> TrustStore {
        let mut store = TrustStore::in_memory();
        store
            .pin(TrustedPeer {
                device_id: id.into(),
                device_name: id.into(),
                fingerprint: fingerprint_text(fp),
            })
            .unwrap();
        store
    }

    #[tokio::test]
    async fn pinned_peers_connect_and_exchange_hellos() {
        let (desktop, desktop_fp) = endpoint("desktop", DeviceKind::Desktop);
        let (phone, phone_fp) = endpoint("phone", DeviceKind::Phone);
        let desktop_trust = pinned(&phone_fp, "phone");
        let phone_trust = pinned(&desktop_fp, "desktop");
        let addr = desktop.local_addr().unwrap();
        let accept = async {
            let incoming = desktop.accept().await.unwrap().unwrap();
            assert_eq!(incoming.peer_fingerprint(), phone_fp);
            desktop
                .admit(incoming, Expect::Trusted(&desktop_trust))
                .await
                .unwrap()
        };
        let dial = phone.connect(addr, Expect::Trusted(&phone_trust));
        let ((server_session, phone_hello), (client_session, desktop_hello)) =
            tokio::join!(accept, async { dial.await.unwrap() });
        assert_eq!(phone_hello.device_id, "phone");
        assert_eq!(desktop_hello.device_kind, DeviceKind::Desktop);
        assert_eq!(server_session.peer_fingerprint(), phone_fp);
        assert_eq!(client_session.peer_fingerprint(), desktop_fp);

        // An envelope each way on the control stream, and a datagram.
        client_session
            .send_message(capability::BATTERY, 1, vec![1, 2, 3])
            .await
            .unwrap();
        let got = server_session.recv().await.unwrap();
        assert_eq!(
            (got.capability, got.kind, got.body),
            (capability::BATTERY, 1, vec![1, 2, 3])
        );
        server_session
            .send_message(capability::FIND, 1, vec![])
            .await
            .unwrap();
        assert_eq!(
            client_session.recv().await.unwrap().capability,
            capability::FIND
        );
        client_session
            .send_datagram(capability::INPUT, 1, vec![9])
            .unwrap();
        assert_eq!(server_session.recv_datagram().await.unwrap().body, vec![9]);

        // A transfer stream named by its id.
        let mut s = client_session.open_transfer(42).await.unwrap();
        s.write_all(b"payload").await.unwrap();
        s.finish().unwrap();
        let (id, mut r) = server_session.accept_transfer().await.unwrap();
        assert_eq!(id, 42);
        assert_eq!(r.read_to_end(1024).await.unwrap(), b"payload");
    }

    #[tokio::test]
    async fn an_unpinned_peer_is_closed_before_any_envelope() {
        let (desktop, _) = endpoint("desktop", DeviceKind::Desktop);
        let (stranger, _) = endpoint("stranger", DeviceKind::Phone);
        let empty = TrustStore::in_memory();
        let addr = desktop.local_addr().unwrap();
        let accept = async {
            let incoming = desktop.accept().await.unwrap().unwrap();
            desktop.admit(incoming, Expect::Trusted(&empty)).await
        };
        let dial = stranger.connect(addr, Expect::Trusted(&empty));
        let (server, client) = tokio::join!(accept, dial);
        assert!(matches!(server, Err(LinkError::Untrusted)));
        // The client fails too: either its own pin check or the server's close.
        assert!(client.is_err());
    }

    #[tokio::test]
    async fn a_pairing_admits_exactly_the_expected_certificate_and_pins_it() {
        let (desktop, desktop_fp) = endpoint("desktop", DeviceKind::Desktop);
        let (phone, phone_fp) = endpoint("phone", DeviceKind::Phone);
        let addr = desktop.local_addr().unwrap();
        let payload = QrPayload {
            device_id: "desktop".into(),
            fingerprint: desktop_fp,
            secret: [0x5a; 32],
            addresses: vec![addr.to_string()],
        };
        let uri = payload.to_uri();

        let desktop_side = async {
            let incoming = desktop.accept().await.unwrap().unwrap();
            let peer = incoming.peer_fingerprint();
            // Pairing: any certificate may come in; the proof decides.
            let (session, _hello) = desktop
                .admit(incoming, Expect::Fingerprint(peer))
                .await
                .unwrap();
            let mut pairing = QrPairing::desktop(payload.secret, desktop_fp, peer);
            let proof = session.recv().await.unwrap();
            let (reply, pinned) = pairing.accept_proof(&proof.body).unwrap();
            session
                .send_message(
                    capability::PAIRING,
                    magnetita_proto::pair::kind::QR_REPLY,
                    reply,
                )
                .await
                .unwrap();
            // Keep the connection until the phone has read the reply; a
            // dropped session closes at once.
            (pinned, session)
        };
        let phone_side = async {
            let scanned = QrPayload::parse_uri(&uri).unwrap();
            let target: SocketAddr = scanned.addresses[0].parse().unwrap();
            let (session, _hello) = phone
                .connect(target, Expect::Fingerprint(scanned.fingerprint))
                .await
                .unwrap();
            let mut pairing =
                QrPairing::phone(&scanned, phone_fp, session.peer_fingerprint()).unwrap();
            session
                .send_message(
                    capability::PAIRING,
                    magnetita_proto::pair::kind::QR_PROOF,
                    pairing.proof_to_send().unwrap(),
                )
                .await
                .unwrap();
            let reply = session.recv().await.unwrap();
            pairing.accept_reply(&reply.body).unwrap()
        };
        let ((pinned_by_desktop, _desktop_session), pinned_by_phone) =
            tokio::join!(desktop_side, phone_side);
        assert_eq!(pinned_by_desktop.peer_fingerprint, phone_fp);
        assert_eq!(pinned_by_phone.peer_fingerprint, desktop_fp);

        // Once pinned, the ordinary trusted path admits them.
        let mut store = TrustStore::in_memory();
        store
            .pin(TrustedPeer {
                device_id: "phone".into(),
                device_name: "phone".into(),
                fingerprint: fingerprint_text(&pinned_by_desktop.peer_fingerprint),
            })
            .unwrap();
        assert!(Trust(&store).peer_by_fingerprint(&phone_fp).is_some());
    }

    #[tokio::test]
    async fn the_session_survives_the_client_moving_sockets() {
        let (desktop, desktop_fp) = endpoint("desktop", DeviceKind::Desktop);
        let (phone, phone_fp) = endpoint("phone", DeviceKind::Phone);
        let desktop_trust = pinned(&phone_fp, "phone");
        let phone_trust = pinned(&desktop_fp, "desktop");
        let addr = desktop.local_addr().unwrap();
        let (server, client) = tokio::join!(
            async {
                let i = desktop.accept().await.unwrap().unwrap();
                desktop
                    .admit(i, Expect::Trusted(&desktop_trust))
                    .await
                    .unwrap()
            },
            async {
                phone
                    .connect(addr, Expect::Trusted(&phone_trust))
                    .await
                    .unwrap()
            }
        );
        let before = server.0.remote_address();
        phone
            .rebind(std::net::UdpSocket::bind("127.0.0.1:0").unwrap())
            .unwrap();
        client
            .0
            .send_message(capability::BATTERY, 2, vec![])
            .await
            .unwrap();
        let got = server.0.recv().await.unwrap();
        assert_eq!(got.kind, 2);
        assert_ne!(
            server.0.remote_address(),
            before,
            "the server now sees the new socket"
        );
    }

    #[tokio::test]
    async fn a_frame_over_the_limit_is_refused_by_its_header() {
        let (desktop, desktop_fp) = endpoint("desktop", DeviceKind::Desktop);
        let (phone, phone_fp) = endpoint("phone", DeviceKind::Phone);
        let desktop_trust = pinned(&phone_fp, "phone");
        let phone_trust = pinned(&desktop_fp, "desktop");
        let addr = desktop.local_addr().unwrap();
        let (server, client) = tokio::join!(
            async {
                let i = desktop.accept().await.unwrap().unwrap();
                desktop
                    .admit(i, Expect::Trusted(&desktop_trust))
                    .await
                    .unwrap()
            },
            async {
                phone
                    .connect(addr, Expect::Trusted(&phone_trust))
                    .await
                    .unwrap()
            }
        );
        // Write a lying header straight onto the control stream.
        {
            let mut s = client.0.control_send.lock().await;
            s.write_all(&u32::MAX.to_be_bytes()).await.unwrap();
        }
        assert!(matches!(
            server.0.recv().await,
            Err(LinkError::FrameTooLarge(u32::MAX))
        ));
    }

    #[test]
    fn the_handshake_budget_is_absolute() {
        let started = Instant::now() - HANDSHAKE_BUDGET;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let r = rt.block_on(deadline(started, async {
            tokio::time::sleep(Duration::from_millis(50)).await
        }));
        assert!(matches!(r, Err(LinkError::HandshakeTimeout)));
    }
}
