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
    /// A rescan of the location is in flight.
    rescanning: bool,
    /// A change arrived after that rescan began, so it may have read the
    /// folder before the change: the rescan's landing cannot declare the
    /// snapshot fresh.
    changed_while_rescanning: bool,
}

impl WatchState {
    #[must_use]
    pub fn active(location: impl Into<PathBuf>) -> Self {
        Self {
            location: location.into(),
            health: WatchHealth::Active,
            freshness: SnapshotFreshness::Fresh,
            rescanning: false,
            changed_while_rescanning: false,
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

    /// Records that a rescan of `location` has begun: a change observed from
    /// now on happened after it may have read the folder.
    pub fn begin_rescan(&mut self, location: &Path) -> bool {
        if location != self.location {
            return false;
        }
        self.rescanning = true;
        self.changed_while_rescanning = false;
        true
    }

    /// Marks the current snapshot stale when the event belongs to its watch.
    pub fn observe_change(&mut self, watched_location: &Path) -> bool {
        if watched_location != self.location {
            return false;
        }
        if self.rescanning {
            self.changed_while_rescanning = true;
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
    ///
    /// A change observed after the rescan began keeps the snapshot stale: the
    /// rescan may have read the folder before that change, and declaring it
    /// fresh would drop the change until some unrelated event arrived. The
    /// caller asks [`Self::freshness`] afterwards and rescans again.
    pub fn mark_rescanned(&mut self, location: &Path) -> bool {
        if location != self.location {
            return false;
        }

        self.rescanning = false;
        self.freshness = if std::mem::take(&mut self.changed_while_rescanning) {
            SnapshotFreshness::Stale
        } else {
            SnapshotFreshness::Fresh
        };
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

    /// A change that arrives while a rescan of the folder is in flight may
    /// postdate its read: the landing keeps the snapshot stale, so the
    /// controller rescans once more instead of dropping the change. A change
    /// from before the rescan began is covered by it.
    #[test]
    fn a_change_during_a_rescan_keeps_the_snapshot_stale() {
        let folder = Path::new("/tmp/folder");
        let mut watch = WatchState::active(folder);

        assert!(watch.observe_change(folder));
        assert!(watch.begin_rescan(folder));
        assert!(watch.mark_rescanned(folder));
        assert_eq!(watch.freshness(), SnapshotFreshness::Fresh);

        assert!(watch.begin_rescan(folder));
        watch.observe_change(folder);
        assert!(watch.mark_rescanned(folder));
        assert_eq!(watch.freshness(), SnapshotFreshness::Stale);

        // The follow-up rescan, with nothing new meanwhile, settles it.
        assert!(watch.begin_rescan(folder));
        assert!(watch.mark_rescanned(folder));
        assert_eq!(watch.freshness(), SnapshotFreshness::Fresh);
    }

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
