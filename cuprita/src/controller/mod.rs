//! The section controllers: one QML singleton per section owning a worker
//! thread over its backend trait. Every command runs on that worker; after it
//! the worker re-reads the section's snapshot, and again whenever the backend
//! reports a change (or, for a backend without change signals yet, every few
//! seconds), and queues it to the Qt thread, where the controller updates its
//! properties and hands the snapshot on to the list models. The backend is
//! built on the worker too: connecting to a bus is blocking IO.
//!
//! A worker thread is detached and never joined. When the window closes, the
//! process exits around it: a real backend call still blocking at that moment
//! (a slow D-Bus reply, say) is abandoned on purpose rather than waited for,
//! because nothing it could still report has anyone left to read it.

pub mod activation;
pub mod app;
pub mod appearance;
pub mod audio;
pub mod bluetooth;
pub mod device_model;
pub mod endpoint_model;
pub mod network;
pub mod network_model;
pub mod notice;
pub mod stream_model;

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// What a worker reports to the Qt thread.
pub enum Report<S> {
    Snapshot(Arc<S>),
    /// A command or a read failed; the text is the error's `message_es()`.
    Failed(String),
    /// A command finished, successfully or not.
    Done,
}

type Job<B> = Box<dyn FnOnce(&mut B) -> Result<(), String> + Send>;

enum Message<B: ?Sized> {
    Job(Job<B>),
    /// A command another section asked for (airplane mode powering the
    /// Bluetooth adapter off): run and re-read like a job, but not counted
    /// in this section's `busy`, so it reports no `Done`.
    Quiet(Job<B>),
    /// A command that a later one with the same key supersedes (a volume
    /// slider's writes): of those queued together, only the last runs.
    Keyed(String, Job<B>),
    /// The backend reported a change: re-read the snapshot.
    Refresh,
}

/// What a batch of queued messages comes to: the jobs to run in order (and
/// whether each reports `Done`), and how many superseded jobs report `Done`
/// without running. Refreshes collapse into the one re-read after the batch.
fn plan<B: ?Sized>(batch: Vec<Message<B>>) -> (Vec<(Job<B>, bool)>, usize) {
    let mut last = std::collections::HashMap::new();
    for (index, message) in batch.iter().enumerate() {
        if let Message::Keyed(key, _) = message {
            last.insert(key.clone(), index);
        }
    }
    let mut jobs = Vec::new();
    let mut superseded = 0;
    for (index, message) in batch.into_iter().enumerate() {
        match message {
            Message::Job(job) => jobs.push((job, true)),
            Message::Quiet(job) => jobs.push((job, false)),
            Message::Keyed(key, job) => {
                if last.get(&key) == Some(&index) {
                    jobs.push((job, true));
                } else {
                    superseded += 1;
                }
            }
            Message::Refresh => {}
        }
    }
    (jobs, superseded)
}

/// Whatever must live as long as the worker's backend: a change watcher.
pub type Keepalive = Box<dyn Send>;

/// Asks a worker to re-read its snapshot; handed to a backend's watcher.
pub struct Refresh<B: ?Sized> {
    inbox: Sender<Message<B>>,
}

impl<B: ?Sized> Refresh<B> {
    /// `false` when the worker is gone.
    pub fn request(&self) -> bool {
        self.inbox.send(Message::Refresh).is_ok()
    }
}

/// The sending half of a section's worker thread.
pub struct Worker<B: ?Sized> {
    jobs: Sender<Message<B>>,
}

impl<B: ?Sized> Clone for Worker<B> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
        }
    }
}

impl<B: ?Sized + Send + 'static> Worker<B> {
    /// Starts the thread. `make` builds the backend on it, given the handle
    /// its watcher calls, and returns whatever keeps that watcher alive;
    /// `poll`, when set, re-reads on a timer as well. `read` takes the
    /// snapshot; `report` is called on the worker thread and must queue to
    /// the Qt thread itself.
    pub fn start<S, M, R, F>(make: M, poll: Option<Duration>, read: R, report: F) -> Option<Self>
    where
        S: Send + Sync + 'static,
        M: FnOnce(Refresh<B>) -> (Box<B>, Option<Keepalive>) + Send + 'static,
        R: Fn(&mut B) -> Result<S, String> + Send + 'static,
        F: Fn(Report<S>) + Send + 'static,
    {
        let (jobs, inbox) = mpsc::channel::<Message<B>>();
        let refresh = Refresh {
            inbox: jobs.clone(),
        };
        let mut read_failed = false;
        let mut publish = move |backend: &mut B, report: &F| match read(backend) {
            Ok(snapshot) => {
                read_failed = false;
                report(Report::Snapshot(Arc::new(snapshot)));
            }
            // A backend that stays down says so once, not every poll.
            Err(message) => {
                if !read_failed {
                    read_failed = true;
                    report(Report::Failed(message));
                }
            }
        };
        std::thread::Builder::new()
            .name("cuprita-worker".to_owned())
            .spawn(move || {
                let (mut backend, _keepalive) = make(refresh);
                publish(&mut backend, &report);
                loop {
                    let message = match poll {
                        Some(every) => inbox.recv_timeout(every),
                        None => inbox.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    let first = match message {
                        Ok(message) => message,
                        Err(RecvTimeoutError::Timeout) => {
                            publish(&mut backend, &report);
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    // Everything already queued is handled as one batch, read
                    // once at its end: a dragged slider or a burst of changes
                    // costs one re-read, not one per message.
                    let mut batch = vec![first];
                    batch.extend(inbox.try_iter());
                    let (run, superseded) = plan(batch);
                    let mut done = superseded;
                    for (job, counted) in run {
                        if let Err(message) = job(&mut backend) {
                            report(Report::Failed(message));
                        }
                        done += usize::from(counted);
                    }
                    publish(&mut backend, &report);
                    for _ in 0..done {
                        report(Report::Done);
                    }
                }
            })
            .ok()
            .map(|_| Self { jobs })
    }

    /// Queues a command; `false` when the worker is gone.
    pub fn run(&self, job: impl FnOnce(&mut B) -> Result<(), String> + Send + 'static) -> bool {
        self.jobs.send(Message::Job(Box::new(job))).is_ok()
    }

    /// Queues a command that a later one with the same `key` supersedes
    /// while both wait; `false` when the worker is gone.
    pub fn run_keyed(
        &self,
        key: String,
        job: impl FnOnce(&mut B) -> Result<(), String> + Send + 'static,
    ) -> bool {
        self.jobs.send(Message::Keyed(key, Box::new(job))).is_ok()
    }

    /// Queues a command that another section asked for: it reports a failure
    /// and the snapshot after it, but no `Done`.
    pub fn run_quiet(
        &self,
        job: impl FnOnce(&mut B) -> Result<(), String> + Send + 'static,
    ) -> bool {
        self.jobs.send(Message::Quiet(Box::new(job))).is_ok()
    }
}

/// The latest snapshot of a section and the list models that want it. A
/// model subscribes when QML creates it and is handed the latest snapshot at
/// once, so it does not matter whether the controller or the model came first.
pub struct Hub<S> {
    inner: Mutex<HubState<S>>,
}

struct HubState<S> {
    latest: Option<Arc<S>>,
    subscribers: Vec<Box<dyn Fn(Arc<S>) -> bool + Send>>,
}

impl<S> Hub<S> {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(HubState {
                latest: None,
                subscribers: Vec::new(),
            }),
        }
    }

    /// `deliver` returns `false` once its model is gone, which unsubscribes it.
    /// It runs under the hub's mutex (here and in `publish`), so it must only
    /// queue work to its model's thread: a subscriber that touched the hub
    /// synchronously would deadlock.
    pub fn subscribe(&self, deliver: impl Fn(Arc<S>) -> bool + Send + 'static) {
        let mut state = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        // A model that subscribes after the first snapshot gets `latest`
        // here, after its controller's `loaded` is already true, so its page
        // could show an empty card for a frame. Every model exists at startup
        // today, before any snapshot, so none does.
        if let Some(latest) = &state.latest {
            if !deliver(latest.clone()) {
                return;
            }
        }
        state.subscribers.push(Box::new(deliver));
    }

    /// Calls every `deliver` under the mutex; see `subscribe`.
    pub fn publish(&self, snapshot: Arc<S>) {
        let mut state = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        state.latest = Some(snapshot.clone());
        state
            .subscribers
            .retain(|deliver| deliver(snapshot.clone()));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::{Report, Worker};

    struct Volumes {
        writes: Arc<Mutex<Vec<f32>>>,
    }

    #[test]
    fn queued_volume_writes_coalesce_into_the_last() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let backend_writes = Arc::clone(&writes);
        let (open, gate) = mpsc::channel::<()>();
        let (reports, received) = mpsc::channel::<&'static str>();
        let worker = Worker::<Volumes>::start(
            move |_| {
                // Hold the worker until all three writes are queued.
                let _ = gate.recv();
                (
                    Box::new(Volumes {
                        writes: backend_writes,
                    }),
                    None,
                )
            },
            None,
            |_: &mut Volumes| Ok(()),
            move |report: Report<()>| {
                let _ = reports.send(match report {
                    Report::Snapshot(_) => "snapshot",
                    Report::Failed(_) => "failed",
                    Report::Done => "done",
                });
            },
        )
        .expect("worker thread");
        for volume in [0.2_f32, 0.5, 0.9] {
            assert!(
                worker.run_keyed("volume:40".to_owned(), move |b: &mut Volumes| {
                    b.writes.lock().expect("lock").push(volume);
                    Ok(())
                })
            );
        }
        open.send(()).expect("gate");
        let seen: Vec<&str> = (0..5)
            .map(|_| {
                received
                    .recv_timeout(Duration::from_secs(5))
                    .expect("report")
            })
            .collect();
        // The first read, one read after the batch, and every command done.
        assert_eq!(seen, ["snapshot", "snapshot", "done", "done", "done"]);
        assert_eq!(*writes.lock().expect("lock"), [0.9]);
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
    }
}
