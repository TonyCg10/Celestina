//! The policy's one-second timer (CEL-2-H design §4.4: "`Tick` is a
//! one-second timer that exists only while the state is not `Idle` or
//! `Unavailable`, or an intent is pending").
//!
//! A thread that blocks in `recv` while the policy does not want the timer
//! (no wake-up at rest) and sleeps one second at a time while it does. It
//! does nothing but post the tick; the policy decides what a tick means.
#![forbid(unsafe_code)]

use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use celestina_shell::lock::Event;

use crate::providers::Worker;

/// The period of the timer.
pub const PERIOD: Duration = Duration::from_secs(1);
/// How long stopping the thread waits for it.
pub const JOIN_WAIT: Duration = Duration::from_secs(2);

/// A running timer thread.
#[derive(Debug)]
pub struct Ticker {
    switch: Sender<bool>,
    worker: Worker,
}

impl Ticker {
    /// Starts the thread, parked. `publish` returns `false` once the loop is
    /// gone.
    ///
    /// # Errors
    ///
    /// The failure to start the thread.
    pub fn start(
        period: Duration,
        publish: impl Fn(Event) -> bool + Send + 'static,
    ) -> io::Result<Self> {
        let (switch, commands) = mpsc::channel::<bool>();
        let stop = Arc::new(AtomicBool::new(false));
        let worker = Worker::spawn("lock-tick", &stop, move || {
            let mut running = false;
            loop {
                if running {
                    match commands.recv_timeout(period) {
                        Ok(on) => running = on,
                        Err(RecvTimeoutError::Timeout) => {
                            if !publish(Event::Tick) {
                                return;
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                } else {
                    match commands.recv() {
                        Ok(on) => running = on,
                        Err(_) => return,
                    }
                }
            }
        })?;
        Ok(Self { switch, worker })
    }

    /// Runs the timer (`true`) or parks it.
    pub fn set(&self, on: bool) {
        // A gone thread has nothing to switch.
        let _ = self.switch.send(on);
    }

    /// Stops the thread and waits for it, at most [`JOIN_WAIT`].
    pub fn stop(self) {
        let Ticker { switch, worker } = self;
        // Dropping the sender ends a parked thread at once and a running one
        // at its next wake-up.
        drop(switch);
        worker.join_within(JOIN_WAIT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Instant;

    #[test]
    fn the_timer_is_parked_until_asked_and_stops_when_parked_again() {
        let (sent, ticks) = channel();
        let ticker = Ticker::start(Duration::from_millis(20), move |event| {
            sent.send(event).is_ok()
        })
        .expect("starts");
        assert!(
            ticks.recv_timeout(Duration::from_millis(120)).is_err(),
            "a parked timer ticks never"
        );
        ticker.set(true);
        assert_eq!(ticks.recv_timeout(Duration::from_secs(2)), Ok(Event::Tick));
        ticker.set(false);
        // Anything in flight arrives; then silence.
        std::thread::sleep(Duration::from_millis(80));
        while ticks.try_recv().is_ok() {}
        assert!(ticks.recv_timeout(Duration::from_millis(120)).is_err());
        let started = Instant::now();
        ticker.stop();
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_running_timer_stops_within_a_period_when_the_loop_is_gone() {
        let (sent, ticks) = channel();
        let ticker = Ticker::start(Duration::from_millis(10), move |event| {
            sent.send(event).is_ok()
        })
        .expect("starts");
        ticker.set(true);
        drop(ticks);
        let started = Instant::now();
        ticker.stop();
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
