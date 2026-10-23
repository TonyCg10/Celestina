//! A fake `org.freedesktop.login1.Manager` for the lock rehearsal
//! (`scripts/lock-rehearsal.sh`, CEL-2-H design §9.2).
//!
//! It starts a `dbus-daemon` of its own, serves the three `Can*` probes the
//! shell makes, `Inhibit` (a pipe whose read end it keeps, so it sees the
//! moment the shell lets go) and `Suspend`, and prints one line,
//! `DBUS_SYSTEM_BUS_ADDRESS=<address>`. Nothing here reaches the real system
//! bus, the real session bus or a compositor, and `Suspend` suspends nothing:
//! it is only written down. There is no `Reboot` and no `PowerOff`, so an
//! accident cannot even be recorded as success.
//!
//! Every call and every release of an inhibitor is appended to the file named
//! by `--log`, one line each: `<epoch milliseconds> <what>`. Commands, one per
//! line on standard input (the run ends when it closes):
//!
//! - `sleep`: emit `PrepareForSleep(true)`, as logind does before it sleeps.
//! - `wake`: emit `PrepareForSleep(false)`.
//!
//! With `--emulate-sleep` a `Suspend` call also emits `PrepareForSleep(true)`
//! at once and `PrepareForSleep(false)` two seconds later, as the real logind
//! would around a sleep (the machine does not sleep).
//!
//! Usage: `fake_logind --log FILE [--emulate-sleep]`.

use std::fs::OpenOptions;
use std::io::{PipeReader, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use celestina_fake_system::{FakeSystem, Services};
use rustix::event::{poll, PollFd, PollFlags};
use zbus::blocking::connection::Builder;
use zbus::blocking::Connection;
use zbus::zvariant;

const LOGIND: &str = "org.freedesktop.login1";
const LOGIND_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";

/// The call log: appended to a file, one timestamped line at a time.
#[derive(Clone)]
struct Log(Arc<Mutex<std::fs::File>>);

impl Log {
    fn write(&self, what: &str) {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        if let Ok(mut file) = self.0.lock() {
            // A failing log must not change what the fake answers.
            let _ = writeln!(file, "{millis} {what}");
        }
    }
}

struct Manager {
    log: Log,
    emulate_sleep: bool,
    /// Set once the bus connection exists, for the emulated signals.
    connection: Arc<Mutex<Option<Connection>>>,
    next_inhibitor: Arc<Mutex<u32>>,
}

/// Emits `PrepareForSleep(starting)` from the fake's own connection.
fn emit(connection: &Arc<Mutex<Option<Connection>>>, starting: bool) {
    if let Ok(guard) = connection.lock() {
        if let Some(connection) = guard.as_ref() {
            let _ = connection.emit_signal(
                None::<&str>,
                LOGIND_PATH,
                MANAGER,
                "PrepareForSleep",
                &(starting,),
            );
        }
    }
}

/// Waits until every write end of the pipe is closed (the shell released the
/// inhibitor) and writes that down.
fn watch(reader: PipeReader, id: u32, log: Log) {
    std::thread::spawn(move || {
        let borrowed = reader.as_fd();
        let mut fds = [PollFd::new(&borrowed, PollFlags::empty())];
        loop {
            // Only a hang-up is asked for; nothing is ever written to it.
            match poll(&mut fds, None) {
                Ok(_) if fds[0].revents().contains(PollFlags::HUP) => break,
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(_) => break,
            }
        }
        log.write(&format!("InhibitorReleased {id}"));
    });
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        _why: &str,
        mode: &str,
    ) -> zbus::fdo::Result<zvariant::OwnedFd> {
        let id = {
            let mut next = self
                .next_inhibitor
                .lock()
                .map_err(|_| zbus::fdo::Error::Failed("poisoned".to_owned()))?;
            *next += 1;
            *next
        };
        self.log.write(&format!("Inhibit {id} {what}|{who}|{mode}"));
        let (reader, writer) =
            std::io::pipe().map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        watch(reader, id, self.log.clone());
        Ok(zvariant::OwnedFd::from(OwnedFd::from(writer)))
    }

    fn can_suspend(&self) -> String {
        self.log.write("CanSuspend");
        "yes".to_owned()
    }

    fn can_reboot(&self) -> String {
        self.log.write("CanReboot");
        "yes".to_owned()
    }

    fn can_power_off(&self) -> String {
        self.log.write("CanPowerOff");
        "yes".to_owned()
    }

    fn suspend(&self, interactive: bool) {
        self.log.write(&format!("Suspend {interactive}"));
        if self.emulate_sleep {
            let connection = Arc::clone(&self.connection);
            let log = self.log.clone();
            std::thread::spawn(move || {
                log.write("emit PrepareForSleep true");
                emit(&connection, true);
                std::thread::sleep(Duration::from_secs(2));
                log.write("emit PrepareForSleep false");
                emit(&connection, false);
            });
        }
    }
}

fn main() -> ExitCode {
    let mut log_path = None;
    let mut emulate_sleep = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--log" => log_path = args.next(),
            "--emulate-sleep" => emulate_sleep = true,
            _ => {
                eprintln!("usage: fake_logind --log FILE [--emulate-sleep]");
                return ExitCode::from(2);
            }
        }
    }
    let Some(log_path) = log_path else {
        eprintln!("usage: fake_logind --log FILE [--emulate-sleep]");
        return ExitCode::from(2);
    };
    let file = match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("fake_logind: cannot open {log_path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let log = Log(Arc::new(Mutex::new(file)));
    let system = match FakeSystem::start("fake-logind", Services::none()) {
        Ok(system) => system,
        Err(error) => {
            eprintln!("fake_logind: {error}");
            return ExitCode::FAILURE;
        }
    };
    let slot: Arc<Mutex<Option<Connection>>> = Arc::default();
    let served = Builder::address(system.address())
        .and_then(|builder| builder.name(LOGIND))
        .and_then(|builder| {
            builder.serve_at(
                LOGIND_PATH,
                Manager {
                    log: log.clone(),
                    emulate_sleep,
                    connection: Arc::clone(&slot),
                    next_inhibitor: Arc::default(),
                },
            )
        })
        .and_then(zbus::blocking::connection::Builder::build);
    let connection = match served {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("fake_logind: cannot serve logind: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(connection);
    }
    let mut out = std::io::stdout();
    if writeln!(out, "DBUS_SYSTEM_BUS_ADDRESS={}", system.address())
        .and_then(|()| out.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    log.write("ready");
    // One command per line; end of input ends the run (and the daemon).
    for line in std::io::stdin().lines() {
        let Ok(line) = line else { break };
        match line.trim() {
            "sleep" => {
                log.write("emit PrepareForSleep true");
                emit(&slot, true);
            }
            "wake" => {
                log.write("emit PrepareForSleep false");
                emit(&slot, false);
            }
            "" => {}
            other => log.write(&format!("unknown command {other:?}")),
        }
    }
    log.write("stopped");
    ExitCode::SUCCESS
}
