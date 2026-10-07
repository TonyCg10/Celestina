//! Running the artwork job over a list of items, one at a time, without
//! letting one item decide the fate of the rest.
//!
//! The pass used to be an explicit button: it submitted a job, waited once,
//! and stopped the *whole* batch the first time a wait came back empty — so a
//! single clip the backend could not answer for left every item after it with
//! no poster. Posters are now produced in the background after every scan,
//! which makes that rule untenable: an item that does not answer in time is
//! cancelled and counted as unanswered, and the pass goes on to the next one.
//!
//! The wait is answerable to the host's token in short slices, and every job
//! carries a generation of its own, so the late answer of an item that was
//! given up on is recognised and dropped rather than credited to the next.

use std::path::Path;
use std::time::{Duration, Instant};

use celestina_core::{CancellationToken, Generation, GenerationClock};

use crate::artwork::PendingArtwork;
use crate::backend::ArtworkJob;
use crate::error::EngineError;
use crate::worker::{EngineWorker, Job, JobOutcome};

/// How long the pass waits between two looks at the host's token.
const CANCEL_SLICE: Duration = Duration::from_millis(100);

/// How one item ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtworkAttempt {
    /// A poster or a cover is now in the shared cache.
    Produced,
    /// The backend answered that this file has nothing to give — a broken
    /// clip, a track without a cover. A normal outcome; the grid keeps its
    /// glyph.
    Failed,
    /// The backend did not answer within the item's budget. The job was
    /// cancelled and the pass moved on.
    Unanswered,
    /// The host asked the pass to stop.
    Cancelled,
    /// The engine thread is gone; nothing more can be asked of it.
    Stopped,
}

impl ArtworkAttempt {
    /// Whether the pass may go on to the next item after this one.
    #[must_use]
    pub const fn continues(self) -> bool {
        matches!(self, Self::Produced | Self::Failed | Self::Unanswered)
    }
}

/// One engine worker's artwork jobs, kept apart by generation.
///
/// Lives as long as the worker it drives, across as many passes as its host
/// runs, so generations and staging names never repeat on it.
pub struct ArtworkPass<'worker> {
    worker: &'worker EngineWorker,
    clock: GenerationClock,
    uniquifier: u64,
}

impl<'worker> ArtworkPass<'worker> {
    #[must_use]
    pub fn new(worker: &'worker EngineWorker) -> Self {
        Self {
            worker,
            clock: GenerationClock::default(),
            uniquifier: 0,
        }
    }

    /// Produces artwork for each of `items` in order, and reports every item
    /// through `report` as it ends. Returns how many were produced.
    ///
    /// `deadline` is the backend's own budget for one item; the pass waits
    /// twice that for the answer, so a backend that honours its deadline is
    /// never given up on early. Stops only when the host cancels or the engine
    /// is gone: an item that fails or does not answer ends that item, not the
    /// pass.
    pub fn run(
        &mut self,
        items: Vec<PendingArtwork>,
        cache_root: &Path,
        deadline: Duration,
        cancellation: &CancellationToken,
        mut report: impl FnMut(&PendingArtwork, ArtworkAttempt),
    ) -> usize {
        let mut produced = 0;
        for item in items {
            if cancellation.is_cancelled() {
                break;
            }
            let attempt = self.attempt(&item, cache_root, deadline, cancellation);
            if attempt == ArtworkAttempt::Produced {
                produced += 1;
            }
            report(&item, attempt);
            if !attempt.continues() {
                break;
            }
        }
        produced
    }

    /// One item: submit, then wait for *its* answer for at most twice
    /// `deadline`.
    pub fn attempt(
        &mut self,
        item: &PendingArtwork,
        cache_root: &Path,
        deadline: Duration,
        cancellation: &CancellationToken,
    ) -> ArtworkAttempt {
        let Ok(generation) = self.clock.issue() else {
            return ArtworkAttempt::Stopped;
        };
        // Two jobs must never stage into the same temporary name, and a job
        // given up on may still be running when the next one starts.
        self.uniquifier = self.uniquifier.wrapping_add(1);
        let job = ArtworkJob {
            source: item.source.clone(),
            cache_root: cache_root.to_path_buf(),
            origin: item.origin,
            source_mtime: item.source_mtime,
            uniquifier: self.uniquifier,
            deadline,
            cancellation: cancellation.clone(),
        };
        if self
            .worker
            .submit(Job::Artwork {
                generation,
                job: Box::new(job),
            })
            .is_err()
        {
            return ArtworkAttempt::Stopped;
        }
        self.await_answer(generation, deadline.saturating_mul(2), cancellation)
    }

    fn await_answer(
        &self,
        generation: Generation,
        budget: Duration,
        cancellation: &CancellationToken,
    ) -> ArtworkAttempt {
        let deadline = Instant::now() + budget;
        loop {
            if cancellation.is_cancelled() {
                // The job inside the worker runs under the worker's own
                // token, not this one, so it is told separately.
                self.worker.cancel_current();
                return ArtworkAttempt::Cancelled;
            }
            let now = Instant::now();
            if now >= deadline {
                self.worker.cancel_current();
                return ArtworkAttempt::Unanswered;
            }
            let Some(outcome) = self.worker.poll(CANCEL_SLICE.min(deadline - now)) else {
                continue;
            };
            // The late answer of an item given up on earlier: not this one's.
            if outcome.generation() != generation {
                continue;
            }
            return match outcome {
                JobOutcome::Artwork { result: Ok(_), .. } => ArtworkAttempt::Produced,
                JobOutcome::Artwork {
                    result: Err(EngineError::Cancelled),
                    ..
                } if cancellation.is_cancelled() => ArtworkAttempt::Cancelled,
                _ => ArtworkAttempt::Failed,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtworkAttempt, ArtworkPass};
    use crate::artwork::PendingArtwork;
    use crate::backend::{
        ArtworkJob, EngineSession, MediaEngine, ProbeBudget, ProbeReport, SessionRequest,
    };
    use crate::error::{EngineError, EngineResult};
    use crate::worker::EngineWorker;
    use celestina_core::CancellationToken;
    use fluorita_core::{ArtworkOrigin, MediaId};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    /// An engine with no decoder: `silent` never answers until cancelled,
    /// `broken` refuses, everything else is produced at once.
    struct ScriptedEngine;

    impl MediaEngine for ScriptedEngine {
        fn probe(
            &self,
            _path: &Path,
            _budget: ProbeBudget,
            _cancellation: &CancellationToken,
        ) -> EngineResult<ProbeReport> {
            Ok(ProbeReport::default())
        }

        fn publish_artwork(&self, request: &ArtworkJob) -> EngineResult<PathBuf> {
            match request.source.file_stem().and_then(|stem| stem.to_str()) {
                Some("silent") => {
                    while !request.cancellation.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(EngineError::Cancelled)
                }
                Some("broken") => Err(EngineError::UnusableSource {
                    path: request.source.clone(),
                    reason: "no frame",
                }),
                _ => Ok(request.source.with_extension("png")),
            }
        }

        fn open_session(&self, _request: SessionRequest) -> EngineResult<Box<dyn EngineSession>> {
            Err(EngineError::WorkerStopped)
        }
    }

    fn item(inode: u64, name: &str) -> PendingArtwork {
        PendingArtwork {
            media: MediaId::filesystem(7, inode),
            source: PathBuf::from(format!("/m/{name}.mkv")),
            origin: ArtworkOrigin::VideoPoster,
            source_mtime: SystemTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn a_pass_continues_past_an_item_whose_poll_returns_nothing() {
        let worker = EngineWorker::with_engine(ScriptedEngine).expect("worker");
        let mut pass = ArtworkPass::new(&worker);
        let mut seen = Vec::new();

        let produced = pass.run(
            vec![
                item(1, "first"),
                item(2, "silent"),
                item(3, "broken"),
                item(4, "last"),
            ],
            Path::new("/cache"),
            // Short, so the silent item's wait comes back empty quickly.
            Duration::from_millis(300),
            &CancellationToken::new(),
            |item, attempt| seen.push((item.source.clone(), attempt)),
        );

        // The empty wait ended the silent item and nothing else: the two after
        // it were still asked, and the silent one's late "cancelled" answer
        // was not mistaken for theirs.
        assert_eq!(produced, 2);
        assert_eq!(
            seen,
            vec![
                (PathBuf::from("/m/first.mkv"), ArtworkAttempt::Produced),
                (PathBuf::from("/m/silent.mkv"), ArtworkAttempt::Unanswered),
                (PathBuf::from("/m/broken.mkv"), ArtworkAttempt::Failed),
                (PathBuf::from("/m/last.mkv"), ArtworkAttempt::Produced),
            ]
        );
    }

    #[test]
    fn a_cancelled_pass_stops_without_asking_for_the_rest() {
        let worker = EngineWorker::with_engine(ScriptedEngine).expect("worker");
        let mut pass = ArtworkPass::new(&worker);
        let cancellation = CancellationToken::new();
        let mut seen = 0;

        let produced = pass.run(
            vec![item(1, "first"), item(2, "second")],
            Path::new("/cache"),
            Duration::from_secs(30),
            &cancellation,
            |_, _| {
                seen += 1;
                cancellation.cancel();
            },
        );

        assert_eq!(produced, 1);
        assert_eq!(seen, 1, "the host stopped the pass after the first item");
    }

    #[test]
    fn cancelling_while_an_item_waits_returns_within_a_slice() {
        let worker = EngineWorker::with_engine(ScriptedEngine).expect("worker");
        let mut pass = ArtworkPass::new(&worker);
        let cancellation = CancellationToken::new();
        let canceller = {
            let cancellation = cancellation.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                cancellation.cancel();
            })
        };

        let started = std::time::Instant::now();
        let attempt = pass.attempt(
            &item(1, "silent"),
            Path::new("/cache"),
            Duration::from_secs(30),
            &cancellation,
        );
        canceller.join().expect("canceller");

        assert_eq!(attempt, ArtworkAttempt::Cancelled);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the wait ignored the token"
        );
    }
}
