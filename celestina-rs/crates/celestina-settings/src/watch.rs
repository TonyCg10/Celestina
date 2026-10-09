//! Following the appearance file while a window is open.
//!
//! The watch is on the file's directory, not the file: [`crate::save`] and
//! any careful editor publish a new file by renaming a sibling over it, which
//! a watch on the old inode would never see. Events are coalesced for 300 ms,
//! the file is read again, and the callback runs only when the value it
//! answers differs from the last one, so a rewrite of the same bytes — or an
//! unrelated file in `~/.config/celestina` — is silent.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::appearance::{load, load_from, located};
use crate::{Appearance, SettingsError};

const COALESCE: Duration = Duration::from_millis(300);

/// A running watch. Dropping it stops the watcher and its thread.
pub struct WatchHandle {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

/// Calls `on_change` on the watcher's thread with the new appearance each time
/// the file's value changes.
///
/// The value the watch starts from is read here, before it returns; a caller
/// that needs the current value reads [`crate::load`] after starting the
/// watch, so nothing that changes in between is missed. The configuration
/// directory is created if it does not exist, since a missing directory cannot
/// be watched.
///
/// # Errors
///
/// [`SettingsError::NoConfigHome`] without a configuration directory,
/// [`SettingsError::Write`] when `~/.config/celestina` cannot be created, and
/// [`SettingsError::Watch`] when the watcher cannot start.
pub fn watch(
    on_change: impl Fn(Appearance) + Send + 'static,
) -> Result<WatchHandle, SettingsError> {
    let path = located().ok_or(SettingsError::NoConfigHome)?;
    let directory = path
        .parent()
        .map(PathBuf::from)
        .ok_or(SettingsError::NoConfigHome)?;
    // inotify can only watch a directory that exists, and the file must be
    // seen from its first save. The directory is therefore created here, and
    // only here — when a window starts following the file — never by `load`.
    std::fs::create_dir_all(&directory).map_err(|source| SettingsError::Write {
        path: directory.clone(),
        source,
    })?;

    let name = path.file_name().map(OsStr::to_os_string);
    let mut last = load_from(&path);
    let file = path.clone();
    let mut debouncer = new_debouncer(COALESCE, None, move |result: DebounceEventResult| {
        let concerned = match &result {
            Ok(events) => events.iter().any(|event| {
                !matches!(event.event.kind, EventKind::Access(_))
                    && event
                        .event
                        .paths
                        .iter()
                        .any(|changed| changed.file_name() == name.as_deref())
            }),
            // Lost events: the file may have changed unseen, so read it.
            Err(_) => true,
        };
        if !concerned {
            return;
        }
        let next = load_from(&file);
        if next != last {
            last = next;
            on_change(next);
        }
    })
    .map_err(|source| SettingsError::Watch {
        path: directory.clone(),
        source,
    })?;
    debouncer
        .watch(&directory, RecursiveMode::NonRecursive)
        .map_err(|source| SettingsError::Watch {
            path: directory,
            source,
        })?;
    Ok(WatchHandle {
        _debouncer: debouncer,
    })
}

/// A running [`follow`]. Dropping it ends the follower's thread, which drops
/// its watch.
pub struct Follower {
    _stop: Option<mpsc::Sender<()>>,
}

/// Follows the appearance file for a window, on a thread of its own.
///
/// The thread starts [`watch`] first and then delivers [`load`]'s answer, so a
/// change made in between reaches `deliver` through one or the other; after
/// that, `deliver` runs on the watcher's thread for each change. The first
/// delivery is the current value: a caller that already seeded itself with
/// [`load`] sees it as no change. A watch that cannot start is logged and the
/// first delivery still happens.
pub fn follow(deliver: impl Fn(Appearance) + Clone + Send + 'static) -> Follower {
    let (stop, stopped) = mpsc::channel::<()>();
    let spawned = std::thread::Builder::new()
        .name("appearance".to_owned())
        .spawn(move || {
            let handle = match watch(deliver.clone()) {
                Ok(handle) => Some(handle),
                Err(error) => {
                    eprintln!("celestina-settings: the appearance file is not followed: {error}");
                    None
                }
            };
            deliver(load());
            // Blocks until the follower is dropped, then drops the watch.
            let _ = stopped.recv();
            drop(handle);
        });
    match spawned {
        Ok(_) => Follower { _stop: Some(stop) },
        Err(error) => {
            eprintln!("celestina-settings: the appearance follower did not start: {error}");
            Follower { _stop: None }
        }
    }
}
