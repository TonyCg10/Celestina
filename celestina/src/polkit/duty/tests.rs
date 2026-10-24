//! The duty against fakes of the launcher and the host, and against a real
//! agent on a private `dbus-daemon` with a fake polkitd and a fake logind
//! (design 10.1). No test touches the session or the system bus.

use std::cell::{Cell, OnceCell};
use std::io;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use celestina_paint::Theme;
use celestina_shell::polkit::RegistrationState;
use celestina_shell::settings::Duty;
use celestina_wayland::{LayerSpec, OutputId, OutputInfo, RuntimeError, SurfaceId};

use super::*;
use crate::polkit::state::Ctx;
use crate::polkit::{HelperHandle, Notice};
use crate::providers::polkit::testing::{
    self, answer_name, begin, eventually, fake_logind, user, Behaviour, FakePolkitd, Seams,
    SessionShape, DISPLAY, UID,
};
use crate::providers::polkit::{SystemBus, ERROR_CANCELLED};
use crate::providers::Reaper;
use crate::tray::test_bus::PrivateBus;

const WAIT: Duration = Duration::from_secs(5);

// ---- fakes ------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeHost {
    next: u32,
    created: Vec<SurfaceId>,
    destroyed: Vec<SurfaceId>,
    notices: Vec<Notice>,
}

impl PromptHost for FakeHost {
    fn create_surface(&mut self, _spec: LayerSpec) -> Result<SurfaceId, RuntimeError> {
        self.next += 1;
        let id = SurfaceId(500 + self.next);
        self.created.push(id);
        Ok(id)
    }

    fn destroy_surface(&mut self, id: SurfaceId) {
        self.destroyed.push(id);
    }

    fn request_redraw(&mut self, _id: SurfaceId) {}

    fn close_keyboard_surfaces(&mut self) {}

    fn notice(&mut self, notice: Notice) {
        self.notices.push(notice);
    }
}

struct NoHelpers;

impl HelperSpawner for NoHelpers {
    fn spawn(&self, _id: RequestId, _input: HelperInput) -> io::Result<Box<dyn HelperHandle>> {
        Err(io::Error::other("no helper in this test"))
    }
}

fn info(id: u32, name: &str) -> OutputInfo {
    OutputInfo {
        id: OutputId(id),
        name: name.to_owned(),
        logical_size: (2560, 1440),
        scale: 1.5,
        refresh_mhz: 60_000,
    }
}

/// A launcher that counts its launches and starts a real agent on `address`,
/// or fails.
struct BusLauncher {
    address: String,
    tx: mpsc::Sender<PolkitEvent>,
    launches: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
}

impl AgentLauncher for BusLauncher {
    fn launch(&self, after: Reaper) -> io::Result<Launched> {
        self.launches.set(self.launches.get() + 1);
        if self.fail.get() {
            return Err(io::Error::other("no thread"));
        }
        let tx = self.tx.clone();
        let config = PolkitConfig {
            bus: SystemBus::Address(self.address.clone()),
            after,
            seams: Seams {
                environment: Some(testing::environment()),
                hook: None,
            },
        };
        let agent = Gated::start(config, move |event| tx.send(event).is_ok())?;
        Ok(Launched {
            agent: Box::new(agent),
            helpers: Box::new(NoHelpers),
            dumpable: DumpableWindow::inert(),
        })
    }
}

// ---- the fixture ------------------------------------------------------------------------------

struct Fx {
    duty: PolkitDuty,
    host: FakeHost,
    events: Receiver<PolkitEvent>,
    launches: Rc<Cell<usize>>,
    /// Whether the launcher fails the next launch.
    fail: Rc<Cell<bool>>,
    theme: Theme,
    polkitd: Option<FakePolkitd>,
    _logind: Option<testing::Service>,
    /// Fields drop in order: the duty (agent) first, then the bus.
    bus: Option<PrivateBus>,
    admitted: usize,
}

impl Fx {
    /// A duty over a private bus with a fake logind and, when `accept` is
    /// given, a fake polkitd that accepts or refuses the registration.
    /// `None` where `dbus-daemon` is not installed.
    fn on_bus(name: &str, accept: Option<bool>) -> Option<Fx> {
        Fx::on_bus_behaving(
            name,
            accept.map(|accept| Behaviour {
                accept,
                ..Behaviour::accepting()
            }),
        )
    }

    /// [`Fx::on_bus`] with a fake polkitd that behaves as `behaviour` says.
    fn on_bus_behaving(name: &str, behaviour: Option<Behaviour>) -> Option<Fx> {
        let bus = PrivateBus::start(&format!("duty-{name}"))?;
        let (logind, _) = fake_logind(&bus, SessionShape::default());
        let polkitd = behaviour.map(|behaviour| FakePolkitd::behaving(&bus, behaviour));
        let address = bus.address().to_owned();
        let mut fx = Fx::over(address, false);
        fx._logind = Some(logind);
        fx.polkitd = polkitd;
        fx.bus = Some(bus);
        Some(fx)
    }

    fn over(address: String, fail: bool) -> Fx {
        let (tx, events) = mpsc::channel();
        let launches = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(fail));
        let slot: LauncherSlot = Rc::new(OnceCell::new());
        let launcher: Box<dyn AgentLauncher> = Box::new(BusLauncher {
            address,
            tx,
            launches: Rc::clone(&launches),
            fail: Rc::clone(&fail),
        });
        assert!(slot.set(launcher).is_ok());
        Fx {
            duty: PolkitDuty::new(slot),
            host: FakeHost::default(),
            events,
            launches,
            fail,
            theme: Theme::dark(),
            polkitd: None,
            _logind: None,
            bus: None,
            admitted: 0,
        }
    }

    fn apply(&mut self, want: Duty) -> Change {
        self.duty
            .apply(want, &self.theme, false, Instant::now(), &mut self.host)
    }

    fn ctx() -> Ctx<'static> {
        Ctx {
            now: Instant::now(),
            focused_output: None,
        }
    }

    /// Delivers one event the way the app does.
    fn deliver(&mut self, event: PolkitEvent) {
        if let PolkitEvent::Registration(state) = &event {
            self.duty.registration_reported(state.clone());
        }
        if matches!(event, PolkitEvent::Admitted(_)) {
            self.admitted += 1;
        }
        let ctx = Fx::ctx();
        if let Some(state) = self.duty.state_mut() {
            state.on_event(event, &ctx, &mut self.host);
        }
    }

    /// Delivers events until `done` holds or the wait is over.
    fn until(&mut self, mut done: impl FnMut(&Fx) -> bool) -> bool {
        let until = Instant::now() + WAIT;
        while !done(self) {
            let Some(left) = until.checked_duration_since(Instant::now()) else {
                return false;
            };
            if let Ok(event) = self
                .events
                .recv_timeout(left.min(Duration::from_millis(50)))
            {
                self.deliver(event);
            }
        }
        true
    }

    fn polkitd(&self) -> &FakePolkitd {
        match &self.polkitd {
            Some(polkitd) => polkitd,
            None => panic!("this fixture has no polkitd"),
        }
    }

    /// Begins a request from the fake polkitd.
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

// ---- the duty off ------------------------------------------------------------------------------------

#[test]
fn duty_off_starts_no_worker_and_refuses_requests() {
    let Some(mut fx) = Fx::on_bus("off", Some(true)) else {
        return;
    };
    fx.duty.note_output(&info(1, "DP-1"));
    assert_eq!(fx.apply(Duty::Off), Change::Unchanged);
    assert_eq!(fx.launches.get(), 0, "no worker is started");
    assert!(!fx.duty.is_running());
    assert!(fx.duty.state().is_none(), "no prompt state exists");
    assert_eq!(*fx.duty.registration(), RegistrationState::Off);
    // Nothing is registered with polkitd: a request has no agent to reach, so
    // polkit fails closed on its own.
    assert!(
        !eventually(Duration::from_millis(400), || !fx
            .polkitd()
            .log()
            .registrations
            .is_empty()),
        "nothing registered"
    );
    assert!(fx.events.try_recv().is_err());
    assert!(fx.host.created.is_empty());
}

#[test]
fn a_launch_that_fails_reports_unavailable_and_is_not_retried_by_the_same_setting() {
    let mut fx = Fx::over("unix:path=/nonexistent/celestina-duty".to_owned(), true);
    assert_eq!(fx.apply(Duty::On), Change::Failed);
    assert!(!fx.duty.is_running());
    assert!(
        matches!(fx.duty.registration(), RegistrationState::Unavailable(_)),
        "{:?}",
        fx.duty.registration()
    );
    assert_eq!(fx.launches.get(), 1);
}

/// M-1: a launch that fails does not lose the teardown the next agent has to
/// wait for. The strict polkitd answers the first agent's unregistration late;
/// the launch in between fails; the one after must still register only once
/// the first is gone, or the strict polkitd refuses it.
#[test]
fn a_launch_that_fails_keeps_the_teardown_the_next_agent_waits_for() {
    let Some(mut fx) = Fx::on_bus_behaving(
        "failed-launch",
        Some(Behaviour {
            strict: true,
            unregister_delay: Duration::from_millis(1200),
            ..Behaviour::accepting()
        }),
    ) else {
        return;
    };
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(fx.until(|fx| *fx.duty.registration() == RegistrationState::Registered));
    assert_eq!(fx.apply(Duty::Off), Change::Stopped);

    fx.fail.set(true);
    assert_eq!(fx.apply(Duty::On), Change::Failed);
    fx.fail.set(false);
    assert_eq!(fx.apply(Duty::On), Change::Started);

    assert!(
        fx.until(|fx| *fx.duty.registration() == RegistrationState::Registered),
        "the agent after the failed launch registered: {:?} {:?}",
        fx.duty.registration(),
        fx.polkitd().log()
    );
    assert_eq!(
        fx.polkitd().log().members,
        [
            "RegisterAuthenticationAgent",
            "UnregisterAuthenticationAgent",
            "RegisterAuthenticationAgent"
        ]
    );
}

// ---- on and off again ----------------------------------------------------------------------------------

#[test]
fn switching_on_registers_and_switching_off_unregisters_and_cancels_every_request() {
    let Some(mut fx) = Fx::on_bus("switch", Some(true)) else {
        return;
    };
    fx.duty.note_output(&info(1, "DP-1"));
    fx.duty.note_output(&info(2, "HDMI-A-1"));
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert_eq!(
        fx.host.created.len(),
        0,
        "outputs are replayed, nothing is shown yet"
    );
    assert!(
        fx.until(|fx| *fx.duty.registration() == RegistrationState::Registered),
        "the duty reports registered: {:?}",
        fx.duty.registration()
    );
    assert_eq!(fx.polkitd().log().registrations.len(), 1);

    // Two requests: one is shown, the other waits.
    let shown = fx.begin("cookie-one");
    let waiting = fx.begin("cookie-two");
    assert!(
        fx.until(|fx| fx.admitted == 2),
        "both requests were admitted"
    );
    assert!(
        fx.duty.state().is_some_and(PolkitState::request_active),
        "one request is up"
    );
    let surfaces = fx.host.created.clone();
    assert_eq!(surfaces.len(), 2, "a card and a dim for two outputs");

    assert_eq!(fx.apply(Duty::Off), Change::Stopped);

    // Every request is answered Cancelled, the agent unregistered, every
    // surface destroyed, and nothing the stopping agent said reaches the loop.
    assert_eq!(outcome(&shown), ERROR_CANCELLED);
    assert_eq!(outcome(&waiting), ERROR_CANCELLED);
    // The reaper unregisters; the loop did not wait for it.
    assert!(
        eventually(WAIT, || fx.polkitd().log().unregistrations
            == vec![DISPLAY.to_owned()]),
        "{:?}",
        fx.polkitd().log()
    );
    for surface in &surfaces {
        assert!(fx.host.destroyed.contains(surface), "{surface:?}");
    }
    assert!(!fx.duty.is_running());
    assert_eq!(*fx.duty.registration(), RegistrationState::Off);
    assert!(
        fx.events.recv_timeout(Duration::from_millis(300)).is_err(),
        "the stopped agent publishes nothing more"
    );
    assert!(fx.host.notices.is_empty(), "switching off raises no notice");

    // On again: a new agent registers again.
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(fx.until(|fx| *fx.duty.registration() == RegistrationState::Registered));
    assert_eq!(fx.polkitd().log().registrations.len(), 2);
    assert_eq!(fx.launches.get(), 2);
}

#[test]
fn a_refused_registration_reports_refused_and_does_not_retry_when_switched_off_and_on_again_it_tries_once(
) {
    assert!(testing::capture_logs(), "the log capture is in place");
    let Some(mut fx) = Fx::on_bus("refused", Some(false)) else {
        return;
    };
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(
        fx.until(|fx| *fx.duty.registration() == RegistrationState::Refused),
        "the duty reports refused: {:?}",
        fx.duty.registration()
    );
    assert!(testing::logged("another agent holds this session"));
    assert_eq!(fx.polkitd().log().registrations.len(), 1, "tried once");

    // The same setting read again, and a long quiet: no retry, no new agent.
    assert_eq!(fx.apply(Duty::On), Change::Unchanged);
    assert!(
        !eventually(Duration::from_millis(500), || fx
            .polkitd()
            .log()
            .registrations
            .len()
            > 1),
        "a refused duty does not retry on its own"
    );
    assert_eq!(fx.launches.get(), 1);
    assert_eq!(*fx.duty.registration(), RegistrationState::Refused);

    // Off and on is the retry, and it tries once.
    assert_eq!(fx.apply(Duty::Off), Change::Stopped);
    assert_eq!(*fx.duty.registration(), RegistrationState::Off);
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(fx.until(|fx| *fx.duty.registration() == RegistrationState::Refused));
    assert_eq!(fx.polkitd().log().registrations.len(), 2, "tried once more");
    assert_eq!(fx.launches.get(), 2);
}

#[test]
fn smoke_with_the_duty_on_and_the_dead_bus_reports_unavailable_and_does_not_panic() {
    // The address smoke.sh's dead system bus stands for: nothing listens.
    let mut fx = Fx::over(
        "unix:path=/nonexistent/celestina-dead-bus".to_owned(),
        false,
    );
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(
        fx.until(|fx| matches!(fx.duty.registration(), RegistrationState::Unavailable(_))),
        "the duty reports unavailable: {:?}",
        fx.duty.registration()
    );
    // It stays so, and switching off and on again is still safe.
    assert_eq!(fx.apply(Duty::Off), Change::Stopped);
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(fx.until(|fx| matches!(fx.duty.registration(), RegistrationState::Unavailable(_))));
    assert_eq!(fx.apply(Duty::Off), Change::Stopped);
}

#[test]
fn a_late_registration_report_after_switching_off_is_ignored() {
    let mut fx = Fx::over("unix:path=/nonexistent/celestina-late".to_owned(), false);
    fx.duty.registration_reported(RegistrationState::Registered);
    assert_eq!(*fx.duty.registration(), RegistrationState::Off);
}

/// N-22: polkitd takes 1.5 s to answer the unregistration; the loop's switch
/// off returns at once, a switch on straight after returns at once too, and
/// the new agent registers only after the old one has unregistered (a strict
/// polkitd would refuse it otherwise).
#[test]
fn switching_off_and_on_never_waits_for_a_slow_polkitd_and_the_new_agent_follows_the_old() {
    let behaviour = Behaviour {
        strict: true,
        unregister_delay: Duration::from_millis(1500),
        ..Behaviour::accepting()
    };
    let Some(mut fx) = Fx::on_bus_behaving("slow-off", Some(behaviour)) else {
        return;
    };
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(fx.until(|fx| *fx.duty.registration() == RegistrationState::Registered));

    let started = Instant::now();
    assert_eq!(fx.apply(Duty::Off), Change::Stopped);
    assert!(
        started.elapsed() < Duration::from_millis(300),
        "switching off waited {:?}",
        started.elapsed()
    );
    let started = Instant::now();
    assert_eq!(fx.apply(Duty::On), Change::Started);
    assert!(
        started.elapsed() < Duration::from_millis(300),
        "switching on waited {:?}",
        started.elapsed()
    );
    assert!(
        fx.until(|fx| *fx.duty.registration() == RegistrationState::Registered),
        "the second agent registers: {:?}",
        fx.duty.registration()
    );
    assert_eq!(
        fx.polkitd().log().members,
        [
            "RegisterAuthenticationAgent",
            "UnregisterAuthenticationAgent",
            "RegisterAuthenticationAgent"
        ]
    );
}
