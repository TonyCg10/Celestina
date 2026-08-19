//! Deferred navigation: a history move that only becomes real once it works.
//!
//! Back, forward, up, home and activate all name a destination *before* anyone
//! knows it can be read. Committing the history first is what leaves a file
//! manager pointing at an unreadable directory while the list still shows the
//! previous one — so the move is staged here and applied only when its scan has
//! succeeded.

use std::path::{Path, PathBuf};

use siderita_core::NavigationHistory;

/// A navigation whose history change is held back until its scan succeeds, so a
/// failed back / forward / up / home / activate never leaves the path pointing
/// at an unreadable directory while the list still shows the previous one.
pub(crate) enum PendingNav {
    Back(PathBuf),
    Forward(PathBuf),
    To(PathBuf),
}

impl PendingNav {
    pub(crate) fn destination(&self) -> &Path {
        match self {
            PendingNav::Back(path) | PendingNav::Forward(path) | PendingNav::To(path) => path,
        }
    }

    /// Applies the navigation to `history` once its scan has succeeded.
    pub(crate) fn commit(self, history: &mut NavigationHistory) {
        match self {
            PendingNav::Back(_) => {
                history.go_back();
            }
            PendingNav::Forward(_) => {
                history.go_forward();
            }
            PendingNav::To(path) => {
                history.navigate_to(path);
            }
        }
    }
}
