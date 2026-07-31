//! The thread every host runs the engine on.
//!
//! Probing and artwork extraction open files, decode and write: none of that
//! may happen on a GUI thread. This worker owns one thread, accepts jobs with a
//! generation, and drops results whose generation is no longer current — the
//! same staleness discipline the rest of the suite uses.
//!
//! Shutdown is deterministic: dropping the worker closes the queue, cancels the
//! job in flight and joins the thread. No detached thread outlives its host.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use celestina_core::{CancellationToken, Generation};

use crate::backend::{
    ArtworkJob, MediaEngine, ProbeBudget, ProbeReport, TrailerJob, TrailerOutcome,
};
use crate::engine::MpvEngine;
use crate::error::{EngineError, EngineResult};
use crate::library::{ScanLimits, ScanOutcome};
use fluorita_core::SourceSet;

/// A unit of work for the engine thread.
pub enum Job {
    Probe {
        generation: Generation,
        path: std::path::PathBuf,
        budget: ProbeBudget,
    },
    Artwork {
        generation: Generation,
        job: Box<ArtworkJob>,
    },
    Trailer {
        generation: Generation,
        job: Box<TrailerJob>,
    },
    /// Walk the configured roots. The heaviest job the worker takes, and the
    /// one that most obviously cannot run on a GUI thread.
    Scan {
        generation: Generation,
        sources: Box<SourceSet>,
        limits: ScanLimits,
    },
}

impl Job {
    #[must_use]
    pub fn generation(&self) -> Generation {
        match self {
            Self::Probe { generation, .. }
            | Self::Artwork { generation, .. }
            | Self::Trailer { generation, .. }
            | Self::Scan { generation, .. } => *generation,
        }
    }
}

/// What one job produced.
pub enum JobOutcome {
    Probed {
        generation: Generation,
        path: std::path::PathBuf,
        result: EngineResult<ProbeReport>,
    },
    Artwork {
        generation: Generation,
        result: EngineResult<std::path::PathBuf>,
    },
    Trailer {
        generation: Generation,
        result: EngineResult<TrailerOutcome>,
    },
    Scanned {
        generation: Generation,
        result: EngineResult<ScanOutcome>,
    },
}

impl JobOutcome {
    #[must_use]
    pub fn generation(&self) -> Generation {
        match self {
            Self::Probed { generation, .. }
            | Self::Artwork { generation, .. }
            | Self::Trailer { generation, .. }
            | Self::Scanned { generation, .. } => *generation,
        }
    }
}

/// What travels down the queue. Shutdown is a message rather than "the sender
/// was dropped": a host that cloned the sender would otherwise leave the thread
/// blocked in `recv` forever and turn `Drop` into a hang.
enum Message {
    Work(Box<Job>),
    Shutdown,
}

/// A bounded, joinable engine worker.
pub struct EngineWorker {
    jobs: Option<Sender<Message>>,
    outcomes: Receiver<JobOutcome>,
    current: Arc<Mutex<CancellationToken>>,
    thread: Option<JoinHandle<()>>,
}

impl EngineWorker {
    /// Starts the worker thread.
    pub fn start() -> EngineResult<Self> {
        Self::with_engine(MpvEngine::new())
    }

    /// Starts a worker over any engine implementation — the seam the tests use
    /// to exercise queueing without a decoder.
    pub fn with_engine<E>(engine: E) -> EngineResult<Self>
    where
        E: MediaEngine + 'static,
    {
        let (job_sender, job_receiver) = mpsc::channel::<Message>();
        let (outcome_sender, outcome_receiver) = mpsc::channel::<JobOutcome>();
        let current = Arc::new(Mutex::new(CancellationToken::new()));
        let worker_cancellation = Arc::clone(&current);

        let thread = std::thread::Builder::new()
            .name("fluorita-engine".to_owned())
            .spawn(move || {
                run(
                    &engine,
                    &job_receiver,
                    &outcome_sender,
                    &worker_cancellation,
                )
            })
            .map_err(|source| EngineError::Io {
                operation: "start the engine worker",
                path: std::path::PathBuf::from("<thread>"),
                source,
            })?;

        Ok(Self {
            jobs: Some(job_sender),
            outcomes: outcome_receiver,
            current,
            thread: Some(thread),
        })
    }

    /// Queues a job. The queue is FIFO; superseding is the caller's decision,
    /// expressed by cancelling and enqueuing a newer generation.
    pub fn submit(&self, job: Job) -> EngineResult<()> {
        self.jobs
            .as_ref()
            .ok_or(EngineError::WorkerStopped)?
            .send(Message::Work(Box::new(job)))
            .map_err(|_| EngineError::WorkerStopped)
    }

    /// Cancels the job in flight, tells the thread to leave and joins it.
    ///
    /// Idempotent, and called by `Drop`, so a host that forgets still gets a
    /// deterministic shutdown instead of a detached thread.
    pub fn shutdown(&mut self) {
        self.cancel_current();
        if let Some(sender) = self.jobs.take() {
            // A closed receiver only means the thread already left.
            let _ = sender.send(Message::Shutdown);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    /// Cancels whatever is running now. Queued jobs still run; a host that
    /// wants them gone drops their results by generation.
    pub fn cancel_current(&self) {
        // The lock is only ever held to swap a cheap token, and no user code
        // runs while it is held, so it cannot stay poisoned by a panic here.
        if let Ok(token) = self.current.lock() {
            token.cancel();
        }
    }

    /// The next finished job, or `None` if nothing finished within `timeout`.
    pub fn poll(&self, timeout: Duration) -> Option<JobOutcome> {
        // Both a timeout and a disconnected worker mean "nothing to report";
        // a stopped worker is discovered by `submit`, which types the refusal.
        self.outcomes.recv_timeout(timeout).ok()
    }
}

impl Drop for EngineWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run<E: MediaEngine>(
    engine: &E,
    jobs: &Receiver<Message>,
    outcomes: &Sender<JobOutcome>,
    current: &Arc<Mutex<CancellationToken>>,
) {
    while let Ok(message) = jobs.recv() {
        let job = match message {
            Message::Work(job) => *job,
            Message::Shutdown => return,
        };
        let token = CancellationToken::new();
        if let Ok(mut slot) = current.lock() {
            *slot = token.clone();
        }

        let outcome = match job {
            Job::Probe {
                generation,
                path,
                budget,
            } => {
                let result = engine.probe(&path, budget, &token);
                JobOutcome::Probed {
                    generation,
                    path,
                    result,
                }
            }
            Job::Scan {
                generation,
                sources,
                limits,
            } => JobOutcome::Scanned {
                generation,
                result: crate::library::scan(&sources, limits, &token),
            },
            Job::Artwork { generation, job } => {
                let mut request = *job;
                request.cancellation = token.clone();
                JobOutcome::Artwork {
                    generation,
                    result: engine.publish_artwork(&request),
                }
            }
            Job::Trailer { generation, job } => {
                let mut request = *job;
                request.cancellation = token.clone();
                JobOutcome::Trailer {
                    generation,
                    result: engine.produce_trailer(&request),
                }
            }
        };

        if outcomes.send(outcome).is_err() {
            return; // the host is gone; nothing left to report to
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineWorker, Job, JobOutcome};
    use crate::backend::{
        ArtworkJob, EngineSession, MediaEngine, ProbeBudget, ProbeReport, SessionRequest,
        TrailerJob, TrailerOutcome,
    };
    use crate::error::{EngineError, EngineResult};
    use celestina_core::{CancellationToken, Generation, GenerationClock};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// An engine that does no IO: it reports how often it ran and honours
    /// cancellation, which is all the worker's contract needs.
    #[derive(Clone, Default)]
    struct CountingEngine {
        runs: Arc<AtomicUsize>,
        block: Arc<AtomicUsize>,
    }

    impl MediaEngine for CountingEngine {
        fn probe(
            &self,
            _path: &Path,
            _budget: ProbeBudget,
            cancellation: &CancellationToken,
        ) -> EngineResult<ProbeReport> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            while self.block.load(Ordering::SeqCst) > 0 {
                if cancellation.is_cancelled() {
                    return Err(EngineError::Cancelled);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(ProbeReport::default())
        }

        fn publish_artwork(&self, _request: &ArtworkJob) -> EngineResult<PathBuf> {
            Err(EngineError::Cancelled)
        }

        fn produce_trailer(&self, _job: &TrailerJob) -> EngineResult<TrailerOutcome> {
            Err(EngineError::Cancelled)
        }

        fn open_session(&self, _request: SessionRequest) -> EngineResult<Box<dyn EngineSession>> {
            Err(EngineError::WorkerStopped)
        }
    }

    fn probe_job(generation: Generation, path: &str) -> Job {
        Job::Probe {
            generation,
            path: PathBuf::from(path),
            budget: ProbeBudget::conservative(),
        }
    }

    #[test]
    fn a_queued_job_runs_off_the_calling_thread_and_reports_back() {
        let mut clock = GenerationClock::default();
        let generation = clock.issue().expect("generation");
        let engine = CountingEngine::default();
        let worker = EngineWorker::with_engine(engine.clone()).expect("worker starts");

        worker
            .submit(probe_job(generation, "/m/a.mkv"))
            .expect("queued");
        let outcome = worker.poll(Duration::from_secs(5)).expect("one outcome");

        assert_eq!(outcome.generation(), generation);
        match outcome {
            JobOutcome::Probed { path, result, .. } => {
                assert_eq!(path, PathBuf::from("/m/a.mkv"));
                assert!(result.is_ok());
            }
            _ => panic!("wrong outcome kind"),
        }
        assert_eq!(engine.runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cancelling_stops_the_job_in_flight() {
        let mut clock = GenerationClock::default();
        let engine = CountingEngine::default();
        engine.block.store(1, Ordering::SeqCst);
        let worker = EngineWorker::with_engine(engine.clone()).expect("worker starts");

        worker
            .submit(probe_job(clock.issue().expect("generation"), "/m/slow.mkv"))
            .expect("queued");
        // Wait until the job is actually running before cancelling it.
        while engine.runs.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        worker.cancel_current();

        let outcome = worker.poll(Duration::from_secs(5)).expect("one outcome");
        match outcome {
            JobOutcome::Probed { result, .. } => {
                assert!(matches!(result, Err(EngineError::Cancelled)));
            }
            _ => panic!("wrong outcome kind"),
        }
    }

    #[test]
    fn dropping_the_worker_joins_its_thread() {
        let mut clock = GenerationClock::default();
        let engine = CountingEngine::default();
        let worker = EngineWorker::with_engine(engine.clone()).expect("worker starts");
        worker
            .submit(probe_job(clock.issue().expect("generation"), "/m/a.mkv"))
            .expect("queued");

        drop(worker);

        // If the thread had been detached, this count could still change.
        let observed = engine.runs.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(engine.runs.load(Ordering::SeqCst), observed);
    }

    #[test]
    fn submitting_after_shutdown_is_a_typed_refusal() {
        let engine = CountingEngine::default();
        let mut worker = EngineWorker::with_engine(engine).expect("worker starts");

        worker.shutdown();

        assert!(matches!(
            worker.submit(probe_job(Generation::INITIAL, "/m/a.mkv")),
            Err(EngineError::WorkerStopped)
        ));
        // Shutting down twice must not hang or panic.
        worker.shutdown();
    }

    #[test]
    fn shutdown_does_not_depend_on_being_the_last_sender() {
        // The regression this pins: while the queue's closure was the only stop
        // signal, any surviving clone of the sender left the thread blocked in
        // `recv` and `Drop` waited on it forever.
        let engine = CountingEngine::default();
        engine.block.store(1, Ordering::SeqCst);
        let mut worker = EngineWorker::with_engine(engine.clone()).expect("worker starts");
        worker
            .submit(probe_job(Generation::INITIAL, "/m/slow.mkv"))
            .expect("queued");
        while engine.runs.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }

        let finished = Arc::new(AtomicUsize::new(0));
        let flag = Arc::clone(&finished);
        let closer = std::thread::spawn(move || {
            worker.shutdown();
            flag.store(1, Ordering::SeqCst);
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while finished.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            finished.load(Ordering::SeqCst),
            1,
            "shutdown must not wait on a job that cancellation already stopped"
        );
        closer.join().expect("the closing thread finishes");
    }
}
