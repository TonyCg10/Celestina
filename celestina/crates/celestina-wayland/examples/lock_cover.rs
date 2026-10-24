//! A throwaway client of the lock profile, for the nest only (CEL-2-H Task 3,
//! step 5): it requests the session lock, lets the toolkit cover every
//! output, prints one line per event with the time since the request, and
//! optionally unlocks after a delay.
//!
//! ```text
//! cargo run -p celestina-wayland --example lock_cover -- [--unlock-after-ms N]
//! ```
//!
//! It must run against the nest's Wayland socket (`WAYLAND_DISPLAY` from
//! `$XDG_RUNTIME_DIR/celestina-dev-session.env`), never the live session.
//! Keys are counted and never printed. It is not shipped.

use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

use celestina_wayland::{
    lock::{run_lock, ConnectPolicy, LockCtl, LockEvent, LockShell, UnlockCapability},
    InputEvent, OutputId,
};

enum Message {
    Unlock,
}

struct Probe {
    start: Instant,
    capability: Option<UnlockCapability>,
    keys: u32,
}

impl Probe {
    fn say(&self, line: &str) {
        let mut out = std::io::stdout().lock();
        // A closed stdout ends the probe quietly.
        let _ = writeln!(out, "{:>6} ms  {line}", self.start.elapsed().as_millis());
    }
}

impl LockShell for Probe {
    type Message = Message;

    fn event(&mut self, ev: LockEvent, _ctl: &mut LockCtl<'_>) {
        match ev {
            LockEvent::Locked(capability) => {
                self.capability = Some(capability);
                self.say("locked");
            }
            LockEvent::Finished => self.say("finished"),
            LockEvent::SurfaceConfigured { output, size } => {
                self.say(&format!(
                    "configured output {} {}x{}",
                    output.0, size.0, size.1
                ));
            }
            LockEvent::OutputAdded(info) => {
                self.say(&format!("output added {} id {}", info.name, info.id.0));
            }
            LockEvent::OutputRemoved(id) => self.say(&format!("output removed id {}", id.0)),
            LockEvent::Keyboard { present } => self.say(&format!("keyboard present={present}")),
            LockEvent::Redraw => self.say("redraw"),
        }
    }

    fn input(&mut self, output: OutputId, ev: InputEvent, _ctl: &mut LockCtl<'_>) {
        match ev {
            InputEvent::Key { pressed: true, .. } => {
                self.keys += 1;
                self.say(&format!("key press #{} on output {}", self.keys, output.0));
            }
            InputEvent::FocusEnter { .. } => {
                self.say(&format!("keyboard enter output {}", output.0))
            }
            InputEvent::FocusLost => self.say(&format!("keyboard leave output {}", output.0)),
            InputEvent::PointerMotion { .. } => {}
            _ => {}
        }
    }

    fn message(&mut self, msg: Message, ctl: &mut LockCtl<'_>) {
        match msg {
            Message::Unlock => {
                let Some(capability) = self.capability.take() else {
                    self.say("unlock requested before locked: ignored");
                    return;
                };
                match capability.unlock() {
                    Ok(()) => self.say("unlocked (round trip confirmed)"),
                    Err(err) => self.say(&format!("unlock not confirmed: {err}")),
                }
                ctl.stop();
            }
        }
    }
}

fn unlock_after_ms() -> Option<u64> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--unlock-after-ms" {
            return args.next().and_then(|value| value.parse().ok());
        }
    }
    None
}

fn main() {
    let unlock_after = unlock_after_ms();
    let probe = Probe {
        start: Instant::now(),
        capability: None,
        keys: 0,
    };
    let result = run_lock(ConnectPolicy::detect(true), probe, |sender, _handle| {
        println!("requested");
        if let Some(ms) = unlock_after {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(ms));
                let _ = sender.send(Message::Unlock);
            });
        }
        Ok(())
    });
    match result {
        Ok(()) => println!("stopped"),
        Err(err) => {
            eprintln!("lock_cover: {err:#}");
            std::process::exit(1);
        }
    }
}
