//! The lock as a client of the toolkit's lock profile (design §4.5, §4.10).
//!
//! [`LockApp`] is the [`LockShell`] the toolkit calls: it turns toolkit events
//! into machine [`Input`]s, runs them through the [`Driver`], and its
//! [`Host`] carries the actions out: status lines to the reporter thread, a
//! verifier attempt to the verify worker, the scene to the presenter, the
//! unlock to [`crate::unlock`], the deadline to the timerfd. Messages from the
//! workers reach it through the loop's channel, so the loop thread is the only
//! one that touches the machine.
//!
//! The process's exit status is decided by the machine (`Action::Exit`) or,
//! when the loop itself fails, by [`exit_for_error`].
#![forbid(unsafe_code)]

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use celestina_lock_proto::{LockExit, StatusLine};
use celestina_wayland::lock::{
    run_lock, ConnectPolicy, LockCtl, LockError, LockEvent, LockShell, UnlockCapability,
};
use celestina_wayland::{InputEvent, OutputId, Sender};

use crate::driver::{AttemptRefused, Driver, Host};
use crate::field::Secret;
use crate::machine::{Config, Input, Scene, UnlockFailure};
use crate::present::Presenter;
use crate::report::Reporter;
use crate::unlock::unlock;
use crate::verify::{Attempts, Authenticated, Outcome};
use crate::wake::Wake;

/// How long the process waits for its last status lines at exit.
const REPORT_WAIT: Duration = Duration::from_secs(2);

/// What the workers and the timer post to the loop.
#[derive(Debug)]
pub enum Message {
    /// The lock was requested: the process says hello.
    Connected,
    /// The verify worker's answer.
    Verdict { attempt: u64, outcome: Outcome },
    /// The deadline timer fired.
    Tick,
    /// Rehearsal probe only: type a made-up password and press Enter.
    #[cfg(feature = "fake-verifier")]
    AutoType,
}

/// What the process owns besides the machine.
struct Parts<P> {
    reporter: SharedReporter,
    presenter: P,
    attempts: Attempts,
    sender: Rc<OnceCell<Sender<Message>>>,
    wake: Wake,
    capability: Option<UnlockCapability>,
    exit: Rc<Cell<Option<LockExit>>>,
}

/// The adapter the toolkit drives.
pub struct LockApp<P: Presenter> {
    driver: Driver,
    parts: Parts<P>,
}

/// The reporter, shared by the setup (which starts its thread, signals
/// already blocked) and the host (which sends lines and finishes it).
type SharedReporter = Rc<RefCell<Option<Reporter>>>;

/// The machine's host for the length of one toolkit call.
struct ToolkitHost<'a, 'c, P: Presenter> {
    parts: &'a mut Parts<P>,
    ctl: &'a mut LockCtl<'c>,
}

impl<P: Presenter> Host for ToolkitHost<'_, '_, P> {
    fn report(&mut self, line: StatusLine) {
        if let Some(reporter) = self.parts.reporter.borrow().as_ref() {
            reporter.send(line);
        }
    }

    fn render(&mut self, scene: &Scene) {
        self.parts.presenter.render(scene, self.ctl);
    }

    fn start_attempt(&mut self, attempt: u64, secret: Secret) -> Result<(), AttemptRefused> {
        let Some(sender) = self.parts.sender.get().cloned() else {
            tracing::error!("no message channel for the verify worker");
            return Err(AttemptRefused);
        };
        self.parts
            .attempts
            .start(attempt, secret, move |attempt, outcome| {
                // A loop that is gone cannot hear the answer: the session
                // stays locked.
                let _ = sender.send(Message::Verdict { attempt, outcome });
            })
            .map_err(|err| {
                tracing::warn!(%err, "attempt not started");
                AttemptRefused
            })
    }

    fn unlock(&mut self, token: Authenticated) -> Result<(), UnlockFailure> {
        confirm_unlock(self.parts.capability.take(), token)
    }

    fn arm(&mut self, at: Option<Instant>) {
        if let Err(err) = self.parts.wake.arm(at, Instant::now()) {
            tracing::error!(%err, "cannot arm the deadline timer");
        }
    }

    fn finish(&mut self, exit: LockExit) {
        self.parts.exit.set(Some(exit));
        // Taken out first: the borrow is not held while the writer drains.
        let reporter = self.parts.reporter.borrow_mut().take();
        if let Some(reporter) = reporter {
            reporter.finish(REPORT_WAIT);
        }
        self.ctl.stop();
    }
}

impl<P: Presenter> LockApp<P> {
    fn feed(&mut self, input: Input, ctl: &mut LockCtl<'_>) {
        let Self { driver, parts } = self;
        driver.feed(&mut ToolkitHost { parts, ctl }, input, Instant::now());
    }
}

impl<P: Presenter + 'static> LockShell for LockApp<P> {
    type Message = Message;

    fn event(&mut self, ev: LockEvent, ctl: &mut LockCtl<'_>) {
        let input = match ev {
            LockEvent::Locked(capability) => {
                self.parts.capability = Some(capability);
                Input::Locked
            }
            LockEvent::Finished => Input::Finished,
            LockEvent::SurfaceConfigured { output, size } => {
                Input::SurfaceConfigured { output, size }
            }
            LockEvent::OutputAdded(info) => Input::OutputAdded(info),
            LockEvent::OutputRemoved(id) => Input::OutputRemoved(id),
            LockEvent::Keyboard { present } => Input::Keyboard { present },
            LockEvent::Redraw => Input::Redraw,
        };
        self.feed(input, ctl);
    }

    fn input(&mut self, output: OutputId, event: InputEvent, ctl: &mut LockCtl<'_>) {
        self.feed(Input::Input { output, event }, ctl);
    }

    fn message(&mut self, msg: Message, ctl: &mut LockCtl<'_>) {
        match msg {
            Message::Connected => {
                self.feed(Input::Connected, ctl);
            }
            Message::Verdict { attempt, outcome } => {
                self.feed(Input::Verdict { attempt, outcome }, ctl);
            }
            Message::Tick => self.feed(Input::Tick, ctl),
            #[cfg(feature = "fake-verifier")]
            Message::AutoType => {
                for (keysym, text) in [(0x78, Some("x")), (0xff0d, None)] {
                    let event = InputEvent::Key {
                        keysym,
                        utf8: text.map(str::to_owned),
                        modifiers: celestina_wayland::Modifiers::default(),
                        pressed: true,
                        serial: 0,
                    };
                    self.feed(
                        Input::Input {
                            output: OutputId(0),
                            event,
                        },
                        ctl,
                    );
                }
            }
        }
    }
}

/// Spends the capability on the unlock and says how a failure should end the
/// process (finding I-A): a lost connection is 16, anything else (a protocol
/// error from the compositor, a missing capability) leaves the session locked
/// with the compositor alive, so the lock dies and the shell relaunches it.
fn confirm_unlock(
    capability: Option<UnlockCapability>,
    token: Authenticated,
) -> Result<(), UnlockFailure> {
    let Some(capability) = capability else {
        tracing::error!("unlock requested without a capability");
        return Err(UnlockFailure::Died);
    };
    unlock(capability, token).map_err(|err| {
        tracing::error!(%err, "the unlock was not confirmed");
        if err.is_protocol_error() {
            UnlockFailure::Died
        } else {
            UnlockFailure::ConnectionLost
        }
    })
}

/// Everything [`run`] needs.
pub struct Run<P> {
    pub config: Config,
    pub attempts: Attempts,
    pub presenter: P,
    pub reporter: Reporter,
    /// How this process says it was launched; the toolkit's connection guard
    /// checks it before anything is sent (round 5, ADR 0022 A4).
    pub connect: ConnectPolicy,
    /// Rehearsal probe only: after `locked`, type a made-up password and
    /// press Enter once this long has passed.
    #[cfg(feature = "fake-verifier")]
    pub auto_type_after: Option<Duration>,
}

/// Connects, locks, and runs until the machine decides the exit.
pub fn run<P: Presenter + 'static>(run: Run<P>) -> LockExit {
    let wake = match Wake::new() {
        Ok(wake) => wake,
        Err(err) => {
            tracing::error!(%err, "cannot create the deadline timer");
            return LockExit::EarlyError;
        }
    };
    let exit = Rc::new(Cell::new(None));
    let sender_cell = Rc::new(OnceCell::new());
    let requested = Rc::new(Cell::new(false));
    let reporter: SharedReporter = Rc::new(RefCell::new(Some(run.reporter)));
    let app = LockApp {
        driver: Driver::new(run.config, Instant::now()),
        parts: Parts {
            reporter: Rc::clone(&reporter),
            presenter: run.presenter,
            attempts: run.attempts,
            sender: Rc::clone(&sender_cell),
            wake: wake.clone(),
            capability: None,
            exit: Rc::clone(&exit),
        },
    };
    #[cfg(feature = "fake-verifier")]
    let auto_type_after = run.auto_type_after;
    let setup_requested = Rc::clone(&requested);
    let setup_reporter = Rc::clone(&reporter);
    let result = run_lock(run.connect, app, move |sender, handle| {
        // The writer thread starts here, after the signal mask and before the
        // request: it inherits the mask (finding C1), and a lock whose lines
        // nobody can write is not requested at all (finding M-b).
        start_reporter(&setup_reporter, Reporter::start)?;
        let tick = sender.clone();
        if let Err(err) = wake.install(handle, move || {
            let _ = tick.send(Message::Tick);
        }) {
            // Without the timer the fade, the countdown and the watchdog do
            // not run; the prompt and the lock itself still do.
            tracing::error!(%err, "cannot register the deadline timer");
        }
        #[cfg(feature = "fake-verifier")]
        if let Some(after) = auto_type_after {
            let typist = sender.clone();
            std::thread::spawn(move || {
                std::thread::sleep(after);
                let _ = typist.send(Message::AutoType);
            });
        }
        let _ = sender_cell.set(sender.clone());
        setup_requested.set(true);
        // Said once the request is on its way: the loop's first dispatch.
        let _ = sender.send(Message::Connected);
        Ok(())
    });
    if let Err(err) = &result {
        // The guard's refusal reaches the author's terminal through this line.
        tracing::error!(error = %err, "the lock ended with an error");
    }
    exit_after(result, exit.get(), requested.get())
}

/// Starts the status writer with `start`; a failure stops the lock before it
/// is requested (exit 15, design §4.3 (b)).
fn start_reporter(
    reporter: &SharedReporter,
    start: impl FnOnce(&mut Reporter) -> std::io::Result<()>,
) -> Result<(), LockError> {
    let mut slot = reporter.borrow_mut();
    let Some(reporter) = slot.as_mut() else {
        return Ok(());
    };
    start(reporter).map_err(|err| {
        tracing::error!(%err, "cannot start the status writer: the lock is not requested");
        LockError::Setup(Box::new(err))
    })
}

/// The process's exit status once the loop ended.
#[must_use]
pub fn exit_after(
    result: Result<(), LockError>,
    decided: Option<LockExit>,
    requested: bool,
) -> LockExit {
    if let Some(exit) = decided {
        return exit;
    }
    match result {
        // A loop that stopped without a decision should not exist; the lock
        // then counts as having died, which the shell answers by relaunching.
        Ok(()) => LockExit::EarlyError,
        Err(err) => exit_for_error(&err, requested),
    }
}

/// The exit status a toolkit failure stands for (design §4.3 (b)).
#[must_use]
pub fn exit_for_error(err: &LockError, requested: bool) -> LockExit {
    match err {
        LockError::NoSessionLock => LockExit::NoLockManager,
        LockError::Setup(_) => LockExit::SetupFailed,
        // The connection guard refused before anything was sent (round 5).
        LockError::Refused(_) => LockExit::SetupFailed,
        LockError::Socket(_) => LockExit::NoConnection,
        LockError::Connect(_) | LockError::Registry(_) => LockExit::NoConnection,
        // A protocol error kills this client while the compositor lives on:
        // status 1 after `requested` is a death and is relaunched (F32).
        LockError::ConnectionLost(_) if requested && err.is_protocol_error() => {
            LockExit::EarlyError
        }
        LockError::ConnectionLost(_) if requested => LockExit::ConnectionLost,
        _ => LockExit::EarlyError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lost() -> LockError {
        LockError::ConnectionLost(calloop::Error::OtherError("broken".into()))
    }

    #[test]
    fn t_o4_connection_lost_after_requested_exits_16() {
        assert_eq!(exit_for_error(&lost(), true), LockExit::ConnectionLost);
        assert_eq!(LockExit::ConnectionLost.code(), 16);
        assert_eq!(
            exit_after(Err(lost()), None, true),
            LockExit::ConnectionLost
        );
        // A decision of the machine wins over a loop error that follows it.
        assert_eq!(
            exit_after(Err(lost()), Some(LockExit::Unlocked), true),
            LockExit::Unlocked
        );
        // Before `requested` the same failure is an early error, not a lost
        // lock.
        assert_eq!(exit_for_error(&lost(), false), LockExit::EarlyError);
    }

    #[test]
    fn a_protocol_error_kills_the_client_and_exits_as_a_death_not_16() {
        // Finding I-A: the loop reports a protocol error as EPROTO; the
        // compositor is alive, so the shell must relaunch (status 1).
        let protocol = LockError::ConnectionLost(calloop::Error::IoError(
            std::io::Error::from_raw_os_error(71),
        ));
        assert_eq!(exit_for_error(&protocol, true), LockExit::EarlyError);
        let broken = LockError::ConnectionLost(calloop::Error::IoError(std::io::Error::from(
            std::io::ErrorKind::BrokenPipe,
        )));
        assert_eq!(exit_for_error(&broken, true), LockExit::ConnectionLost);
    }

    #[test]
    fn a_protocol_error_as_the_event_loop_yields_it_exits_as_a_death_not_16() {
        // Finding I-1: `EventLoop::run` hands the Wayland source's
        // `calloop::Error::IoError(EPROTO)` back wrapped in `OtherError`
        // (calloop 0.14 wraps what a source returns), not at the top level.
        // The real-loop proof is `error_the_loop_yields` in
        // `celestina-wayland`; this is the exit mapping over that shape.
        let wrapped = LockError::ConnectionLost(calloop::Error::OtherError(Box::new(
            calloop::Error::IoError(std::io::Error::from_raw_os_error(71)),
        )));
        assert_eq!(exit_for_error(&wrapped, true), LockExit::EarlyError);
        let broken = LockError::ConnectionLost(calloop::Error::OtherError(Box::new(
            calloop::Error::IoError(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
        )));
        assert_eq!(exit_for_error(&broken, true), LockExit::ConnectionLost);
    }

    #[test]
    fn an_unlock_without_a_capability_is_a_death_and_sends_nothing() {
        // Finding I-A: the defensive exit sent nothing, so it is not a lost
        // connection; the shell relaunches.
        let result = confirm_unlock(None, crate::verify::test_token());
        assert_eq!(result, Err(UnlockFailure::Died));
    }

    #[test]
    fn a_status_writer_that_cannot_start_ends_the_lock_with_exit_15() {
        // Finding M-b: a lock whose lines nobody reads must not lock at all.
        let reporter = Rc::new(RefCell::new(Some(Reporter::new(std::io::sink()))));
        let failed = start_reporter(&reporter, |reporter| {
            reporter.start_with(|_, _| Err(std::io::Error::other("no threads")))
        });
        let err = failed.expect_err("the writer did not start");
        assert!(matches!(err, LockError::Setup(_)));
        assert_eq!(exit_after(Err(err), None, false), LockExit::SetupFailed);
        assert_eq!(LockExit::SetupFailed.code(), 15);
        // And one that starts is Ok.
        let good = Rc::new(RefCell::new(Some(Reporter::new(std::io::sink()))));
        assert!(start_reporter(&good, Reporter::start).is_ok());
    }

    #[test]
    fn startup_failures_map_to_the_exit_table() {
        assert_eq!(
            exit_for_error(&LockError::NoSessionLock, false),
            LockExit::NoLockManager
        );
        assert_eq!(
            exit_after(Ok(()), None, true),
            LockExit::EarlyError,
            "a loop that ended without a decision counts as a death"
        );
    }
}
