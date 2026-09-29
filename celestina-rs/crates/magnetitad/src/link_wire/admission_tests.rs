//! Loopback tests for who may become a session and under which id: the
//! session id bound to the pinned certificate (MAG-1), handshakes kept out
//! of the accept loop (MAG-2), a QR window that only a verified proof
//! closes (MAG-3), and a wire that sweeps partials only once it holds its
//! port (MAG-16).

use super::tests::{phone, prove, test_adapters, test_daemon};
use super::*;
use magnetita_link::endpoint::Expect as PeerExpect;
use magnetita_proto::pair::QrPayload;

/// A phone endpoint whose hello names `claimed` instead of the id its
/// certificate gives it.
fn phone_claiming(name: &str, claimed: &str) -> (Endpoint, Fingerprint) {
    phone_with(name, Some(claimed), "127.0.0.1:0")
}

/// A phone endpoint bound at `bind`, naming `claimed` or, by default, the id
/// its certificate gives it.
fn phone_with(name: &str, claimed: Option<&str>, bind: &str) -> (Endpoint, Fingerprint) {
    let cert = DeviceCert::generate(name);
    let fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let hello = Hello {
        device_id: claimed.map_or_else(|| magnetita_link::device_id_of(&fp), str::to_owned),
        device_name: name.into(),
        device_kind: DeviceKind::Phone,
        capabilities: vec![CapabilityVersion {
            capability: capability::BATTERY,
            version: 1,
        }],
    };
    (
        Endpoint::bind(EndpointConfig { cert, hello }, bind.parse().unwrap()).unwrap(),
        fp,
    )
}

/// Dials from a fresh endpoint at `bind` and sends a QR proof made with the
/// wrong secret; returns once the desktop has closed the connection, or has
/// refused it outright.
fn send_wrong_proof(rt: &tokio::runtime::Runtime, bind: &str, uri: &str, desktop: SocketAddr) {
    let (guesser, guesser_fp) = rt.block_on(async { phone_with("guesser", None, bind) });
    let scanned = QrPayload::parse_uri(uri).unwrap();
    let wrong = QrPayload {
        secret: [0x11; 32],
        ..scanned.clone()
    };
    let Ok((session, _)) =
        rt.block_on(guesser.connect(desktop, PeerExpect::Fingerprint(scanned.fingerprint)))
    else {
        return;
    };
    let pairing = QrPairing::phone(&wrong, guesser_fp, session.peer_fingerprint()).unwrap();
    let _ = rt.block_on(session.send_message(
        capability::PAIRING,
        pair_kind::QR_PROOF,
        pairing.proof_to_send().unwrap(),
    ));
    assert!(closed_within(rt, &session, Duration::from_secs(5)));
}

fn pin(daemon: &Daemon, device_id: &str, fp: &Fingerprint) {
    daemon
        .trust
        .lock_ok()
        .pin(TrustedPeer {
            device_id: device_id.into(),
            device_name: device_id.into(),
            fingerprint: fingerprint_text(fp),
        })
        .unwrap();
}

/// Whether the desktop closes `session` within `limit`, reading past any
/// greeting it sends first.
fn closed_within(rt: &tokio::runtime::Runtime, session: &Session, limit: Duration) -> bool {
    rt.block_on(async {
        let until = tokio::time::Instant::now() + limit;
        loop {
            match tokio::time::timeout_at(until, session.recv()).await {
                Ok(Ok(_greeting)) => continue,
                Ok(Err(_)) => break true,
                Err(_) => break false,
            }
        }
    })
}

/// A UDP relay that hands the desktop exactly one datagram, the dialler's
/// QUIC Initial, and drops everything after it both ways: the handshake it
/// starts can never finish. Returns the address to dial.
fn stalling_relay(desktop: SocketAddr) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let relay = socket.local_addr().unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    thread::spawn(move || {
        let mut buf = vec![0u8; 65_536];
        let Ok((n, _)) = socket.recv_from(&mut buf) else {
            return;
        };
        let _ = socket.send_to(&buf[..n], desktop);
        while socket.recv_from(&mut buf).is_ok() {}
    });
    relay
}

#[test]
fn a_hello_naming_another_pinned_device_is_refused() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        PairingArm::default(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        test_adapters(),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    // The author's phone is pinned; so is a second device, which claims the
    // phone's id in its hello.
    let victim = "00112233aabbccdd";
    pin(&daemon, victim, &[0x44; 32]);
    let (impostor, impostor_fp) = rt.block_on(async { phone_claiming("impostor", victim) });
    let impostor_id = magnetita_link::device_id_of(&impostor_fp);
    pin(&daemon, &impostor_id, &impostor_fp);

    // The desktop may close before the dial has read its hello, or just
    // after; either way no session runs.
    let refused = match rt.block_on(impostor.connect(addr, PeerExpect::Fingerprint(desktop_fp))) {
        Err(_) => true,
        Ok((session, _)) => closed_within(&rt, &session, Duration::from_secs(5)),
    };
    assert!(
        refused,
        "a hello that names another device's id closes the session"
    );
    let devices = daemon.devices.lock_ok();
    assert!(!devices.contains_key(victim), "the claimed id is not taken");
    assert!(!devices.contains_key(&impostor_id));
    drop(devices);
    drop(wire);
}

#[test]
fn a_pairing_phone_must_name_the_id_its_certificate_gives() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        test_adapters(),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let (liar, liar_fp) = rt.block_on(async { phone_claiming("liar", "chosen-id") });
    let scanned = QrPayload::parse_uri(&uri).unwrap();
    let outcome = rt.block_on(async {
        let (session, _) = liar
            .connect(addr, PeerExpect::Fingerprint(desktop_fp))
            .await?;
        let pairing = QrPairing::phone(&scanned, liar_fp, session.peer_fingerprint())?;
        session
            .send_message(
                capability::PAIRING,
                pair_kind::QR_PROOF,
                pairing.proof_to_send()?,
            )
            .await?;
        loop {
            let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                .await
                .map_err(|_| LinkError::HandshakeTimeout)??;
            if env.capability == capability::PAIRING {
                break Ok::<_, LinkError>(env);
            }
        }
    });
    assert!(outcome.is_err(), "the desktop never answers the proof");
    let trust = daemon.trust.lock_ok();
    assert!(!trust.is_trusted("chosen-id"));
    assert!(!trust.is_trusted(&magnetita_link::device_id_of(&liar_fp)));
    drop(trust);
    assert!(arm.is_armed(), "a refused phone does not burn the QR");
    drop(wire);
}

#[test]
fn an_unpinned_connection_does_not_burn_the_qr() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        test_adapters(),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);

    // A scanner on the LAN connects while the QR is shown and leaves; a
    // second one sends a proof made with the wrong secret.
    let (scanner, _) = rt.block_on(async { phone("scanner") });
    let (session, _) = rt
        .block_on(scanner.connect(addr, PeerExpect::Fingerprint(desktop_fp)))
        .unwrap();
    session.close("just looking");
    let (guesser, guesser_fp) = rt.block_on(async { phone("guesser") });
    let wrong = QrPayload {
        secret: [0x11; 32],
        ..QrPayload::parse_uri(&uri).unwrap()
    };
    let (session, _) = rt
        .block_on(guesser.connect(addr, PeerExpect::Fingerprint(desktop_fp)))
        .unwrap();
    let pairing = QrPairing::phone(&wrong, guesser_fp, session.peer_fingerprint()).unwrap();
    rt.block_on(session.send_message(
        capability::PAIRING,
        pair_kind::QR_PROOF,
        pairing.proof_to_send().unwrap(),
    ))
    .unwrap();
    assert!(closed_within(&rt, &session, Duration::from_secs(5)));
    assert!(arm.is_armed(), "neither connection closed the window");

    // The author's phone then scans the same QR and pairs.
    let (phone, phone_fp) = rt.block_on(async { phone("author") });
    let (paired, fp) = rt.block_on(prove(&phone, phone_fp, &uri));
    assert_eq!(fp, desktop_fp);
    assert!(daemon
        .trust
        .lock_ok()
        .is_trusted(&magnetita_link::device_id_of(&phone_fp)));
    assert!(!arm.is_armed(), "the verified proof closed the window");
    paired.close("done");
    drop(wire);
}

#[test]
fn a_stalled_handshake_does_not_hold_the_door() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        PairingArm::default(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        test_adapters(),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();

    // A host sends its Initial and never answers again.
    let relay = stalling_relay(addr);
    let (staller, _) = rt.block_on(async { phone("staller") });
    let stalled = rt.spawn(async move {
        let _ = staller
            .connect(relay, PeerExpect::Fingerprint(desktop_fp))
            .await;
    });
    thread::sleep(Duration::from_millis(300));

    // The author's phone, pinned, dials right after and is let in at once,
    // well inside the ten seconds the stalled handshake may take.
    let (phone, phone_fp) = rt.block_on(async { phone("author") });
    let id = magnetita_link::device_id_of(&phone_fp);
    pin(&daemon, &id, &phone_fp);
    let started = Instant::now();
    let session = rt.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(3),
            phone.connect(addr, PeerExpect::Fingerprint(desktop_fp)),
        )
        .await
    });
    let session = match session {
        Ok(Ok((session, _))) => session,
        other => panic!(
            "the second peer waited behind the stalled handshake: {:?}",
            other.map(|r| r.map(|_| ()))
        ),
    };
    while !daemon.devices.lock_ok().contains_key(&id) {
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "the second peer was not published promptly"
        );
        thread::sleep(Duration::from_millis(20));
    }
    session.close("done");
    stalled.abort();
    drop(wire);
}

#[test]
fn paired_sessions_do_not_hold_the_pairing_attempts() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        test_adapters(),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    // More phones than there are pairing attempts pair one after the other
    // and all stay connected.
    let mut kept = Vec::new();
    for n in 0..=admission::MAX_PAIRING_ATTEMPTS {
        let (phone, phone_fp) = rt.block_on(async { phone(&format!("phone{n}")) });
        let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
        let (session, fp) = rt.block_on(prove(&phone, phone_fp, &uri));
        assert_eq!(fp, desktop_fp, "phone {n} paired");
        kept.push((phone, session));
    }
    for (_, session) in &kept {
        session.close("done");
    }
    drop(wire);
}

#[test]
fn wrong_proofs_refuse_their_address_and_leave_the_qr_open() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        test_adapters(),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);

    // A host on the LAN keeps guessing; after a few tries it is not even let
    // in, and the QR stays open for everyone else.
    for _ in 0..8 {
        send_wrong_proof(&rt, "127.0.0.1:0", &uri, addr);
    }
    assert!(arm.is_armed(), "wrong proofs never close the window");

    let (phone, phone_fp) = rt.block_on(async { phone_with("author", None, "127.0.0.2:0") });
    let (session, fp) = rt.block_on(prove(&phone, phone_fp, &uri));
    assert_eq!(fp, desktop_fp);
    assert!(daemon
        .trust
        .lock_ok()
        .is_trusted(&magnetita_link::device_id_of(&phone_fp)));
    session.close("done");
    drop(wire);
}

#[test]
fn only_a_wire_that_bound_its_port_sweeps_the_partials() {
    let downloads =
        std::env::temp_dir().join(format!("magnetita-sweep-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&downloads);
    std::fs::create_dir_all(&downloads).unwrap();
    // A partial of the daemon already running (another process).
    let live = downloads.join(".magnetita-receive-4000000000-1.part");
    std::fs::write(&live, b"in flight").unwrap();
    let cert = DeviceCert::generate("desktop");
    let adapters = || Adapters {
        download_dir: downloads.clone(),
        ..test_adapters()
    };

    // A second daemon cannot bind the port the first one holds.
    let taken = UdpSocket::bind("127.0.0.1:0").unwrap();
    let refused = open_wire(
        test_daemon(&cert),
        cert.clone(),
        "desktop".into(),
        PairingArm::default(),
        taken.local_addr().unwrap(),
        false,
        adapters(),
    );
    assert!(refused.is_err());
    assert!(live.exists(), "a daemon that failed to bind swept nothing");

    // The daemon that binds sweeps what no running process will finish.
    let (wire, _) = open_wire(
        test_daemon(&cert),
        cert.clone(),
        "desktop".into(),
        PairingArm::default(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        adapters(),
    )
    .unwrap();
    assert!(!live.exists());
    drop(wire);
    let _ = std::fs::remove_dir_all(&downloads);
}
