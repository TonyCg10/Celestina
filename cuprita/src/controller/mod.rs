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

/// How often an idle worker re-reads its snapshot when its backend has no
/// change signals of its own yet (Bluetooth and audio until CUP-1-D and E).
const POLL: Duration = Duration::from_secs(5);

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
    /// The backend reported a change: re-read the snapshot.
    Refresh,
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

impl<B: ?Sized + Send + 'static> Worker<B> {
    /// Starts a polled worker over a backend that has no change signals.
    pub fn spawn<S, R, F>(backend: Box<B>, read: R, report: F) -> Option<Self>
    where
        S: Send + Sync + 'static,
        R: Fn(&mut B) -> Result<S, String> + Send + 'static,
        F: Fn(Report<S>) + Send + 'static,
    {
        Self::start(move |_| (backend, None), Some(POLL), read, report)
    }

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
                    match message {
                        Ok(Message::Job(job)) => {
                            if let Err(message) = job(&mut backend) {
                                report(Report::Failed(message));
                            }
                            publish(&mut backend, &report);
                            report(Report::Done);
                        }
                        Ok(Message::Refresh) | Err(RecvTimeoutError::Timeout) => {
                            publish(&mut backend, &report);
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
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
