//! The desktop adapters' own thread.
//!
//! Writing the phone's clipboard runs `wl-copy` and waits for it, and posting
//! or closing a notification is a blocking D-Bus call. Neither may run on the
//! link's two runtime workers, where one slow adapter would stall every
//! session, the accept loop and the revocation tick. The session loop hands
//! them here instead: one owned thread that runs them in the order they were
//! queued (a notification is posted before it is withdrawn), behind a bounded
//! queue, and is joined when the wire stops.

use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use crate::runtime::log;

/// How many adapter calls may wait. A desktop that cannot keep up drops the
/// newest with a log line instead of growing.
const QUEUE: usize = 256;

type Job = Box<dyn FnOnce() + Send>;

pub(crate) struct DesktopWorker {
    tx: Option<SyncSender<Job>>,
    join: Option<JoinHandle<()>>,
}

impl DesktopWorker {
    pub(crate) fn spawn() -> std::io::Result<Self> {
        let (tx, rx) = sync_channel::<Job>(QUEUE);
        let join = thread::Builder::new()
            .name("magnetita-desktop".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    job();
                }
            })?;
        Ok(Self {
            tx: Some(tx),
            join: Some(join),
        })
    }

    /// Queues `job` behind the calls already waiting; `what` names it in the
    /// log when the queue is full and the call is dropped.
    pub(crate) fn run(&self, what: &str, job: impl FnOnce() + Send + 'static) {
        let Some(tx) = &self.tx else { return };
        match tx.try_send(Box::new(job)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                log("link", &format!("desktop adapters busy; {what} dropped"));
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

impl Drop for DesktopWorker {
    /// Lets the queued calls finish, then joins the thread.
    fn drop(&mut self) {
        self.tx.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn calls_run_in_order_off_the_caller_and_finish_before_the_join() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let worker = DesktopWorker::spawn().unwrap();
        for n in 0..5 {
            let seen = Arc::clone(&seen);
            worker.run("test", move || {
                seen.lock()
                    .unwrap()
                    .push((n, thread::current().name().map(str::to_owned)))
            });
        }
        drop(worker);
        let seen = seen.lock().unwrap();
        assert_eq!(
            seen.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
        assert!(seen
            .iter()
            .all(|(_, name)| name.as_deref() == Some("magnetita-desktop")));
    }
}
