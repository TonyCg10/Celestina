//! The libmpv-backed implementation of the narrow contract.
//!
//! Each operation builds its own backend instance and drops it when finished.
//! That costs a little more than keeping one warm, and buys two things worth
//! more: a file that wedges the backend cannot poison the next job, and a
//! catalogue scan never shares mutable state with a playing session.

use std::path::{Path, PathBuf};

use celestina_core::CancellationToken;

use crate::backend::{
    ArtworkJob, EngineSession, MediaEngine, ProbeBudget, ProbeReport, SessionRequest, TrailerJob,
    TrailerOutcome,
};
use crate::error::EngineResult;
use crate::session::MpvSession;
use crate::{artwork, probe, trailer};

/// The engine every host talks to. Cheap to create and to clone by reference:
/// it holds no backend state of its own.
#[derive(Clone, Copy, Debug, Default)]
pub struct MpvEngine;

impl MpvEngine {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl MediaEngine for MpvEngine {
    fn probe(
        &self,
        path: &Path,
        budget: ProbeBudget,
        cancellation: &CancellationToken,
    ) -> EngineResult<ProbeReport> {
        probe::probe(path, budget, cancellation)
    }

    fn publish_artwork(&self, request: &ArtworkJob) -> EngineResult<PathBuf> {
        artwork::publish(request)
    }

    fn produce_trailer(&self, job: &TrailerJob) -> EngineResult<TrailerOutcome> {
        trailer::produce(job)
    }

    fn open_session(&self, request: SessionRequest) -> EngineResult<Box<dyn EngineSession>> {
        Ok(Box::new(MpvSession::open(request)?))
    }
}

#[cfg(test)]
mod tests {
    use super::MpvEngine;
    use crate::backend::{MediaEngine, ProbeBudget};
    use celestina_core::CancellationToken;
    use std::path::Path;

    #[test]
    fn a_cancelled_probe_returns_before_touching_the_backend() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = MpvEngine::new()
            .probe(
                Path::new("/home/toni/Vídeos/clip.mp4"),
                ProbeBudget::conservative(),
                &cancellation,
            )
            .expect_err("cancelled before starting");

        assert!(error.is_retryable());
    }

    #[test]
    fn a_missing_file_fails_with_a_typed_error_rather_than_a_panic() {
        let error = MpvEngine::new()
            .probe(
                Path::new("/nonexistent/fluorita/definitely-not-here.mkv"),
                ProbeBudget::conservative(),
                &CancellationToken::new(),
            )
            .expect_err("the file does not exist");

        assert!(!error.is_retryable(), "a missing file is not transient");
    }
}
