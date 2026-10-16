//! The desktop index: the session's `.desktop` applications for the
//! launcher (design §5.5, Launcher).
//!
//! A named worker reads the XDG application directories through
//! `celestina_core::desktop_entry::scan` (bounded to [`MAX_INDEXED_ENTRIES`]
//! files of at most 64 KiB each, a user override shadowing a system entry of
//! the same id) at start and every [`RESCAN`], keeps the entries a person
//! should be offered on this desktop, and publishes them as
//! [`Message::DesktopIndex`] whenever they differ from the last ones sent.
//! The wait between scans looks at the shutdown flag every
//! [`WAIT_SLICE`], and the scan itself stops within one file read once the
//! indexer is joined, which cancels its token.

use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use celestina_core::{
    desktop_entry::{self, DesktopEntry, ScanCancelled},
    CancellationToken,
};
use celestina_wayland::Sender;

use super::Worker;
use crate::message::Message;

/// How often the index is read again.
pub const RESCAN: Duration = Duration::from_secs(5 * 60);
/// More ids than this is not a session's applications but a directory tree
/// gone wrong; a desktop offers a few hundred to a couple of thousand.
pub const MAX_INDEXED_ENTRIES: usize = 4096;
/// How often the wait between scans looks at the shutdown flag.
pub const WAIT_SLICE: Duration = Duration::from_millis(100);

/// The index's worker and the token that stops a scan in flight.
#[derive(Debug)]
pub struct DesktopIndexer {
    worker: Worker,
    cancel: CancellationToken,
}

impl DesktopIndexer {
    /// Cancels a scan in flight and joins the worker, waiting at most
    /// `wait`; whether it stopped in time.
    pub fn join_within(self, wait: Duration) -> bool {
        self.cancel.cancel();
        self.worker.join_within(wait)
    }
}

/// The desktop name `OnlyShowIn` and `NotShowIn` are compared with: the
/// first name of `$XDG_CURRENT_DESKTOP`; empty when the session names none,
/// which excludes nothing.
fn current_desktop() -> String {
    std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .and_then(|value| value.split(':').next().map(str::to_owned))
        .unwrap_or_default()
}

/// The entries of one scan of `dirs` a person should be offered on
/// `desktop`.
///
/// # Errors
///
/// [`ScanCancelled`] when `cancel` was cancelled before the scan ended.
pub fn index_in(
    dirs: &[PathBuf],
    cancel: &CancellationToken,
    desktop: &str,
) -> Result<Vec<DesktopEntry>, ScanCancelled> {
    let scan = desktop_entry::scan(dirs, cancel, MAX_INDEXED_ENTRIES)?;
    if scan.truncated {
        tracing::warn!(
            limit = MAX_INDEXED_ENTRIES,
            "desktop index: more applications than the limit; the rest are not offered"
        );
    }
    Ok(scan
        .entries
        .into_iter()
        .filter(|entry| entry.is_listable() && entry.shows_in(desktop))
        .collect())
}

/// Starts the index's worker, publishing into `sender`; it stops when
/// `shutdown` is raised or the indexer is joined.
///
/// # Errors
///
/// The failure to start the thread.
pub fn spawn(sender: Sender<Message>, shutdown: &Arc<AtomicBool>) -> io::Result<DesktopIndexer> {
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let flag = Arc::clone(shutdown);
    let worker = Worker::spawn("desktop-index", shutdown, move || {
        run(&sender, &flag, &token);
    })?;
    Ok(DesktopIndexer { worker, cancel })
}

fn run(sender: &Sender<Message>, shutdown: &AtomicBool, cancel: &CancellationToken) {
    let desktop = current_desktop();
    let mut last: Option<Vec<DesktopEntry>> = None;
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let dirs = desktop_entry::application_search_dirs();
        let started = Instant::now();
        let Ok(entries) = index_in(&dirs, cancel, &desktop) else {
            return;
        };
        tracing::debug!(
            entries = entries.len(),
            ms = started.elapsed().as_millis(),
            "desktop index: scanned"
        );
        if last.as_ref() != Some(&entries) {
            if sender.send(Message::DesktopIndex(entries.clone())).is_err() {
                return;
            }
            last = Some(entries);
        }
        if !wait(RESCAN, shutdown, cancel) {
            return;
        }
    }
}

/// Sleeps `total` in slices of [`WAIT_SLICE`]; false as soon as the
/// shutdown flag or the token says to stop.
fn wait(total: Duration, shutdown: &AtomicBool, cancel: &CancellationToken) -> bool {
    let deadline = Instant::now() + total;
    loop {
        if shutdown.load(Ordering::Acquire) || cancel.is_cancel_requested() {
            return false;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        thread::sleep(left.min(WAIT_SLICE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test, removed when the guard drops.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir()
                .join(format!("celestina-desktop-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(&directory).expect("a test directory");
            Self(directory)
        }

        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.0.join(name);
            std::fs::create_dir_all(&dir).expect("a directory");
            dir
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(dir: &std::path::Path, id: &str, body: &str) {
        std::fs::write(dir.join(id), body).expect("a desktop file");
    }

    #[test]
    fn the_index_offers_listable_entries_for_this_desktop_and_honours_overrides() {
        let scratch = Scratch::new("index");
        let user = scratch.dir("user");
        let system = scratch.dir("system");
        let app = |name: &str, extra: &str| {
            format!("[Desktop Entry]\nType=Application\nName={name}\nExec={name}\n{extra}")
        };
        write(&system, "files.desktop", &app("Files", ""));
        write(&system, "hidden.desktop", &app("Gone", ""));
        write(&user, "hidden.desktop", &app("Gone", "Hidden=true\n"));
        write(
            &system,
            "nodisplay.desktop",
            &app("Helper", "NoDisplay=true\n"),
        );
        write(&system, "kde.desktop", &app("Kde", "OnlyShowIn=KDE;\n"));
        write(
            &system,
            "link.desktop",
            "[Desktop Entry]\nType=Link\nName=Web\n",
        );
        write(&system, "notes.txt", &app("Notes", ""));

        let entries =
            index_in(&[user, system], &CancellationToken::new(), "niri").expect("not cancelled");
        let mut ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["files.desktop"]);
    }

    #[test]
    fn a_cancelled_scan_publishes_nothing() {
        let scratch = Scratch::new("cancelled");
        let dir = scratch.dir("apps");
        write(
            &dir,
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=A\nExec=a\n",
        );
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(index_in(&[dir], &cancel, ""), Err(ScanCancelled));
    }

    #[test]
    fn the_wait_ends_at_once_on_shutdown_or_cancel() {
        let started = Instant::now();
        let stop = AtomicBool::new(true);
        assert!(!wait(RESCAN, &stop, &CancellationToken::new()));
        let token = CancellationToken::new();
        token.cancel();
        assert!(!wait(RESCAN, &AtomicBool::new(false), &token));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(wait(
            Duration::from_millis(10),
            &AtomicBool::new(false),
            &CancellationToken::new()
        ));
    }
}
