use std::path::{Path, PathBuf};

use celestina_core::{CancellationToken, Generation, GenerationClock, GenerationExhausted};

use crate::scan::{DirectorySnapshot, ScanError, ScanRequest};

#[derive(Debug)]
struct ActiveScan {
    generation: Generation,
    location: PathBuf,
    cancellation: CancellationToken,
}

#[derive(Debug, Default)]
pub struct ScanCoordinator {
    clock: GenerationClock,
    active: Option<ActiveScan>,
}

impl ScanCoordinator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin(
        &mut self,
        location: impl AsRef<Path>,
    ) -> Result<ScanRequest, GenerationExhausted> {
        if let Some(active) = &self.active {
            active.cancellation.cancel();
        }

        let generation = self.clock.issue()?;
        let location = location.as_ref().to_path_buf();
        let cancellation = CancellationToken::new();
        self.active = Some(ActiveScan {
            generation,
            location: location.clone(),
            cancellation: cancellation.clone(),
        });

        Ok(ScanRequest::new(generation, location, cancellation))
    }

    pub fn publish(&mut self, snapshot: DirectorySnapshot) -> PublishOutcome {
        let is_current = self.matches_active(snapshot.generation(), snapshot.location());

        if is_current {
            self.active = None;
            PublishOutcome::Accepted(snapshot)
        } else {
            PublishOutcome::Stale(snapshot)
        }
    }

    /// Accepts a scan failure only when it belongs to the active request.
    ///
    /// This gives failures the same stale-result protection as snapshots, so
    /// an error from an older directory cannot replace newer navigation state.
    pub fn publish_error(&mut self, error: &ScanError) -> bool {
        let is_current = self.matches_active(error.generation(), error.location());
        if is_current {
            self.active = None;
        }
        is_current
    }

    pub fn cancel_active(&mut self) {
        if let Some(active) = self.active.take() {
            active.cancellation.cancel();
        }
    }

    fn matches_active(&self, generation: Generation, location: &Path) -> bool {
        self.active.as_ref().is_some_and(|active| {
            active.generation == generation
                && active.location == location
                && !active.cancellation.is_cancelled()
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    Accepted(DirectorySnapshot),
    Stale(DirectorySnapshot),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{PublishOutcome, ScanCoordinator};
    use crate::scan::{DirectorySnapshot, ScanError};

    #[test]
    fn older_snapshot_never_replaces_new_navigation() {
        let mut coordinator = ScanCoordinator::new();
        let first = coordinator.begin("A").expect("first request");
        let second = coordinator.begin("B").expect("second request");

        let stale = DirectorySnapshot::empty(first.generation(), PathBuf::from("A"));
        let current = DirectorySnapshot::empty(second.generation(), PathBuf::from("B"));

        assert!(matches!(
            coordinator.publish(stale),
            PublishOutcome::Stale(_)
        ));
        assert!(matches!(
            coordinator.publish(current),
            PublishOutcome::Accepted(_)
        ));
    }

    #[test]
    fn cancelling_active_scan_rejects_its_result() {
        let mut coordinator = ScanCoordinator::new();
        let request = coordinator.begin("A").expect("scan request");
        let result = DirectorySnapshot::empty(request.generation(), PathBuf::from("A"));

        coordinator.cancel_active();

        assert!(matches!(
            coordinator.publish(result),
            PublishOutcome::Stale(_)
        ));
    }

    #[test]
    fn an_older_error_never_replaces_new_navigation() {
        let mut coordinator = ScanCoordinator::new();
        let first = coordinator.begin("A").expect("first request");
        let second = coordinator.begin("B").expect("second request");
        let stale = ScanError::Cancelled {
            generation: first.generation(),
            location: PathBuf::from("A"),
        };

        assert!(!coordinator.publish_error(&stale));

        let current = DirectorySnapshot::empty(second.generation(), PathBuf::from("B"));
        assert!(matches!(
            coordinator.publish(current),
            PublishOutcome::Accepted(_)
        ));
    }
}
