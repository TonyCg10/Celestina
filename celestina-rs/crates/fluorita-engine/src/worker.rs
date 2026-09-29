//! The thread every host runs the engine on.
//!
//! Probing and artwork extraction open files, decode and write: none of that
//! may happen on a GUI thread. This worker owns one thread, accepts jobs with a
//! generation, and drops results whose generation is no longer current — the
//! same staleness discipline the rest of the suite uses.
//!
//! Shutdown is deterministic: dropping the worker closes the queue, cancels the
//! job in flight and every queued one, and joins the thread. No detached thread
//! outlives its host.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use celestina_core::{CancellationToken, Generation};

use crate::backend::{ArtworkJob, MediaEngine, ProbeBudget, ProbeReport};
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
            | Self::Scanned { generation, .. } => *generation,
        }
    }
}

/// What travels down the queue. Shutdown is a message rather than "the sender
/// was dropped": a host that cloned the sender would otherwise leave the thread
/// blocked in `recv` forever and turn `Drop` into a hang.
enum Message {
    /// A job with the token it runs under and its place in [`Outstanding`].
    Work {
        job: Box<Job>,
        token: CancellationToken,
        ticket: u64,
    },
    Shutdown,
}

/// The tokens of every job submitted and not yet finished, in queue order.
///
/// A job's token exists from the moment it is submitted, not from the moment
/// the thread dequeues it, so a cancel that arrives in between reaches the job
/// it followed instead of the one before it.
#[derive(Default)]
struct Outstanding {
    next_ticket: u64,
    tokens: Vec<(u64, CancellationToken)>,
}

/// Only cheap token bookkeeping ever runs under this lock, so a poisoned lock
/// still holds consistent data and is used as it is.
fn lock(outstanding: &Mutex<Outstanding>) -> MutexGuard<'_, Outstanding> {
    outstanding.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A bounded, joinable engine worker.
pub struct EngineWorker {
    jobs: Option<Sender<Message>>,
    outcomes: Receiver<JobOutcome>,
    outstanding: Arc<Mutex<Outstanding>>,
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
        let outstanding = Arc::new(Mutex::new(Outstanding::default()));
        let worker_outstanding = Arc::clone(&outstanding);

        let thread = std::thread::Builder::new()
            .name("fluorita-engine".to_owned())
            .spawn(move || run(&engine, &job_receiver, &outcome_sender, &worker_outstanding))
            .map_err(|source| EngineError::Io {
                operation: "start the engine worker",
                path: std::path::PathBuf::from("<thread>"),
                source,
            })?;

        Ok(Self {
            jobs: Some(job_sender),
            outcomes: outcome_receiver,
            outstanding,
            thread: Some(thread),
        })
    }

    /// Queues a job. The queue is FIFO; superseding is the caller's decision,
    /// expressed by cancelling and enqueuing a newer generation.
    pub fn submit(&self, job: Job) -> EngineResult<()> {
        let sender = self.jobs.as_ref().ok_or(EngineError::WorkerStopped)?;
        let token = CancellationToken::new();
        // The token is registered and the job sent under one lock, so the
        // worker can never finish the job before it is registered, and a cancel
        // can never fall between the two.
        let mut outstanding = lock(&self.outstanding);
        let ticket = outstanding.next_ticket;
        outstanding.next_ticket = ticket.wrapping_add(1);
        outstanding.tokens.push((ticket, token.clone()));
        let sent = sender.send(Message::Work {
            job: Box::new(job),
            token,
            ticket,
        });
        if sent.is_err() {
            outstanding.tokens.retain(|(held, _)| *held != ticket);
            return Err(EngineError::WorkerStopped);
        }
        Ok(())
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

    /// Cancels the job running now and every job already queued behind it:
    /// each one still reports, as [`EngineError::Cancelled`], and a queued one
    /// never reaches the engine. A job submitted after this call runs.
    pub fn cancel_current(&self) {
        for (_, token) in &lock(&self.outstanding).tokens {
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
    outstanding: &Mutex<Outstanding>,
) {
    while let Ok(message) = jobs.recv() {
        let (job, token, ticket) = match message {
            Message::Work { job, token, ticket } => (*job, token, ticket),
            Message::Shutdown => return,
        };

        let outcome = if token.is_cancel_requested() {
            cancelled(job)
        } else {
            perform(engine, job, &token)
        };
        lock(outstanding).tokens.retain(|(held, _)| *held != ticket);

        if outcomes.send(outcome).is_err() {
            return; // the host is gone; nothing left to report to
        }
    }
}

fn perform<E: MediaEngine>(engine: &E, job: Job, token: &CancellationToken) -> JobOutcome {
    match job {
        Job::Probe {
            generation,
            path,
            budget,
        } => {
            let result = engine.probe(&path, budget, token);
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
            result: crate::library::scan(&sources, limits, token),
        },
        Job::Artwork { generation, job } => {
            let mut request = *job;
            request.cancellation = token.clone();
            JobOutcome::Artwork {
                generation,
                result: engine.publish_artwork(&request),
            }
        }
    }
}

/// The outcome of a job cancelled before it started: it reports, so a host
/// waiting on it is answered, and the engine never sees it.
fn cancelled(job: Job) -> JobOutcome {
    match job {
        Job::Probe {
            generation, path, ..
        } => JobOutcome::Probed {
            generation,
            path,
            result: Err(EngineError::Cancelled),
        },
        Job::Scan { generation, .. } => JobOutcome::Scanned {
            generation,
            result: Err(EngineError::Cancelled),
        },
        Job::Artwork { generation, .. } => JobOutcome::Artwork {
            generation,
            result: Err(EngineError::Cancelled),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineWorker, Job, JobOutcome};
    use crate::backend::{
        ArtworkJob, EngineSession, MediaEngine, ProbeBudget, ProbeReport, SessionRequest,
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
    fn a_cancel_right_after_submit_reaches_the_job_it_followed() {
        // The race this pins: the worker used to create a job's token only
        // when it dequeued the job, so a cancel that arrived first cancelled
        // the previous token and the new job ran to its end. Run many times,
        // because the window is a thread switch wide.
        for _ in 0..200 {
            let engine = CountingEngine::default();
            engine.block.store(1, Ordering::SeqCst);
            let worker = EngineWorker::with_engine(engine.clone()).expect("worker starts");

            worker
                .submit(probe_job(Generation::INITIAL, "/m/slow.mkv"))
                .expect("queued");
            worker.cancel_current();

            let outcome = worker
                .poll(Duration::from_secs(5))
                .expect("a cancelled job still reports, and promptly");
            match outcome {
                JobOutcome::Probed { result, .. } => {
                    assert!(matches!(result, Err(EngineError::Cancelled)));
                }
                _ => panic!("wrong outcome kind"),
            }
        }
    }

    #[test]
    fn cancelling_reaches_a_queued_job_and_spares_a_later_one() {
        let engine = CountingEngine::default();
        engine.block.store(1, Ordering::SeqCst);
        let worker = EngineWorker::with_engine(engine.clone()).expect("worker starts");

        worker
            .submit(probe_job(Generation::INITIAL, "/m/running.mkv"))
            .expect("queued");
        while engine.runs.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        worker
            .submit(probe_job(Generation::INITIAL, "/m/queued.mkv"))
            .expect("queued");
        worker.cancel_current();

        for _ in 0..2 {
            match worker.poll(Duration::from_secs(5)).expect("an outcome") {
                JobOutcome::Probed { result, .. } => {
                    assert!(matches!(result, Err(EngineError::Cancelled)));
                }
                _ => panic!("wrong outcome kind"),
            }
        }
        assert_eq!(
            engine.runs.load(Ordering::SeqCst),
            1,
            "the queued job was cancelled before it reached the engine"
        );

        engine.block.store(0, Ordering::SeqCst);
        worker
            .submit(probe_job(Generation::INITIAL, "/m/later.mkv"))
            .expect("queued");
        match worker.poll(Duration::from_secs(5)).expect("an outcome") {
            JobOutcome::Probed { result, .. } => assert!(result.is_ok(), "a later job runs"),
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
