//! One filesystem watch per folder, shared by every tab looking at it.
//!
//! A watch used to belong to a controller, and a controller belongs to a tab.
//! Three tabs on the same folder therefore held three inotify watches on it and
//! woke three times for one write — the kernel doing the same work three times
//! so the application could reach the same conclusion three times.
//!
//! The register below keeps one debouncer for the process and one watch per
//! folder, with the controllers interested in it. A change wakes exactly the
//! tabs showing that folder, each of which rescans and — since a projection
//! that matches the published one is not republished — usually stops there.

use core::pin::Pin;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use cxx_qt::CxxQtThread;
use notify_debouncer_full::notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};

use super::qobject;
use crate::controller::FsDebouncer;

/// How long changes are gathered before a folder is told about them. Long
/// enough that a program writing a file in bursts wakes us once, short enough
/// that a person moving a file sees it appear.
const COALESCE: std::time::Duration = std::time::Duration::from_millis(200);

/// The process's watches: one debouncer, and who cares about each folder.
struct Watches {
    debouncer: Option<FsDebouncer>,
    /// Folder → the controllers showing it. A folder with no listeners left is
    /// unwatched, so a closed tab releases the kernel's watch with it.
    interested: HashMap<PathBuf, Vec<CxxQtThread<qobject::SideritaController>>>,
}

fn watches() -> &'static Mutex<Watches> {
    static WATCHES: OnceLock<Mutex<Watches>> = OnceLock::new();
    WATCHES.get_or_init(|| {
        Mutex::new(Watches {
            debouncer: None,
            interested: HashMap::new(),
        })
    })
}

/// Wakes every controller showing `folder`.
///
/// This is also where a closed tab is forgotten: nothing signals the register
/// when a controller dies, but its queue starts failing, and a folder left with
/// no listeners is unwatched on the spot.
///
/// A queue that fails names a controller whose tab is gone; it is dropped here,
/// and a folder left with nobody watching it is unwatched.
fn wake(folder: &Path, degraded: bool) {
    let Ok(mut state) = watches().lock() else {
        return;
    };
    let Some(listeners) = state.interested.get_mut(folder) else {
        return;
    };
    listeners.retain(|listener| {
        listener
            .queue(move |controller: Pin<&mut qobject::SideritaController>| {
                controller.on_fs_change(degraded);
            })
            .is_ok()
    });
    if listeners.is_empty() {
        state.interested.remove(folder);
        if let Some(debouncer) = state.debouncer.as_mut() {
            let _ = debouncer.unwatch(folder);
        }
    }
}

/// Creates the one debouncer, the first time a folder is watched.
///
/// Its callback runs on the notify thread and only ever marshals "this folder
/// changed" back to the Qt thread; it never touches Qt state directly.
fn ensure_debouncer(state: &mut Watches) -> bool {
    if state.debouncer.is_some() {
        return true;
    }
    let created = new_debouncer(
        COALESCE,
        None,
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                // Access events (open/close/read) are ignored: our own scan opens
                // the directory, which notify reports as IN_OPEN, and reacting to
                // that would loop scan → open → scan.
                let mut changed: Vec<PathBuf> = Vec::new();
                for event in &events {
                    if matches!(event.event.kind, EventKind::Access(_)) {
                        continue;
                    }
                    for path in &event.event.paths {
                        // The watch is on the folder; an event names the entry
                        // inside it, so the folder is its parent — and, for the
                        // folder itself, the path.
                        for candidate in [path.parent().map(Path::to_path_buf), Some(path.clone())]
                            .into_iter()
                            .flatten()
                        {
                            if !changed.contains(&candidate) {
                                changed.push(candidate);
                            }
                        }
                    }
                }
                for folder in changed {
                    wake(&folder, false);
                }
            }
            Err(_errors) => {
                let folders: Vec<PathBuf> = watches()
                    .lock()
                    .map(|state| state.interested.keys().cloned().collect())
                    .unwrap_or_default();
                for folder in folders {
                    wake(&folder, true);
                }
            }
        },
    );
    match created {
        Ok(debouncer) => {
            state.debouncer = Some(debouncer);
            true
        }
        Err(_) => false,
    }
}

/// Registers `controller` as interested in `folder`, dropping its interest in
/// `previous`. Answers whether the folder is actually being watched.
pub(crate) fn follow(
    thread: CxxQtThread<qobject::SideritaController>,
    previous: Option<&Path>,
    folder: &Path,
) -> bool {
    let Ok(mut state) = watches().lock() else {
        return false;
    };
    if let Some(previous) = previous {
        if previous != folder {
            release_locked(&mut state, previous);
        }
    }
    if !ensure_debouncer(&mut state) {
        return false;
    }
    if let Some(listeners) = state.interested.get_mut(folder) {
        // Somebody is already watching it: joining costs nothing.
        listeners.push(thread);
        return true;
    }
    let Some(debouncer) = state.debouncer.as_mut() else {
        return false;
    };
    if debouncer
        .watch(folder, RecursiveMode::NonRecursive)
        .is_err()
    {
        return false;
    }
    state.interested.insert(folder.to_path_buf(), vec![thread]);
    true
}

fn release_locked(state: &mut Watches, folder: &Path) {
    let Some(listeners) = state.interested.get_mut(folder) else {
        return;
    };
    // One interest, not one controller: a tab that navigated away is the one
    // calling, and its entry is indistinguishable from another tab's.
    listeners.pop();
    if listeners.is_empty() {
        state.interested.remove(folder);
        if let Some(debouncer) = state.debouncer.as_mut() {
            let _ = debouncer.unwatch(folder);
        }
    }
}
