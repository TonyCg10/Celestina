//! The shared activation hand-off: its pure rules, and two real claims on a
//! private bus started with `dbus-run-session` (never the author's session).

#![cfg(feature = "activation")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use celestina_core::activation::{
    claim, decide, object_path, open_in, Activatable, ActivationName, Claim, HandOff, Inbox,
    Request, INBOX_LIMIT,
};

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<Request>>>);

impl Recorder {
    fn seen(&self) -> Vec<Request> {
        self.0.lock().expect("recorder").clone()
    }
}

impl Activatable for Recorder {
    fn activate(&self) {
        self.0.lock().expect("recorder").push(Request::Activate);
    }

    fn open(&self, paths: Vec<PathBuf>) {
        self.0.lock().expect("recorder").push(Request::Open(paths));
    }
}

#[test]
fn the_object_path_is_the_name_with_slashes() {
    assert_eq!(
        object_path(&ActivationName("org.celestina.Grafita")),
        "/org/celestina/Grafita"
    );
}

#[test]
fn the_inbox_replays_in_order_once_attached() {
    let recorder = Recorder::default();
    let mut inbox = Inbox::new(Box::new(recorder.clone()));
    inbox.deliver(Request::Activate);
    inbox.deliver(Request::Open(vec![PathBuf::from("/tmp/a")]));
    assert!(
        recorder.seen().is_empty(),
        "nothing reaches Qt before attach"
    );
    inbox.attach();
    assert_eq!(
        recorder.seen(),
        vec![
            Request::Activate,
            Request::Open(vec![PathBuf::from("/tmp/a")])
        ]
    );
    inbox.deliver(Request::Activate);
    assert_eq!(
        recorder.seen().len(),
        3,
        "later requests go straight through"
    );
}

#[test]
fn the_inbox_keeps_the_newest_sixteen_and_counts_the_rest() {
    assert_eq!(INBOX_LIMIT, 16);
    let recorder = Recorder::default();
    let mut inbox = Inbox::new(Box::new(recorder.clone()));
    for n in 0..(INBOX_LIMIT + 4) {
        inbox.deliver(Request::Open(vec![PathBuf::from(format!("/{n}"))]));
    }
    assert_eq!(inbox.dropped(), 4);
    inbox.attach();
    let seen = recorder.seen();
    assert_eq!(seen.len(), INBOX_LIMIT);
    assert_eq!(seen[0], Request::Open(vec![PathBuf::from("/4")]));
    assert_eq!(
        seen[INBOX_LIMIT - 1],
        Request::Open(vec![PathBuf::from(format!("/{}", INBOX_LIMIT + 3))])
    );
}

#[test]
fn the_hand_off_follows_the_owner_and_the_arguments() {
    assert_eq!(decide(true, false), HandOff::Serve);
    assert_eq!(decide(false, false), HandOff::Serve);
    assert_eq!(decide(true, true), HandOff::Activate);
    assert_eq!(decide(false, true), HandOff::Open);
}

const PRIVATE_BUS: &str = "CELESTINA_ACTIVATION_PRIVATE_BUS";

/// Re-runs this test inside `dbus-run-session`, so the claims land on a bus of
/// their own; skipped with a message when the tool is missing.
#[test]
fn two_claims_make_one_owner_and_one_hand_off() {
    if std::env::var_os(PRIVATE_BUS).is_none() {
        let exe = std::env::current_exe().expect("test binary");
        let run = std::process::Command::new("dbus-run-session")
            .arg("--")
            .arg(exe)
            .args([
                "--exact",
                "two_claims_make_one_owner_and_one_hand_off",
                "--nocapture",
            ])
            .env(PRIVATE_BUS, "1")
            .status();
        match run {
            Ok(status) => assert!(status.success(), "the private-bus run failed"),
            Err(error) => eprintln!("skipped: dbus-run-session unavailable ({error})"),
        }
        return;
    }
    on_private_bus();
}

fn on_private_bus() {
    use std::os::unix::ffi::OsStringExt;

    const NAME: ActivationName = ActivationName("org.celestina.ActivationTest");
    let wanted = vec![
        PathBuf::from("/tmp/a b.txt"),
        PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/mal-\xFF".to_vec())),
    ];
    let timeout = std::time::Duration::from_secs(3);
    assert_eq!(
        open_in(NAME, &wanted, timeout),
        Ok(false),
        "nobody owns it yet"
    );

    let first = Recorder::default();
    let second = Recorder::default();
    let (a, b) = (first.clone(), second.clone());
    let paths = wanted.clone();
    let one = std::thread::spawn(move || claim(NAME, Box::new(a), &[]));
    let two = std::thread::spawn(move || claim(NAME, Box::new(b), &paths));
    let (one, two) = (one.join().expect("first"), two.join().expect("second"));

    let (owner, owner_seen, handed, loser_had_paths) = match (one, two) {
        (Claim::Owner(owner), other) => (owner, first, other, true),
        (other, Claim::Owner(owner)) => (owner, second, other, false),
        _ => panic!("neither claim owns the name"),
    };
    assert!(
        matches!(handed, Claim::HandedOff),
        "the loser did not hand off"
    );
    assert!(
        owner_seen.seen().is_empty(),
        "nothing reaches Qt before attach"
    );

    owner.attach();
    let expected = if loser_had_paths {
        Request::Open(wanted.clone())
    } else {
        Request::Activate
    };
    assert_eq!(
        owner_seen.seen(),
        vec![expected],
        "the loser's request replays"
    );
    assert_eq!(open_in(NAME, &wanted, timeout), Ok(true));
    let seen = owner_seen.seen();
    assert_eq!(seen.last(), Some(&Request::Open(wanted.clone())));
    eprintln!("private bus: one owner, one hand-off, Open delivered {seen:?}");
}
