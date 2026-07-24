#![forbid(unsafe_code)]

mod coordinator;
mod entry;
mod executor;
mod name_filter;
mod name_order;
mod navigation;
mod scan;
mod view;
mod watch;

pub use coordinator::{PublishOutcome, ScanCoordinator};
pub use entry::{DirectoryEntry, EntryId, EntryKind};
pub use executor::{ExecutorStopped, ScanExecutor, ScanResult};
pub use name_filter::{matches as name_matches, matches_any as name_matches_any};
pub use name_order::compare_names;
pub use navigation::NavigationHistory;
pub use scan::{scan_directory, DirectorySnapshot, ScanError, ScanRequest};
pub use view::{project_snapshot, SortDirection, SortField, ViewOptions};
pub use watch::{SnapshotFreshness, WatchHealth, WatchState};
