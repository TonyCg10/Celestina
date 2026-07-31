//! One owned thread that keeps probing, reading and writing off the UI thread.
//!
//! A host may not call [`crate::open`] or [`crate::save`] from its GUI thread:
//! both stat, read and fsync real files. This is the bounded worker that runs
//! them — one thread, owned by the host, cancelled and joined deterministically
//! when it is dropped.
//!
//! Two rules shape the queue. Probes and opens are *questions about the
//! present*: a newer one makes the older one worthless, so submitting one
//! cancels and replaces any that has not started. Saves are *work already
//! promised to the user*: they queue in order and are never dropped, and only
//! shutdown cancels one.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use celestina_core::{CancellationToken, Generation};

use crate::history::Revision;
use crate::open::{open, probe, Limits, OpenRefusal, OpenedFile, ProbeOutcome};
use crate::save::{perform, SaveRefusal, SaveReport, SaveRequest};

/// A piece of blocking work for the document worker.
#[derive(Clone, Debug)]
pub enum Job {
    /// Classify a file cheaply, to decide whether the editor may be offered.
    ///
    /// Superseded by a newer one: this answers "should I offer to open the file
    /// the user is looking at *now*", so an older answer is worthless.
    Probe {
        path: PathBuf,
        generation: Generation,
        limits: Limits,
    },
    /// Classify a file to decide which application opens it.
    ///
    /// Never superseded, unlike [`Job::Probe`]. Each one answers a separate
    /// thing the user did — activating a file — and dropping one would silently
    /// fail to open that file at all.
    Classify {
        path: PathBuf,
        generation: Generation,
        limits: Limits,
    },
    /// Read a file completely and decode it.
    Open {
        path: PathBuf,
        generation: Generation,
        limits: Limits,
    },
    /// Write a document, or refuse without touching the original.
    ///
    /// `generation` names the open document this write belongs to. Without it a
    /// host that closed one file and opened another could not tell a late
    /// report for the first from a report for the second, since a revision only
    /// orders states *within* one document.
    Save {
        request: Box<SaveRequest>,
        generation: Generation,
    },
}

impl Job {
    /// Whether a newer job of the same kind makes this one pointless.
    ///
    /// Only questions about the current state of a file are superseded. A save
    /// is never discarded to make room for anything.
    const fn is_supersedable(&self) -> bool {
        matches!(self, Self::Probe { .. } | Self::Open { .. })
    }
}

/// The answer to a [`Job`], carrying the stamp that decides whether it is still
/// worth applying.
#[derive(Clone, Debug)]
pub enum Completion {
    Probed {
        generation: Generation,
        result: Box<Result<ProbeOutcome, OpenRefusal>>,
    },
    Opened {
        generation: Generation,
        result: Box<Result<OpenedFile, OpenRefusal>>,
    },
    Saved {
        generation: Generation,
        revision: Revision,
        result: Box<Result<SaveReport, SaveRefusal>>,
    },
}

#[derive(Debug)]
struct Queued {
    job: Job,
    cancellation: CancellationToken,
}

#[derive(Debug)]
struct Running {
    /// Kept beside the token so a discard can stop a query in flight without
    /// ever stopping a save in flight.
    supersedable: bool,
    cancellation: CancellationToken,
}

#[derive(Debug)]
struct WorkerState {
    pending: VecDeque<Queued>,
    running: Option<Running>,
    shutting_down: bool,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<WorkerState>,
    wake: Condvar,
}

/// Owns the worker thread and its queue.
///
/// Dropping cancels the running job and everything queued, then joins the
/// thread before returning, so a closing window never leaves a write in flight.
pub struct DocumentWorker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for DocumentWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DocumentWorker")
            .finish_non_exhaustive()
    }
}

impl DocumentWorker {
    /// Starts the worker. `publish` is called on the worker thread with every
    /// completion; a host marshals it back to its own thread from there.
    ///
    /// Returns the error the thread could not be created with rather than
    /// panicking, so a host can degrade instead of dying at startup.
    pub fn new(publish: impl Fn(Completion) + Send + 'static) -> Result<Self, std::io::Error> {
        let shared = Arc::new(Shared {
            state: Mutex::new(WorkerState {
                pending: VecDeque::new(),
                running: None,
                shutting_down: false,
            }),
            wake: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("grafita-document".to_owned())
            .spawn(move || worker_loop(&worker_shared, &publish))?;

        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Queues a job. A probe or open replaces any queued probe or open,
    /// cancelling it; a save always joins the queue.
    pub fn submit(&self, job: Job) -> Result<(), WorkerStopped> {
        let mut state = lock(&self.shared.state);
        if state.shutting_down {
            return Err(WorkerStopped);
        }
        if job.is_supersedable() {
            state.pending.retain(|queued| {
                if queued.job.is_supersedable() {
                    queued.cancellation.cancel();
                    false
                } else {
                    true
                }
            });
        }
        state.pending.push_back(Queued {
            job,
            cancellation: CancellationToken::new(),
        });
        self.shared.wake.notify_one();
        Ok(())
    }

    /// Abandons every probe and open, queued or running.
    ///
    /// Closing the editor uses this: reading a file the user has navigated away
    /// from is waste, while a save they already asked for still finishes.
    pub fn discard_queries(&self) {
        let mut state = lock(&self.shared.state);
        state.pending.retain(|queued| {
            if queued.job.is_supersedable() {
                queued.cancellation.cancel();
                false
            } else {
                true
            }
        });
        if let Some(running) = state.running.as_ref() {
            if running.supersedable {
                running.cancellation.cancel();
            }
        }
    }
}

impl Drop for DocumentWorker {
    fn drop(&mut self) {
        {
            let mut state = lock(&self.shared.state);
            state.shutting_down = true;
            for queued in state.pending.drain(..) {
                queued.cancellation.cancel();
            }
            if let Some(running) = state.running.take() {
                running.cancellation.cancel();
            }
            self.shared.wake.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The worker is shutting down and will not take more work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerStopped;

impl fmt::Display for WorkerStopped {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the document worker is shutting down")
    }
}

impl Error for WorkerStopped {}

fn worker_loop(shared: &Shared, publish: &impl Fn(Completion)) {
    loop {
        let queued = {
            let mut state = lock(&shared.state);
            while state.pending.is_empty() && !state.shutting_down {
                state = shared
                    .wake
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if state.shutting_down {
                return;
            }
            let Some(queued) = state.pending.pop_front() else {
                continue;
            };
            state.running = Some(Running {
                supersedable: queued.job.is_supersedable(),
                cancellation: queued.cancellation.clone(),
            });
            queued
        };

        let completion = run(&queued);
        let publishable = {
            let mut state = lock(&shared.state);
            state.running = None;
            !state.shutting_down
        };
        if publishable {
            publish(completion);
        }
    }
}

fn run(queued: &Queued) -> Completion {
    let cancellation = &queued.cancellation;
    match &queued.job {
        Job::Probe {
            path,
            generation,
            limits,
        }
        | Job::Classify {
            path,
            generation,
            limits,
        } => Completion::Probed {
            generation: *generation,
            result: Box::new(probe(path, *generation, *limits, cancellation)),
        },
        Job::Open {
            path,
            generation,
            limits,
        } => Completion::Opened {
            generation: *generation,
            result: Box::new(open(path, *generation, *limits, cancellation)),
        },
        Job::Save {
            request,
            generation,
        } => Completion::Saved {
            generation: *generation,
            revision: request.revision(),
            result: Box::new(perform(request, cancellation)),
        },
    }
}

fn lock(mutex: &Mutex<WorkerState>) -> MutexGuard<'_, WorkerState> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    use celestina_core::{Generation, GenerationClock};

    use super::{Completion, DocumentWorker, Job};
    use crate::open::Limits;
    use crate::testing::scratch_directory;

    fn generation(value: u64) -> Generation {
        let mut clock = GenerationClock::default();
        let mut issued = Generation::INITIAL;
        for _ in 0..value {
            issued = clock.issue().expect("a generation");
        }
        issued
    }

    #[test]
    fn a_probe_comes_back_stamped_with_the_generation_that_asked() {
        let root = scratch_directory("worker-probe");
        let path = root.join("nota.txt");
        fs::write(&path, b"contenido de texto\n").expect("write");

        let (sender, receiver) = mpsc::channel();
        let worker = DocumentWorker::new(move |completion| {
            let _ = sender.send(completion);
        })
        .expect("start the worker");

        worker
            .submit(Job::Probe {
                path,
                generation: generation(7),
                limits: Limits::default(),
            })
            .expect("submit");

        match receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("a completion")
        {
            Completion::Probed {
                generation: g,
                result,
            } => {
                assert_eq!(g.value(), 7);
                assert!(result.expect("a probe").classification.is_editable());
            }
            other => panic!("unexpected completion: {other:?}"),
        }

        drop(worker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_newest_question_is_always_the_last_one_answered() {
        let root = scratch_directory("worker-supersede");
        let paths: Vec<_> = (1..=4)
            .map(|index| {
                let path = root.join(format!("archivo-{index}.txt"));
                fs::write(&path, format!("contenido {index}\n")).expect("write");
                path
            })
            .collect();

        let (sender, receiver) = mpsc::channel();
        let worker = DocumentWorker::new(move |completion| {
            let _ = sender.send(completion);
        })
        .expect("start the worker");

        for (index, path) in paths.into_iter().enumerate() {
            worker
                .submit(Job::Open {
                    path,
                    generation: generation(index as u64 + 1),
                    limits: Limits::default(),
                })
                .expect("submit");
        }

        // Whether the worker drained one question or all four depends on
        // timing; what may never differ is which answer comes last.
        let mut answers = Vec::new();
        let first = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("at least one completion");
        answers.push(first);
        while let Ok(completion) = receiver.recv_timeout(Duration::from_millis(300)) {
            answers.push(completion);
        }

        let last = answers.pop().expect("a last completion");
        match last {
            Completion::Opened { generation, result } => {
                assert_eq!(generation.value(), 4);
                assert_eq!(result.expect("an open").text, "contenido 4\n");
            }
            other => panic!("unexpected completion: {other:?}"),
        }
        assert!(
            answers.len() < 3,
            "queued questions must be superseded, not all executed"
        );

        drop(worker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discarding_queries_leaves_a_promised_save_alone() {
        let root = scratch_directory("worker-save-survives");
        let path = root.join("documento.txt");
        fs::write(&path, b"antes\n").expect("write");

        let opened = crate::open::open(
            &path,
            generation(1),
            Limits::default(),
            &celestina_core::CancellationToken::new(),
        )
        .expect("open");
        let mut document = crate::Document::from_opened(opened);
        document
            .insert(document.buffer().end_position(), "despues\n")
            .expect("insert");

        let (sender, receiver) = mpsc::channel();
        let worker = DocumentWorker::new(move |completion| {
            let _ = sender.send(completion);
        })
        .expect("start the worker");

        worker
            .submit(Job::Save {
                request: Box::new(document.save_request()),
                generation: document.generation(),
            })
            .expect("submit");
        worker.discard_queries();

        match receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("a completion")
        {
            Completion::Saved {
                generation,
                revision,
                result,
            } => {
                assert_eq!(generation, document.generation());
                assert_eq!(revision, document.revision());
                result.expect("the save must succeed");
            }
            other => panic!("unexpected completion: {other:?}"),
        }
        assert_eq!(fs::read(&path).expect("read back"), b"antes\ndespues\n");

        drop(worker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dropping_an_idle_worker_joins_its_thread() {
        let worker = DocumentWorker::new(|_| {}).expect("start the worker");
        drop(worker);
    }
}
