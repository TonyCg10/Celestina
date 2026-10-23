// language-contract: allow-non-english
// (the tests type international input: accents, symbols, emoji)
//! Carries out what the machine asks for, through a [`Host`] (design §4.5).
//!
//! The [`Driver`] owns the machine and pumps it: every [`Action`] goes to the
//! host, and the host's answers that the machine must hear (a refused attempt,
//! the unlock's result) are fed back as inputs in order. The host is the only
//! impure part, so the whole loop of "input, action, answer" is tested with a
//! fake host and no compositor, and the real one is a thin adapter over the
//! toolkit ([`crate::app`]).
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::Instant;

use celestina_lock_proto::{LockExit, StatusLine};

use crate::field::Secret;
use crate::machine::{Action, Config, Input, Machine, Scene, State, UnlockFailure};
use crate::verify::{Authenticated, Denial, Outcome};

/// The verifier could not be started (busy, or no thread).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptRefused;

/// What the process can do.
pub trait Host {
    /// Prints a status line.
    fn report(&mut self, line: StatusLine);
    /// Draws a scene.
    fn render(&mut self, scene: &Scene);
    /// Starts the verifier for an attempt. The verdict comes back later as an
    /// [`Input::Verdict`]; an `Err` is answered by the driver with an
    /// unavailable verdict at once.
    ///
    /// # Errors
    ///
    /// [`AttemptRefused`] when no verifier can start now.
    fn start_attempt(&mut self, attempt: u64, secret: Secret) -> Result<(), AttemptRefused>;
    /// Ends the session lock and waits for the compositor to confirm.
    ///
    /// # Errors
    ///
    /// [`UnlockFailure`] when the confirmation failed.
    fn unlock(&mut self, token: Authenticated) -> Result<(), UnlockFailure>;
    /// Arms the single deadline timer (`None` disarms it).
    fn arm(&mut self, at: Option<Instant>);
    /// The process is done: flush and stop with this status.
    fn finish(&mut self, exit: LockExit);
}

/// The machine and its pump.
pub struct Driver {
    machine: Machine,
}

impl Driver {
    #[must_use]
    pub fn new(config: Config, now: Instant) -> Self {
        Self {
            machine: Machine::new(config, now),
        }
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.machine.state()
    }

    /// Feeds `input` and carries out everything it causes.
    pub fn feed(&mut self, host: &mut impl Host, input: Input, now: Instant) {
        let mut queue = VecDeque::from([input]);
        while let Some(input) = queue.pop_front() {
            for action in self.machine.handle(input, now) {
                match action {
                    Action::Report(line) => host.report(line),
                    Action::Render(scene) => host.render(&scene),
                    Action::StartAttempt { attempt, secret } => {
                        if host.start_attempt(attempt, secret).is_err() {
                            queue.push_back(Input::Verdict {
                                attempt,
                                outcome: Outcome::Denied(Denial::Unavailable),
                            });
                        }
                    }
                    Action::Unlock(token) => {
                        queue.push_back(match host.unlock(token) {
                            Ok(()) => Input::UnlockConfirmed,
                            Err(failure) => Input::UnlockFailed(failure),
                        });
                    }
                    Action::Arm(at) => host.arm(at),
                    Action::Exit(code) => host.finish(code),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use celestina_wayland::{InputEvent, Modifiers, OutputId, OutputInfo};

    use crate::machine::Status;
    use crate::verify::{Attempts, PinnedBinary, StartError, Timings, Verifier};

    #[derive(Debug, PartialEq)]
    enum Event {
        Report(StatusLine),
        Render,
        Start(u64),
        Unlock,
        Finish(LockExit),
    }

    /// A host whose attempts run the real verify worker over `/bin/sh`.
    struct FakeHost {
        events: Vec<Event>,
        attempts: Attempts,
        verdicts_tx: Sender<(u64, Outcome)>,
        unlock_result: Result<(), UnlockFailure>,
        refuse_attempts: bool,
        scenes: Vec<Scene>,
    }

    impl FakeHost {
        fn new(script: &str) -> (Self, Receiver<(u64, Outcome)>) {
            let (verdicts_tx, rx) = channel();
            let verifier = Verifier::with_args(
                PinnedBinary::open(Path::new("/bin/sh")).expect("pin"),
                vec!["-c".into(), script.into(), "fake".into()],
            );
            (
                Self {
                    events: Vec::new(),
                    attempts: Attempts::new(
                        verifier,
                        Timings {
                            deadline: Duration::from_secs(5),
                            reap_wait: Duration::from_secs(2),
                        },
                    ),
                    verdicts_tx,
                    unlock_result: Ok(()),
                    refuse_attempts: false,
                    scenes: Vec::new(),
                },
                rx,
            )
        }
    }

    impl Host for FakeHost {
        fn report(&mut self, line: StatusLine) {
            self.events.push(Event::Report(line));
        }
        fn render(&mut self, scene: &Scene) {
            self.events.push(Event::Render);
            self.scenes.push(scene.clone());
        }
        fn start_attempt(&mut self, attempt: u64, secret: Secret) -> Result<(), AttemptRefused> {
            self.events.push(Event::Start(attempt));
            if self.refuse_attempts {
                return Err(AttemptRefused);
            }
            let tx = self.verdicts_tx.clone();
            self.attempts
                .start(attempt, secret, move |attempt, outcome| {
                    let _ = tx.send((attempt, outcome));
                })
                .map_err(|_: StartError| AttemptRefused)
        }
        fn unlock(&mut self, _token: Authenticated) -> Result<(), UnlockFailure> {
            self.events.push(Event::Unlock);
            self.unlock_result
        }
        fn arm(&mut self, _at: Option<Instant>) {}
        fn finish(&mut self, exit: LockExit) {
            self.events.push(Event::Finish(exit));
        }
    }

    struct Rig {
        driver: Driver,
        host: FakeHost,
        verdicts: Receiver<(u64, Outcome)>,
        now: Instant,
    }

    impl Rig {
        fn new(script: &str) -> Self {
            let now = Instant::now();
            let (host, verdicts) = FakeHost::new(script);
            let mut rig = Self {
                driver: Driver::new(Config::new(false, false, 0), now),
                host,
                verdicts,
                now,
            };
            rig.feed(Input::OutputAdded(OutputInfo {
                id: OutputId(1),
                name: "DP-1".to_owned(),
                logical_size: (1920, 1080),
                scale: 1.0,
                refresh_mhz: 60_000,
            }));
            rig.feed(Input::Connected);
            rig.feed(Input::Locked);
            rig
        }

        fn feed(&mut self, input: Input) {
            self.driver.feed(&mut self.host, input, self.now);
        }

        fn type_and_enter(&mut self, text: &str) {
            for ch in text.chars() {
                self.key(u32::from(ch), Some(ch.to_string()));
            }
            self.key(0xff0d, Some("\r".to_owned()));
        }

        fn key(&mut self, keysym: u32, utf8: Option<String>) {
            self.feed(Input::Input {
                output: OutputId(1),
                event: InputEvent::Key {
                    keysym,
                    utf8,
                    modifiers: Modifiers::default(),
                    pressed: true,
                    serial: 0,
                },
            });
        }

        /// Waits for the worker's answer and feeds it, like the loop does.
        fn deliver(&mut self) {
            let (attempt, outcome) = self
                .verdicts
                .recv_timeout(Duration::from_secs(10))
                .expect("a verdict");
            self.feed(Input::Verdict { attempt, outcome });
        }
    }

    #[test]
    fn unlocked_is_printed_only_after_the_sync_round_trip_and_exit_is_0() {
        let mut rig = Rig::new("cat >/dev/null");
        rig.type_and_enter("right");
        rig.deliver();
        let tail: Vec<&Event> = rig
            .host
            .events
            .iter()
            .skip_while(|e| **e != Event::Start(1))
            .collect();
        assert_eq!(
            tail,
            [
                &Event::Start(1),
                &Event::Render,
                &Event::Unlock,
                &Event::Report(StatusLine::Unlocked),
                &Event::Finish(LockExit::Unlocked),
            ],
            "the unlock round trip comes before the line, and the exit last"
        );
    }

    #[test]
    fn a_failed_unlock_confirmation_prints_nothing_and_exits_as_a_death() {
        let mut rig = Rig::new("cat >/dev/null");
        rig.host.unlock_result = Err(UnlockFailure::Died);
        rig.type_and_enter("right");
        rig.deliver();
        assert!(!rig
            .host
            .events
            .contains(&Event::Report(StatusLine::Unlocked)));
        assert_eq!(
            rig.host.events.last(),
            Some(&Event::Finish(LockExit::EarlyError))
        );
    }

    #[test]
    fn a_wrong_password_never_reaches_unlock_and_reports_the_attempt() {
        let mut rig = Rig::new("cat >/dev/null; exit 1");
        rig.type_and_enter("wrong");
        rig.deliver();
        assert!(!rig.host.events.contains(&Event::Unlock));
        assert!(rig.host.events.contains(&Event::Report(StatusLine::Attempt(
            celestina_lock_proto::AttemptKind::Refused
        ))));
        assert_eq!(rig.driver.state(), State::Prompt);
    }

    #[test]
    fn an_attempt_the_host_could_not_start_is_answered_with_unavailable_at_once() {
        let mut rig = Rig::new("exit 0");
        rig.host.refuse_attempts = true;
        rig.type_and_enter("pw");
        assert_eq!(rig.driver.state(), State::Prompt, "no verdict to wait for");
        assert_eq!(
            rig.host.scenes.last().map(|s| s.status),
            Some(Status::Unavailable)
        );
        assert!(!rig.host.events.contains(&Event::Unlock));
        assert!(rig.host.events.contains(&Event::Report(StatusLine::Attempt(
            celestina_lock_proto::AttemptKind::Unavailable
        ))));
    }

    #[test]
    fn double_enter_through_the_real_worker_starts_one_attempt() {
        let mut rig = Rig::new("sleep 0.2");
        for ch in "pw".chars() {
            rig.key(u32::from(ch), Some(ch.to_string()));
        }
        rig.key(0xff0d, Some("\r".to_owned()));
        rig.key(0xff0d, Some("\r".to_owned()));
        let starts = rig
            .host
            .events
            .iter()
            .filter(|e| matches!(e, Event::Start(_)))
            .count();
        assert_eq!(starts, 1);
        rig.deliver();
        assert!(rig
            .verdicts
            .recv_timeout(Duration::from_millis(400))
            .is_err());
    }

    /// Collects what a subscriber at TRACE let through.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Capture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn t_l1_no_typed_byte_appears_in_any_tracing_event_at_trace_level_through_a_full_attempt() {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .finish();
        // Characters no log line of this crate contains.
        let typed = "Ωψ§¶Ξ";
        tracing::subscriber::with_default(subscriber, || {
            // Other tests of this process hit the same call sites without a
            // subscriber; make every call site consult this one.
            tracing::callsite::rebuild_interest_cache();
            let mut rig = Rig::new("cat >/dev/null; exit 1");
            // Log everything a careless caller might: the events and inputs
            // as Debug, at TRACE.
            let event = InputEvent::Key {
                keysym: 0x3a9,
                utf8: Some(typed.to_owned()),
                modifiers: Modifiers::default(),
                pressed: true,
                serial: 0,
            };
            tracing::trace!(?event, "key");
            tracing::trace!(input = ?Input::Input { output: OutputId(1), event: event.clone() }, "input");
            // A full attempt: typed, submitted, refused; then a second one
            // that succeeds, and a stale verdict for good measure.
            rig.type_and_enter(typed);
            rig.deliver();
            rig.feed(Input::Verdict {
                attempt: 99,
                outcome: Outcome::Denied(Denial::Refused),
            });
        });
        let text = String::from_utf8(capture.0.lock().unwrap_or_else(|e| e.into_inner()).clone())
            .expect("utf-8");
        // The capture works (the test's own TRACE events are in it, with the
        // key's `Debug` redacted); the crate's own call sites add to it but a
        // parallel test can leave one cached as disabled, so none is demanded.
        assert!(
            text.contains("redacted") && text.contains("TRACE"),
            "the capture saw TRACE events: {text}"
        );
        for ch in typed.chars() {
            assert!(!text.contains(ch), "{ch:?} leaked into the log: {text}");
        }
        assert!(!text.contains("keysym: 937"), "{text}");
    }
}
