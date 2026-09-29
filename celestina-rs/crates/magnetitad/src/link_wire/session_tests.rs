//! Loopback tests for a session's liveness: a phone that stops reading its
//! control stream is closed at the send deadline instead of wedging the
//! session (MAG-5), and what a phone holds down is released when its
//! session ends (MAG-6).

use super::tests::{phone, prove, test_adapters, test_daemon};
use super::*;
use magnetita_link::endpoint::Expect;
use magnetita_proto::control::input::{Button, Key, PointerButton};

/// Waits up to `limit` for `done`, polling.
fn within(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    done()
}

#[test]
fn a_phone_that_stops_reading_is_closed_at_the_send_deadline() {
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
    let (phone, phone_fp) = rt.block_on(async { phone("stalled") });
    let id = magnetita_link::device_id_of(&phone_fp);
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    // The phone pairs and then never reads its control stream again.
    let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
    assert!(within(Duration::from_secs(5), || daemon
        .devices
        .lock_ok()
        .contains_key(&id)));

    // Far more than the stream's flow-control window: the desktop's write
    // blocks once the phone's receive buffer is full.
    let sender = daemon.commands.lock_ok().get(&id).cloned().unwrap();
    for _ in 0..24 {
        sender
            .try_send(Command::NotificationReply {
                key: "stalled".into(),
                text: "x".repeat(100 * 1024),
            })
            .unwrap();
    }
    assert!(
        within(writer::SEND_DEADLINE + Duration::from_secs(6), || !daemon
            .devices
            .lock_ok()
            .contains_key(&id)),
        "a session whose phone stopped reading must end at the send deadline"
    );
    drop(session);
    drop(wire);
}

#[test]
fn what_a_phone_holds_is_released_when_its_session_ends() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let recorder = Arc::new(input::testing::Recorder::default());
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        Adapters {
            input: recorder.clone(),
            ..test_adapters()
        },
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (phone, phone_fp) = rt.block_on(async { phone("dragging") });
    let id = magnetita_link::device_id_of(&phone_fp);
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
    rt.block_on(async {
        session
            .send_message(
                capability::INPUT,
                Key::KIND,
                Key {
                    code: 29,
                    pressed: true,
                }
                .encode(),
            )
            .await
            .unwrap();
        session
            .send_message(
                capability::INPUT,
                PointerButton::KIND,
                PointerButton {
                    button: Button::Left,
                    pressed: true,
                }
                .encode(),
            )
            .await
            .unwrap();
    });
    let seen = |line: &str| recorder.0.lock_ok().iter().any(|l| l == line);
    assert!(within(Duration::from_secs(5), || seen("key 29 true")
        && seen("button Left true")));

    // Mid-drag and mid-Ctrl, the phone drops off.
    session.close("wifi lost");
    assert!(
        within(Duration::from_secs(10), || seen("key 29 false")
            && seen("button Left false")),
        "the held key and button stay pressed: {:?}",
        recorder.0.lock_ok()
    );
    assert!(within(Duration::from_secs(5), || !daemon
        .devices
        .lock_ok()
        .contains_key(&id)));
    drop(wire);
}

/// MAG-27: a stopping wire ends each session through its own cleanup, so
/// the phone hears why, and everything the session held (its registry
/// entry, its storage client, its phone book, its notifications) is gone
/// before the wire's thread ends. A session cut at an await instead of
/// ending on its own leaves its storage client and book behind.
#[test]
fn a_stopping_wire_closes_its_sessions_before_it_ends() {
    use magnetita_proto::daily::notifications::NotificationPosted;
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let server = Arc::new(notifications::testing::Recorder::default());
    let bridge = Arc::new(notifications::Bridge::default());
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        Adapters {
            notification_server: server.clone(),
            notifications: Arc::clone(&bridge),
            ..test_adapters()
        },
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (phone, phone_fp) = rt.block_on(async { phone("leaving") });
    let id = magnetita_link::device_id_of(&phone_fp);
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
    assert!(within(Duration::from_secs(5), || daemon
        .devices
        .lock_ok()
        .contains_key(&id)));
    let note = NotificationPosted {
        key: "0|com.example|1".into(),
        app_name: "Messages".into(),
        title: "Ana".into(),
        body: "leaving soon".into(),
        timestamp_ms: 1,
        replyable: false,
        actions: Vec::new(),
        icon: None,
        media: false,
    };
    rt.block_on(session.send_message(
        capability::NOTIFICATIONS,
        NotificationPosted::KIND,
        note.encode(),
    ))
    .unwrap();
    assert!(within(Duration::from_secs(5), || bridge
        .origin(1, false)
        .is_some()));
    phone::store().with(&id, |book| book.contacts_version = 7);
    assert!(storage::clients().lock_ok().contains_key(&id));

    drop(wire);
    assert!(
        !daemon.devices.lock_ok().contains_key(&id),
        "the session left the registry before the wire's thread ended"
    );
    assert!(
        !storage::clients().lock_ok().contains_key(&id),
        "the session's own cleanup removed its storage client"
    );
    assert_eq!(
        phone::store().with(&id, |book| book.contacts_version),
        0,
        "the session's own cleanup forgot its phone book"
    );
    assert!(
        bridge.origin(1, false).is_none(),
        "the desktop worker ran the notification cleanup before it was joined"
    );
    // A datagram read reports the connection's close with its reason.
    let reason = rt.block_on(async {
        loop {
            match tokio::time::timeout(Duration::from_secs(5), session.recv_datagram()).await {
                Ok(Ok(_)) => continue,
                Ok(Err(e)) => break e.to_string(),
                Err(_) => break "still open".to_owned(),
            }
        }
    });
    assert!(reason.contains("daemon stopping"), "closed with: {reason}");
}

/// The drain waits for a task's slow cleanup instead of cutting it.
#[test]
fn the_drain_lets_a_slow_cleanup_finish() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cleaned = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let ended = rt.block_on(async {
        let mut tasks = tokio::task::JoinSet::new();
        let (cleaned, stop2) = (Arc::clone(&cleaned), Arc::clone(&stop));
        tasks.spawn(async move {
            while !stop2.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
            cleaned.store(true, Ordering::SeqCst);
        });
        stop.store(true, Ordering::SeqCst);
        drain(&mut tasks, Duration::from_secs(5)).await
    });
    assert!(ended);
    assert!(cleaned.load(Ordering::SeqCst), "the cleanup was cut");
}

/// A cleanup that outlives the limit is cut, and the drain says so.
#[test]
fn the_drain_cuts_what_outlives_its_limit() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ended = rt.block_on(async {
        let mut tasks = tokio::task::JoinSet::new();
        tasks.spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        drain(&mut tasks, Duration::from_millis(100)).await
    });
    assert!(!ended);
}

/// A phone that dials again while its earlier session is open, and is
/// forgotten while the new session waits for the old one to leave, is not
/// registered: the old session's teardown clears the revocation, so the new
/// one must find its pin gone on its own.
#[test]
fn a_session_forgotten_while_it_waits_for_its_slot_is_refused() {
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
    let (phone, phone_fp) = rt.block_on(async { phone("twice") });
    let id = magnetita_link::device_id_of(&phone_fp);
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let (first, _) = rt.block_on(prove(&phone, phone_fp, &uri));
    assert!(within(Duration::from_secs(5), || daemon
        .devices
        .lock_ok()
        .contains_key(&id)));

    // The same phone dials again: admitted by its pin, it waits for the
    // earlier session to leave. Forget lands right then.
    let mut pins = TrustStore::in_memory();
    pins.pin(TrustedPeer {
        device_id: "desktop".into(),
        device_name: "Celestina".into(),
        fingerprint: fingerprint_text(&desktop_fp),
    })
    .unwrap();
    let (second, _) = rt
        .block_on(phone.connect(addr, Expect::Trusted(&pins)))
        .unwrap();
    daemon
        .revocations
        .request_if_and_apply(&id, || true, || daemon.trust.lock_ok().forget(&id))
        .unwrap();

    thread::sleep(SUPERSEDE_WAIT / 2);
    assert!(
        within(Duration::from_secs(1), || !daemon
            .devices
            .lock_ok()
            .contains_key(&id)),
        "a forgotten phone registered again"
    );
    thread::sleep(Duration::from_secs(1));
    assert!(!daemon.devices.lock_ok().contains_key(&id));
    drop((first, second));
    drop(wire);
}

/// MAG-27: what a stopping session had queued for the phone reaches it
/// before the connection closes (the reviewer's probe: without the finished
/// and acknowledged control stream, the phone received 0 or 5 of 21).
#[test]
fn a_queued_send_reaches_the_phone_before_the_stop_closes() {
    use magnetita_proto::daily::find::FindRing;
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
    let (phone, phone_fp) = rt.block_on(async { phone("ring") });
    let id = magnetita_link::device_id_of(&phone_fp);
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
    assert!(within(Duration::from_secs(5), || daemon
        .devices
        .lock_ok()
        .contains_key(&id)));
    let sender = daemon.commands.lock_ok().get(&id).cloned().unwrap();
    for _ in 0..20 {
        sender
            .try_send(Command::NotificationReply {
                key: "k".into(),
                text: "y".repeat(2000),
            })
            .unwrap();
    }
    sender.try_send(Command::Ring).unwrap();
    drop(wire);
    let (rings, replies) = rt.block_on(async {
        let (mut rings, mut replies) = (0, 0);
        loop {
            match tokio::time::timeout(Duration::from_secs(5), session.recv()).await {
                Ok(Ok(env)) if env.capability == capability::FIND && env.kind == FindRing::KIND => {
                    rings += 1
                }
                Ok(Ok(env)) if env.capability == capability::NOTIFICATIONS => replies += 1,
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => break (rings, replies),
            }
        }
    });
    assert_eq!((replies, rings), (20, 1), "every queued send arrives");
}

/// A phone abandons one upload, by ending its stream short or by a hostile
/// reset, then uploads another on the same session: the desktop receives the
/// second whole. One abandoned stream never stops the bulk streams after it.
fn a_later_upload_arrives_after_an_abandoned_one(reset: bool) {
    use magnetita_proto::daily::share::{ShareAccept, ShareDone, ShareOffer};
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let downloads = std::env::temp_dir().join(format!(
        "magnetita-abandon-{}-{}",
        std::process::id(),
        u8::from(reset)
    ));
    let _ = std::fs::remove_dir_all(&downloads);
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        Adapters {
            download_dir: downloads.clone(),
            ..test_adapters()
        },
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (phone, phone_fp) = rt.block_on(async { phone("abandoning") });
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
    let next_share = |kind: u16| {
        rt.block_on(async {
            loop {
                let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                    .await
                    .expect("a share message in time")
                    .unwrap();
                if env.capability == capability::SHARE && env.kind == kind {
                    break env;
                }
            }
        })
    };
    let offer = |transfer: u32, name: &str| ShareOffer {
        transfer,
        name: name.into(),
        size: 10,
        mime: String::new(),
    };
    rt.block_on(session.send_message(
        capability::SHARE,
        ShareOffer::KIND,
        offer(7, "a.bin").encode(),
    ))
    .unwrap();
    next_share(ShareAccept::KIND);
    rt.block_on(async {
        let mut stream = session.open_transfer(7).await.unwrap();
        if reset {
            let _ = stream.reset(0u32.into());
        } else {
            stream.write_all(&[1u8; 3]).await.unwrap();
            let _ = stream.finish();
        }
        session
            .send_message(
                capability::SHARE,
                ShareDone::KIND,
                ShareDone {
                    transfer: 7,
                    complete: false,
                }
                .encode(),
            )
            .await
            .unwrap();
    });
    rt.block_on(session.send_message(
        capability::SHARE,
        ShareOffer::KIND,
        offer(8, "b.bin").encode(),
    ))
    .unwrap();
    let accepted = loop {
        let accept = ShareAccept::decode(&next_share(ShareAccept::KIND).body).unwrap();
        if accept.transfer == 8 {
            break accept;
        }
    };
    assert_eq!(accepted.offset, 0);
    rt.block_on(async {
        let mut stream = session.open_transfer(8).await.unwrap();
        stream.write_all(&[2u8; 10]).await.unwrap();
        stream.finish().unwrap();
        let _ = stream.stopped().await;
    });
    let done = loop {
        let done = ShareDone::decode(&next_share(ShareDone::KIND).body).unwrap();
        if done.transfer == 8 {
            break done;
        }
    };
    assert!(
        done.complete,
        "the upload after the abandoned one arrived whole"
    );
    assert_eq!(
        std::fs::read(downloads.join("b.bin")).unwrap(),
        vec![2u8; 10]
    );
    session.close("done");
    drop(wire);
    let _ = std::fs::remove_dir_all(&downloads);
}

#[test]
fn a_later_upload_arrives_after_one_abandoned_short() {
    a_later_upload_arrives_after_an_abandoned_one(false);
}

#[test]
fn a_later_upload_arrives_after_one_reset_before_its_id() {
    a_later_upload_arrives_after_an_abandoned_one(true);
}
