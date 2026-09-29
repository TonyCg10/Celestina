//! The documents Grafita opened last, so a new tab is not a blank wall.
//!
//! Deliberately Grafita's own small file rather than freedesktop's
//! `recently-used.xbel`: that format is XML, and parsing it correctly would earn
//! a dependency for a list of paths. One absolute path per line is enough, and
//! a file this shape can be read by anything, including a person.
//!
//! Two rules keep it honest:
//!
//! - **A path that no longer exists is not offered.** A recent list that opens
//!   nothing is worse than a short one.
//! - **Recording is best-effort.** A history that cannot be written must never
//!   stop a document from opening; the editor's job is editing.
//!
//! Reading the list, checking which entries still exist and writing it back
//! are all blocking: a write syncs the file and its folder, and a `stat` of a
//! path on a dead network mount can hang for as long as the mount does. None
//! of it may run on a host's GUI thread. Two entry points touch the disk, and
//! the document worker runs each as a job:
//!
//! - [`change`] ([`crate::worker::Job::RecentChange`]) records or forgets one
//!   document. It never asks whether anything exists, so opening a document,
//!   including every Siderita preview, never waits on another file's mount.
//! - [`list`] ([`crate::worker::Job::RecentList`]) answers which remembered
//!   documents still exist. The checks run on one detached prober thread and
//!   the answer waits at most [`PROBE_WAIT`]: an entry the prober has not
//!   answered by then is left out, and while the prober is stuck on a dead
//!   mount the list is answered at once and unverified, every remembered
//!   entry included. Offering a missing file there costs little: opening it
//!   fails and forgets it. A hung `stat` therefore costs a short or unchecked
//!   list, never a worker that cannot be joined, and never a list that stays
//!   empty for as long as the mount stays dead.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use celestina_core::{atomic_file, xdg, CancellationToken};

/// How many documents are remembered. Enough to cover "the thing I was just in",
/// short enough that the list stays scannable.
const LIMIT: usize = 12;

/// The recently opened documents, newest first.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Recent {
    paths: Vec<PathBuf>,
}

/// One change to the remembered list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecentChange {
    /// A document opened: it moves to the front.
    Record(PathBuf),
    /// A remembered document no longer opens: it stops being offered.
    Forget(PathBuf),
}

/// Serialises the read-change-write of the list within one process.
///
/// Every session has its own worker, so two tabs that open documents at the
/// same moment would otherwise each read the old list and the second write
/// would drop the first document. It covers only that read, change and write;
/// nothing that can hang on another file is done while it is held.
static UPDATING: Mutex<()> = Mutex::new(());

/// How long [`list`] waits for the prober before answering without it.
pub const PROBE_WAIT: Duration = Duration::from_secs(1);

/// How long one existence check may run before the prober counts as stuck
/// and [`list`] stops asking it. A healthy `stat` takes microseconds; this is
/// shorter than [`PROBE_WAIT`] so that a list asked for just after a stuck one
/// timed out is answered at once rather than waiting out a second period.
const STUCK_AFTER: Duration = Duration::from_millis(500);

/// How often a waiting [`list`] looks at its cancellation token.
const POLL: Duration = Duration::from_millis(20);

/// Applies `change` to the list kept in `store`.
///
/// Blocking: it reads the list and writes it back when it changed. It never
/// asks whether any entry exists. It belongs on the document worker, never on
/// a GUI thread.
pub fn change(store: &Path, change: &RecentChange) {
    let _serialised = UPDATING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut recent = Recent::load_from(store);
    let before = recent.clone();
    match change {
        RecentChange::Record(path) => recent.record(path),
        RecentChange::Forget(path) => recent.forget(path),
    }
    if recent != before {
        recent.store_to(store);
    }
}

/// The documents remembered in `store` that still exist, newest first.
///
/// Blocking for at most about [`PROBE_WAIT`] beyond reading the list, and
/// sooner when `cancellation` is cancelled, which answers an empty list. The
/// existence checks run on the process's prober thread; while it is stuck the
/// stored list is answered unverified. See the module documentation for what
/// a check that never returns costs.
#[must_use]
pub fn list(store: &Path, cancellation: &CancellationToken) -> Vec<PathBuf> {
    list_with(store, &Prober::shared(), cancellation)
}

pub(crate) fn list_with(
    store: &Path,
    prober: &Prober,
    cancellation: &CancellationToken,
) -> Vec<PathBuf> {
    // An atomic replace never shows a half-written list, so reading needs no
    // lock.
    let paths = Recent::load_from(store).paths;
    if paths.is_empty() || cancellation.is_cancelled() {
        return Vec::new();
    }
    let Some(Submitted { answers, abandoned }) = prober.submit(paths.clone()) else {
        // Nothing can be checked now, and an empty list would hide the healthy
        // entries for as long as the mount stays dead.
        return paths;
    };
    let deadline = Instant::now() + PROBE_WAIT;
    let mut exists = vec![false; paths.len()];
    let mut answered = 0;
    let mut cancelled = false;
    while answered < paths.len() {
        if cancellation.is_cancelled() {
            cancelled = true;
            break;
        }
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        match answers.recv_timeout(left.min(POLL)) {
            Ok((index, found)) => {
                if let Some(slot) = exists.get_mut(index) {
                    *slot = found;
                }
                answered += 1;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // Whatever of this batch is still queued is no longer wanted: once the
    // prober recovers it must not check it entry by entry.
    abandoned.store(true, Ordering::Release);
    if cancelled {
        return Vec::new();
    }
    paths
        .into_iter()
        .zip(exists)
        .filter_map(|(path, found)| found.then_some(path))
        .collect()
}

/// Whether a path names a file that can be offered. A function pointer so a
/// test can stand in a check that hangs the way a dead mount does.
pub(crate) type Check = fn(&Path) -> bool;

/// One batch of existence checks and where its answers go: the index of each
/// path in the batch and whether it exists.
struct Probe {
    paths: Vec<PathBuf>,
    answers: mpsc::Sender<(usize, bool)>,
    /// Set by the asker when it stops waiting; checked before every check.
    abandoned: Arc<AtomicBool>,
}

/// What [`Prober::submit`] hands back: where the answers arrive, and the flag
/// that tells the prober the rest of the batch is no longer wanted.
struct Submitted {
    answers: mpsc::Receiver<(usize, bool)>,
    abandoned: Arc<AtomicBool>,
}

/// A detached thread that answers existence checks.
///
/// It holds nothing but its queue, its check and the time the check in
/// progress started, so nothing ever needs to join it: a check that never
/// returns strands this thread and no other. The process has one, shared by
/// every worker; a test builds its own with a check of its choosing.
#[derive(Clone)]
pub(crate) struct Prober {
    inner: Arc<ProberInner>,
}

struct ProberInner {
    check: Check,
    /// The queue into the thread, started on first use.
    requests: Mutex<Option<mpsc::Sender<Probe>>>,
    /// When the check in progress started, or `None` while idle.
    checking_since: Arc<Mutex<Option<Instant>>>,
}

impl Prober {
    /// The process's prober, checking with [`Path::is_file`].
    pub(crate) fn shared() -> Self {
        static SHARED: OnceLock<Prober> = OnceLock::new();
        SHARED.get_or_init(|| Self::new(Path::is_file)).clone()
    }

    pub(crate) fn new(check: Check) -> Self {
        Self {
            inner: Arc::new(ProberInner {
                check,
                requests: Mutex::new(None),
                checking_since: Arc::new(Mutex::new(None)),
            }),
        }
    }

    /// Queues `paths` and answers where their results will arrive, or `None`
    /// when the prober has been stuck on one check for [`STUCK_AFTER`] or its
    /// thread cannot be started.
    fn submit(&self, paths: Vec<PathBuf>) -> Option<Submitted> {
        let stuck =
            lock(&self.inner.checking_since).is_some_and(|since| since.elapsed() >= STUCK_AFTER);
        if stuck {
            return None;
        }
        let mut requests = lock(&self.inner.requests);
        if requests.is_none() {
            let (sender, receiver) = mpsc::channel::<Probe>();
            let check = self.inner.check;
            let checking_since = Arc::clone(&self.inner.checking_since);
            thread::Builder::new()
                .name("grafita-recent-probe".to_owned())
                .spawn(move || probe_loop(&receiver, check, &checking_since))
                .ok()?;
            *requests = Some(sender);
        }
        let (answers, receiver) = mpsc::channel();
        let abandoned = Arc::new(AtomicBool::new(false));
        let probe = Probe {
            paths,
            answers,
            abandoned: Arc::clone(&abandoned),
        };
        let queued = requests
            .as_ref()
            .is_some_and(|sender| sender.send(probe).is_ok());
        if !queued {
            // The thread is gone; the next list starts another.
            *requests = None;
            return None;
        }
        Some(Submitted {
            answers: receiver,
            abandoned,
        })
    }
}

fn probe_loop(
    requests: &mpsc::Receiver<Probe>,
    check: Check,
    checking_since: &Mutex<Option<Instant>>,
) {
    for probe in requests {
        for (index, path) in probe.paths.iter().enumerate() {
            // A batch queued behind a hung check and given up on while it
            // waited is skipped whole, not re-checked one entry at a time.
            if probe.abandoned.load(Ordering::Acquire) {
                break;
            }
            *lock(checking_since) = Some(Instant::now());
            let found = check(path);
            *lock(checking_since) = None;
            // The asker stopped waiting: the rest of its batch is not needed.
            if probe.answers.send((index, found)).is_err() {
                break;
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Recent {
    /// Reads the list kept in `store`, or an empty one.
    ///
    /// A missing or unreadable file is an empty history, never an error: there
    /// is nothing a user could do about it and nothing lost.
    #[must_use]
    pub fn load_from(store: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(store) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    #[must_use]
    fn parse(text: &str) -> Self {
        let mut paths = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            // Only absolute paths: a relative one would mean something
            // different depending on where Grafita was launched from.
            if line.is_empty() || !line.starts_with('/') {
                continue;
            }
            let candidate = PathBuf::from(line);
            if !paths.contains(&candidate) {
                paths.push(candidate);
            }
        }
        paths.truncate(LIMIT);
        Self { paths }
    }

    /// The remembered paths, newest first.
    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Moves `path` to the front, adding it if it is new.
    ///
    /// Opening a document you already had recently should reorder the list, not
    /// grow it.
    pub fn record(&mut self, path: &Path) {
        if !path.is_absolute() {
            return;
        }
        self.paths.retain(|kept| kept != path);
        self.paths.insert(0, path.to_path_buf());
        self.paths.truncate(LIMIT);
    }

    /// Forgets `path` — used when opening one turns out to fail.
    pub fn forget(&mut self, path: &Path) {
        self.paths.retain(|kept| kept != path);
    }

    /// Writes the list to `store`. Best-effort: a history that cannot be saved
    /// is not worth failing an edit over.
    pub fn store_to(&self, store: &Path) {
        let mut text = String::new();
        for entry in &self.paths {
            // A path that is not valid UTF-8 is skipped rather than mangled:
            // writing a lossy name would remember the wrong file.
            if let Some(line) = entry.to_str() {
                text.push_str(line);
                text.push('\n');
            }
        }
        let _ = atomic_file::replace(store, text.as_bytes());
    }
}

/// Where the list lives. `state` in spirit, but the suite's XDG helper offers
/// data, which is the closest thing that survives a cache clear.
///
/// Resolved from the environment alone; nothing is read.
#[must_use]
pub fn storage() -> Option<PathBuf> {
    Some(xdg::data_home()?.join("grafita").join("recent"))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::time::{Duration, Instant};

    use celestina_core::CancellationToken;

    use super::{change, list_with, Prober, Recent, RecentChange, LIMIT, PROBE_WAIT};
    use crate::testing::scratch_directory;

    #[test]
    fn recording_moves_a_repeat_to_the_front_rather_than_duplicating_it() {
        let mut recent = Recent::default();
        recent.record(&PathBuf::from("/uno"));
        recent.record(&PathBuf::from("/dos"));
        recent.record(&PathBuf::from("/uno"));

        assert_eq!(
            recent.paths(),
            [PathBuf::from("/uno"), PathBuf::from("/dos")]
        );
    }

    #[test]
    fn the_list_is_bounded_and_keeps_the_newest() {
        let mut recent = Recent::default();
        for index in 0..(LIMIT + 5) {
            recent.record(&PathBuf::from(format!("/documento-{index}")));
        }

        assert_eq!(recent.paths().len(), LIMIT);
        assert_eq!(
            recent.paths()[0],
            PathBuf::from(format!("/documento-{}", LIMIT + 4))
        );
    }

    #[test]
    fn a_relative_path_is_never_remembered() {
        let mut recent = Recent::default();
        recent.record(&PathBuf::from("relativo/nota.txt"));

        assert!(
            recent.paths().is_empty(),
            "it would mean a different file elsewhere"
        );
    }

    #[test]
    fn parsing_skips_blanks_relatives_and_repeats() {
        let recent = Recent::parse("/uno\n\n  \nrelativo\n/dos\n/uno\n");

        assert_eq!(
            recent.paths(),
            [PathBuf::from("/uno"), PathBuf::from("/dos")]
        );
    }

    #[test]
    fn forgetting_removes_exactly_one_entry() {
        let mut recent = Recent::default();
        recent.record(&PathBuf::from("/uno"));
        recent.record(&PathBuf::from("/dos"));
        recent.forget(&PathBuf::from("/uno"));

        assert_eq!(recent.paths(), [PathBuf::from("/dos")]);
    }

    /// A check that hangs the way a `stat` on a dead mount does. The prober
    /// thread it strands is detached and ends with the test process.
    fn hangs(_path: &Path) -> bool {
        std::thread::sleep(Duration::from_secs(30));
        true
    }

    #[test]
    fn a_change_records_and_forgets_without_asking_what_exists() {
        let root = scratch_directory("recent-change");
        let store = root.join("datos").join("recent");
        let gone = root.join("borrado.txt");
        let kept = root.join("sigue.txt");

        change(&store, &RecentChange::Record(gone.clone()));
        change(&store, &RecentChange::Record(kept.clone()));
        assert_eq!(
            Recent::load_from(&store).paths(),
            [kept.clone(), gone.clone()],
            "neither file exists, and both are remembered"
        );

        change(&store, &RecentChange::Forget(gone));
        assert_eq!(Recent::load_from(&store).paths(), [kept]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_list_offers_only_what_still_exists_newest_first() {
        let root = scratch_directory("recent-list");
        let store = root.join("recent");
        let older = root.join("antiguo.txt");
        let gone = root.join("borrado.txt");
        let newer = root.join("nuevo.txt");
        std::fs::write(&older, b"x").expect("write");
        std::fs::write(&newer, b"x").expect("write");
        let prober = Prober::new(Path::is_file);
        let token = CancellationToken::new();

        assert!(list_with(&store, &prober, &token).is_empty(), "no list yet");
        assert!(!store.exists(), "asking writes nothing");
        for path in [&older, &gone, &newer] {
            change(&store, &RecentChange::Record(path.clone()));
        }

        assert_eq!(list_with(&store, &prober, &token), vec![newer, older]);

        let _ = std::fs::remove_dir_all(root);
    }

    /// Hangs on the dead mount's entry only; everything else is checked for
    /// real.
    fn hangs_on_the_dead_mount(path: &Path) -> bool {
        if path.to_string_lossy().contains("montaje-muerto") {
            return hangs(path);
        }
        path.is_file()
    }

    #[test]
    fn a_hung_check_bounds_the_first_list_and_then_the_stored_list_is_offered_unverified() {
        let root = scratch_directory("recent-hung");
        let store = root.join("recent");
        let dead = root.join("en-un-montaje-muerto.txt");
        let alive = root.join("vivo.txt");
        std::fs::write(&alive, b"x").expect("write");
        change(&store, &RecentChange::Record(dead.clone()));
        change(&store, &RecentChange::Record(alive.clone()));
        let prober = Prober::new(hangs_on_the_dead_mount);
        let token = CancellationToken::new();

        let started = Instant::now();
        assert_eq!(list_with(&store, &prober, &token), vec![alive.clone()]);
        let first = started.elapsed();
        assert!(first < PROBE_WAIT + Duration::from_millis(500), "{first:?}");

        // Still stuck from the first call: the next one does not wait, and it
        // offers what is stored rather than nothing.
        let started = Instant::now();
        assert_eq!(list_with(&store, &prober, &token), vec![alive, dead]);
        let second = started.elapsed();
        assert!(second < Duration::from_millis(200), "{second:?}");

        let _ = std::fs::remove_dir_all(root);
    }

    /// How many slow checks [`recovers`] has started.
    static SLOW_CHECKS: AtomicUsize = AtomicUsize::new(0);

    /// A mount that answers, but only after 800 ms: stuck long enough to time
    /// a list out, not for ever.
    fn recovers(path: &Path) -> bool {
        if path.to_string_lossy().contains("montaje-lento") {
            SLOW_CHECKS.fetch_add(1, AtomicOrdering::SeqCst);
            std::thread::sleep(Duration::from_millis(800));
            return true;
        }
        path.is_file()
    }

    #[test]
    fn a_batch_given_up_on_is_not_checked_after_the_prober_recovers() {
        let root = scratch_directory("recent-abandoned");
        let store = root.join("recent");
        for name in ["montaje-lento-2", "montaje-lento-1"] {
            change(&store, &RecentChange::Record(root.join(name)));
        }
        let prober = Prober::new(recovers);
        let token = CancellationToken::new();

        // The first list starts the first slow check; a second one, asked for
        // before the prober counts as stuck, queues its batch behind it.
        let queued = {
            let (store, prober, token) = (store.clone(), prober.clone(), token.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                list_with(&store, &prober, &token)
            })
        };
        let first = list_with(&store, &prober, &token);
        let second = queued.join().expect("the queued list");
        assert_eq!(first.len(), 1, "one slow answer fits in the wait");
        assert!(second.is_empty(), "{second:?}");

        // The first batch's second check ends at 1.6 s. Without the flag the
        // prober would then start on the abandoned second batch.
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(SLOW_CHECKS.load(AtomicOrdering::SeqCst), 2);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_list_waiting_on_a_hung_check_holds_no_lock_and_stops_when_cancelled() {
        let root = scratch_directory("recent-cancel");
        let store = root.join("recent");
        change(
            &store,
            &RecentChange::Record(root.join("en-un-montaje-muerto.txt")),
        );
        let prober = Prober::new(hangs);
        let token = CancellationToken::new();

        let waiting = {
            let (store, prober, token) = (store.clone(), prober.clone(), token.clone());
            std::thread::spawn(move || list_with(&store, &prober, &token))
        };
        std::thread::sleep(Duration::from_millis(100));

        // Another tab records the document it just opened meanwhile.
        let started = Instant::now();
        change(&store, &RecentChange::Record(root.join("otra-pestana.txt")));
        let recorded = started.elapsed();
        assert!(recorded < Duration::from_millis(500), "{recorded:?}");

        let started = Instant::now();
        token.cancel();
        assert!(waiting.join().expect("the listing thread").is_empty());
        let stopped = started.elapsed();
        assert!(stopped < Duration::from_millis(300), "{stopped:?}");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_changes_in_one_process_lose_no_document() {
        let root = scratch_directory("recent-concurrent");
        let store = root.join("recent");
        let threads: Vec<_> = (0..8)
            .map(|index| {
                let store = store.clone();
                std::thread::spawn(move || {
                    let path = PathBuf::from(format!("/documento-{index}"));
                    change(&store, &RecentChange::Record(path));
                })
            })
            .collect();
        for thread in threads {
            thread.join().expect("an updating thread");
        }

        assert_eq!(Recent::load_from(&store).paths().len(), 8);

        let _ = std::fs::remove_dir_all(root);
    }
}
