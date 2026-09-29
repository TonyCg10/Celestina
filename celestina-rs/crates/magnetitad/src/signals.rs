//! `Devices1`'s two signals, `Changed` and `Event`, sent from one owned
//! thread.
//!
//! Sessions report changes from the link's tokio runtime, and zbus's blocking
//! API may never run on a tokio thread (it drives zbus's own runtime there).
//! So a change only raises a flag, and this thread emits. It also coalesces:
//! a burst of changes, such as a phone's media ticks or a contact sync, is at
//! most one `Changed` per [`COALESCE`], and every consumer re-reads the whole
//! list once per burst instead of once per report.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::lock::LockOk;

/// The shortest gap between two emissions of the same signal.
const COALESCE: Duration = Duration::from_millis(100);

/// Which signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Signal {
    /// The device set or a device's state changed.
    Changed,
    /// A connection-log entry landed.
    Event,
}

#[derive(Default)]
struct Pending {
    changed: bool,
    event: bool,
    stopping: bool,
}

type Shared = Arc<(Mutex<Pending>, Condvar)>;

/// The emitting thread; raising a signal never blocks on the bus.
pub(crate) struct Signals {
    shared: Shared,
    join: Option<JoinHandle<()>>,
}

impl Signals {
    /// Emits on `connection`'s bus; with no bus, raising a signal is a no-op.
    pub(crate) fn on_bus(connection: Option<zbus::blocking::Connection>) -> Self {
        match connection {
            Some(connection) => Self::spawn(move |signal| {
                let name = match signal {
                    Signal::Changed => crate::devices::CHANGED_SIGNAL,
                    Signal::Event => crate::devices::EVENT_SIGNAL,
                };
                // Best-effort: a broken bus is not fatal.
                let _ = connection.emit_signal(
                    Option::<&str>::None,
                    crate::devices::OBJECT_PATH,
                    crate::devices::INTERFACE,
                    name,
                    &(),
                );
            }),
            None => Self {
                shared: Arc::default(),
                join: None,
            },
        }
    }

    /// Emits through `emit`, on a thread of its own.
    fn spawn(emit: impl Fn(Signal) + Send + 'static) -> Self {
        let shared: Shared = Arc::default();
        let worker = Arc::clone(&shared);
        let join = thread::Builder::new()
            .name("magnetita-signals".into())
            .spawn(move || run(&worker, emit))
            .ok();
        Self { shared, join }
    }

    pub(crate) fn raise(&self, signal: Signal) {
        if self.join.is_none() {
            return;
        }
        let (pending, wake) = &*self.shared;
        let mut pending = pending.lock_ok();
        match signal {
            Signal::Changed => pending.changed = true,
            Signal::Event => pending.event = true,
        }
        wake.notify_one();
    }
}

fn run(shared: &Shared, emit: impl Fn(Signal)) {
    let (pending, wake) = &**shared;
    loop {
        let (changed, event) = {
            let mut state = pending.lock_ok();
            while !state.changed && !state.event && !state.stopping {
                state = wake.wait(state).unwrap_or_else(|e| e.into_inner());
            }
            // What was raised before the stop is still sent.
            let taken = (state.changed, state.event);
            if taken == (false, false) {
                return;
            }
            state.changed = false;
            state.event = false;
            taken
        };
        if changed {
            emit(Signal::Changed);
        }
        if event {
            emit(Signal::Event);
        }
        thread::sleep(COALESCE);
    }
}

impl Drop for Signals {
    /// Sends what is pending, then joins the thread.
    fn drop(&mut self) {
        let (pending, wake) = &*self.shared;
        pending.lock_ok().stopping = true;
        wake.notify_one();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_is_one_emission_off_the_callers_thread_and_the_last_is_never_lost() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        let signals = Signals::spawn(move |signal| {
            record
                .lock_ok()
                .push((signal, thread::current().name().map(str::to_owned)));
        });
        for _ in 0..500 {
            signals.raise(Signal::Changed);
        }
        signals.raise(Signal::Event);
        drop(signals);
        let seen = seen.lock_ok();
        let changed = seen.iter().filter(|(s, _)| *s == Signal::Changed).count();
        assert!(
            (1..=2).contains(&changed),
            "500 changes, {changed} emissions"
        );
        assert_eq!(seen.iter().filter(|(s, _)| *s == Signal::Event).count(), 1);
        assert!(seen
            .iter()
            .all(|(_, thread)| thread.as_deref() == Some("magnetita-signals")));
    }

    #[test]
    fn without_a_bus_raising_is_a_no_op() {
        let signals = Signals::on_bus(None);
        signals.raise(Signal::Changed);
        drop(signals);
    }
}
