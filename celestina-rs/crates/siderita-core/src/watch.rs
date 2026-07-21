use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchHealth {
    Active,
    Degraded { reason: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotFreshness {
    Fresh,
    Stale,
}

/// Tracks watcher truth without interpreting filesystem events as model edits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchState {
    location: PathBuf,
    health: WatchHealth,
    freshness: SnapshotFreshness,
}

impl WatchState {
    #[must_use]
    pub fn active(location: impl Into<PathBuf>) -> Self {
        Self {
            location: location.into(),
            health: WatchHealth::Active,
            freshness: SnapshotFreshness::Fresh,
        }
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        &self.location
    }

    #[must_use]
    pub const fn health(&self) -> &WatchHealth {
        &self.health
    }

    #[must_use]
    pub const fn freshness(&self) -> SnapshotFreshness {
        self.freshness
    }

    /// Marks the current snapshot stale when the event belongs to its watch.
    pub fn observe_change(&mut self, watched_location: &Path) -> bool {
        if watched_location != self.location {
            return false;
        }

        let changed = self.freshness != SnapshotFreshness::Stale;
        self.freshness = SnapshotFreshness::Stale;
        changed
    }

    /// Records loss of watch coverage and requests a full rescan.
    pub fn degrade(&mut self, watched_location: &Path, reason: impl Into<String>) -> bool {
        if watched_location != self.location {
            return false;
        }

        self.health = WatchHealth::Degraded {
            reason: reason.into(),
        };
        self.freshness = SnapshotFreshness::Stale;
        true
    }

    /// Records a successful rescan. It does not claim the watcher recovered.
    pub fn mark_rescanned(&mut self, location: &Path) -> bool {
        if location != self.location {
            return false;
        }

        self.freshness = SnapshotFreshness::Fresh;
        true
    }

    /// Reattaches watch coverage. A rescan is still required afterwards.
    pub fn recover(&mut self, location: &Path) -> bool {
        if location != self.location {
            return false;
        }

        self.health = WatchHealth::Active;
        self.freshness = SnapshotFreshness::Stale;
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{SnapshotFreshness, WatchHealth, WatchState};

    #[test]
    fn change_invalidates_but_never_changes_location() {
        let mut state = WatchState::active("/current");

        assert!(state.observe_change(Path::new("/current")));
        assert_eq!(state.freshness(), SnapshotFreshness::Stale);
        assert_eq!(state.location(), Path::new("/current"));
        assert_eq!(state.health(), &WatchHealth::Active);
    }

    #[test]
    fn events_for_old_location_are_ignored() {
        let mut state = WatchState::active("/current");

        assert!(!state.observe_change(Path::new("/old")));
        assert!(!state.degrade(Path::new("/old"), "old watcher failed"));
        assert_eq!(state.freshness(), SnapshotFreshness::Fresh);
        assert_eq!(state.health(), &WatchHealth::Active);
    }

    #[test]
    fn rescan_restores_truth_without_claiming_watch_recovery() {
        let mut state = WatchState::active("/current");
        state.degrade(Path::new("/current"), "watch queue overflow");

        assert!(state.mark_rescanned(Path::new("/current")));

        assert_eq!(state.freshness(), SnapshotFreshness::Fresh);
        assert!(matches!(state.health(), WatchHealth::Degraded { .. }));
    }

    #[test]
    fn recovered_watch_remains_stale_until_rescan() {
        let mut state = WatchState::active("/current");
        state.degrade(Path::new("/current"), "watch lost");

        assert!(state.recover(Path::new("/current")));

        assert_eq!(state.health(), &WatchHealth::Active);
        assert_eq!(state.freshness(), SnapshotFreshness::Stale);
    }
}
