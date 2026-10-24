//! The agent against a private `dbus-daemon` with a fake polkitd and a fake
//! logind (design §10.1, plan Task 4). No test touches the session or the
//! system bus.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use celestina_shell::polkit::{Ending, Promotion, RegistrationState, Reply, RequestId, MAX_QUEUED};
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::MessageIterator;
use zbus::message::Type;
use zbus::names::BusName;

use super::testing::{
    self, answer_name, begin, cancel, capture_logs, client, eventually, fake_logind, logged,
    logged_line, unique, user, Behaviour, FakePolkitd, Hook, LogindCalls, Monitor, Seams, Service,
    SessionShape, DISPLAY, UID,
};
use super::{
    spawn, PolkitAgent, PolkitConfig, PolkitEvent, ReplyHandle, SystemBus, AGENT_INTERFACE,
    AGENT_PATH, ERROR_ACCESS_DENIED, ERROR_CANCELLED, ERROR_FAILED, POLKIT_NAME,
};
use crate::providers::Reaper;
use crate::tray::test_bus::PrivateBus;

const WAIT: Duration = Duration::from_secs(5);
/// How long "nothing happens" is watched for.
const QUIET: Duration = Duration::from_millis(400);

/// One agent on a private bus with its fakes. Fields drop in order: the
/// agent first, then the fakes, then the bus.
struct Harness {
    agent: Option<PolkitAgent>,
    events: Receiver<PolkitEvent>,
    polkitd: Option<FakePolkitd>,
    _logind: Service,
    logind_calls: LogindCalls,
    /// Watches the bus when the test asks to see the agent's own calls.
    monitor: Option<Monitor>,
    bus: PrivateBus,
}

struct Options {
    polkitd: Option<bool>,
    /// A polkitd that behaves in a chosen way instead of `polkitd`.
    behaviour: Option<Behaviour>,
    shape: SessionShape,
    hint: Option<String>,
    hook: Option<Hook>,
    monitor: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            polkitd: Some(true),
            behaviour: None,
            shape: SessionShape::default(),
            hint: None,
            hook: None,
            monitor: false,
        }
    }
}

impl Harness {
    /// `None` (the test is skipped) where `dbus-daemon` is not installed.
    fn start(name: &str, options: Options) -> Option<Harness> {
        let bus = PrivateBus::start(&format!("polkit-{name}"))?;
        let (logind, logind_calls) = fake_logind(&bus, options.shape);
        let polkitd = match options.behaviour {
            Some(behaviour) => Some(FakePolkitd::behaving(&bus, behaviour)),
            None => options
                .polkitd
                .map(|accept| FakePolkitd::start(&bus, accept)),
        };
        let monitor = options.monitor.then(|| Monitor::start(&bus));
        let mut environment = testing::environment();
        environment.session_hint = options.hint;
        let config = PolkitConfig {
            bus: SystemBus::Address(bus.address().to_owned()),
            after: Reaper::default(),
            seams: Seams {
                environment: Some(environment),
                hook: options.hook,
            },
        };
        let (tx, events) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let agent = match spawn(config, move |event| tx.send(event).is_ok(), &shutdown) {
            Ok(agent) => agent,
            Err(err) => panic!("the agent starts: {err}"),
        };
        Some(Harness {
            agent: Some(agent),
            events,
            polkitd,
            _logind: logind,
            logind_calls,
            monitor,
            bus,
        })
    }

    /// A harness whose agent registered.
    fn registered(name: &str, options: Options) -> Option<Harness> {
        let harness = Harness::start(name, options)?;
        assert!(
            harness.until(|e| *e == PolkitEvent::Registration(RegistrationState::Registered)),
            "the agent registers"
        );
        Some(harness)
    }

    fn agent(&self) -> &PolkitAgent {
        match &self.agent {
            Some(agent) => agent,
            None => panic!("the agent was already joined"),
        }
    }

    fn polkitd(&self) -> &FakePolkitd {
        match &self.polkitd {
            Some(polkitd) => polkitd,
            None => panic!("this harness has no polkitd"),
        }
    }

    /// Waits for an event matching `want`, skipping others.
    fn until(&self, want: impl Fn(&PolkitEvent) -> bool) -> bool {
        let until = Instant::now() + WAIT;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            match self.events.recv_timeout(left) {
                Ok(event) if want(&event) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    }

    /// The id of the next admitted request.
    fn admitted(&self) -> RequestId {
        let until = Instant::now() + WAIT;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            match self.events.recv_timeout(left) {
                Ok(PolkitEvent::Admitted(id)) => return id,
                Ok(_) => {}
                Err(_) => break,
            }
        }
        panic!("no request was admitted")
    }

    /// Whether no event arrives for [`QUIET`].
    fn quiet(&self) -> bool {
        self.events.recv_timeout(QUIET).is_err()
    }

    /// Begins a request from the fake polkitd with `cookie`, for the person's
    /// own account.
    fn begin(&self, cookie: &str) -> Receiver<zbus::Result<zbus::Message>> {
        let polkitd = self.polkitd();
        begin(
            &polkitd.service.connection,
            &polkitd.agent(WAIT),
            "org.example.act",
            "Do the thing",
            cookie.to_owned(),
            user(UID),
        )
    }
}

fn outcome(call: &Receiver<zbus::Result<zbus::Message>>) -> String {
    match call.recv_timeout(WAIT) {
        Ok(outcome) => answer_name(&outcome),
        Err(_) => "no answer".to_owned(),
    }
}

// ---- N-12: who may call, and who keeps the owner cache ------------------------------------

#[test]
fn a_fake_caller_gets_access_denied_and_nothing_else_happens() {
    assert!(capture_logs(), "the log capture is in place");
    let Some(h) = Harness::registered("fake-caller", Options::default()) else {
        return;
    };
    let agent = h.polkitd().agent(WAIT);
    let caller = client(&h.bus, WAIT);
    let forged = begin(
        &caller,
        &agent,
        "org.example.forgedmarkeraction",
        "forged-message-marker",
        "forged-cookie-marker".to_owned(),
        user(UID),
    );
    assert_eq!(outcome(&forged), ERROR_ACCESS_DENIED);
    assert_eq!(
        answer_name(&cancel(&caller, &agent, "forged-cookie-marker")),
        ERROR_ACCESS_DENIED
    );
    assert!(h.quiet(), "nothing reached the loop");
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Idle);
    for marker in [
        "forged-cookie-marker",
        "forged-message-marker",
        "forgedmarkeraction",
    ] {
        assert!(!logged(marker), "{marker} was logged");
    }
    // The refusal itself is logged, so the capture did see this agent's bus
    // thread (its lines carry its own bus's address).
    assert!(logged_line(&[
        h.bus.address(),
        "from a peer that is not polkitd is refused"
    ]));
}

/// F-1 for the cancellation: a peer that knows a cookie that is really pending
/// still cannot end the request with it.
#[test]
fn a_forged_cancel_with_a_pending_cookie_is_denied_and_the_request_survives() {
    let Some(h) = Harness::registered("forged-cancel", Options::default()) else {
        return;
    };
    let call = h.begin("cookie-really-pending");
    let id = h.admitted();
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Show(id));
    let agent = h.polkitd().agent(WAIT);
    let caller = client(&h.bus, WAIT);
    assert_eq!(
        answer_name(&cancel(&caller, &agent, "cookie-really-pending")),
        ERROR_ACCESS_DENIED
    );
    assert!(h.quiet(), "the forged cancel ended nothing");
    assert!(
        call.recv_timeout(QUIET).is_err(),
        "the request was answered"
    );
    // It is still the active request: only its own ending answers it.
    assert!(h
        .agent()
        .end_active(id, Ending::Escaped, false, Instant::now())
        .is_some());
    assert_eq!(outcome(&call), ERROR_CANCELLED);
}

#[test]
fn a_unicast_forged_name_owner_changed_leaves_the_cache_unchanged() {
    let Some(h) = Harness::registered("forged-signal", Options::default()) else {
        return;
    };
    let agent = h.polkitd().agent(WAIT);
    let forger = client(&h.bus, WAIT);
    let forger_name = unique(&forger);
    let body = (POLKIT_NAME, h.polkitd().unique(), forger_name.as_str());
    for destination in [Some(agent.as_str()), None] {
        if let Err(err) = forger.emit_signal(
            destination,
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
            &body,
        ) {
            panic!("the forged signal is sent: {err}");
        }
    }
    // The forger's call follows its signals, so the agent read them first:
    // the forger did not become polkitd.
    let forged = begin(
        &forger,
        &agent,
        "org.example.act",
        "Do the thing",
        "forged".to_owned(),
        user(UID),
    );
    assert_eq!(outcome(&forged), ERROR_ACCESS_DENIED);
    assert!(h.quiet(), "no teardown and no new registration");
    assert_eq!(h.polkitd().log().registrations.len(), 1);
    // The real polkitd still reaches the loop.
    let _call = h.begin("real-cookie");
    let _id = h.admitted();
}

// ---- requests -----------------------------------------------------------------------------

#[test]
fn begin_from_the_owner_reaches_the_loop() {
    let Some(h) = Harness::registered("begin", Options::default()) else {
        return;
    };
    let polkitd = h.polkitd();
    let call = begin(
        &polkitd.service.connection,
        &polkitd.agent(WAIT),
        "org.freedesktop.NetworkManager.network-control",
        "Do\u{202e} the\nthing",
        "cookie-begin".to_owned(),
        user(UID),
    );
    let id = h.admitted();
    let agent = h.agent();
    assert_eq!(agent.promote(Instant::now()), Promotion::Show(id));
    let Some(prompt) = agent.prompt(id) else {
        panic!("the active request has a prompt");
    };
    assert_eq!(
        prompt.action_id,
        "org.freedesktop.NetworkManager.network-control"
    );
    assert_eq!(prompt.account, "tester");
    assert!(!prompt.message.contains('\u{202e}') && !prompt.message.contains('\n'));
    assert!(prompt.message.contains("the"));
    assert_eq!(agent.prompt(RequestId::new(id.get() + 1)), None);
    assert!(agent
        .end_active(id, Ending::Escaped, false, Instant::now())
        .is_some());
    assert_eq!(outcome(&call), ERROR_CANCELLED);
}

/// N-1: a request that is admitted, shown and ended leaves its cookie out of
/// the log at every level (the capture is `TRACE`).
#[test]
fn the_cookie_of_a_real_request_is_never_logged() {
    assert!(capture_logs(), "the log capture is in place");
    let Some(h) = Harness::registered("cookie-log", Options::default()) else {
        return;
    };
    let call = h.begin("cookie-trace-marker-5528");
    let id = h.admitted();
    let agent = h.agent();
    assert_eq!(agent.promote(Instant::now()), Promotion::Show(id));
    assert!(agent.helper_input(id).is_some());
    assert!(agent
        .end_active(id, Ending::Escaped, false, Instant::now())
        .is_some());
    assert_eq!(outcome(&call), ERROR_CANCELLED);
    assert!(!logged("cookie-trace-marker-5528"), "the cookie was logged");
}

#[test]
fn the_active_request_gives_the_helper_its_account_and_cookie() {
    let Some(h) = Harness::registered("helper-input", Options::default()) else {
        return;
    };
    let call = h.begin("cookie-helper-input");
    let id = h.admitted();
    let agent = h.agent();
    assert!(
        agent.helper_input(id).is_none(),
        "a request that is only waiting has no conversation"
    );
    assert_eq!(agent.promote(Instant::now()), Promotion::Show(id));
    let Some(input) = agent.helper_input(id) else {
        panic!("the active request has a helper input");
    };
    assert_eq!(input.user(), "tester");
    assert_eq!(input.cookie().as_bytes(), b"cookie-helper-input");
    assert!(agent.helper_input(RequestId::new(id.get() + 1)).is_none());
    assert!(agent
        .end_active(id, Ending::Escaped, false, Instant::now())
        .is_some());
    assert!(
        agent.helper_input(id).is_none(),
        "an ended request has none"
    );
    assert_eq!(outcome(&call), ERROR_CANCELLED);
}

#[test]
fn only_the_helpers_success_answers_begin_normally() {
    let Some(h) = Harness::registered("success", Options::default()) else {
        return;
    };
    let agent = h.agent();
    for (cookie, ending, expected) in [
        ("cookie-ok", Ending::Authenticated, "normal"),
        ("cookie-fault", Ending::HelperProtocol, ERROR_FAILED),
        (
            "cookie-attempts",
            Ending::AttemptsExhausted,
            ERROR_CANCELLED,
        ),
    ] {
        let call = h.begin(cookie);
        let id = h.admitted();
        // The regrab gap holds the next prompt back; promote until shown.
        assert!(eventually(WAIT, || agent.promote(Instant::now())
            == Promotion::Show(id)));
        assert!(agent.end_active(id, ending, true, Instant::now()).is_some());
        assert_eq!(outcome(&call), expected, "{ending:?}");
    }
}

#[test]
fn refusals_are_answered_at_once_and_never_reach_the_loop() {
    let Some(h) = Harness::registered("refusals", Options::default()) else {
        return;
    };
    let polkitd = h.polkitd();
    let connection = &polkitd.service.connection;
    let agent = polkitd.agent(WAIT);
    // F-2: a bad cookie, a bad action id, a body of the wrong shape.
    let bad_cookie = begin(
        connection,
        &agent,
        "org.example.act",
        "m",
        "a\ncookie".into(),
        user(UID),
    );
    assert_eq!(outcome(&bad_cookie), ERROR_FAILED);
    let bad_action = begin(
        connection,
        &agent,
        "org.example act",
        "m",
        "c1".into(),
        user(UID),
    );
    assert_eq!(outcome(&bad_action), ERROR_FAILED);
    let shape = connection.call_method(
        Some(agent.as_str()),
        AGENT_PATH,
        Some(AGENT_INTERFACE),
        "BeginAuthentication",
        &("only one string",),
    );
    assert_eq!(answer_name(&shape), ERROR_FAILED);
    // F-4: not the person's own account.
    let other = begin(
        connection,
        &agent,
        "org.example.act",
        "m",
        "c2".into(),
        user(0),
    );
    assert_eq!(outcome(&other), ERROR_FAILED);
    assert!(h.quiet());
    // F-3: the second call with a pending cookie is cancelled, the first
    // continues.
    let _first = h.begin("dup");
    let first = h.admitted();
    let second = h.begin("dup");
    assert_eq!(outcome(&second), ERROR_CANCELLED);
    // F-5: the active slot and MAX_QUEUED waiting; the next is refused.
    let mut pending = Vec::new();
    for n in 0..MAX_QUEUED {
        pending.push(h.begin(&format!("queued-{n}")));
        let _ = h.admitted();
    }
    let full = h.begin("one-too-many");
    assert_eq!(outcome(&full), ERROR_CANCELLED);
    assert!(h.quiet());
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Show(first));
}

#[test]
fn cancel_is_seen_while_a_begin_reply_is_pending() {
    let Some(h) = Harness::registered("cancel", Options::default()) else {
        return;
    };
    let call = h.begin("cookie-cancel");
    let id = h.admitted();
    let agent = h.agent();
    assert_eq!(agent.promote(Instant::now()), Promotion::Show(id));
    let polkitd = h.polkitd();
    let cancelled = cancel(
        &polkitd.service.connection,
        &polkitd.agent(WAIT),
        "cookie-cancel",
    );
    assert_eq!(answer_name(&cancelled), "normal");
    assert_eq!(outcome(&call), ERROR_CANCELLED);
    assert!(h.until(|e| *e
        == PolkitEvent::Ended {
            id,
            ending: Ending::CancelledByPolkit
        }));
    // Answered once: the loop's own ending finds nothing left.
    assert_eq!(
        agent.end_active(id, Ending::Escaped, false, Instant::now()),
        None
    );
    // F-22: an unknown or invalid cookie is answered normally and ignored.
    for cookie in ["never-seen", "bad\ncookie"] {
        let answer = cancel(&polkitd.service.connection, &polkitd.agent(WAIT), cookie);
        assert_eq!(answer_name(&answer), "normal");
    }
    assert!(h.quiet());
}

#[test]
fn a_cancel_removes_a_waiting_request_without_showing_it() {
    let Some(h) = Harness::registered("cancel-waiting", Options::default()) else {
        return;
    };
    let _active = h.begin("active");
    let active = h.admitted();
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Show(active));
    let waiting_call = h.begin("waiting");
    let waiting = h.admitted();
    let polkitd = h.polkitd();
    let _ = cancel(&polkitd.service.connection, &polkitd.agent(WAIT), "waiting");
    assert_eq!(outcome(&waiting_call), ERROR_CANCELLED);
    assert!(h.until(|e| *e
        == PolkitEvent::Ended {
            id: waiting,
            ending: Ending::CancelledByPolkit
        }));
    assert!(h
        .agent()
        .end_active(active, Ending::Escaped, false, Instant::now())
        .is_some());
    // Nothing else waits.
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Idle);
}

// ---- F-8: polkitd's owner ------------------------------------------------------------------

#[test]
fn an_owner_change_is_seen_while_a_begin_reply_is_pending() {
    let Some(h) = Harness::registered("owner-change", Options::default()) else {
        return;
    };
    let call = h.begin("cookie-owner");
    let id = h.admitted();
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Show(id));
    if let Err(err) = h.polkitd().service.connection.release_name(POLKIT_NAME) {
        panic!("the fake polkitd releases its name: {err}");
    }
    assert!(h.until(|e| *e
        == PolkitEvent::Ended {
            id,
            ending: Ending::OwnerChanged
        }));
    assert!(h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Waiting)));
    // No reply to the old owner.
    assert!(
        call.recv_timeout(QUIET).is_err(),
        "the old owner got a reply"
    );
    assert_eq!(
        h.agent()
            .end_active(id, Ending::Escaped, false, Instant::now()),
        None
    );
}

#[test]
fn a_restart_of_the_fake_polkitd_emits_owner_changed_and_registers_once() {
    let Some(mut h) = Harness::registered("restart", Options::default()) else {
        return;
    };
    let _call = h.begin("cookie-restart");
    let id = h.admitted();
    if let Some(mut old) = h.polkitd.take() {
        assert_eq!(old.log().registrations.len(), 1);
        old.service.stop();
    }
    assert!(h.until(|e| *e
        == PolkitEvent::Ended {
            id,
            ending: Ending::OwnerChanged
        }));
    assert!(h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Waiting)));
    h.polkitd = Some(FakePolkitd::start(&h.bus, true));
    assert!(h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Registered)));
    assert!(h.quiet());
    let log = h.polkitd().log();
    assert_eq!(
        log.registrations.len(),
        1,
        "registered once with the new owner"
    );
    assert_eq!(log.members, ["RegisterAuthenticationAgent"]);
}

#[test]
fn polkitd_absent_at_start_waits_and_registers_once_when_it_appears() {
    let options = Options {
        polkitd: None,
        ..Options::default()
    };
    let Some(mut h) = Harness::start("absent", options) else {
        return;
    };
    assert!(h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Waiting)));
    h.polkitd = Some(FakePolkitd::start(&h.bus, true));
    assert!(h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Registered)));
    assert!(h.quiet());
    assert_eq!(h.polkitd().log().registrations.len(), 1);
}

// ---- N-14, F-24: registration ----------------------------------------------------------------

#[test]
fn a_refused_registration_is_refused_and_never_retried() {
    let options = Options {
        polkitd: Some(false),
        ..Options::default()
    };
    let Some(mut h) = Harness::start("refused", options) else {
        return;
    };
    assert!(h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Refused)));
    assert_eq!(h.polkitd().log().registrations.len(), 1);
    // A new polkitd that would accept is not tried: only the person retries.
    if let Some(mut old) = h.polkitd.take() {
        old.service.stop();
    }
    h.polkitd = Some(FakePolkitd::start(&h.bus, true));
    assert!(!eventually(Duration::from_millis(800), || !h
        .polkitd()
        .log()
        .members
        .is_empty()));
    assert!(!h.until(|e| matches!(e, PolkitEvent::Registration(_))));
}

/// Registering may start polkitd itself (bus activation), so the call has a
/// long timeout of its own: an answer after three and a half seconds is still an answer,
/// and a timeout is neither "absent" nor retried.
#[test]
fn a_slow_registration_is_waited_for_and_not_given_up_on() {
    let options = Options {
        behaviour: Some(Behaviour {
            register_delay: Duration::from_millis(3500),
            ..Behaviour::accepting()
        }),
        ..Options::default()
    };
    let Some(h) = Harness::start("slow-register", options) else {
        return;
    };
    assert!(
        h.until(|e| matches!(e, PolkitEvent::Registration(RegistrationState::Registered))),
        "the registration that took three and a half seconds counts"
    );
    assert_eq!(h.polkitd().log().registrations.len(), 1, "asked once");
}

/// The long timeout holds nothing up: stopping the agent closes the
/// connection, which ends the registration call in flight.
#[test]
fn stopping_during_a_registration_in_flight_is_not_held_up_by_its_timeout() {
    let options = Options {
        behaviour: Some(Behaviour {
            register_delay: Duration::from_secs(30),
            ..Behaviour::accepting()
        }),
        ..Options::default()
    };
    let Some(mut h) = Harness::start("stop-while-registering", options) else {
        return;
    };
    // The registration call is in flight once polkitd's name is known.
    assert!(h.until(|e| matches!(e, PolkitEvent::Registration(RegistrationState::Waiting))));
    thread::sleep(Duration::from_millis(300));
    let Some(agent) = h.agent.take() else {
        panic!("the agent runs");
    };
    let started = Instant::now();
    assert!(agent.join_within(Duration::from_secs(2)));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

/// Only polkit's own "already exists" means another agent holds the session;
/// any other error is a fault of the registration, said as such.
#[test]
fn a_registration_error_that_is_not_another_agent_is_unavailable_not_refused() {
    let options = Options {
        behaviour: Some(Behaviour {
            register_error: Some((
                "org.freedesktop.PolicyKit1.Error.Failed",
                "The session-id passed is not for the caller's session",
            )),
            ..Behaviour::accepting()
        }),
        ..Options::default()
    };
    let Some(h) = Harness::start("register-fault", options) else {
        return;
    };
    assert!(
        h.until(|e| matches!(
            e,
            PolkitEvent::Registration(RegistrationState::Unavailable(why))
                if why.contains("not for the caller")
        )),
        "the fault is named, not called a refusal"
    );
    assert_eq!(h.polkitd().log().registrations.len(), 1, "never retried");
}

/// polkitd's error text reaches the journal and the duty's state: whatever a
/// stranger could have put in it, no control or direction character survives
/// (spec M-6; the same set the card's text is cleaned of).
#[test]
fn a_registration_error_text_is_cleaned_of_control_characters() {
    let options = Options {
        behaviour: Some(Behaviour {
            register_error: Some((
                "org.freedesktop.PolicyKit1.Error.Failed",
                "\u{1b}[31mnot for the caller\nFORGED: registered\r\u{202e}\u{7}done",
            )),
            ..Behaviour::accepting()
        }),
        ..Options::default()
    };
    let Some(h) = Harness::start("register-fault-controls", options) else {
        return;
    };
    let said = std::cell::RefCell::new(None);
    assert!(
        h.until(|e| match e {
            PolkitEvent::Registration(RegistrationState::Unavailable(why)) => {
                *said.borrow_mut() = Some(why.clone());
                true
            }
            _ => false,
        }),
        "the fault is reported"
    );
    let Some(said) = said.into_inner() else {
        panic!("a reason was reported");
    };
    assert!(said.contains("not for the caller"), "{said:?}");
    assert!(
        !said.chars().any(|c| c.is_control() || c == '\u{202e}'),
        "{said:?}"
    );
}

#[test]
fn session_comes_from_user_display_when_get_session_by_pid_fails() {
    let Some(h) = Harness::registered("session", Options::default()) else {
        return;
    };
    let log = h.polkitd().log();
    assert_eq!(
        log.registrations,
        [(
            DISPLAY.to_owned(),
            "es_ES.UTF-8".to_owned(),
            AGENT_PATH.to_owned()
        )]
    );
    // N-14: the one registration call, never the options variant.
    assert_eq!(log.members, ["RegisterAuthenticationAgent"]);
    let calls = match h.logind_calls.lock() {
        Ok(calls) => calls.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    assert!(calls.iter().any(|c| c == "GetUser"));
    assert!(!calls.iter().any(|c| c == "GetSessionByPID"));
}

#[test]
fn a_session_that_is_not_this_persons_local_wayland_one_makes_the_agent_unavailable() {
    for (name, shape, hint) in [
        (
            "x11",
            SessionShape {
                kind: "x11",
                ..SessionShape::default()
            },
            None,
        ),
        (
            "remote",
            SessionShape {
                remote: true,
                ..SessionShape::default()
            },
            None,
        ),
        (
            "other-user",
            SessionShape {
                uid: 0,
                ..SessionShape::default()
            },
            None,
        ),
        ("hint", SessionShape::default(), Some("99".to_owned())),
    ] {
        let options = Options {
            shape,
            hint,
            ..Options::default()
        };
        let Some(h) = Harness::start(&format!("unusable-{name}"), options) else {
            return;
        };
        assert!(
            h.until(|e| matches!(
                e,
                PolkitEvent::Registration(RegistrationState::Unavailable(why))
                    if why.contains("session")
            )),
            "{name}"
        );
        // The reason is not replaced by a generic one when the thread ends.
        assert!(h.quiet(), "{name}");
        assert!(h.polkitd().log().registrations.is_empty(), "{name}");
    }
}

// ---- N-13: nothing else is answered ----------------------------------------------------------

#[test]
fn introspection_and_other_interfaces_are_not_answered() {
    let Some(h) = Harness::registered("introspect", Options::default()) else {
        return;
    };
    let agent = h.polkitd().agent(WAIT);
    let caller = client(&h.bus, Duration::from_millis(300));
    let calls: [(&str, &str, &str); 5] = [
        (
            AGENT_PATH,
            "org.freedesktop.DBus.Introspectable",
            "Introspect",
        ),
        ("/", "org.freedesktop.DBus.Introspectable", "Introspect"),
        (AGENT_PATH, "org.freedesktop.DBus.Peer", "Ping"),
        (AGENT_PATH, "org.freedesktop.DBus.Properties", "GetAll"),
        ("/elsewhere", AGENT_INTERFACE, "BeginAuthentication"),
    ];
    for (path, interface, member) in calls {
        let answer = caller.call_method(Some(agent.as_str()), path, Some(interface), member, &());
        assert_eq!(
            answer_name(&answer),
            "timeout",
            "{interface}.{member} at {path}"
        );
    }
    // The agent's connection holds no well-known name.
    let dbus = match DBusProxy::new(&caller) {
        Ok(dbus) => dbus,
        Err(err) => panic!("a bus proxy: {err}"),
    };
    let names = match dbus.list_names() {
        Ok(names) => names,
        Err(err) => panic!("the bus lists its names: {err}"),
    };
    for name in names.iter().filter(|n| !n.as_str().starts_with(':')) {
        let owner = BusName::try_from(name.as_str())
            .ok()
            .and_then(|n| dbus.get_name_owner(n).ok());
        assert_ne!(
            owner.map(|o| o.as_str().to_owned()),
            Some(agent.clone()),
            "{name}"
        );
    }
}

// ---- N-23: the reply handle ------------------------------------------------------------------

#[test]
fn dropping_a_reply_handle_replies_cancelled() {
    let Some(bus) = PrivateBus::start("polkit-reply-handle") else {
        return;
    };
    let server = client(&bus, WAIT);
    let messages = MessageIterator::from(&server);
    let mut messages = messages.filter_map(Result::ok);
    let caller = client(&bus, Duration::from_millis(500));
    let server_name = unique(&server);
    let mut next_call = |caller: &zbus::blocking::Connection| {
        let (done, answer) = mpsc::channel();
        let caller = caller.clone();
        let server_name = server_name.clone();
        std::thread::spawn(move || {
            let _ = done.send(caller.call_method(
                Some(server_name.as_str()),
                "/x",
                Some("org.example.Agent"),
                "Begin",
                &(),
            ));
        });
        let call = messages
            .find(|m| m.header().message_type() == Type::MethodCall)
            .map(|call| ReplyHandle::new(server.clone(), call));
        match call {
            Some(handle) => (handle, answer),
            None => panic!("the call arrives"),
        }
    };
    let (dropped, answer) = next_call(&caller);
    drop(dropped);
    assert_eq!(outcome(&answer), ERROR_CANCELLED);
    let (answered, answer) = next_call(&caller);
    answered.reply(Reply::Normal);
    assert_eq!(outcome(&answer), "normal");
    let (abandoned, answer) = next_call(&caller);
    abandoned.abandon();
    assert_eq!(outcome(&answer), "timeout");
}

// ---- F-23, F-9: the ways out -------------------------------------------------------------------

#[test]
fn a_worker_panic_drops_the_connection() {
    let hook: Hook = Arc::new(|member: &str| {
        if member == "CancelAuthentication" {
            std::panic::panic_any("a bug in the bus thread, on purpose");
        }
    });
    let options = Options {
        hook: Some(hook),
        ..Options::default()
    };
    let Some(h) = Harness::registered("panic", options) else {
        return;
    };
    let polkitd = h.polkitd();
    let agent = polkitd.agent(WAIT);
    let call = h.begin("cookie-panic");
    let id = h.admitted();
    // The loop still holds the request: only the guard can end it.
    let _ = cancel(&polkitd.service.connection, &agent, "cookie-panic");
    let watcher = client(&h.bus, WAIT);
    let dbus = match DBusProxy::new(&watcher) {
        Ok(dbus) => dbus,
        Err(err) => panic!("a bus proxy: {err}"),
    };
    let gone = eventually(WAIT, || {
        BusName::try_from(agent.as_str())
            .ok()
            .is_some_and(|name| dbus.name_has_owner(name) == Ok(false))
    });
    assert!(gone, "the agent's unique name vanished");
    assert!(h.until(|e| *e
        == PolkitEvent::Ended {
            id,
            ending: Ending::WorkerPanic
        }));
    // The bus tells the caller that the agent went away without answering.
    assert_eq!(outcome(&call), "org.freedesktop.DBus.Error.NoReply");
    assert_eq!(
        h.agent()
            .end_active(id, Ending::Authenticated, true, Instant::now()),
        None
    );
}

#[test]
fn shutdown_joins_within_the_bound() {
    let Some(mut h) = Harness::registered("shutdown", Options::default()) else {
        return;
    };
    let agent_name = h.polkitd().agent(WAIT);
    let call = h.begin("cookie-exit");
    let id = h.admitted();
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Show(id));
    let Some(agent) = h.agent.take() else {
        panic!("the agent runs");
    };
    let started = Instant::now();
    assert!(agent.join_within(Duration::from_secs(2)));
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(outcome(&call), ERROR_CANCELLED);
    let log = h.polkitd().log();
    assert_eq!(log.unregistrations, [DISPLAY]);
    assert_eq!(
        log.members,
        [
            "RegisterAuthenticationAgent",
            "UnregisterAuthenticationAgent"
        ]
    );
    assert!(h.until(|e| *e
        == PolkitEvent::Ended {
            id,
            ending: Ending::ShellExit
        }));
    assert!(h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Off)));
    let watcher = client(&h.bus, WAIT);
    let gone = match DBusProxy::new(&watcher) {
        Ok(dbus) => eventually(WAIT, || {
            BusName::try_from(agent_name.as_str())
                .ok()
                .is_some_and(|name| dbus.name_has_owner(name) == Ok(false))
        }),
        Err(err) => panic!("a bus proxy: {err}"),
    };
    assert!(gone, "the agent's connection is closed");
}

// ---- N-22 and the ids: the loop never waits, and nothing is reused ----------------------------

/// Another agent on `bus`, which registers once `after` is over.
fn agent_after(bus: &PrivateBus, after: Reaper) -> (PolkitAgent, Receiver<PolkitEvent>) {
    let config = PolkitConfig {
        bus: SystemBus::Address(bus.address().to_owned()),
        after,
        seams: Seams {
            environment: Some(testing::environment()),
            hook: None,
        },
    };
    let (tx, events) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    match spawn(config, move |event| tx.send(event).is_ok(), &shutdown) {
        Ok(agent) => (agent, events),
        Err(err) => panic!("the next agent starts: {err}"),
    }
}

#[test]
fn retiring_returns_at_once_and_the_next_agent_registers_only_after_it_is_gone() {
    let options = Options {
        behaviour: Some(Behaviour {
            strict: true,
            unregister_delay: Duration::from_millis(1500),
            ..Behaviour::accepting()
        }),
        ..Options::default()
    };
    let Some(mut h) = Harness::registered("retire", options) else {
        return;
    };
    let call = h.begin("cookie-retire");
    let _id = h.admitted();
    let Some(agent) = h.agent.take() else {
        panic!("the agent runs");
    };

    // The loop's side: polkitd takes 1.5 s to answer the unregistration, and
    // the call returns at once all the same.
    let started = Instant::now();
    let mut reaper = agent.retire();
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "retire waited {:?} for the agent",
        started.elapsed()
    );

    // The next agent is started straight away; it must not register while the
    // old one still holds the session, or the strict polkitd refuses it.
    let (second, events) = agent_after(&h.bus, std::mem::take(&mut reaper));
    let registered = eventually(WAIT, || h.polkitd().log().members.len() == 3);
    assert!(registered, "{:?}", h.polkitd().log());
    let log = h.polkitd().log();
    assert_eq!(
        log.members,
        [
            "RegisterAuthenticationAgent",
            "UnregisterAuthenticationAgent",
            "RegisterAuthenticationAgent"
        ],
        "the second registration came after the first one's unregistration"
    );
    assert!(log.holder.is_some(), "the second agent holds the session");
    assert_eq!(outcome(&call), ERROR_CANCELLED);
    assert!(eventually(WAIT, || events.try_iter().any(
        |e| e == PolkitEvent::Registration(RegistrationState::Registered)
    )));
    assert!(second.join_within(Duration::from_secs(5)));
}

/// I-1: the chain of reapers is transitive. A's teardown is slow (the strict
/// polkitd answers its unregistration late), B is started behind it and
/// switched off at once with a short bound on its reaper, and C is started
/// behind B's. Were B's reaper to end after its own bound alone, C would
/// register while A still holds the session and the strict polkitd would
/// refuse it. B's reaper waits for A's too, so C registers once A is gone.
#[test]
fn three_quick_switches_over_a_slow_unregistration_end_with_the_last_registered() {
    let options = Options {
        behaviour: Some(Behaviour {
            strict: true,
            unregister_delay: Duration::from_millis(1500),
            ..Behaviour::accepting()
        }),
        ..Options::default()
    };
    let Some(mut h) = Harness::registered("chain", options) else {
        return;
    };
    let Some(first) = h.agent.take() else {
        panic!("the agent runs");
    };

    // A: off. Its reaper is given the time polkitd needs.
    let after_first = first.retire_within(Duration::from_secs(4));
    // B: on behind A, and off again at once, with a bound far shorter than
    // A's teardown.
    let (second, second_events) = agent_after(&h.bus, after_first);
    let after_second = second.retire_within(Duration::from_millis(300));
    // C: on behind B.
    let (third, third_events) = agent_after(&h.bus, after_second);

    let outcome = eventually(WAIT, || {
        third_events.try_iter().any(|e| {
            matches!(
                e,
                PolkitEvent::Registration(
                    RegistrationState::Registered | RegistrationState::Refused
                )
            )
        })
    });
    assert!(
        outcome,
        "the third agent reported where its registration stands"
    );
    let log = h.polkitd().log();
    assert_eq!(
        log.members,
        [
            "RegisterAuthenticationAgent",
            "UnregisterAuthenticationAgent",
            "RegisterAuthenticationAgent"
        ],
        "B never registered and C came after A's unregistration: {log:?}"
    );
    assert!(log.holder.is_some(), "the third agent holds the session");
    assert!(
        second_events
            .try_iter()
            .all(|e| e != PolkitEvent::Registration(RegistrationState::Registered)),
        "the agent switched off at once never registered"
    );
    assert!(third.join_within(Duration::from_secs(5)));
}

#[test]
fn request_ids_are_never_reused_by_a_later_agent() {
    let mut seen = Vec::new();
    for name in ["ids-a", "ids-b"] {
        let Some(h) = Harness::registered(name, Options::default()) else {
            return;
        };
        let _call = h.begin("cookie-ids");
        seen.push(h.admitted());
    }
    assert_ne!(seen[0], seen[1], "{seen:?}");
}

#[test]
fn ending_a_request_that_is_not_the_active_one_ends_nothing() {
    let Some(h) = Harness::registered("end-names-id", Options::default()) else {
        return;
    };
    let call = h.begin("cookie-named");
    let id = h.admitted();
    let agent = h.agent();
    assert_eq!(agent.promote(Instant::now()), Promotion::Show(id));
    let stranger = RequestId::new(id.get() + 1);
    assert_eq!(
        agent.end_active(stranger, Ending::Authenticated, true, Instant::now()),
        None
    );
    assert!(
        call.recv_timeout(QUIET).is_err(),
        "a request that was not the one named was answered"
    );
    assert!(agent
        .end_active(id, Ending::Escaped, false, Instant::now())
        .is_some());
    assert_eq!(outcome(&call), ERROR_CANCELLED);
}

// ---- a flood of forged signals while the agent is busy with its own calls --------------------

/// The unique name of the agent's connection: the one the bus lists that none
/// of `known` has.
fn agent_name(watcher: &zbus::blocking::Connection, known: &[String]) -> String {
    let dbus = match DBusProxy::new(watcher) {
        Ok(dbus) => dbus,
        Err(err) => panic!("a bus proxy: {err}"),
    };
    let mut found = None;
    eventually(WAIT, || {
        found = dbus.list_names().ok().and_then(|names| {
            names
                .iter()
                .map(|name| name.as_str().to_owned())
                .find(|name| name.starts_with(':') && !known.contains(name))
        });
        found.is_some()
    });
    match found {
        Some(name) => name,
        None => panic!("the agent never appeared on the bus"),
    }
}

/// Any local user may send a signal to the agent's unique name on the system
/// bus. The agent's own calls (the registration waits for polkitd) must not
/// depend on nobody doing so: a flood over the iterator's queue would stop
/// the reply to a call in flight from being read.
#[test]
fn a_flood_of_unicast_signals_during_the_registration_does_not_stop_it() {
    let options = Options {
        behaviour: Some(Behaviour {
            register_delay: Duration::from_millis(1500),
            ..Behaviour::accepting()
        }),
        ..Options::default()
    };
    let Some(h) = Harness::start("flood", options) else {
        return;
    };
    let forger = client(&h.bus, WAIT);
    let known = vec![
        unique(&forger),
        h.polkitd().unique(),
        unique(&h._logind.connection),
    ];
    let agent = agent_name(&forger, &known);
    let body = (POLKIT_NAME, h.polkitd().unique(), unique(&forger));
    for _ in 0..400 {
        if let Err(err) = forger.emit_signal(
            Some(agent.as_str()),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
            &body,
        ) {
            panic!("the forged signal is sent: {err}");
        }
    }
    assert!(
        h.until(|e| *e == PolkitEvent::Registration(RegistrationState::Registered)),
        "the registration completes under the flood"
    );
    // And real requests still arrive afterwards.
    let _call = h.begin("cookie-after-the-flood");
    let _id = h.admitted();
}

/// N-12, spec L-2: the match for polkitd's name goes on the wire before the
/// owner is asked for, or a restart in between would leave a stale cache.
#[test]
fn the_owner_match_is_installed_before_the_owner_is_asked_for() {
    let options = Options {
        monitor: true,
        ..Options::default()
    };
    let Some(h) = Harness::registered("match-order", options) else {
        return;
    };
    let Some(monitor) = &h.monitor else {
        panic!("this test watches the bus");
    };
    let calls = monitor.calls();
    let position = |member: &str, mentions: &str| {
        calls
            .iter()
            .position(|(m, first)| m == member && first.contains(mentions))
    };
    let (Some(add), Some(ask)) = (
        position("AddMatch", "NameOwnerChanged"),
        position("GetNameOwner", POLKIT_NAME),
    ) else {
        panic!("the monitor did not see both calls: {calls:?}");
    };
    assert!(add < ask, "the owner was asked for first: {calls:?}");
    assert!(
        calls[add].1.contains(POLKIT_NAME),
        "the match is for polkitd's name only: {:?}",
        calls[add]
    );
}

/// The bus going away (not a panic): the requests are torn down unanswered,
/// the loop is told, and the duty says why.
#[test]
fn the_bus_going_away_tears_the_requests_down_and_says_why() {
    let Some(mut h) = Harness::registered("bus-gone", Options::default()) else {
        return;
    };
    let _call = h.begin("cookie-bus-gone");
    let id = h.admitted();
    assert_eq!(h.agent().promote(Instant::now()), Promotion::Show(id));
    h.bus.kill();
    assert!(h.until(|e| *e
        == PolkitEvent::Ended {
            id,
            ending: Ending::OwnerChanged
        }));
    assert!(h.until(|e| matches!(
        e,
        PolkitEvent::Registration(RegistrationState::Unavailable(why))
            if why.contains("bus connection ended")
    )));
    assert_eq!(
        h.agent()
            .end_active(id, Ending::Authenticated, true, Instant::now()),
        None,
        "nothing is left to end"
    );
}
