//! Desktop notices for the lock (CEL-2-H design §4.9, review S-I2).
//!
//! A log line is invisible at the moment it matters, so every refusal of an
//! explicit request, a suspend dropped because the lock was not confirmed,
//! the relaunch limit and a missing PAM service are also posted as a desktop
//! notification: a `Notify` call to whoever owns
//! `org.freedesktop.Notifications` (another shell during coexistence, this
//! one afterwards), best effort, bounded by the call timeout, and at most one
//! identical notice per [`REPEAT`].
//!
//! The call is made on a worker of its own (`lock-notice`), started on the
//! first notice, so the loop never waits for a bus and the notice of a lock
//! that is switched off (a refused `lock suspend`) has a thread to travel on.
//! The notice names no file, no user and no secret: the words are the
//! product's (`crate::copy`), chosen by a reason, never built from text the
//! lock or the bus sent.
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use celestina_shell::lock::Notice;
use zbus::blocking::connection::Builder;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

use crate::energy::logind::connect_with;
use crate::providers::Worker;

/// The notification service's name, path and interface.
const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
/// Who the notice is reported as.
const APP_NAME: &str = "Celestina";
/// The server's default expiry.
const DEFAULT_EXPIRY: i32 = -1;
/// The same notice is not posted twice within this.
pub const REPEAT: Duration = Duration::from_secs(10);
/// The notices that may wait for the worker.
pub const QUEUE: usize = 4;
/// How long stopping the worker waits for it.
pub const JOIN_WAIT: Duration = Duration::from_secs(2);

/// Which bus the notices go to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum NoticeBus {
    /// The user's session bus: production.
    #[default]
    Session,
    /// A bus at this address: tests, on a private bus.
    Address(String),
}

impl NoticeBus {
    fn builder(&self) -> zbus::Result<Builder<'static>> {
        match self {
            Self::Session => Builder::session(),
            Self::Address(address) => Builder::address(address.as_str()),
        }
    }
}

/// "At most one identical notice per [`REPEAT`]", over a clock the caller
/// passes in.
#[derive(Debug, Default)]
pub struct NoticeLimiter {
    /// Few distinct notices exist; a list is the whole index.
    last: Vec<(Notice, Instant)>,
}

impl NoticeLimiter {
    /// Whether `notice` may be posted at `now`; if so, it is recorded.
    pub fn allow(&mut self, notice: Notice, now: Instant) -> bool {
        // Past the window the old entries mean nothing.
        self.last
            .retain(|(_, at)| now.saturating_duration_since(*at) < REPEAT);
        if self.last.iter().any(|(seen, _)| *seen == notice) {
            return false;
        }
        self.last.push((notice, now));
        true
    }
}

/// The loop's side: rate limit, then hand to the worker.
pub struct Notifier {
    bus: NoticeBus,
    limiter: RefCell<NoticeLimiter>,
    worker: RefCell<Option<Posting>>,
}

struct Posting {
    queue: SyncSender<Notice>,
    worker: Worker,
}

impl std::fmt::Debug for Notifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notifier").field("bus", &self.bus).finish()
    }
}

impl Notifier {
    /// A notifier that posts to `bus`. No thread exists until the first
    /// notice.
    #[must_use]
    pub fn new(bus: NoticeBus) -> Self {
        Self {
            bus,
            limiter: RefCell::new(NoticeLimiter::default()),
            worker: RefCell::new(None),
        }
    }

    /// Posts `notice` unless an identical one was posted within [`REPEAT`].
    pub fn post(&self, notice: Notice, now: Instant) {
        if !self.limiter.borrow_mut().allow(notice, now) {
            tracing::debug!(?notice, "lock: the same notice was posted a moment ago");
            return;
        }
        let mut slot = self.worker.borrow_mut();
        if slot.is_none() {
            let (queue, notices) = mpsc::sync_channel(QUEUE);
            let stop = Arc::new(AtomicBool::new(false));
            let bus = self.bus.clone();
            let spawned = Worker::spawn("lock-notice", &stop, move || post_all(&bus, &notices));
            match spawned {
                Ok(worker) => *slot = Some(Posting { queue, worker }),
                Err(error) => {
                    tracing::warn!(%error, "lock: no thread to post the notice on");
                    return;
                }
            }
        }
        if let Some(posting) = slot.as_ref() {
            match posting.queue.try_send(notice) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    tracing::warn!(?notice, "lock: the notice queue is full; dropped");
                }
                Err(TrySendError::Disconnected(_)) => {
                    tracing::warn!(?notice, "lock: the notice worker is gone; dropped");
                }
            }
        }
    }

    /// Stops the worker, if any, at most [`JOIN_WAIT`].
    pub fn stop(&self) {
        if let Some(Posting { queue, worker }) = self.worker.borrow_mut().take() {
            // Dropping the queue ends a parked worker; a call in flight is
            // bounded by the call timeout.
            drop(queue);
            worker.join_within(JOIN_WAIT);
        }
    }
}

impl Drop for Notifier {
    fn drop(&mut self) {
        self.stop();
    }
}

fn post_all(bus: &NoticeBus, notices: &Receiver<Notice>) {
    let mut connection: Option<Connection> = None;
    while let Ok(notice) = notices.recv() {
        if connection.is_none() {
            connection = bus
                .builder()
                .map_err(|error| crate::error_chain(&error))
                .and_then(|builder| {
                    connect_with(builder).map_err(|error| crate::error_chain(&error))
                })
                .map_err(|why| tracing::warn!("lock: no bus for the notice: {why}"))
                .ok();
        }
        let Some(open) = connection.as_ref() else {
            continue;
        };
        if let Err(error) = notify(open, notice) {
            tracing::warn!(error = %crate::error_chain(&error), "lock: the notice was not posted");
            connection = None;
        }
    }
}

/// The `Notify` call, one notice.
fn notify(connection: &Connection, notice: Notice) -> zbus::Result<()> {
    let (title, body) = crate::copy::lock_notice(notice);
    let hints: HashMap<&str, Value<'_>> = HashMap::new();
    let actions: Vec<&str> = Vec::new();
    // The connection's method timeout bounds the call.
    connection.call_method(
        Some(NAME),
        PATH,
        Some(NAME),
        "Notify",
        &(
            APP_NAME,
            0u32,
            "",
            title,
            body.as_str(),
            actions,
            hints,
            DEFAULT_EXPIRY,
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use celestina_shell::lock::Why;

    #[test]
    fn at_most_one_identical_notice_per_10_s() {
        let mut limiter = NoticeLimiter::default();
        let start = Instant::now();
        assert!(limiter.allow(Notice::NotLocked, start));
        assert!(!limiter.allow(Notice::NotLocked, start + Duration::from_secs(9)));
        // A different notice is its own.
        assert!(limiter.allow(Notice::NotConfirmed, start + Duration::from_secs(9)));
        assert!(limiter.allow(Notice::Unavailable(Why::DutyOff), start));
        assert!(limiter.allow(Notice::Unavailable(Why::NotPinned), start));
        assert!(!limiter.allow(Notice::Unavailable(Why::DutyOff), start));
        // Past the window the first goes through again.
        assert!(limiter.allow(Notice::NotLocked, start + Duration::from_secs(10)));
        assert!(!limiter.allow(Notice::NotLocked, start + Duration::from_secs(11)));
    }
}
