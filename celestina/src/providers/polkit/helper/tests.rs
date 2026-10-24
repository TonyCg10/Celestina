//! The helper conversation against a fixture helper: a Unix socket in a
//! temporary directory, served by a test thread that scripts PAM's lines
//! (design §10.1, plan Task 5). Nothing here touches `/run/polkit`: the
//! production path is a constant, and the trust a real helper needs (root)
//! is replaced by the test's own uid through the private `connect_at`.

use std::fmt::Debug;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use celestina_shell::polkit::{Ending, Exchange, Reply, MAX_ATTEMPTS, MAX_HELPER_LINES};

use super::*;

const WAIT: Duration = Duration::from_secs(5);
/// How long "nothing happens" is watched for.
const QUIET: Duration = Duration::from_millis(400);
const COOKIE: &str = "3-0123456789abcdef-1-fedcba98";

fn must<T, E: Debug>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("{what}: {err:?}"),
    }
}

fn me() -> u32 {
    rustix::process::getuid().as_raw()
}

/// The test's own uid stands in for root.
fn trust() -> Trust {
    Trust {
        files: me(),
        listener: me(),
    }
}

fn timing(silence: Duration) -> Timing {
    Timing {
        poll: Duration::from_millis(20),
        silence,
    }
}

fn long_silence() -> Timing {
    timing(Duration::from_secs(30))
}

fn cookie() -> Cookie {
    must(Cookie::new(COOKIE), "a valid cookie")
}

/// A secret holding `text`.
fn typed(text: &str) -> Secret {
    let mut secret = Secret::new();
    for c in text.chars() {
        must(secret.push(c), "the character fits");
    }
    secret
}

// ---- the fixture helper -------------------------------------------------------------------

/// A scripted helper. `script(n, socket)` runs once per accepted connection,
/// `n` counting from zero, on the fixture's thread.
struct Fixture {
    dir: PathBuf,
    path: PathBuf,
    accepted: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

impl Fixture {
    fn start(script: impl FnMut(usize, UnixStream) + Send + 'static) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "celestina-polkit-helper-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        must(
            fs::DirBuilder::new().mode(0o700).create(&dir),
            "the fixture directory",
        );
        let path = dir.join("agent-helper.socket");
        let listener = must(UnixListener::bind(&path), "the fixture socket");
        must(listener.set_nonblocking(true), "a non-blocking listener");
        let accepted = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (accepted, stop) = (Arc::clone(&accepted), Arc::clone(&stop));
            let mut script = script;
            thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((socket, _)) => {
                            let n = accepted.fetch_add(1, Ordering::AcqRel);
                            let _ = socket.set_nonblocking(false);
                            script(n, socket);
                        }
                        Err(_) => thread::sleep(Duration::from_millis(5)),
                    }
                }
            })
        };
        Fixture {
            dir,
            path,
            accepted,
            stop,
            thread: Some(thread),
        }
    }

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::Acquire)
    }

    fn connect(&self, user: &str) -> Result<Conversation, ConnectRefusal> {
        connect_at(&self.path, user, &cookie(), trust(), long_silence())
    }

    /// The way the runner opens each conversation.
    fn connector(
        &self,
        timing: Timing,
    ) -> impl FnMut(&HelperInput) -> Result<Conversation, ConnectRefusal> + Send + 'static {
        let path = self.path.clone();
        move |input| connect_at(&path, input.user(), input.cookie(), trust(), timing)
    }

    /// Whether no connection arrives for [`QUIET`].
    fn stays_unconnected(&self) -> bool {
        thread::sleep(QUIET);
        self.accepted() == 0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// One line to the agent.
fn send(socket: &mut UnixStream, line: &[u8]) {
    let _ = socket.write_all(line);
    let _ = socket.write_all(b"\n");
}

/// One line from the agent, without its newline; `None` at end of input or
/// after [`WAIT`].
fn receive(socket: &mut UnixStream) -> Option<Vec<u8>> {
    let _ = socket.set_read_timeout(Some(WAIT));
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match socket.read(&mut byte) {
            Ok(1) if byte[0] == b'\n' => return Some(line),
            Ok(1) if line.len() < 4096 => line.push(byte[0]),
            _ => return None,
        }
    }
}

/// Blocks until the agent closes its end.
fn until_closed(socket: &mut UnixStream) -> bool {
    let _ = socket.set_read_timeout(Some(WAIT));
    let mut buf = [0u8; 64];
    loop {
        match socket.read(&mut buf) {
            Ok(0) => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

/// A helper that reads the two opening lines and then speaks `lines`.
fn speaking(lines: Vec<Vec<u8>>) -> Fixture {
    Fixture::start(move |_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        for line in &lines {
            send(&mut socket, line);
        }
        until_closed(&mut socket);
    })
}

// ---- driving the thread ---------------------------------------------------------------------

fn input() -> HelperInput {
    HelperInput::new("tester".to_owned(), cookie())
}

struct Run {
    helper: Helper,
    events: Receiver<HelperEvent>,
}

fn run_with<S: Answer>(
    fixture: &Fixture,
    timing: Timing,
) -> (Helper<S>, Receiver<HelperEvent>, Arc<AtomicBool>) {
    let (tx, events) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let helper = must(
        spawn_with(
            input(),
            move |event| tx.send(event).is_ok(),
            &shutdown,
            fixture.connector(timing),
        ),
        "the helper thread starts",
    );
    (helper, events, shutdown)
}

fn start(fixture: &Fixture) -> Run {
    let (helper, events, _) = run_with(fixture, long_silence());
    Run { helper, events }
}

/// Collects events, answering each prompt with the next of `answers`, until
/// an ending or until a prompt finds no answer left (then the run is
/// cancelled and the events so far are returned).
fn drive(fixture: &Fixture, timing: Timing, answers: &[&str]) -> Vec<HelperEvent> {
    let (helper, events, _) = run_with::<Secret>(fixture, timing);
    let mut answers = answers.iter();
    let mut seen = Vec::new();
    loop {
        let Ok(event) = events.recv_timeout(WAIT) else {
            panic!("no ending after {seen:?}");
        };
        let prompt = matches!(event, HelperEvent::Prompt(_));
        let over = matches!(event, HelperEvent::Ended(_));
        seen.push(event);
        if over {
            return seen;
        }
        if prompt {
            let Some(answer) = answers.next() else {
                helper.cancel();
                return seen;
            };
            must(helper.answer(typed(answer)), "the answer is taken");
        }
    }
}

fn shown(event: &HelperEvent) -> String {
    match event {
        HelperEvent::Prompt(text) | HelperEvent::Notice(text) => text.display(),
        other => panic!("not a text event: {other:?}"),
    }
}

// ---- N-16: the checks before the first byte ---------------------------------------------------

#[test]
fn the_production_path_is_one_constant() {
    assert_eq!(HELPER_SOCKET, "/run/polkit/agent-helper.socket");
    assert_eq!(
        Trust::ROOT,
        Trust {
            files: 0,
            listener: 0
        }
    );
}

#[test]
fn directory_facts_are_judged_by_type_owner_and_mode() {
    let good = Facts {
        is_dir: true,
        is_socket: false,
        uid: 0,
        mode: 0o40755,
    };
    assert!(check_directory(&good, 0).is_ok());
    assert!(check_directory(
        &Facts {
            mode: 0o40700,
            ..good
        },
        0
    )
    .is_ok());
    assert!(matches!(
        check_directory(
            &Facts {
                is_dir: false,
                ..good
            },
            0
        ),
        Err(ConnectRefusal::DirectoryNotADirectory)
    ));
    assert!(matches!(
        check_directory(&Facts { uid: 1000, ..good }, 0),
        Err(ConnectRefusal::DirectoryNotOwned)
    ));
    for mode in [0o40775, 0o40757, 0o40777, 0o41777, 0o40770, 0o40702] {
        assert!(
            matches!(
                check_directory(&Facts { mode, ..good }, 0),
                Err(ConnectRefusal::DirectoryWritable)
            ),
            "mode {mode:o} is writable by group or others"
        );
    }
}

#[test]
fn socket_facts_are_judged_by_type_and_owner_and_never_by_mode() {
    let good = Facts {
        is_dir: false,
        is_socket: true,
        uid: 0,
        mode: 0o140666,
    };
    assert!(check_socket(&good, 0).is_ok(), "polkit makes it 0666");
    assert!(check_socket(
        &Facts {
            mode: 0o140600,
            ..good
        },
        0
    )
    .is_ok());
    assert!(matches!(
        check_socket(
            &Facts {
                is_socket: false,
                ..good
            },
            0
        ),
        Err(ConnectRefusal::SocketNotASocket)
    ));
    assert!(matches!(
        check_socket(&Facts { uid: 1000, ..good }, 0),
        Err(ConnectRefusal::SocketNotOwned)
    ));
}

#[test]
fn the_listener_is_judged_by_its_uid() {
    assert!(check_listener(0, 0).is_ok());
    assert!(matches!(
        check_listener(1000, 0),
        Err(ConnectRefusal::ListenerNotTrusted)
    ));
}

#[test]
fn connects_only_after_the_directory_type_and_owner_checks() {
    let fixture = speaking(Vec::new());
    assert!(fixture.connect("tester").is_ok(), "every check passes");
    assert!(eventually(|| fixture.accepted() == 1), "the helper saw it");
    // The same socket, judged by a different owner: refused before connecting.
    let strangers = Trust {
        files: me() + 1,
        listener: me(),
    };
    let before = fixture.accepted();
    let refused = connect_at(
        &fixture.path,
        "tester",
        &cookie(),
        strangers,
        long_silence(),
    );
    assert!(matches!(refused, Err(ConnectRefusal::DirectoryNotOwned)));
    thread::sleep(QUIET);
    assert_eq!(fixture.accepted(), before, "no connection was attempted");
}

#[test]
fn a_world_writable_directory_is_refused() {
    let fixture = speaking(Vec::new());
    for mode in [0o777, 0o775, 0o757, 0o722] {
        must(
            fs::set_permissions(&fixture.dir, fs::Permissions::from_mode(mode)),
            "chmod",
        );
        assert!(
            matches!(
                fixture.connect("tester"),
                Err(ConnectRefusal::DirectoryWritable)
            ),
            "{mode:o}"
        );
    }
    assert!(fixture.stays_unconnected());
}

#[test]
fn a_symlink_in_place_of_the_directory_or_the_socket_is_refused() {
    let fixture = speaking(Vec::new());
    // A symlink to the real socket, in a real directory.
    let link = fixture.dir.join("link.socket");
    must(
        std::os::unix::fs::symlink(&fixture.path, &link),
        "a symlink",
    );
    let refused = connect_at(&link, "tester", &cookie(), trust(), long_silence());
    assert!(matches!(refused, Err(ConnectRefusal::SocketNotASocket)));
    // A symlink to the real directory.
    let other = std::env::temp_dir().join(format!(
        "celestina-polkit-helper-link-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_file(&other);
    must(
        std::os::unix::fs::symlink(&fixture.dir, &other),
        "a symlink",
    );
    let through = connect_at(
        &other.join("agent-helper.socket"),
        "tester",
        &cookie(),
        trust(),
        long_silence(),
    );
    let _ = fs::remove_file(&other);
    assert!(matches!(
        through,
        Err(ConnectRefusal::DirectoryNotADirectory)
    ));
    assert!(fixture.stays_unconnected());
}

#[test]
fn a_regular_file_or_a_missing_path_is_refused() {
    let fixture = speaking(Vec::new());
    let file = fixture.dir.join("plain");
    must(fs::write(&file, b"x"), "a plain file");
    assert!(matches!(
        connect_at(&file, "tester", &cookie(), trust(), long_silence()),
        Err(ConnectRefusal::SocketNotASocket)
    ));
    assert!(matches!(
        connect_at(
            &fixture.dir.join("absent"),
            "tester",
            &cookie(),
            trust(),
            long_silence()
        ),
        Err(ConnectRefusal::Inspect(_))
    ));
    assert!(fixture.stays_unconnected());
}

#[test]
fn a_socket_not_owned_by_the_trusted_uid_is_refused_before_connecting() {
    let fixture = speaking(Vec::new());
    // Directory passes (it is the test's), the socket path does not, by the
    // facts: exercised through the pure check above; here, the end to end
    // case where both are judged by a stranger.
    let strangers = Trust {
        files: me() + 1,
        listener: me() + 1,
    };
    assert!(connect_at(
        &fixture.path,
        "tester",
        &cookie(),
        strangers,
        long_silence()
    )
    .is_err());
    assert!(fixture.stays_unconnected());
}

#[test]
fn a_socket_mode_of_0666_is_accepted() {
    let fixture = speaking(Vec::new());
    must(
        fs::set_permissions(&fixture.path, fs::Permissions::from_mode(0o666)),
        "chmod",
    );
    assert!(fixture.connect("tester").is_ok());
}

#[test]
fn a_non_root_listener_is_refused_before_the_first_byte() {
    let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let fixture = {
        let received = Arc::clone(&received);
        Fixture::start(move |_, mut socket| {
            let _ = socket.set_read_timeout(Some(Duration::from_millis(600)));
            let mut buf = [0u8; 256];
            if let Ok(count) = socket.read(&mut buf) {
                if count > 0 {
                    if let Ok(mut bytes) = received.lock() {
                        bytes.push(buf[..count].to_vec());
                    }
                }
            }
        })
    };
    // The files are the test's; the listener is judged to be someone else.
    let listener_is_stranger = Trust {
        files: me(),
        listener: me() + 1,
    };
    let refused = connect_at(
        &fixture.path,
        "tester",
        &cookie(),
        listener_is_stranger,
        long_silence(),
    );
    assert!(matches!(refused, Err(ConnectRefusal::ListenerNotTrusted)));
    drop(fixture);
    assert!(
        received.lock().map(|b| b.is_empty()).unwrap_or(false),
        "nothing was written to a listener that is not root"
    );
}

// ---- the opening lines ---------------------------------------------------------------------------

#[test]
fn writes_user_then_cookie_each_with_a_newline() {
    let (tx, lines) = mpsc::channel();
    let fixture = Fixture::start(move |_, mut socket| {
        let first = receive(&mut socket);
        let second = receive(&mut socket);
        let _ = tx.send((first, second));
        until_closed(&mut socket);
    });
    let _conversation = must(fixture.connect("tester"), "connects");
    let (first, second) = must(lines.recv_timeout(WAIT), "the opening lines");
    assert_eq!(first.as_deref(), Some(b"tester".as_slice()));
    assert_eq!(second.as_deref(), Some(COOKIE.as_bytes()));
}

#[test]
fn a_user_that_cannot_go_on_a_line_is_refused_before_connecting() {
    let fixture = speaking(Vec::new());
    let long = "u".repeat(MAX_USER_NAME_BYTES + 1);
    for user in [
        "",
        "a\nb",
        "a\rb",
        "a\0b",
        "a\u{7}b",
        "a\u{85}b",
        long.as_str(),
    ] {
        assert!(
            matches!(fixture.connect(user), Err(ConnectRefusal::BadUser)),
            "{user:?}"
        );
    }
    assert!(fixture.stays_unconnected());
}

// ---- N-17: what the helper says ---------------------------------------------------------------------

#[test]
fn prompt_text_is_decoded_then_sanitised() {
    let fixture = speaking(vec![b"PAM_PROMPT_ECHO_OFF Pass\\167ord\\072 ".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(shown(&events[0]), "Password:");
}

#[test]
fn escaped_esc_nul_and_rlo_in_pam_text_are_removed() {
    // ESC, NUL and U+202E as g_strescape writes them: octal.
    let fixture = speaking(vec![
        b"PAM_TEXT_INFO a\\033b\\000c\\342\\200\\256d\\nz".to_vec(),
        b"PAM_PROMPT_ECHO_OFF ok".to_vec(),
    ]);
    let events = drive(&fixture, long_silence(), &[]);
    let text = shown(&events[0]);
    assert!(
        !text.chars().any(|c| c.is_control() || c == '\u{202e}'),
        "{text:?}"
    );
    for kept in ['a', 'b', 'c', 'd', 'z'] {
        assert!(text.contains(kept), "{text:?}");
    }
}

#[test]
fn multibyte_pam_text_arriving_as_octal_is_decoded_validated_and_sanitised() {
    // U+00F1, U+20AC and U+1F600 as octal bytes, then an RLO inside.
    let fixture = speaking(vec![
        b"PAM_TEXT_INFO Pa\\303\\261 \\342\\202\\254 \\360\\237\\230\\200\\342\\200\\256x".to_vec(),
        b"PAM_PROMPT_ECHO_OFF p".to_vec(),
    ]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(shown(&events[0]), "Pa\u{f1} \u{20ac} \u{1f600}x");
}

#[test]
fn octal_that_is_not_utf8_ends_the_conversation_failed() {
    let fixture = speaking(vec![b"PAM_TEXT_INFO bad\\377\\376".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperProtocol)]);
}

#[test]
fn success_before_any_prompt_ends_with_success() {
    let fixture = speaking(vec![b"SUCCESS".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::Authenticated)]);
}

#[test]
fn an_unknown_line_ends_failed() {
    let fixture = speaking(vec![b"PAM_SOMETHING_ELSE x".to_vec(), b"SUCCESS".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperProtocol)]);
    assert_eq!(Ending::HelperProtocol.reply(), Some(Reply::Failed));
}

#[test]
fn a_verdict_is_a_whole_line_and_nothing_else() {
    for line in [
        &b"SUCCESS "[..],
        b"success",
        b"SUCCESSFUL",
        b" SUCCESS",
        b"FAILURE\\n",
    ] {
        let fixture = speaking(vec![line.to_vec()]);
        let events = drive(&fixture, long_silence(), &[]);
        assert_eq!(
            events,
            vec![HelperEvent::Ended(Ending::HelperProtocol)],
            "{line:?}"
        );
    }
}

#[test]
fn a_4097_byte_line_ends_failed() {
    let mut line = b"PAM_TEXT_INFO ".to_vec();
    line.resize(4097, b'x');
    let fixture = speaking(vec![line, b"SUCCESS".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperProtocol)]);
}

#[test]
fn a_line_of_exactly_the_limit_is_read() {
    let mut line = b"PAM_TEXT_INFO ".to_vec();
    line.resize(MAX_HELPER_LINE, b'x');
    let fixture = speaking(vec![line, b"PAM_PROMPT_ECHO_OFF p".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(events[0], HelperEvent::Notice(_)));
}

#[test]
fn a_line_without_a_newline_ends_failed_at_the_bound() {
    let fixture = Fixture::start(|_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        // Five thousand bytes and no newline, the socket kept open.
        let _ = socket.write_all(&vec![b'x'; 5000]);
        until_closed(&mut socket);
    });
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperProtocol)]);
}

#[test]
fn eof_before_a_verdict_ends_failed() {
    let fixture = Fixture::start(|_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        // Closed at once.
    });
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperEof)]);
    assert_eq!(Ending::HelperEof.reply(), Some(Reply::Failed));
}

#[test]
fn a_verdict_cut_off_before_its_newline_is_end_of_input_not_success() {
    let fixture = Fixture::start(|_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        let _ = socket.write_all(b"SUCCESS");
    });
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperEof)]);
}

#[test]
fn a_helper_that_never_speaks_ends_failed_at_helper_silence() {
    let fixture = speaking(Vec::new());
    let events = drive(&fixture, timing(Duration::from_millis(150)), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperSilent)]);
}

#[test]
fn silence_after_the_answer_ends_failed_at_helper_silence() {
    let fixture = speaking(vec![b"PAM_PROMPT_ECHO_OFF Password: ".to_vec()]);
    let events = drive(&fixture, timing(Duration::from_millis(150)), &["secret"]);
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(events[0], HelperEvent::Prompt(_)));
    assert_eq!(events[1], HelperEvent::Ended(Ending::HelperSilent));
}

#[test]
fn silence_while_the_person_types_does_not_end_the_conversation() {
    let fixture = speaking(vec![b"PAM_PROMPT_ECHO_OFF Password: ".to_vec()]);
    let run = start_with(&fixture, timing(Duration::from_millis(100)));
    assert!(matches!(
        must(run.events.recv_timeout(WAIT), "the prompt"),
        HelperEvent::Prompt(_)
    ));
    assert!(
        run.events.recv_timeout(Duration::from_millis(600)).is_err(),
        "the helper owes nothing while the card waits for the person"
    );
}

fn start_with(fixture: &Fixture, timing: Timing) -> Run {
    let (helper, events, _) = run_with(fixture, timing);
    Run { helper, events }
}

#[test]
fn more_lines_after_success_are_ignored() {
    let fixture = speaking(vec![
        b"SUCCESS".to_vec(),
        b"PAM_PROMPT_ECHO_OFF again".to_vec(),
        b"FAILURE".to_vec(),
    ]);
    let run = start(&fixture);
    assert_eq!(
        must(run.events.recv_timeout(WAIT), "the ending"),
        HelperEvent::Ended(Ending::Authenticated)
    );
    assert!(
        run.events.recv_timeout(QUIET).is_err(),
        "nothing follows the success"
    );
}

#[test]
fn failure_before_any_prompt_is_failed_and_does_not_retry() {
    let fixture = speaking(vec![b"FAILURE".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::HelperProtocol)]);
    assert_eq!(Ending::HelperProtocol.reply(), Some(Reply::Failed));
    thread::sleep(QUIET);
    assert_eq!(fixture.accepted(), 1, "no second conversation");
}

#[test]
fn an_echo_on_prompt_ends_cancelled() {
    let fixture = speaking(vec![b"PAM_PROMPT_ECHO_ON Login: ".to_vec()]);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(events, vec![HelperEvent::Ended(Ending::EchoOnPrompt)]);
    assert_eq!(Ending::EchoOnPrompt.reply(), Some(Reply::Cancelled));
}

#[test]
fn a_flood_of_lines_ends_failed() {
    let lines = vec![b"PAM_TEXT_INFO x".to_vec(); MAX_HELPER_LINES + 6];
    let fixture = speaking(lines);
    let events = drive(&fixture, long_silence(), &[]);
    assert_eq!(
        events.last(),
        Some(&HelperEvent::Ended(Ending::HelperProtocol))
    );
    let notices = events
        .iter()
        .filter(|e| matches!(e, HelperEvent::Notice(_)))
        .count();
    assert_eq!(notices, MAX_HELPER_LINES);
}

// ---- the answer ---------------------------------------------------------------------------------

#[test]
fn a_wrong_answer_below_the_cap_opens_a_second_conversation_with_the_same_cookie() {
    let (tx, seen) = mpsc::channel();
    let fixture = Fixture::start(move |n, mut socket| {
        let user = receive(&mut socket);
        let cookie = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let answer = receive(&mut socket);
        let _ = tx.send((n, user, cookie, answer));
        send(&mut socket, b"FAILURE");
        until_closed(&mut socket);
    });
    let events = drive(&fixture, long_silence(), &["first", "second"]);
    assert_eq!(
        events.iter().map(kind).collect::<Vec<_>>(),
        ["prompt", "failure", "prompt", "ended"],
        "{events:?}"
    );
    assert_eq!(
        events.last(),
        Some(&HelperEvent::Ended(Ending::AttemptsExhausted))
    );
    assert_eq!(MAX_ATTEMPTS, 2);
    for (n, answer) in [(0, "first"), (1, "second")] {
        let (index, user, cookie, got) = must(seen.recv_timeout(WAIT), "a conversation");
        assert_eq!(index, n);
        assert_eq!(user.as_deref(), Some(b"tester".as_slice()));
        assert_eq!(cookie.as_deref(), Some(COOKIE.as_bytes()));
        assert_eq!(got.as_deref(), Some(answer.as_bytes()));
    }
    assert_eq!(fixture.accepted(), 2, "exactly two conversations");
    assert_eq!(Ending::AttemptsExhausted.reply(), Some(Reply::Cancelled));
}

fn kind(event: &HelperEvent) -> &'static str {
    match event {
        HelperEvent::Prompt(_) => "prompt",
        HelperEvent::Notice(_) => "notice",
        HelperEvent::Failure => "failure",
        HelperEvent::Ended(_) => "ended",
    }
}

#[test]
fn a_correct_answer_ends_authenticated() {
    let fixture = Fixture::start(|_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let _ = receive(&mut socket);
        send(&mut socket, b"SUCCESS");
        until_closed(&mut socket);
    });
    let events = drive(&fixture, long_silence(), &["right"]);
    assert_eq!(
        events.last(),
        Some(&HelperEvent::Ended(Ending::Authenticated))
    );
    assert_eq!(Ending::Authenticated.reply(), Some(Reply::Normal));
}

/// N-1: a `TRACE` capture through a whole conversation (a wrong answer, a
/// second prompt, the right answer, `SUCCESS`) never sees the typed answers or
/// the cookie.
#[test]
fn a_full_conversation_never_logs_the_answers_or_the_cookie() {
    use crate::providers::polkit::testing::{capture_logs, logged};
    assert!(capture_logs(), "the log capture is in place");
    let fixture = Fixture::start(|_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_ERROR_MSG Authentication failure");
        send(&mut socket, b"FAILURE");
        until_closed(&mut socket);
    });
    let events = drive(&fixture, long_silence(), &["trace-marker-wrong-4471"]);
    assert!(!events.is_empty(), "the conversation ran");
    let fixture = Fixture::start(|_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let _ = receive(&mut socket);
        send(&mut socket, b"SUCCESS");
        until_closed(&mut socket);
    });
    let events = drive(&fixture, long_silence(), &["trace-marker-right-9382"]);
    assert_eq!(
        events.last(),
        Some(&HelperEvent::Ended(Ending::Authenticated))
    );
    for marker in ["trace-marker-wrong-4471", "trace-marker-right-9382", COOKIE] {
        assert!(!logged(marker), "{marker} was logged");
    }
}

#[test]
fn a_511_byte_secret_never_reaches_the_socket() {
    let (tx, got) = mpsc::channel();
    let fixture = Fixture::start(move |_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let _ = tx.send(receive(&mut socket));
        until_closed(&mut socket);
    });
    let mut secret = Secret::new();
    let results: Vec<_> = (0..511).map(|_| secret.push('x')).collect();
    assert!(results[..510].iter().all(Result::is_ok));
    assert!(results[510].is_err(), "the 511th byte is refused");
    assert!(secret.push('\n').is_err() && secret.push('\0').is_err());
    let (helper, events, _) = run_with::<Secret>(&fixture, long_silence());
    assert!(matches!(
        must(events.recv_timeout(WAIT), "the prompt"),
        HelperEvent::Prompt(_)
    ));
    must(helper.answer(secret), "the answer is taken");
    let line = must(got.recv_timeout(WAIT), "a line").unwrap_or_default();
    assert_eq!(
        line,
        vec![b'x'; 510],
        "510 bytes and the newline that ends them"
    );
}

#[test]
fn an_answer_with_no_open_prompt_is_not_written() {
    let (tx, got) = mpsc::channel();
    let fixture = Fixture::start(move |_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        // No prompt: the agent must not write anything now.
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
        let mut buf = [0u8; 16];
        let _ = tx.send(socket.read(&mut buf).unwrap_or(0));
        until_closed(&mut socket);
    });
    let mut conversation = must(fixture.connect("tester"), "connects");
    let mut exchange = Exchange::new();
    let (dropped, probe) = Probe::new(false);
    assert_eq!(conversation.answer(&mut exchange, probe), Ok(()));
    assert!(dropped.load(Ordering::Acquire), "the answer was dropped");
    assert_eq!(
        must(got.recv_timeout(WAIT), "the read"),
        0,
        "nothing arrived"
    );
    assert!(!exchange.has_answered());
}

// ---- the answer is dropped on every path ---------------------------------------------------------------

/// An answer that records being dropped and being written.
struct Probe {
    dropped: Arc<AtomicBool>,
    fails: bool,
}

impl Probe {
    fn new(fails: bool) -> (Arc<AtomicBool>, Probe) {
        let dropped = Arc::new(AtomicBool::new(false));
        let probe = Probe {
            dropped: Arc::clone(&dropped),
            fails,
        };
        (dropped, probe)
    }
}

impl Answer for Probe {
    fn write_line_to(&self, out: &mut dyn Write) -> std::io::Result<()> {
        if self.fails {
            return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        }
        out.write_all(b"probe\n")
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

fn eventually(mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    done()
}

#[test]
fn the_secret_is_dropped_after_the_write_and_on_every_error() {
    // Written: dropped.
    let fixture = speaking(vec![b"PAM_PROMPT_ECHO_OFF Password: ".to_vec()]);
    let (helper, events, _) = run_with::<Probe>(&fixture, timing(Duration::from_millis(200)));
    assert!(matches!(
        must(events.recv_timeout(WAIT), "the prompt"),
        HelperEvent::Prompt(_)
    ));
    let (written, probe) = Probe::new(false);
    must(helper.answer(probe), "taken");
    assert!(
        eventually(|| written.load(Ordering::Acquire)),
        "written, then dropped"
    );

    // A write that fails: dropped, and the conversation ends as end of input.
    let fixture = speaking(vec![b"PAM_PROMPT_ECHO_OFF Password: ".to_vec()]);
    let (helper, events, _) = run_with::<Probe>(&fixture, long_silence());
    assert!(matches!(
        must(events.recv_timeout(WAIT), "the prompt"),
        HelperEvent::Prompt(_)
    ));
    let (failed, probe) = Probe::new(true);
    must(helper.answer(probe), "taken");
    assert!(
        eventually(|| failed.load(Ordering::Acquire)),
        "dropped after a failed write"
    );
    assert_eq!(
        must(events.recv_timeout(WAIT), "the ending"),
        HelperEvent::Ended(Ending::HelperEof)
    );

    // A second answer while one is waiting: refused and dropped on the spot.
    let fixture = speaking(vec![b"PAM_NOTICE_FOR_NOTHING".to_vec()]);
    let (helper, _events, _) = run_with::<Probe>(&fixture, long_silence());
    thread::sleep(QUIET);
    // The thread has ended on the unknown line; an answer to it is refused.
    let (late, probe) = Probe::new(false);
    assert_eq!(helper.answer(probe), Err(AnswerRefused::Over));
    assert!(
        late.load(Ordering::Acquire),
        "an answer to a finished conversation is dropped"
    );
}

#[test]
fn the_answer_channel_has_one_slot() {
    // A prompt that is never answered keeps the thread waiting; the first
    // answer is taken into the slot only if the thread has not yet taken it,
    // so fill the slot while the thread is busy connecting.
    let (hold, held) = mpsc::channel::<()>();
    let fixture = speaking(Vec::new());
    let path = fixture.path.clone();
    let (tx, _events) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let helper = must(
        spawn_with::<Probe>(
            input(),
            move |event| tx.send(event).is_ok(),
            &shutdown,
            move |input| {
                // Busy until the test has filled the slot.
                let _ = held.recv_timeout(WAIT);
                connect_at(&path, input.user(), input.cookie(), trust(), long_silence())
            },
        ),
        "starts",
    );
    let (first_dropped, first) = Probe::new(false);
    let (second_dropped, second) = Probe::new(false);
    must(helper.answer(first), "the slot is free");
    assert_eq!(helper.answer(second), Err(AnswerRefused::Busy));
    assert!(
        second_dropped.load(Ordering::Acquire),
        "the refused answer is dropped"
    );
    assert!(
        !first_dropped.load(Ordering::Acquire),
        "the first waits in the slot"
    );
    helper.cancel();
    drop(hold);
    assert!(
        eventually(|| first_dropped.load(Ordering::Acquire)),
        "an answer left in the slot is dropped when the thread ends"
    );
}

#[test]
fn cancel_closes_the_socket_and_drops_the_answer() {
    let (tx, closed) = mpsc::channel();
    let fixture = Fixture::start(move |_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let _ = tx.send(until_closed(&mut socket));
    });
    let (helper, events, _) = run_with::<Probe>(&fixture, long_silence());
    assert!(matches!(
        must(events.recv_timeout(WAIT), "the prompt"),
        HelperEvent::Prompt(_)
    ));
    let (dropped, probe) = Probe::new(false);
    must(helper.answer(probe), "taken");
    helper.cancel();
    // The thread drops it; the loop that cancels does not wait for that.
    assert!(
        eventually(|| dropped.load(Ordering::Acquire)),
        "the answer is gone"
    );
    assert!(
        must(closed.recv_timeout(WAIT), "the fixture saw the end"),
        "the socket was closed"
    );
}

#[test]
fn cancel_while_waiting_for_the_person_closes_the_socket() {
    let (tx, closed) = mpsc::channel();
    let fixture = Fixture::start(move |_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let _ = tx.send(until_closed(&mut socket));
    });
    let run = start(&fixture);
    assert!(matches!(
        must(run.events.recv_timeout(WAIT), "the prompt"),
        HelperEvent::Prompt(_)
    ));
    run.helper.cancel();
    assert!(must(closed.recv_timeout(WAIT), "the fixture saw the end"));
}

#[test]
fn the_shells_shutdown_flag_stops_the_thread() {
    let (tx, closed) = mpsc::channel();
    let fixture = Fixture::start(move |_, mut socket| {
        let _ = receive(&mut socket);
        let _ = receive(&mut socket);
        send(&mut socket, b"PAM_PROMPT_ECHO_OFF Password: ");
        let _ = tx.send(until_closed(&mut socket));
    });
    let (_helper, events, shutdown) = run_with::<Secret>(&fixture, long_silence());
    assert!(matches!(
        must(events.recv_timeout(WAIT), "the prompt"),
        HelperEvent::Prompt(_)
    ));
    shutdown.store(true, Ordering::Release);
    assert!(
        must(closed.recv_timeout(WAIT), "the end"),
        "the socket was closed"
    );
}

#[test]
fn an_unreachable_helper_ends_failed_and_is_not_retried() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (tx, events) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let counted = Arc::clone(&calls);
    let _helper = must(
        spawn_with::<Secret>(
            input(),
            move |event| tx.send(event).is_ok(),
            &shutdown,
            move |_| {
                counted.fetch_add(1, Ordering::AcqRel);
                Err(ConnectRefusal::ListenerNotTrusted)
            },
        ),
        "starts",
    );
    assert_eq!(
        must(events.recv_timeout(WAIT), "the ending"),
        HelperEvent::Ended(Ending::HelperUnreachable)
    );
    assert_eq!(Ending::HelperUnreachable.reply(), Some(Reply::Failed));
    thread::sleep(QUIET);
    assert_eq!(calls.load(Ordering::Acquire), 1);
}

#[test]
fn the_helper_input_copies_the_requests_account_and_cookie() {
    let request = must(
        celestina_shell::polkit::Request::new(
            celestina_shell::polkit::RequestId::new(1),
            COOKIE,
            "org.example.act",
            "message",
            &[celestina_shell::polkit::Identity::User(1000)],
            1000,
            "root:x:0:0::/root:/bin/sh\ntester:x:1000:1000::/home/tester:/bin/sh\n",
        ),
        "a request",
    );
    let input = must(
        HelperInput::from_request(&request).ok_or("none"),
        "an input",
    );
    assert_eq!(input.user(), "tester");
    assert_eq!(input.cookie().as_bytes(), COOKIE.as_bytes());
}

/// N-22: the loop calls `cancel`, so it must return at once even when the
/// thread cannot stop for a while (a connect that does not come back), and
/// whatever the thread says afterwards is nobody's news.
#[test]
fn cancel_returns_at_once_and_a_stuck_thread_says_nothing_after_it() {
    let release = Arc::new(AtomicBool::new(false));
    let (tx, events) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let gate = Arc::clone(&release);
    let helper = must(
        spawn_with::<Secret>(
            input(),
            move |event| tx.send(event).is_ok(),
            &shutdown,
            move |_| {
                // Ignores every flag, as a blocked `connect` would.
                while !gate.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(ConnectRefusal::ListenerNotTrusted)
            },
        ),
        "starts",
    );
    thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    helper.cancel();
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "cancel waited {:?} for the thread",
        started.elapsed()
    );
    release.store(true, Ordering::Release);
    assert!(
        events.recv_timeout(QUIET).is_err(),
        "an abandoned conversation published its ending"
    );
}
