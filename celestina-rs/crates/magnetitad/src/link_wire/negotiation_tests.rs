//! Loopback tests for negotiated capabilities (MAG-8): a session uses only
//! what both hellos offer. An older phone that offers battery and find
//! keeps both and is refused the rest both ways; the phone crate itself
//! agrees with this daemon on every capability both build.

use super::tests::{phone_offering, prove, test_daemon};
use super::*;

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

fn adapters_recording(received: Arc<Mutex<Vec<String>>>) -> Adapters {
    Adapters {
        clipboard_sink: Arc::new(move |text: &str| {
            received.lock_ok().push(text.to_owned());
            true
        }),
        ..super::tests::test_adapters()
    }
}

#[test]
fn an_older_phone_keeps_what_both_offer_and_is_refused_the_rest() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let received = Arc::new(Mutex::new(Vec::new()));
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        adapters_recording(Arc::clone(&received)),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    // What the phone's hello listed before capabilities were negotiated.
    let old = [capability::BATTERY, capability::FIND]
        .into_iter()
        .map(|capability| CapabilityVersion {
            capability,
            version: 1,
        })
        .collect();
    let (phone, phone_fp) = rt.block_on(async { phone_offering("older", old) });
    let id = magnetita_link::device_id_of(&phone_fp);
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let (session, _) = rt.block_on(prove(&phone, phone_fp, &uri));
    assert!(within(Duration::from_secs(5), || daemon
        .devices
        .lock_ok()
        .contains_key(&id)));

    // What both offer flows as before.
    rt.block_on(async {
        session
            .send_message(
                capability::BATTERY,
                BatteryStatus::KIND,
                BatteryStatus {
                    level: 44,
                    charging: false,
                    low: false,
                }
                .encode(),
            )
            .await
            .unwrap();
        // The phone did not offer the clipboard: its text is refused.
        session
            .send_message(
                capability::CLIPBOARD,
                ClipboardText::KIND,
                ClipboardText {
                    text: "not negotiated".into(),
                }
                .encode(),
            )
            .await
            .unwrap();
    });
    assert!(within(Duration::from_secs(5), || daemon
        .devices
        .lock_ok()
        .get(&id)
        .is_some_and(|e| e.battery == 44)));

    // The desktop's clipboard and a ring: only the ring may reach the phone.
    daemon
        .pending_clipboards
        .replace_for([id.clone()], "copied on the desk".into());
    daemon
        .commands
        .lock_ok()
        .get(&id)
        .cloned()
        .unwrap()
        .try_send(Command::Ring)
        .unwrap();
    let seen = rt.block_on(async {
        let mut seen = Vec::new();
        loop {
            let env = tokio::time::timeout(Duration::from_secs(5), session.recv())
                .await
                .expect("the ring within the budget")
                .unwrap();
            seen.push(env.capability);
            if env.capability == capability::FIND {
                break seen;
            }
        }
    });
    assert!(
        seen.iter()
            .all(|c| matches!(*c, capability::FIND | capability::PAIRING)),
        "only negotiated capabilities reach the phone: {seen:?}"
    );
    assert!(
        received.lock_ok().is_empty(),
        "the refused text never landed"
    );
    session.close("done");
    drop(wire);
}

#[test]
fn the_phone_crate_and_the_daemon_agree_on_every_capability() {
    let cert = DeviceCert::generate("desktop");
    let daemon = test_daemon(&cert);
    let arm = PairingArm::default();
    let received = Arc::new(Mutex::new(Vec::new()));
    let (wire, addr) = spawn(
        Arc::clone(&daemon),
        cert.clone(),
        "desktop".into(),
        arm.clone(),
        "127.0.0.1:0".parse().unwrap(),
        false,
        adapters_recording(Arc::clone(&received)),
    )
    .unwrap();
    let desktop_fp = magnetita_link::fingerprint_of(&cert.chain().unwrap()[0]);
    let uri = arm.arm("desktop", desktop_fp, vec![addr.to_string()]);
    let dir = std::env::temp_dir().join(format!("magnetita-agree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let session = rt.block_on(async {
        let phone = magnetita_mobile::Phone::open(&dir, "current").unwrap();
        let (_, session) = phone.pair(&uri).await.unwrap();
        session
    });
    let mut agreed: Vec<u16> = session
        .negotiated()
        .capabilities()
        .iter()
        .map(|c| c.capability)
        .collect();
    agreed.sort_unstable();
    let mut ours: Vec<u16> = offered().iter().map(|c| c.capability).collect();
    ours.sort_unstable();
    assert_eq!(
        agreed, ours,
        "the phone offers every capability this daemon does"
    );
    rt.block_on(session.send_clipboard("copied on the phone"))
        .unwrap();
    assert!(within(Duration::from_secs(5), || !received
        .lock_ok()
        .is_empty()));
    assert_eq!(received.lock_ok().as_slice(), ["copied on the phone"]);
    session.close("done");
    drop(wire);
    let _ = std::fs::remove_dir_all(&dir);
}
