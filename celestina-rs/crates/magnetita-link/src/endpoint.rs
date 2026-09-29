//! The endpoint: one socket, both roles, and the only door into a session.
//!
//! Every path to a [`Session`] runs the same gate: the TLS handshake yields
//! the peer's certificate, its fingerprint is looked up in the trust store
//! (or compared with the one fingerprint a pairing expects), the hello is
//! exchanged, and only then does a session exist — all inside one absolute
//! deadline. An unpinned peer that is not pairing is closed before it can
//! send a single envelope.
//!
//! Accepting does not wait for any of that. [`Endpoint::accept`] hands back
//! a [`Pending`] attempt as soon as a peer's first packet arrives, and the
//! caller runs [`Pending::handshake`] in a task of its own, so one peer that
//! sends an Initial and falls silent holds its own deadline and nobody
//! else's. What bounds those tasks is a handshake slot: at most
//! [`MAX_HANDSHAKES`] attempts are in flight at once, at most
//! [`MAX_HANDSHAKES_PER_ADDRESS`] of them from one address, and an attempt
//! beyond either is refused on arrival. A slot lasts until the attempt is
//! admitted, refused or dropped. Once half the slots are taken, a source
//! that has not yet shown it can receive what is sent to its address is
//! answered with a QUIC Retry instead of a slot: a real peer answers it at
//! once and comes back validated, while a spoofed source never does, so
//! forged Initials cannot fill the rest.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
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

/// How many handshakes may be in flight at once, from every address.
pub const MAX_HANDSHAKES: usize = 16;
/// How many of them one address may hold. A phone that redials while its
/// last attempt is still ending needs two; a host that stalls on purpose
/// gets no more than this.
pub const MAX_HANDSHAKES_PER_ADDRESS: usize = 4;

/// Both roles on one UDP socket.
pub struct Endpoint {
    inner: quinn::Endpoint,
    hello: Hello,
    slots: Arc<Slots>,
}

/// The handshakes in flight: how many in all and from each address.
#[derive(Default)]
struct Slots {
    held: Mutex<SlotCount>,
}

#[derive(Default)]
struct SlotCount {
    total: usize,
    by_address: HashMap<IpAddr, usize>,
}

/// One handshake's place in [`Slots`], given back when it is dropped.
struct Slot {
    slots: Arc<Slots>,
    address: IpAddr,
}

impl Slots {
    /// Whether half the slots or more are taken.
    fn under_pressure(&self) -> bool {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .total
            >= MAX_HANDSHAKES / 2
    }

    /// A slot for one more handshake from `address`, or `None` when either
    /// bound is reached.
    fn claim(self: &Arc<Self>, address: IpAddr) -> Option<Slot> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let from_address = held.by_address.get(&address).copied().unwrap_or(0);
        if held.total >= MAX_HANDSHAKES || from_address >= MAX_HANDSHAKES_PER_ADDRESS {
            return None;
        }
        held.total += 1;
        held.by_address.insert(address, from_address + 1);
        Some(Slot {
            slots: Arc::clone(self),
            address,
        })
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut held = self
            .slots
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        held.total = held.total.saturating_sub(1);
        if let Some(count) = held.by_address.get_mut(&self.address) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                held.by_address.remove(&self.address);
            }
        }
    }
}

/// A peer's first packet, let in under a handshake slot and not yet through
/// TLS. Run [`Pending::handshake`] in a task of its own.
pub struct Pending {
    incoming: quinn::Incoming,
    started: Instant,
    slot: Slot,
}

/// A connection that has passed TLS and the pin check but not yet the hello:
/// the caller decides whether to admit it. It still holds its handshake slot.
pub struct Incoming {
    conn: quinn::Connection,
    peer_fingerprint: Fingerprint,
    started: Instant,
    _slot: Slot,
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
            slots: Arc::default(),
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

    /// Waits for the next peer to knock and returns at once with its
    /// attempt, before any TLS. An attempt beyond [`MAX_HANDSHAKES`] or
    /// [`MAX_HANDSHAKES_PER_ADDRESS`] is refused here and never returned, and
    /// under pressure an unvalidated one is sent a Retry. `None` when the
    /// endpoint is closed.
    pub async fn accept(&self) -> Option<Pending> {
        loop {
            let incoming = self.inner.accept().await?;
            if !incoming.remote_address_validated()
                && incoming.may_retry()
                && self.slots.under_pressure()
            {
                if let Err(refused) = incoming.retry() {
                    refused.into_incoming().refuse();
                }
                continue;
            }
            let started = Instant::now();
            match self.slots.claim(incoming.remote_address().ip()) {
                Some(slot) => {
                    return Some(Pending {
                        incoming,
                        started,
                        slot,
                    })
                }
                None => incoming.refuse(),
            }
        }
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
            _slot,
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

impl Pending {
    pub fn remote_address(&self) -> SocketAddr {
        self.incoming.remote_address()
    }

    /// Runs TLS within what remains of the handshake budget, which began
    /// when the attempt arrived.
    pub async fn handshake(self) -> Result<Incoming, LinkError> {
        let Self {
            incoming,
            started,
            slot,
        } = self;
        let connecting = incoming.accept()?;
        let conn = deadline(started, connecting).await??;
        let peer_fingerprint = peer_certificate(&conn)?;
        Ok(Incoming {
            conn,
            peer_fingerprint,
            started,
            _slot: slot,
        })
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
        endpoint_at(name, kind, "127.0.0.1:0")
    }

    fn endpoint_at(name: &str, kind: DeviceKind, bind: &str) -> (Endpoint, Fingerprint) {
        let cert = DeviceCert::generate(name);
        let fp = fingerprint_of(&cert.chain().unwrap()[0]);
        let ep = Endpoint::bind(
            EndpointConfig {
                cert,
                hello: hello(name, kind),
            },
            bind.parse().unwrap(),
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
            let incoming = desktop.accept().await.unwrap().handshake().await.unwrap();
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
            let incoming = desktop.accept().await.unwrap().handshake().await.unwrap();
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
            let incoming = desktop.accept().await.unwrap().handshake().await.unwrap();
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
                let i = desktop.accept().await.unwrap().handshake().await.unwrap();
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
                let i = desktop.accept().await.unwrap().handshake().await.unwrap();
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

    /// A UDP relay that hands `target` the dialler's first datagram (its
    /// QUIC Initial) and drops everything after it, so the handshake it
    /// starts never finishes. `bind` picks the address the endpoint sees.
    async fn stalling_relay(bind: &str, target: SocketAddr) -> SocketAddr {
        let socket = tokio::net::UdpSocket::bind(bind).await.unwrap();
        let relay = socket.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65_536];
            let Ok((n, _)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let _ = socket.send_to(&buf[..n], target).await;
            while socket.recv_from(&mut buf).await.is_ok() {}
        });
        relay
    }

    /// Starts a dial through a stalling relay bound on `bind`.
    async fn stall(bind: &str, target: SocketAddr, expect: Fingerprint) {
        let relay = stalling_relay(bind, target).await;
        let (staller, _) = endpoint("staller", DeviceKind::Phone);
        tokio::spawn(async move {
            let _ = staller.connect(relay, Expect::Fingerprint(expect)).await;
        });
    }

    #[tokio::test]
    async fn a_stalled_handshake_does_not_hold_the_next_accept() {
        let (desktop, desktop_fp) = endpoint("desktop", DeviceKind::Desktop);
        let (phone, phone_fp) = endpoint("phone", DeviceKind::Phone);
        let addr = desktop.local_addr().unwrap();
        stall("127.0.0.1:0", addr, desktop_fp).await;
        let stalled = desktop.accept().await.unwrap();
        let stalled = tokio::spawn(stalled.handshake());

        let desktop_trust = pinned(&phone_fp, "phone");
        let phone_trust = pinned(&desktop_fp, "desktop");
        let started = Instant::now();
        let accept = async {
            let incoming = desktop.accept().await.unwrap().handshake().await.unwrap();
            desktop
                .admit(incoming, Expect::Trusted(&desktop_trust))
                .await
                .unwrap()
        };
        let dial = async { phone.connect(addr, Expect::Trusted(&phone_trust)).await };
        let (_, dialled) = tokio::join!(accept, dial);
        assert!(dialled.is_ok());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the second peer was served while the first still stalls"
        );
        assert!(
            !stalled.is_finished(),
            "the stalled handshake is still pending"
        );
        stalled.abort();
    }

    #[tokio::test]
    async fn one_address_holds_a_bounded_number_of_handshakes() {
        let (desktop, desktop_fp) = endpoint("desktop", DeviceKind::Desktop);
        let addr = desktop.local_addr().unwrap();
        let mut held = Vec::new();
        for _ in 0..MAX_HANDSHAKES_PER_ADDRESS {
            stall("127.0.0.1:0", addr, desktop_fp).await;
            held.push(desktop.accept().await.unwrap());
        }
        // One more from the same address is refused on arrival: the next
        // accept skips it and returns a dialler from another address.
        let (crowded, _) = endpoint("crowded", DeviceKind::Phone);
        let crowded = tokio::spawn(async move {
            crowded
                .connect(addr, Expect::Fingerprint(desktop_fp))
                .await
                .map(|_| ())
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        stall("127.0.0.2:0", addr, desktop_fp).await;
        let other = desktop.accept().await.unwrap();
        assert_eq!(other.remote_address().ip(), IpAddr::from([127, 0, 0, 2]));
        let refused = tokio::time::timeout(Duration::from_secs(3), crowded)
            .await
            .expect("a refused dial ends at once")
            .unwrap();
        assert!(refused.is_err());

        // Dropping an attempt gives its slot back.
        drop(held.pop());
        stall("127.0.0.1:0", addr, desktop_fp).await;
        let again = desktop.accept().await.unwrap();
        assert_eq!(again.remote_address().ip(), IpAddr::from([127, 0, 0, 1]));
    }

    #[tokio::test]
    async fn under_pressure_an_unvalidated_address_must_prove_it_can_answer() {
        let (desktop, desktop_fp) = endpoint("desktop", DeviceKind::Desktop);
        let addr = desktop.local_addr().unwrap();
        // Half the slots are taken by hosts that never answer.
        let mut held = Vec::new();
        for n in 0..MAX_HANDSHAKES / 2 {
            let host = format!("127.0.0.{}:0", 1 + n / MAX_HANDSHAKES_PER_ADDRESS);
            stall(&host, addr, desktop_fp).await;
            held.push(desktop.accept().await.unwrap());
        }
        // A spoofed-looking source that cannot receive the Retry takes no
        // slot; a real peer answers the Retry and is let in.
        stall("127.0.0.20:0", addr, desktop_fp).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let (phone, _) = endpoint_at("phone", DeviceKind::Phone, "127.0.0.3:0");
        let dial = tokio::spawn(async move {
            let _ = phone.connect(addr, Expect::Fingerprint(desktop_fp)).await;
        });
        let next = tokio::time::timeout(Duration::from_secs(3), desktop.accept())
            .await
            .expect("the real peer is let in")
            .unwrap();
        assert_eq!(next.remote_address().ip(), IpAddr::from([127, 0, 0, 3]));
        assert!(next.incoming.remote_address_validated());
        assert_eq!(
            desktop.slots.held.lock().unwrap().total,
            MAX_HANDSHAKES / 2 + 1,
            "the silent source holds nothing"
        );
        dial.abort();
    }

    #[test]
    fn the_slots_bound_the_total_and_each_address() {
        let slots = Arc::new(Slots::default());
        let address = |n: u8| IpAddr::from([10, 0, 0, n]);
        let mut held: Vec<Slot> = (0..MAX_HANDSHAKES)
            .map(|n| slots.claim(address(n as u8)).unwrap())
            .collect();
        assert!(slots.claim(address(200)).is_none(), "the total is bounded");
        held.pop();
        let one = slots.claim(address(200)).unwrap();
        drop(held);
        let more: Vec<Slot> = (1..MAX_HANDSHAKES_PER_ADDRESS)
            .map(|_| slots.claim(address(200)).unwrap())
            .collect();
        assert!(
            slots.claim(address(200)).is_none(),
            "one address is bounded"
        );
        assert!(slots.claim(address(201)).is_some());
        drop((one, more));
        let held = slots.held.lock().unwrap();
        assert_eq!(held.total, 0);
        assert!(held.by_address.is_empty(), "released slots leave no entry");
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
