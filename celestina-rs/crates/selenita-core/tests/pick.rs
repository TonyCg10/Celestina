use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use selenita_core::pick::{
    clear_request, is_output_name, request_file, take_request, write_request, PickError,
    PickRequest, Taken, FRESHNESS, NAME_LIMIT,
};

/// A private runtime folder of the test's own, removed afterwards.
struct Runtime(PathBuf);

impl Runtime {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("selenita-pick-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
        Self(dir)
    }

    fn file(&self) -> PathBuf {
        request_file(&self.0)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn at(millis: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis)
}

fn leftovers(folder: &Path) -> Vec<String> {
    std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_request_lives_in_the_private_selenita_folder() {
    assert_eq!(
        request_file(Path::new("/run/user/1000")),
        PathBuf::from("/run/user/1000/selenita/pick-output")
    );
}

#[test]
fn a_request_round_trips_through_its_text() {
    let request = PickRequest::new("DP-1", at(1_760_140_325_123)).expect("request");
    assert_eq!(request.encode(), "output=DP-1\nwritten=1760140325123\n");
    assert_eq!(
        PickRequest::parse(&request.encode()).expect("parsed"),
        request
    );
    assert_eq!(request.chooser_line(), "Monitor: DP-1");
    assert_eq!(request.output(), "DP-1");
    assert_eq!(request.written(), at(1_760_140_325_123));
    // Unknown keys and lines without a key are ignored, in any order.
    let parsed =
        PickRequest::parse("# note\nwritten=5\nversion=2\noutput=HDMI-A-1\n").expect("parsed");
    assert_eq!(parsed.output(), "HDMI-A-1");
    assert_eq!(parsed.written(), at(5));
}

#[test]
fn a_file_that_is_not_a_request_is_refused() {
    for text in [
        "",
        "output=DP-1\n",
        "written=5\n",
        "output=DP-1\nwritten=soon\n",
        "output=DP-1\nwritten=-5\n",
        "output=DP-1\noutput=DP-2\nwritten=5\n",
    ] {
        assert!(
            matches!(PickRequest::parse(text), Err(PickError::Malformed(_))),
            "{text:?}"
        );
    }
    for text in ["output=\nwritten=5\n", "output=DP 1\nwritten=5\n"] {
        assert!(
            matches!(PickRequest::parse(text), Err(PickError::InvalidOutput(_))),
            "{text:?}"
        );
    }
}

#[test]
fn an_output_name_stays_on_one_line() {
    assert!(is_output_name("DP-1"));
    assert!(is_output_name("eDP-1"));
    assert!(is_output_name("FAKE-1"));
    assert!(is_output_name(&"A".repeat(NAME_LIMIT)));
    assert!(!is_output_name(""));
    assert!(!is_output_name(&"A".repeat(NAME_LIMIT + 1)));
    assert!(!is_output_name("DP-1\nMonitor: DP-2"));
    assert!(!is_output_name("DP 1"));
    assert!(!is_output_name("DP-1\t"));
    assert!(!is_output_name("DP-\u{7}1"));
    assert!(matches!(
        PickRequest::new("DP-1\n", at(0)),
        Err(PickError::InvalidOutput(_))
    ));
}

#[test]
fn a_request_is_fresh_for_thirty_seconds_and_never_from_the_future() {
    let written = at(1_000_000);
    let request = PickRequest::new("DP-1", written).expect("request");
    assert_eq!(FRESHNESS, Duration::from_secs(30));
    assert!(request.is_fresh(written));
    assert!(request.is_fresh(written + Duration::from_secs(29)));
    assert!(request.is_fresh(written + FRESHNESS));
    assert!(!request.is_fresh(written + FRESHNESS + Duration::from_millis(1)));
    assert!(!request.is_fresh(written - Duration::from_millis(1)));
}

#[test]
fn a_written_request_is_private_and_consumed_once() {
    let runtime = Runtime::new("once");
    let file = runtime.file();
    let now = SystemTime::now();
    write_request(&file, "DP-2", now).expect("written");
    let folder = file.parent().expect("folder");
    let mode = |path: &Path| std::fs::metadata(path).expect("meta").permissions().mode() & 0o777;
    assert_eq!(mode(folder), 0o700, "the folder is private");
    assert_eq!(mode(&file), 0o600, "the request is private");
    // A second write replaces the first.
    write_request(&file, "DP-1", now).expect("rewritten");
    match take_request(&file, now + Duration::from_secs(1)).expect("taken") {
        Taken::Fresh(request) => assert_eq!(request.chooser_line(), "Monitor: DP-1"),
        other => panic!("expected a fresh request, got {other:?}"),
    }
    assert!(!file.exists(), "consumed");
    assert!(leftovers(folder).is_empty(), "{:?}", leftovers(folder));
    assert!(matches!(
        take_request(&file, now).expect("nothing"),
        Taken::Absent
    ));
}

#[test]
fn a_stale_request_is_removed_and_ignored() {
    let runtime = Runtime::new("stale");
    let file = runtime.file();
    let then = SystemTime::now();
    write_request(&file, "DP-1", then).expect("written");
    match take_request(&file, then + FRESHNESS + Duration::from_secs(1)).expect("taken") {
        Taken::Stale(request) => assert_eq!(request.output(), "DP-1"),
        other => panic!("expected a stale request, got {other:?}"),
    }
    assert!(!file.exists(), "a stale request is removed");
    assert!(leftovers(file.parent().expect("folder")).is_empty());
}

#[test]
fn a_file_that_is_not_a_request_is_removed_and_ignored() {
    let runtime = Runtime::new("invalid");
    let file = runtime.file();
    std::fs::create_dir_all(file.parent().expect("folder")).expect("folder");
    std::fs::write(&file, b"Monitor: DP-1\n").expect("junk");
    assert!(matches!(
        take_request(&file, SystemTime::now()).expect("taken"),
        Taken::Invalid(PickError::Malformed(_))
    ));
    assert!(!file.exists());
    std::fs::write(&file, [0xff, 0xfe]).expect("bytes");
    assert!(matches!(
        take_request(&file, SystemTime::now()).expect("taken"),
        Taken::Invalid(_)
    ));
    std::fs::write(&file, vec![b'x'; 4096]).expect("long");
    assert!(matches!(
        take_request(&file, SystemTime::now()).expect("taken"),
        Taken::Invalid(PickError::Read(_))
    ));
    assert!(leftovers(file.parent().expect("folder")).is_empty());
}

#[test]
fn an_unconsumed_request_is_cleared() {
    let runtime = Runtime::new("clear");
    let file = runtime.file();
    assert!(!clear_request(&file).expect("nothing to clear"));
    write_request(&file, "DP-1", SystemTime::now()).expect("written");
    assert!(clear_request(&file).expect("cleared"));
    assert!(!file.exists());
}

#[test]
fn a_bad_name_writes_nothing() {
    let runtime = Runtime::new("bad");
    let file = runtime.file();
    assert!(matches!(
        write_request(&file, "", SystemTime::now()),
        Err(PickError::InvalidOutput(_))
    ));
    assert!(!file.exists());
}
