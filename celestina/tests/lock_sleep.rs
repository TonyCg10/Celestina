//! The lock's sleep side, end to end, on private buses (CEL-2-H design §4.4,
//! §4.9, §9.1: T-S1, the notices, the shutdown bound of spike Q10).
//!
//! The real workers (`lock`, `lock-sleep`, `logind`, `lock-tick`, `lock-notice`)
//! run behind a [`LockHandle`] whose loop is played by the test. The "system
//! bus" is a `dbus-daemon` of the test's own with a fake
//! `org.freedesktop.login1.Manager` on it; the "session bus" is another one
//! with a fake `org.freedesktop.Notifications`; the "lock" is a copy of `bash`
//! running a script. Nothing touches the real system bus, the real session
//! bus or a compositor.

use std::collections::HashMap;
use std::fs;
use std::io::{PipeReader, Read};
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use celestina::command::Verb;
use celestina::copy;
use celestina::energy::logind::{LogindBus, LogindEvent, Outcome};
use celestina::energy::EnergyEvent;
use celestina::lock::notify::NoticeBus;
use celestina::lock::sleep::PREPARING_RECHECK;
use celestina::lock::supervisor::Config;
use celestina::lock::{Live, LockHandle, LockMessage, Wiring};
use celestina::message::Message;
use celestina::wallpaper::thumbnail::ThumbnailStore;
use celestina_fake_system::{FakeSystem, Services};
use celestina_shell::session::PowerAction;
use celestina_shell::settings::{Duty, Settings};
use zbus::blocking::connection::Builder;
use zbus::blocking::Connection;
use zbus::zvariant::{self, OwnedValue};

const WAIT: Duration = Duration::from_secs(10);
const LOGIND: &str = "org.freedesktop.login1";
const LOGIND_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const NOTIFICATIONS: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";

/// One world at a time (round 4, I-α). The fake logind and the shell's workers
/// share this process, so a descriptor the bus library receives for one test
/// (`Inhibit` returns one, without close-on-exec) is inherited by whatever
/// another test starts in the same instant: a fake lock then held an inhibitor
/// of a different test, and `let_go` timed out (2 of 99 runs in debug). The
/// worlds therefore take turns; the production fix is in the lock itself,
/// which closes what it inherits, and in the worker, which drops the reply at
/// once.
static ONE_WORLD_AT_A_TIME: Mutex<()> = Mutex::new(());

type Lines = Arc<Mutex<Vec<String>>>;
type Readers = Arc<Mutex<Vec<PipeReader>>>;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.login1")]
enum LoginError {
    #[zbus(error)]
    ZBus(zbus::Error),
    OperationInProgress(String),
}

/// A stand-in logind: records every call, hands out the write end of a pipe
/// for `Inhibit` (keeping the read end, to see when the client lets go), and
/// notes whether the lock had said `locked` when `Suspend` arrived.
struct FakeLogind {
    calls: Lines,
    readers: Readers,
    busy: Arc<AtomicBool>,
    /// What the `PreparingForSleep` property answers.
    preparing: Arc<AtomicBool>,
    /// How many reads of the property still fail.
    fail_reads: Arc<AtomicUsize>,
    /// How many times the property was read.
    reads: Arc<AtomicUsize>,
    locked_marker: PathBuf,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl FakeLogind {
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> Result<zvariant::OwnedFd, LoginError> {
        let _ = why;
        self.calls
            .lock()
            .expect("calls")
            .push(format!("Inhibit {what}|{who}|{mode}"));
        if self.busy.load(Ordering::Acquire) {
            return Err(LoginError::OperationInProgress(
                "a sleep is pending".to_owned(),
            ));
        }
        let (reader, writer) = std::io::pipe()
            .map_err(|err| LoginError::ZBus(zbus::Error::InputOutput(err.into())))?;
        self.readers.lock().expect("readers").push(reader);
        Ok(zvariant::OwnedFd::from(OwnedFd::from(writer)))
    }

    #[zbus(property)]
    fn preparing_for_sleep(&self) -> zbus::fdo::Result<bool> {
        self.reads.fetch_add(1, Ordering::AcqRel);
        // `fetch_update` is deprecated and its `try_update` successor needs
        // Rust 1.95 (MSRV is 1.89), so decrement-if-positive by hand.
        let mut left = self.fail_reads.load(Ordering::Acquire);
        let failing = loop {
            let Some(next) = left.checked_sub(1) else {
                break false;
            };
            match self.fail_reads.compare_exchange_weak(
                left,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break true,
                Err(seen) => left = seen,
            }
        };
        if failing {
            return Err(zbus::fdo::Error::Failed("a wedged logind".to_owned()));
        }
        Ok(self.preparing.load(Ordering::Acquire))
    }

    fn can_suspend(&self) -> String {
        self.calls.lock().expect("calls").push("CanSuspend".into());
        "yes".to_owned()
    }

    fn suspend(&self, interactive: bool) {
        let locked = self.locked_marker.exists();
        self.calls
            .lock()
            .expect("calls")
            .push(format!("Suspend {interactive} locked={locked}"));
    }
}

/// A stand-in notification server.
struct FakeNotifications {
    notes: Lines,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl FakeNotifications {
    fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        _icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        _timeout: i32,
    ) -> u32 {
        self.notes
            .lock()
            .expect("notes")
            .push(format!("{app_name}|{summary}|{body}"));
        1
    }
}

/// A `dbus-daemon` on an address the test chooses, so it can be killed (by
/// its handle) and started again where it was.
struct Daemon {
    child: Child,
    socket: PathBuf,
    stopped: bool,
}

impl Daemon {
    fn start(socket: PathBuf) -> Daemon {
        let _ = fs::remove_file(&socket);
        let child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile"])
            .arg(format!("--address=unix:path={}", socket.display()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon starts");
        let end = Instant::now() + WAIT;
        while !socket.exists() {
            assert!(Instant::now() < end, "the bus socket never appeared");
            std::thread::sleep(Duration::from_millis(10));
        }
        Daemon {
            child,
            socket,
            stopped: false,
        }
    }

    /// Kills the daemon by its handle and waits for it.
    fn stop(&mut self) {
        if !self.stopped {
            self.stopped = true;
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = fs::remove_file(&self.socket);
        }
    }

    fn address(&self) -> String {
        format!("unix:path={}", self.socket.display())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The private "system bus": a throw-away one, or one that can restart.
enum SystemBus {
    Fixed(FakeSystem),
    Restartable(Daemon),
}

impl SystemBus {
    fn address(&self) -> String {
        match self {
            SystemBus::Fixed(bus) => bus.address().to_owned(),
            SystemBus::Restartable(daemon) => daemon.address(),
        }
    }
}

/// Both buses, the fakes on them, and the lock's rig.
struct World {
    // Connections close before their daemons go, and the turn is given back
    // last.
    logind: Option<Connection>,
    _notifications: Connection,
    system: SystemBus,
    session: FakeSystem,
    dir: PathBuf,
    calls: Lines,
    readers: Readers,
    busy: Arc<AtomicBool>,
    preparing: Arc<AtomicBool>,
    fail_reads: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    notes: Lines,
    _turn: MutexGuard<'static, ()>,
}

/// The inheritable descriptors the test process was started with, as
/// `N -> target`. A runner may pass extra pipes down without close-on-exec;
/// they are not the code under test's leaks, so the descriptor tests ignore
/// exactly those (same number and same target).
static INHERITED: OnceLock<Vec<String>> = OnceLock::new();

fn take_turn() -> MutexGuard<'static, ()> {
    let turn = ONE_WORLD_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // The first holder gets here before any world, bus or worker exists, so
    // this scan is what the process inherited and nothing else.
    INHERITED.get_or_init(raw_inheritable_descriptors);
    turn
}

fn serve_logind(address: &str, fake: FakeLogind) -> Connection {
    Builder::address(address)
        .expect("address")
        .name(LOGIND)
        .expect("name")
        .serve_at(LOGIND_PATH, fake)
        .expect("path")
        .build()
        .expect("the fake logind starts")
}

impl World {
    fn start(name: &str) -> World {
        let turn = take_turn();
        let system = FakeSystem::start(&format!("sleep-sys-{name}"), Services::none())
            .expect("a private system bus");
        Self::on(name, SystemBus::Fixed(system), turn)
    }

    /// A world whose system bus can be killed and brought back.
    fn start_restartable(name: &str) -> World {
        let turn = take_turn();
        let socket = std::env::temp_dir().join(format!(
            "celestina-lock-sleep-bus-{name}-{}",
            std::process::id()
        ));
        Self::on(name, SystemBus::Restartable(Daemon::start(socket)), turn)
    }

    fn on(name: &str, system: SystemBus, turn: MutexGuard<'static, ()>) -> World {
        let session = FakeSystem::start(&format!("sleep-ses-{name}"), Services::none())
            .expect("a private session bus");
        let dir = std::env::temp_dir().join(format!(
            "celestina-lock-sleep-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        // Links, not copies: nothing is written, so no concurrent `fork` of
        // another test can hold a write descriptor open on a file this one
        // executes (`ETXTBSY`).
        std::os::unix::fs::symlink("/usr/bin/bash", dir.join("celestina-lock"))
            .expect("a fake lock");
        std::os::unix::fs::symlink("/usr/bin/true", dir.join("celestina-lock-verify"))
            .expect("a fake verifier");
        let calls = Lines::default();
        let readers = Readers::default();
        let busy = Arc::new(AtomicBool::new(false));
        let preparing = Arc::new(AtomicBool::new(false));
        let fail_reads = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let notes = Lines::default();
        let logind = serve_logind(
            &system.address(),
            FakeLogind {
                calls: Arc::clone(&calls),
                readers: Arc::clone(&readers),
                busy: Arc::clone(&busy),
                preparing: Arc::clone(&preparing),
                fail_reads: Arc::clone(&fail_reads),
                reads: Arc::clone(&reads),
                locked_marker: dir.join("locked"),
            },
        );
        let notifications = Builder::address(session.address())
            .expect("address")
            .name(NOTIFICATIONS)
            .expect("name")
            .serve_at(
                NOTIFICATIONS_PATH,
                FakeNotifications {
                    notes: Arc::clone(&notes),
                },
            )
            .expect("path")
            .build()
            .expect("the fake notification server starts");
        World {
            logind: Some(logind),
            _notifications: notifications,
            system,
            session,
            dir,
            calls,
            readers,
            busy,
            preparing,
            fail_reads,
            reads,
            notes,
            _turn: turn,
        }
    }

    /// Kills the system bus (and with it the fake logind and the worker's
    /// connection) and starts it again at the same address with a fresh fake
    /// logind that answers as the fields say.
    fn restart_system_bus(&mut self) {
        self.logind = None;
        let SystemBus::Restartable(old) = &mut self.system else {
            panic!("this world's bus cannot restart");
        };
        old.stop();
        let socket = old.socket.clone();
        self.system = SystemBus::Restartable(Daemon::start(socket));
        self.logind = Some(serve_logind(
            &self.system.address(),
            FakeLogind {
                calls: Arc::clone(&self.calls),
                readers: Arc::clone(&self.readers),
                busy: Arc::clone(&self.busy),
                preparing: Arc::clone(&self.preparing),
                fail_reads: Arc::clone(&self.fail_reads),
                reads: Arc::clone(&self.reads),
                locked_marker: self.dir.join("locked"),
            },
        ));
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls").clone()
    }

    fn inhibits(&self) -> usize {
        self.calls()
            .iter()
            .filter(|call| call.starts_with("Inhibit"))
            .count()
    }

    /// Emits logind's `PrepareForSleep` from a connection of its own, as the
    /// name's owner would.
    fn prepare_for_sleep(&self, starting: bool) {
        // The property agrees with the signal, as logind's does.
        self.preparing.store(starting, Ordering::Release);
        self.logind
            .as_ref()
            .expect("logind is up")
            .emit_signal(
                None::<&str>,
                LOGIND_PATH,
                MANAGER,
                "PrepareForSleep",
                &(starting,),
            )
            .expect("the signal goes out");
    }

    fn config(&self, script: &str, pam_installed: bool) -> Config {
        Config {
            dir: self.dir.clone(),
            lock_file: "celestina-lock",
            verifier_file: "celestina-lock-verify",
            args: vec!["-c".into(), script.into()],
            env: vec![("XDG_RUNTIME_DIR".into(), self.dir.clone().into())],
            pam_existing: Arc::new(move || {
                if pam_installed {
                    vec![PathBuf::from("/etc/pam.d/celestina-lock")]
                } else {
                    Vec::new()
                }
            }),
            thumbnails: ThumbnailStore::default(),
            utc_offset: Arc::new(|| None),
        }
    }
}

impl World {
    /// Ends the fake locks this world started that still run, by the pids
    /// their scripts recorded and never by name (finding I-D). A pid is
    /// signalled only while its command line still names this world's
    /// scratch directory, so a reused pid is left alone.
    fn end_fake_locks(&self) {
        let Ok(pids) = fs::read_to_string(self.file("pids")) else {
            return;
        };
        let here = self.dir.display().to_string();
        for pid in pids
            .lines()
            .filter_map(|line| line.trim().parse::<i32>().ok())
        {
            let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) else {
                continue;
            };
            let cmdline = String::from_utf8_lossy(&cmdline);
            if !cmdline.starts_with("celestina-lock") || !cmdline.contains(&here) {
                continue;
            }
            if let Some(pid) = rustix::process::Pid::from_raw(pid) {
                let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
            }
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        self.end_fake_locks();
        let _ = fs::remove_dir_all(&self.dir);
        let _ = (&self.system, &self.session);
    }
}

/// The shell's loop, played by the test.
struct Shell {
    handle: LockHandle,
    messages: Receiver<Message>,
    availability: Arc<Mutex<Vec<bool>>>,
    logind_events: Vec<LogindEvent>,
}

fn shell(world: &World, script: &str, pam_installed: bool) -> Shell {
    let (sent, messages): (Sender<Message>, Receiver<Message>) = channel();
    let sent = Mutex::new(sent);
    let availability = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&availability);
    let config = world.config(script, pam_installed);
    let system = LogindBus::Address(world.system.address());
    let notices = NoticeBus::Address(world.session.address().to_owned());
    let handle = LockHandle::with_effects(
        move |generation| {
            Box::new(Live::new(Wiring {
                send: Arc::new(move |message| sent.lock().expect("sender").send(message).is_ok()),
                availability: Box::new(move |available| {
                    seen.lock().expect("availability").push(available);
                }),
                generation,
                config: Ok(config),
                system,
                notices,
            }))
        },
        Box::new(Instant::now),
    );
    Shell {
        handle,
        messages,
        availability,
        logind_events: Vec::new(),
    }
}

fn settings(lock: Duty) -> Settings {
    let mut settings = Settings::default();
    settings.duties.lock = lock;
    settings
}

impl Shell {
    /// Delivers what the workers sent so far to the handle, as the loop would.
    fn pump(&mut self) {
        while let Ok(message) = self.messages.try_recv() {
            match message {
                Message::Lock(message) => self.handle.message(message),
                Message::Energy(EnergyEvent::Logind(event)) => self.logind_events.push(event),
                _ => {}
            }
        }
    }

    fn available(&self) -> Vec<bool> {
        self.availability.lock().expect("availability").clone()
    }
}

/// Waits until `condition` holds.
fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A script that covers (with a pause before `locked`). The first child
/// waits to be told to unlock; a later one stays locked until the scratch
/// directory is gone.
fn covering_script(world: &World) -> String {
    format!(
        // Every loop ends when the scratch directory is gone (finding I-D):
        // `World::drop` removes it, and a loop that only waited for the
        // unlock file would run for ever, forking, in the user manager.
        "echo $$ >> {dir}/pids; \
         n=$(ls {dir}/run.* 2>/dev/null | wc -l); touch {dir}/run.$n; \
         echo 'hello 1'; echo requested; sleep 0.3; touch {locked}; echo locked; \
         if [ \"$n\" -eq 0 ]; then \
           while [ -d {dir} ] && [ ! -e {unlock} ]; do sleep 0.02; done; \
           [ -e {unlock} ] && echo unlocked; exit 0; \
         fi; \
         while [ -d {dir} ]; do sleep 0.05; done; exit 0",
        dir = world.dir.display(),
        locked = world.file("locked").display(),
        unlock = world.file("unlock").display()
    )
}

/// Whether the pipe's other end is closed: the inhibitor was let go.
fn let_go(readers: &Readers, index: usize) -> bool {
    let mut readers = readers.lock().expect("readers");
    let Some(reader) = readers.get_mut(index) else {
        return false;
    };
    let mut buf = [0u8; 1];
    // Nothing is written: the read blocks while a writer lives, so poll first.
    let fd = &*reader;
    let mut fds = [rustix::event::PollFd::new(fd, rustix::event::PollFlags::IN)];
    let zero = rustix::time::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    match rustix::event::poll(&mut fds, Some(&zero)) {
        Ok(n) if n > 0 => matches!(reader.read(&mut buf), Ok(0)),
        _ => false,
    }
}

#[test]
fn t_s1_inhibit_sleep_delay_released_on_locked_reheld_after_unlock_and_suspend_only_after_locked() {
    let world = World::start("s1");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    assert_eq!(
        world.calls()[0],
        "Inhibit sleep|Celestina|delay",
        "the documented delay inhibitor"
    );
    assert_eq!(shell.available(), [true], "the idle chain may use it");

    // A person asks for lock and suspend. The inhibitor is let go on
    // `locked`, and `Suspend` arrives after it, never before.
    shell.handle.verb(&Verb::LockAndSuspend);
    eventually("the suspend", || {
        shell.pump();
        {
            world.calls().iter().any(|call| call.starts_with("Suspend"))
        }
    });
    assert!(
        world
            .calls()
            .contains(&"Suspend false locked=true".to_owned()),
        "{:?}",
        world.calls()
    );
    eventually("the inhibitor to be released", || let_go(&world.readers, 0));
    eventually("the suspend outcome", || {
        shell.pump();
        shell
            .logind_events
            .contains(&LogindEvent::Power(PowerAction::Suspend, Outcome::Done))
    });

    // After the unlock the inhibitor is taken again.
    fs::write(world.file("unlock"), b"").expect("unlock");
    eventually("the second inhibitor", || {
        shell.pump();
        world.inhibits() == 2
    });
}

#[test]
fn no_inhibitor_is_taken_while_preparing() {
    let world = World::start("preparing");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });

    // logind announces a sleep: the policy asks the compositor for a lock
    // (the inhibitor is kept until it is confirmed), then lets go.
    world.prepare_for_sleep(true);
    eventually("the inhibitor to be let go", || {
        shell.pump();
        let_go(&world.readers, 0)
    });
    assert!(world.file("locked").exists());
    // The person unlocks during the sleep: the lock is asked for again at
    // once (the session was just uncovered), and no inhibitor is taken now.
    fs::write(world.file("unlock"), b"").expect("unlock");
    eventually("the second lock child", || {
        shell.pump();
        world.file("run.1").exists()
    });
    // Let it lock, with the messages delivered meanwhile.
    let settle = Instant::now() + Duration::from_millis(800);
    eventually("the second child to lock", || {
        shell.pump();
        Instant::now() >= settle
    });
    assert_eq!(world.inhibits(), 1, "none while the sleep is prepared");

    // The sleep ends with the session covered: still none to take.
    world.prepare_for_sleep(false);
    let settle = Instant::now() + Duration::from_millis(400);
    eventually("the end of the sleep to be delivered", || {
        shell.pump();
        Instant::now() >= settle
    });
    assert_eq!(world.inhibits(), 1, "a covered session needs none");
}

#[test]
fn a_busy_answer_is_not_a_failure_and_the_inhibitor_returns_after_the_sleep() {
    let world = World::start("busy");
    // logind refuses delay inhibitors (a sleep is pending): the expected
    // answer to a late request, not a bus failure.
    world.busy.store(true, Ordering::Release);
    let mut shell = shell(&world, "exit 0", true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the refused request", || {
        shell.pump();
        world.inhibits() == 1
    });
    // It is not asked for again until something changes.
    let settle = Instant::now() + Duration::from_millis(400);
    eventually("quiet", || {
        shell.pump();
        Instant::now() >= settle
    });
    assert_eq!(world.inhibits(), 1);
    assert!(
        world.readers.lock().expect("readers").is_empty(),
        "nothing was granted"
    );

    // The sleep goes by and ends: the policy takes one again.
    world.busy.store(false, Ordering::Release);
    world.prepare_for_sleep(true);
    world.prepare_for_sleep(false);
    eventually("the inhibitor to be taken after the sleep", || {
        shell.pump();
        world.inhibits() >= 2 && !world.readers.lock().expect("readers").is_empty()
    });
}

#[test]
fn the_notices_reach_the_notification_owner_once_each() {
    // The duty is off: a refused `lock suspend` still reaches the person, once
    // within the window.
    let world = World::start("notices");
    let mut shell = shell(&world, "exit 0", true);
    shell.handle.verb(&Verb::LockAndSuspend);
    shell.handle.verb(&Verb::LockAndSuspend);
    shell.handle.verb(&Verb::Lock);
    eventually("the notice", || {
        shell.pump();
        {
            !world.notes.lock().expect("notes").is_empty()
        }
    });
    // The window is 10 s: three identical requests, one notice.
    std::thread::sleep(Duration::from_millis(300));
    eventually("quiet", || {
        shell.pump();
        true
    });
    let notes = world.notes.lock().expect("notes").clone();
    assert_eq!(notes.len(), 1, "{notes:?}");
    let (title, body) = copy::lock_notice(celestina_shell::lock::Notice::Unavailable(
        celestina_shell::lock::Why::DutyOff,
    ));
    assert_eq!(notes[0], format!("Celestina|{title}|{body}"));
}

#[test]
fn a_missing_pam_file_posts_its_notice_at_duty_on_and_availability_stays() {
    let world = World::start("pam");
    let mut shell = shell(&world, "exit 0", false);
    shell.handle.apply(&settings(Duty::On));
    eventually("the PAM notice", || {
        shell.pump();
        {
            !world.notes.lock().expect("notes").is_empty()
        }
    });
    let notes = world.notes.lock().expect("notes").clone();
    assert!(notes[0].ends_with(copy::NOTICE_PAM_MISSING), "{notes:?}");
    assert_eq!(shell.available(), [true], "the lock still covers");
}

#[test]
fn shutdown_joins_within_the_bound() {
    // Spike Q10 in practice: the workers, a hand-polled signal stream among
    // them, end promptly with a live bus.
    let world = World::start("shutdown");
    let mut shell = shell(&world, "exit 0", true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the inhibitor", || {
        shell.pump();
        world.inhibits() == 1 && !world.readers.lock().expect("readers").is_empty()
    });
    let started = Instant::now();
    shell.handle.stop();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stopping took {:?}",
        started.elapsed()
    );
    // The inhibitor is closed with the worker.
    // The fake's own side may still be dropping its copy of the descriptor it
    // sent; the worker's end is closed when it has been joined.
    eventually("the inhibitor to be closed with the worker", || {
        let_go(&world.readers, 0)
    });
    // Once the loop's side is gone a late worker message is harmless.
    drop(shell);
}

#[test]
fn switching_the_duty_off_stops_the_workers_and_lets_the_inhibitor_go() {
    let world = World::start("off");
    let mut shell = shell(&world, "exit 0", true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    shell.handle.apply(&settings(Duty::Off));
    eventually("the inhibitor to be let go", || {
        shell.pump();
        {
            let_go(&world.readers, 0)
        }
    });
    assert_eq!(shell.available(), [true, false]);
    // And back on: a fresh run takes a fresh inhibitor.
    shell.handle.apply(&settings(Duty::On));
    eventually("the second inhibitor", || {
        shell.pump();
        world.inhibits() == 2
    });
    let _ = LockMessage::Idle(celestina_shell::lock::Event::Tick);
}

/// Sends a `PrepareForSleep` from a connection that is not logind's, as a
/// unicast signal to every other connection of the bus: the match rule of
/// the worker does not apply to a message addressed to it.
fn spoof_prepare_for_sleep(world: &World, starting: bool) {
    let spoofer = Builder::address(world.system.address().as_str())
        .expect("address")
        .build()
        .expect("a second client");
    let me = spoofer.unique_name().expect("a name").to_string();
    let names: Vec<String> = spoofer
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListNames",
            &(),
        )
        .expect("ListNames")
        .body()
        .deserialize()
        .expect("names");
    let mut sent = 0;
    for name in names.iter().filter(|n| n.starts_with(':') && **n != me) {
        spoofer
            .emit_signal(
                Some(name.as_str()),
                LOGIND_PATH,
                MANAGER,
                "PrepareForSleep",
                &(starting,),
            )
            .expect("the unicast goes out");
        sent += 1;
    }
    assert!(sent >= 2, "the worker and the fake logind were addressed");
}

/// Lets `shell` deliver for `span`.
fn settle(shell: &mut Shell, span: Duration) {
    let end = Instant::now() + span;
    eventually("the settling time", || {
        shell.pump();
        Instant::now() >= end
    });
}

#[test]
fn a_unicast_prepare_for_sleep_from_another_process_is_ignored() {
    // Finding C2 (b): any process of the user can address a signal to the
    // worker; only the owner of logind's name is believed.
    let world = World::start("spoof");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    spoof_prepare_for_sleep(&world, true);
    settle(&mut shell, Duration::from_millis(700));
    assert!(
        !world.file("run.0").exists(),
        "a forged announcement started a lock"
    );
    assert!(
        !let_go(&world.readers, 0),
        "a forged announcement released the inhibitor"
    );
    // The real announcement, in the same rig, is served.
    world.prepare_for_sleep(true);
    eventually("the lock for the real announcement", || {
        shell.pump();
        world.file("run.0").exists()
    });
}

#[test]
fn a_sleep_already_under_way_when_the_worker_subscribes_is_learned_from_the_property() {
    // Finding C2: no signal is ever sent; the property says it.
    let world = World::start("property");
    world.preparing.store(true, Ordering::Release);
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the lock the property asked for", || {
        shell.pump();
        world.file("run.0").exists()
    });
}

#[test]
fn a_burst_of_signals_cannot_push_an_order_out_of_the_worker() {
    // Finding I3: orders and bus wake-ups used to share a queue of eight.
    let world = World::start("burst");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    // A flood the worker has not read yet, from a connection of the fake
    // logind, then the policy's order to let go.
    for _ in 0..200 {
        world.prepare_for_sleep(false);
    }
    shell.handle.verb(&Verb::Lock);
    eventually("the lock to be confirmed", || {
        shell.pump();
        world.file("locked").exists()
    });
    eventually("the inhibitor to be let go", || {
        shell.pump();
        let_go(&world.readers, 0)
    });
}

#[test]
fn after_the_bus_comes_back_the_property_is_read_again() {
    // Finding C2 (a): the worker lost its connection, perhaps a
    // `PrepareForSleep` with it. On reconnecting it asks logind, and the
    // policy is told without any signal.
    let mut world = World::start_restartable("reconnect-property");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    world.restart_system_bus();
    world.preparing.store(true, Ordering::Release);
    // The worker notices the loss, waits its reconnect period and asks.
    let end = Instant::now() + Duration::from_secs(20);
    while !world.file("run.0").exists() {
        assert!(Instant::now() < end, "never resynchronised");
        shell.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_failed_inhibitor_is_asked_for_again_when_the_bus_comes_back() {
    // Finding I4.
    let mut world = World::start_restartable("reconnect-inhibitor");
    world.busy.store(true, Ordering::Release);
    let mut shell = shell(&world, "exit 0", true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the refused request", || {
        shell.pump();
        world.inhibits() == 1
    });
    settle(&mut shell, Duration::from_millis(300));
    assert_eq!(world.inhibits(), 1, "not asked for again by itself");
    world.busy.store(false, Ordering::Release);
    world.restart_system_bus();
    let end = Instant::now() + Duration::from_secs(20);
    while world.readers.lock().expect("readers").is_empty() {
        assert!(Instant::now() < end, "the inhibitor was never retried");
        shell.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_lost_end_of_sleep_does_not_relock_the_session_after_every_unlock() {
    // Finding I-C: the `false` of the resume is lost (here: never sent; in
    // life: a batch whose sender could not be vouched for). The policy stays
    // `Preparing`, and every unlock used to start another lock. The worker
    // keeps reading the property while the last value it published is `true`.
    let world = World::start("lost-false");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    world.prepare_for_sleep(true);
    eventually("the lock", || {
        shell.pump();
        world.file("locked").exists()
    });
    // The policy let the inhibitor go on `locked`, and the worker's reading
    // that follows that order is done: only then does the machine resume.
    eventually("the inhibitor to be let go", || {
        shell.pump();
        let_go(&world.readers, 0)
    });
    settle(&mut shell, Duration::from_millis(500));
    // logind's property says it is over; no signal arrives.
    world.preparing.store(false, Ordering::Release);
    settle(&mut shell, PREPARING_RECHECK + Duration::from_millis(1500));
    // The person unlocks: the sleep is over, so nothing locks again and the
    // inhibitor comes back.
    fs::write(world.file("unlock"), b"").expect("unlock");
    eventually("the inhibitor after the unlock", || {
        shell.pump();
        world.inhibits() == 2
    });
    settle(&mut shell, Duration::from_millis(800));
    assert!(
        !world.file("run.1").exists(),
        "the session was locked again after the sleep had ended"
    );
}

/// The numbers of the descriptors of `pid` at 3 or above, from `/proc`.
fn open_descriptors(pid: &str) -> Vec<u32> {
    let mut numbers: Vec<u32> = fs::read_dir(format!("/proc/{pid}/fd"))
        .map(|dir| {
            dir.filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().to_string_lossy().parse().ok())
                .filter(|number| *number >= 3)
                .collect()
        })
        .unwrap_or_default();
    numbers.sort_unstable();
    numbers
}

/// The descriptors of this process that a child would inherit: open, 3 or
/// above and without close-on-exec, except those the process started with
/// (`INHERITED`).
fn inheritable_descriptors() -> Vec<String> {
    let inherited = INHERITED.get().map(Vec::as_slice).unwrap_or_default();
    let mut found = raw_inheritable_descriptors();
    found.retain(|entry| !inherited.contains(entry));
    found
}

/// Every inheritable descriptor of this process, unfiltered. The directory
/// listing's own descriptor is not one (it is closed when the listing ends).
fn raw_inheritable_descriptors() -> Vec<String> {
    let mut found = Vec::new();
    for number in open_descriptors("self") {
        let Ok(info) = fs::read_to_string(format!("/proc/self/fdinfo/{number}")) else {
            continue;
        };
        let flags = info
            .lines()
            .find_map(|line| line.strip_prefix("flags:"))
            .and_then(|value| u32::from_str_radix(value.trim(), 8).ok());
        // O_CLOEXEC is 0o2000000.
        if flags.is_some_and(|flags| flags & 0o2000000 == 0) {
            let target = fs::read_link(format!("/proc/self/fd/{number}"))
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            found.push(format!("{number} -> {target}"));
        }
    }
    found
}

#[test]
fn the_inhibit_reply_leaves_no_inheritable_descriptor_in_the_shell() {
    // Round 4, finding I-α: the bus library receives the inhibitor's
    // descriptor without close-on-exec, and the original lives as long as the
    // reply message does (here: until the worker next read its stream, which
    // at rest is never). Whatever the shell started in that time inherited it.
    let world = World::start("no-leak");
    let mut shell = shell(&world, "exit 0", true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the inhibitor", || {
        shell.pump();
        world.inhibits() == 1 && !world.readers.lock().expect("readers").is_empty()
    });
    let end = Instant::now() + Duration::from_secs(3);
    let mut leaked = inheritable_descriptors();
    while !leaked.is_empty() && Instant::now() < end {
        shell.pump();
        std::thread::sleep(Duration::from_millis(10));
        leaked = inheritable_descriptors();
    }
    assert!(
        leaked.is_empty(),
        "the shell holds inheritable descriptors: {leaked:?}"
    );
}

#[test]
fn a_lock_started_with_the_inhibitor_held_inherits_only_the_standard_streams() {
    // Round 4, finding I-α, as it happens in life: `Ending` with something
    // pending asks for a spawn and for the inhibitor in one batch, and the
    // child must not keep the inhibitor (it would delay every suspend of a
    // locked session by `InhibitDelayMaxSec`). The fake lock is a bash that
    // does not close anything itself, so this is the shell's own share.
    let world = World::start("child-fds");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the inhibitor", || {
        shell.pump();
        world.inhibits() == 1 && !world.readers.lock().expect("readers").is_empty()
    });
    shell.handle.verb(&Verb::Lock);
    eventually("the fake lock to run", || {
        shell.pump();
        world.file("run.0").exists()
    });
    let pids = fs::read_to_string(world.file("pids")).expect("the script wrote its pid");
    let pid = pids.lines().next().expect("a pid").trim().to_owned();
    let inherited = INHERITED.get().map(Vec::as_slice).unwrap_or_default();
    let held: Vec<u32> = open_descriptors(&pid)
        .into_iter()
        .filter(|number| {
            let target = fs::read_link(format!("/proc/{pid}/fd/{number}"))
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            !inherited.contains(&format!("{number} -> {target}"))
        })
        .collect();
    assert_eq!(
        held,
        Vec::<u32>::new(),
        "the child holds descriptors beyond 0, 1 and 2"
    );
    // And the inhibitor is let go on `locked`, which only happens if no child
    // keeps a copy of it.
    eventually("the inhibitor to be let go", || {
        shell.pump();
        let_go(&world.readers, 0)
    });
}

#[test]
fn a_property_read_that_fails_once_is_retried_and_the_sleep_is_covered() {
    // Round 4, m-3: a forged unicast reaches the worker (it is not vouched
    // for, so only the property can settle it), logind's property says
    // `true`, and the first read fails. The read stays owed and is tried
    // again after the base period: a lock appears within three seconds. A
    // worker that dropped the mark after a failed read (or that went back to
    // sleeping on `recv`) would never start one.
    let world = World::start("failed-first-read");
    let mut shell = shell(&world, &covering_script(&world), true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    // Everything the start asked of the property is done.
    settle(&mut shell, Duration::from_millis(500));
    world.preparing.store(true, Ordering::Release);
    world.fail_reads.store(1, Ordering::Release);
    let sent = Instant::now();
    spoof_prepare_for_sleep(&world, false);
    let end = sent + Duration::from_secs(3);
    while !world.file("run.0").exists() {
        assert!(
            Instant::now() < end,
            "no lock within three seconds of a failed read"
        );
        shell.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        world.fail_reads.load(Ordering::Acquire),
        0,
        "the failing read was the one the worker made"
    );
}

#[test]
fn a_logind_that_keeps_failing_is_not_hammered_with_reads() {
    // Round 4, m-3 residual: the reply to a failed read wakes the worker like
    // any message of its stream, and it read again at once, again and again
    // (6936 reads in 1.5 s were measured). A failed read now waits its
    // backoff, 2 s after the first failure, whatever wakes the worker.
    let world = World::start("hammer");
    let mut shell = shell(&world, "exit 0", true);
    shell.handle.apply(&settings(Duty::On));
    eventually("the first inhibitor", || {
        shell.pump();
        world.inhibits() == 1
    });
    settle(&mut shell, Duration::from_millis(500));
    world.preparing.store(true, Ordering::Release);
    world.fail_reads.store(usize::MAX, Ordering::Release);
    let before = world.reads.load(Ordering::Acquire);
    spoof_prepare_for_sleep(&world, false);
    settle(&mut shell, Duration::from_millis(1500));
    let made = world.reads.load(Ordering::Acquire) - before;
    assert!(
        (1..=2).contains(&made),
        "{made} reads of a failing property in 1.5 s"
    );
}
