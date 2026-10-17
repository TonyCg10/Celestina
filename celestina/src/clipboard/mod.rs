//! Clipboard history (CEL-2-G): see celestina/docs/evidence/2026-10-17-clipboard-privacy-design.md.
//!
//! This module is the wiring. [`ClipboardDuty`] holds the clipboard worker
//! when `duties.clipboard` is `on` and nothing when it is `off`, and
//! [`ClipboardDuty::apply`] moves from one to the other whenever the shell's
//! settings change, with no restart (design §4):
//!
//! - turning the duty on starts the worker, which loads the history file
//!   itself, on its own thread (the file may be tens of megabytes of JSON;
//!   the loop never reads it). A refused file is logged by the worker, with
//!   its reason and never any content, and rewritten or deleted at once;
//! - turning it off, or finding it off at start, raises the worker's stop
//!   flag and hands the rest to a reaper thread, so the loop never blocks on
//!   the disk or on a join: the reaper revokes the worker's
//!   [`store::Keeper`] and deletes the state file under the keeper's lock (a
//!   save in flight finishes first and is deleted with the file; a later one
//!   writes nothing), then joins the worker. Turning the duty on again waits
//!   for that reaper first, and starts **empty**: it does not load the file,
//!   which an off in this process has deleted. The worker never deletes on
//!   stop, because the shell's own exit keeps the file
//!   ([`ClipboardDuty::shutdown`]); only the duty being off removes it;
//! - an `off` that comes from the protected defaults (the shell settings file
//!   exists but could not be read, so `ShellSettings` runs on the defaults
//!   and will not overwrite the file) is not trusted to delete anything: the
//!   deletion is deferred until the first clean read of the settings says
//!   `off` ([`ClipboardDuty::deletion_deferred`]), and a clean `on` loads the
//!   file as usual.
//!
//! Starting a worker is behind the [`Spawner`] seam so the rules are tested
//! without a Wayland connection. The worker's news reaches the loop as
//! [`Message::Clipboard`], tagged with the generation of the worker that sent
//! it so a stopped worker's late events are ignored; [`ClipboardState`] keeps
//! the latest rows, previews only, and the snapshot number they belong to,
//! for the surfaces.

pub mod store;
pub mod thumb;
pub mod verbs;
pub mod worker;

use std::cell::RefCell;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use celestina_shell::settings::{Duty, Settings};
use celestina_wayland::Sender;

use crate::error_chain;
use crate::message::Message;
use crate::providers::Worker;

use self::store::{Keeper, Store};
use self::verbs::{ClipboardVerb, Refused};
use self::worker::{ClipboardEvent, ClipboardHandle, Persist, PersistError, Restored, Row};

/// How long stopping the worker waits for it. The worker looks at its stop
/// flag every poll, a read in progress included, so this is generous.
const JOIN_WAIT: Duration = Duration::from_secs(2);

/// What the clipboard worker reports to the loop, with the generation of the
/// worker that said it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardMessage {
    pub generation: u64,
    pub event: ClipboardEvent,
}

impl ClipboardEvent {
    /// Writes the event to the journal: counts and reasons, never content.
    /// A change of the history happens on every copy, so it is a debug line.
    pub fn log(&self) {
        match self {
            Self::Changed {
                rows,
                total,
                snapshot,
            } => {
                tracing::debug!(
                    rows = rows.len(),
                    total,
                    snapshot,
                    "clipboard: the history changed"
                );
            }
            Self::Unavailable(why) => tracing::warn!("clipboard: unavailable: {why}"),
            Self::Refused(why) => tracing::warn!("clipboard: refused: {why}"),
            Self::Paused(paused) => tracing::info!(paused, "clipboard: reading toggled"),
        }
    }
}

/// The latest the clipboard worker published, for the surfaces to read:
/// bounded previews and sizes, never an entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardState {
    rows: Vec<Row>,
    total: usize,
    snapshot: u64,
    paused: bool,
    unavailable: Option<String>,
}

impl ClipboardState {
    /// The newest rows, at most
    /// [`celestina_shell::clipboard::MAX_PUBLISHED_ROWS`].
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// How many entries the history holds in all.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    /// The snapshot number of [`ClipboardState::rows`]. A surface passes it
    /// with the row's index ([`ClipboardVerb::SelectShown`],
    /// [`ClipboardVerb::RemoveShown`]) so the worker refuses the verb, rather
    /// than acting on another entry, when the history changed in between.
    /// Zero before the worker published anything.
    #[must_use]
    pub fn snapshot(&self) -> u64 {
        self.snapshot
    }

    /// Whether reading selections is paused.
    #[must_use]
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Why the duty cannot work in this session, when it cannot.
    #[must_use]
    pub fn unavailable(&self) -> Option<&str> {
        self.unavailable.as_deref()
    }

    /// Takes in one event of the worker. Returns whether anything changed.
    pub fn apply(&mut self, event: ClipboardEvent) -> bool {
        let before = self.clone();
        match event {
            ClipboardEvent::Changed {
                rows,
                total,
                snapshot,
            } => {
                self.rows = rows;
                self.total = total;
                self.snapshot = snapshot;
            }
            ClipboardEvent::Unavailable(why) => self.unavailable = Some(why),
            ClipboardEvent::Paused(paused) => self.paused = paused,
            ClipboardEvent::Refused(_) => {}
        }
        *self != before
    }

    /// Forgets everything: the duty is off, so nothing is kept in memory.
    /// Returns whether there was anything.
    pub fn reset(&mut self) -> bool {
        let had = *self != Self::default();
        *self = Self::default();
        had
    }
}

/// The loop's side of a running clipboard worker. `seen` is the snapshot a
/// surface read the index from, `None` for the latest rows.
pub trait Control: Send {
    /// # Errors
    ///
    /// Why the worker did not take the command.
    fn select(&self, index: usize, seen: Option<u64>) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`Control::select`].
    fn remove(&self, index: usize, seen: Option<u64>) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`Control::select`].
    fn clear(&self) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`Control::select`].
    fn pause(&self) -> Result<(), String>;
    /// # Errors
    ///
    /// As [`Control::select`].
    fn resume(&self) -> Result<(), String>;
    /// Asks the worker to end; the caller joins it.
    fn stop(&self);
}

impl Control for ClipboardHandle {
    fn select(&self, index: usize, seen: Option<u64>) -> Result<(), String> {
        ClipboardHandle::select(self, index, seen).map_err(|e| e.to_string())
    }
    fn remove(&self, index: usize, seen: Option<u64>) -> Result<(), String> {
        ClipboardHandle::remove(self, index, seen).map_err(|e| e.to_string())
    }
    fn clear(&self) -> Result<(), String> {
        ClipboardHandle::clear(self).map_err(|e| e.to_string())
    }
    fn pause(&self) -> Result<(), String> {
        ClipboardHandle::pause(self).map_err(|e| e.to_string())
    }
    fn resume(&self) -> Result<(), String> {
        ClipboardHandle::resume(self).map_err(|e| e.to_string())
    }
    fn stop(&self) {
        ClipboardHandle::stop(self);
    }
}

/// How the worker comes to exist: the seam between the rules and Wayland.
pub trait Spawner {
    /// Starts a worker that persists through `keeper` (nowhere when there is
    /// none), loading its starting history through it when `load` (else it
    /// starts empty), and tagging what it publishes with `generation`.
    ///
    /// # Errors
    ///
    /// The failure to start the thread.
    fn spawn(
        &self,
        load: bool,
        keeper: Option<Keeper>,
        generation: u64,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn Control>, Worker)>;
}

/// [`Persist`] over the history file's keeper; nowhere when the shell has no
/// state directory.
struct StorePersist(Option<Keeper>);

impl Persist for StorePersist {
    fn load(&mut self, now: u64) -> Restored {
        let Some(keeper) = &self.0 else {
            tracing::warn!("clipboard: no state directory; the history is not kept");
            return Restored::default();
        };
        let loaded = keeper.load(now);
        if let Some(refusal) = &loaded.refusal {
            tracing::warn!(
                "clipboard: the history file was not loaded, and is replaced: {}",
                error_chain(refusal)
            );
        }
        if loaded.dropped > 0 {
            tracing::info!(
                dropped = loaded.dropped,
                "clipboard: the history file held entries that are not kept; it is rewritten"
            );
        }
        tracing::info!(entries = loaded.history.len(), "clipboard: history loaded");
        let stale = loaded.stale();
        Restored {
            history: loaded.history,
            stale,
        }
    }

    fn save(
        &mut self,
        history: &celestina_shell::clipboard::History,
        now: u64,
    ) -> Result<(), PersistError> {
        match &self.0 {
            Some(keeper) => keeper.save(history, now).map(|_| ()).map_err(Into::into),
            None => Ok(()),
        }
    }

    fn delete(&mut self) -> Result<(), PersistError> {
        match &self.0 {
            Some(keeper) => keeper.delete().map(|_| ()).map_err(Into::into),
            None => Ok(()),
        }
    }
}

/// The real spawner: Wayland.
struct Live {
    sender: Sender<Message>,
}

impl Spawner for Live {
    fn spawn(
        &self,
        load: bool,
        keeper: Option<Keeper>,
        generation: u64,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn Control>, Worker)> {
        let sender = self.sender.clone();
        let (handle, worker) = worker::spawn(
            load,
            StorePersist(keeper),
            worker::system_now,
            move |event| {
                sender
                    .send(Message::Clipboard(ClipboardMessage { generation, event }))
                    .is_ok()
            },
            shutdown,
        )?;
        Ok((Box::new(handle), worker))
    }
}

/// A running worker and the keeper it persists through.
struct Slot {
    control: Box<dyn Control>,
    worker: Worker,
    keeper: Option<Keeper>,
}

impl Slot {
    /// Stops the worker and waits for it on this thread, keeping the file:
    /// the shell's exit.
    fn stop(self) {
        let Slot {
            control, worker, ..
        } = self;
        control.stop();
        drop(control);
        if !worker.join_within(JOIN_WAIT) {
            tracing::warn!("clipboard: the worker did not stop in time");
        }
    }
}

/// What a reaper deletes: the stopped worker's keeper (revoked first), or,
/// with no worker, the file itself.
enum Forget {
    Keeper(Keeper),
    Store(Store),
    Nothing,
}

impl Forget {
    /// Deletes what is persisted, revoking the keeper first under its lock.
    fn run(self) {
        let (outcome, persisted) = match self {
            Self::Keeper(keeper) => (keeper.revoke_and_delete(), true),
            Self::Store(store) => (store.delete(), true),
            Self::Nothing => (Ok(()), false),
        };
        match outcome {
            Ok(()) if persisted => tracing::info!("clipboard: the history file was deleted"),
            Ok(()) => {}
            Err(error) => tracing::warn!(
                "clipboard: the history file was not deleted: {}",
                error_chain(&error)
            ),
        }
    }
}

/// The thread that forgets the history of a duty turned off: it deletes the
/// state file (revoking the stopped worker's keeper first), then joins the
/// worker within [`JOIN_WAIT`]. The energy duties' reaper only joins; this
/// one must also delete, and must delete even when no thread can be started,
/// so it keeps its body where the loop can still run it then.
#[derive(Default)]
struct Reaper(Option<std::thread::JoinHandle<()>>);

/// A reaper's work, taken by whichever thread runs it.
type Body = Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>;

impl Reaper {
    fn spawn(body: Box<dyn FnOnce() + Send>) -> Self {
        let body: Body = Arc::new(Mutex::new(Some(body)));
        let theirs = Arc::clone(&body);
        let take = |body: &Body| body.lock().unwrap_or_else(PoisonError::into_inner).take();
        let spawned = std::thread::Builder::new()
            .name("reap-clipboard".to_owned())
            .spawn(move || {
                if let Some(body) = take(&theirs) {
                    body();
                }
            });
        match spawned {
            Ok(handle) => Self(Some(handle)),
            Err(err) => {
                tracing::warn!(%err, "no thread to forget the clipboard history; it is done here");
                if let Some(body) = take(&body) {
                    body();
                }
                Self(None)
            }
        }
    }

    /// Waits for the pending reap, if any, so a duty turned off and on again
    /// never runs a new worker while the old file is still being deleted.
    /// Bounded: the reaper gives up on its worker after [`JOIN_WAIT`].
    fn wait(&mut self) {
        if let Some(handle) = self.0.take() {
            if handle.join().is_err() {
                tracing::error!("the clipboard reaper panicked");
            }
        }
    }

    /// Whether a reap is still running.
    fn pending(&self) -> bool {
        self.0.as_ref().is_some_and(|handle| !handle.is_finished())
    }
}

struct Inner {
    spawner: Box<dyn Spawner>,
    store: Option<Store>,
    running: Option<Slot>,
    /// Counts workers; a message of any other generation is stale.
    generation: u64,
    /// The reaper of the last `off`, which turning on waits for.
    reaper: Reaper,
    /// An `off` in this process deleted the history: the next worker starts
    /// empty instead of loading the file.
    forgotten: bool,
    /// An `off` of the protected defaults was not trusted to delete; the
    /// first clean `off` will.
    deferred: bool,
}

/// The clipboard duty of the shell: the worker the settings ask for.
pub struct ClipboardDuty {
    inner: RefCell<Inner>,
}

impl std::fmt::Debug for ClipboardDuty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardDuty")
            .field("running", &self.running())
            .finish()
    }
}

impl ClipboardDuty {
    /// The duty over the real system: the worker publishes into `sender` and
    /// keeps its file under `$XDG_STATE_HOME`. Nothing runs until
    /// [`ClipboardDuty::apply`] finds the duty on.
    #[must_use]
    pub fn new(sender: Sender<Message>) -> Self {
        Self::with_spawner(Box::new(Live { sender }), Store::from_env())
    }

    /// The duty over `spawner` and the history file `store` (none: nothing
    /// is read or written), for tests.
    #[must_use]
    pub fn with_spawner(spawner: Box<dyn Spawner>, store: Option<Store>) -> Self {
        Self {
            inner: RefCell::new(Inner {
                spawner,
                store,
                running: None,
                generation: 0,
                reaper: Reaper::default(),
                forgotten: false,
                deferred: false,
            }),
        }
    }

    /// Whether the worker exists.
    #[must_use]
    pub fn running(&self) -> bool {
        self.inner.borrow().running.is_some()
    }

    /// Whether a message of `generation` comes from the worker that runs
    /// now. A stopped worker's late events are not.
    #[must_use]
    pub fn accepts(&self, generation: u64) -> bool {
        let inner = self.inner.borrow();
        inner.running.is_some() && inner.generation == generation
    }

    /// Whether an `off` read from protected default settings left the
    /// history file in place, to be deleted by the first clean `off`.
    #[must_use]
    pub fn deletion_deferred(&self) -> bool {
        self.inner.borrow().deferred
    }

    /// Whether a reaper is still deleting the history of an `off`.
    #[must_use]
    pub fn forgetting(&self) -> bool {
        self.inner.borrow().reaper.pending()
    }

    /// Brings the worker to what `settings` ask for. Idempotent, so it is
    /// called at start and after every settings change.
    ///
    /// On: starts the worker when none runs (after any reaper of an earlier
    /// `off`). Off: stops the worker and hands the deletion of the state
    /// file, and the join, to a reaper, whether or not a worker ran, so a
    /// duty found off at start removes what an earlier run left.
    ///
    /// `protected` says `settings` are the defaults the shell fell back to
    /// because its settings file exists but could not be read
    /// (`ShellSettings::is_protected`). Such an `off` is not the person's
    /// word: it stops a running worker but deletes nothing, and the deletion
    /// waits for the first clean read that says `off`.
    pub fn apply(&self, settings: &Settings, protected: bool) {
        let mut inner = self.inner.borrow_mut();
        if settings.duties.clipboard == Duty::On {
            inner.deferred = false;
            inner.start();
        } else if protected {
            inner.defer();
        } else {
            inner.deferred = false;
            inner.stop_and_forget();
        }
    }

    /// Passes a verb to the worker.
    ///
    /// # Errors
    ///
    /// [`Refused::DutyOff`] while no worker runs, [`Refused::Worker`] when it
    /// did not take the command.
    pub fn command(&self, verb: ClipboardVerb) -> Result<(), Refused> {
        let inner = self.inner.borrow();
        let Some(slot) = inner.running.as_ref() else {
            return Err(Refused::DutyOff);
        };
        let control = &slot.control;
        match verb {
            ClipboardVerb::Select(index) => control.select(index, None),
            ClipboardVerb::SelectShown { index, snapshot } => control.select(index, Some(snapshot)),
            ClipboardVerb::Remove(index) => control.remove(index, None),
            ClipboardVerb::RemoveShown { index, snapshot } => control.remove(index, Some(snapshot)),
            ClipboardVerb::Clear => control.clear(),
            ClipboardVerb::Pause => control.pause(),
            ClipboardVerb::Resume => control.resume(),
        }
        .map_err(Refused::Worker)
    }

    /// The shell is ending: waits for a pending reaper, stops and joins the
    /// worker and keeps the file. Also what dropping the duty does.
    pub fn shutdown(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.generation += 1;
        inner.reaper.wait();
        if let Some(slot) = inner.running.take() {
            slot.stop();
        }
    }
}

impl Drop for ClipboardDuty {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Inner {
    fn start(&mut self) {
        if self.running.is_some() {
            return;
        }
        // The file of an earlier `off` is gone before anything reads it.
        self.reaper.wait();
        let load = !self.forgotten;
        let keeper = self.store.clone().map(Keeper::new);
        self.generation += 1;
        let shutdown = Arc::new(AtomicBool::new(false));
        match self
            .spawner
            .spawn(load, keeper.clone(), self.generation, &shutdown)
        {
            Ok((control, worker)) => {
                self.running = Some(Slot {
                    control,
                    worker,
                    keeper,
                });
                tracing::info!(load, "clipboard duty on");
            }
            Err(err) => tracing::warn!(%err, "clipboard: the worker did not start"),
        }
    }

    /// An `off` of the protected defaults: stop what runs, delete nothing.
    fn defer(&mut self) {
        self.generation += 1;
        if let Some(slot) = self.running.take() {
            slot.control.stop();
            let Slot { worker, .. } = slot;
            self.reaper.wait();
            self.reaper = Reaper::spawn(Box::new(move || {
                worker.join_within(JOIN_WAIT);
            }));
        }
        if !self.deferred {
            tracing::warn!(
                "clipboard: the shell settings could not be read; the history file is kept \
                 until a clean read says the duty is off"
            );
        }
        self.deferred = true;
    }

    fn stop_and_forget(&mut self) {
        // Late events of the old worker are stale from here on.
        self.generation += 1;
        let slot = self.running.take();
        let was_running = slot.is_some();
        // Raised here, at once: the worker persists nothing from now on even
        // before the reaper revokes its keeper.
        let (worker, forget) = match slot {
            Some(Slot {
                control,
                worker,
                keeper,
            }) => {
                control.stop();
                drop(control);
                let forget = match (keeper, &self.store) {
                    (Some(keeper), _) => Forget::Keeper(keeper),
                    (None, Some(store)) => Forget::Store(store.clone()),
                    (None, None) => Forget::Nothing,
                };
                (Some(worker), forget)
            }
            None => (
                None,
                self.store.clone().map_or(Forget::Nothing, Forget::Store),
            ),
        };
        self.forgotten = true;
        // One reaper at a time: a second `off` waits for the first.
        self.reaper.wait();
        self.reaper = Reaper::spawn(Box::new(move || {
            forget.run();
            if let Some(worker) = worker {
                if !worker.join_within(JOIN_WAIT) {
                    tracing::warn!("clipboard: the worker did not stop in time");
                }
            }
        }));
        if was_running {
            tracing::info!("clipboard duty off: memory emptied, the history file is being deleted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::worker::Preview;
    use celestina_shell::clipboard::{Content, History};
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;

    const SECRET: &str = "hunter2-correct-horse";

    type Log = Arc<Mutex<Vec<String>>>;

    fn said(log: &Log) -> Vec<String> {
        log.lock().expect("log").clone()
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "celestina-clipboard-duty-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a test directory");
            Self(dir)
        }

        fn store(&self) -> Store {
            Store::in_state_home(&self.0)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn history_of(values: &[&str]) -> History {
        let mut history = History::new();
        for value in values {
            let content = Content::text((*value).to_owned()).expect("recordable test text");
            history.record(content, worker::system_now());
        }
        history
    }

    struct FakeControl {
        log: Log,
        refuse: bool,
        stop: Arc<AtomicBool>,
    }

    impl FakeControl {
        fn say(&self, line: String) -> Result<(), String> {
            if self.refuse {
                return Err("the clipboard worker has stopped".to_owned());
            }
            self.log.lock().expect("log").push(line);
            Ok(())
        }
    }

    impl Control for FakeControl {
        fn select(&self, index: usize, seen: Option<u64>) -> Result<(), String> {
            self.say(format!("select {index} {seen:?}"))
        }
        fn remove(&self, index: usize, seen: Option<u64>) -> Result<(), String> {
            self.say(format!("remove {index} {seen:?}"))
        }
        fn clear(&self) -> Result<(), String> {
            self.say("clear".to_owned())
        }
        fn pause(&self) -> Result<(), String> {
            self.say("pause".to_owned())
        }
        fn resume(&self) -> Result<(), String> {
            self.say("resume".to_owned())
        }
        fn stop(&self) {
            self.log.lock().expect("log").push("stop".to_owned());
            self.stop.store(true, Ordering::Release);
        }
    }

    /// How the fake worker behaves after its stop flag is raised.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Lingering {
        /// It ends at once.
        No,
        /// It keeps saving for a while, as a worker caught in a read or a
        /// decode that has not looked at its flag yet; only the keeper's
        /// revocation keeps it from the file.
        Saving,
    }

    struct Fake {
        log: Log,
        refuse: bool,
        fail_to_start: bool,
        lingering: Lingering,
    }

    impl Fake {
        fn new() -> (Self, Log) {
            let log = Log::default();
            (
                Self {
                    log: Arc::clone(&log),
                    refuse: false,
                    fail_to_start: false,
                    lingering: Lingering::No,
                },
                log,
            )
        }
    }

    impl Spawner for Fake {
        fn spawn(
            &self,
            load: bool,
            keeper: Option<Keeper>,
            generation: u64,
            shutdown: &Arc<AtomicBool>,
        ) -> io::Result<(Box<dyn Control>, Worker)> {
            if self.fail_to_start {
                return Err(io::Error::other("no thread"));
            }
            self.log.lock().expect("log").push(format!(
                "spawn load={load} generation={generation} store={}",
                keeper.is_some()
            ));
            let stop = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&stop);
            let log = Arc::clone(&self.log);
            let lingering = self.lingering;
            let worker = Worker::spawn("fake-clipboard", shutdown, move || {
                // Like the real worker: it loads on its own thread, writes
                // its history while it lives and does not delete on stop.
                let mut history = History::new();
                if let Some(keeper) = &keeper {
                    if load {
                        history = keeper.load(worker::system_now()).history;
                    }
                    log.lock()
                        .expect("log")
                        .push(format!("worker entries={}", history.len()));
                    if !history.is_empty() {
                        let _ = keeper.save(&history, worker::system_now());
                    }
                }
                while !flag.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                if lingering == Lingering::Saving {
                    for _ in 0..50 {
                        if let Some(keeper) = &keeper {
                            let _ = keeper.save(&history_of(&["late"]), worker::system_now());
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
                log.lock().expect("log").push("worker stopped".to_owned());
            })?;
            Ok((
                Box::new(FakeControl {
                    log: Arc::clone(&self.log),
                    refuse: self.refuse,
                    stop,
                }),
                worker,
            ))
        }
    }

    fn settings(on: bool) -> Settings {
        let mut settings = Settings::default();
        settings.duties.clipboard = if on { Duty::On } else { Duty::Off };
        settings
    }

    fn until(what: &str, mut ready: impl FnMut() -> bool) {
        for _ in 0..1000 {
            if ready() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("timed out waiting for {what}");
    }

    /// Waits until the fake worker has said `line`.
    fn heard(log: &Log, line: &str) {
        until(line, || said(log).iter().any(|said| said == line));
    }

    /// Waits for the reaper of the last `off`.
    fn reaped(duty: &ClipboardDuty) {
        until("the reaper", || !duty.forgetting());
    }

    fn seeded(name: &str, values: &[&str]) -> (Scratch, Store) {
        let scratch = Scratch::new(name);
        let store = scratch.store();
        let published = store.save(&history_of(values), worker::system_now());
        assert!(published.is_ok(), "a seeded history file");
        (scratch, store)
    }

    #[test]
    fn nothing_runs_while_the_duty_is_off() {
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), None);
        duty.apply(&settings(false), false);
        assert!(!duty.running());
        reaped(&duty);
        assert!(said(&log).is_empty());
    }

    #[test]
    fn on_starts_the_worker_which_loads_the_persisted_history_once() {
        let (_scratch, store) = seeded("on", &["one", "two"]);
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store));
        duty.apply(&settings(true), false);
        duty.apply(&settings(true), false);
        assert!(duty.running());
        heard(&log, "worker entries=2");
        assert_eq!(
            said(&log),
            [
                "spawn load=true generation=1 store=true",
                "worker entries=2"
            ]
        );
        assert!(duty.accepts(1));
        assert!(!duty.accepts(0));
    }

    #[test]
    fn off_stops_and_deletes_the_file_off_the_loop_thread() {
        let (_scratch, store) = seeded("off", &["one"]);
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store.clone()));
        duty.apply(&settings(true), false);
        heard(&log, "worker entries=1");
        duty.apply(&settings(false), false);
        assert!(!duty.running());
        assert!(!duty.accepts(1), "late events of the old worker are stale");
        reaped(&duty);
        assert!(!store.path().exists(), "the file is deleted");
        let said = said(&log);
        assert_eq!(said[2..], ["stop", "worker stopped"], "{said:?}");
    }

    #[test]
    fn a_worker_still_saving_after_off_never_recreates_the_file() {
        let (_scratch, store) = seeded("linger", &["one"]);
        let (mut fake, log) = Fake::new();
        fake.lingering = Lingering::Saving;
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store.clone()));
        duty.apply(&settings(true), false);
        heard(&log, "worker entries=1");
        duty.apply(&settings(false), false);
        // The worker goes on trying to save for ~100 ms after its stop.
        heard(&log, "worker stopped");
        reaped(&duty);
        assert!(!store.path().exists(), "the revoked keeper wrote nothing");
    }

    #[test]
    fn on_after_off_waits_for_the_reaper_and_starts_empty() {
        let (_scratch, store) = seeded("again", &["one", "two"]);
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store.clone()));
        duty.apply(&settings(true), false);
        heard(&log, "worker entries=2");
        duty.apply(&settings(false), false);
        duty.apply(&settings(true), false);
        // `on` joined the reaper: the old worker was gone before the new one.
        let lines = said(&log);
        let stopped = lines.iter().position(|l| l == "worker stopped");
        let respawned = lines.iter().position(|l| l.starts_with("spawn load=false"));
        assert!(stopped < respawned && stopped.is_some(), "{lines:?}");
        heard(&log, "worker entries=0");
        assert!(lines.contains(&"spawn load=false generation=3 store=true".to_owned()));
        assert!(duty.accepts(3));
        assert!(!duty.accepts(1));
        assert!(!store.path().exists());
        duty.shutdown();
    }

    #[test]
    fn a_duty_found_off_at_start_deletes_what_an_earlier_run_left() {
        let (_scratch, store) = seeded("start-off", &["left behind"]);
        let stray = store.path().with_file_name(".clipboard.json.77-1.tmp");
        std::fs::write(&stray, b"plaintext").expect("stray");
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store.clone()));
        duty.apply(&settings(false), false);
        reaped(&duty);
        assert!(!store.path().exists());
        assert!(!stray.exists(), "its temporaries too");
        assert!(said(&log).is_empty(), "no worker was needed");
        duty.apply(&settings(false), false);
        reaped(&duty);
    }

    #[test]
    fn an_off_of_protected_defaults_deletes_nothing_until_a_clean_off() {
        let (_scratch, store) = seeded("protected", &["kept for now"]);
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store.clone()));
        duty.apply(&settings(false), true);
        reaped(&duty);
        assert!(
            store.path().exists(),
            "unreadable settings are not the person's off"
        );
        assert!(duty.deletion_deferred());
        assert!(said(&log).is_empty());
        duty.apply(&settings(false), false);
        reaped(&duty);
        assert!(!store.path().exists(), "the first clean off deletes");
        assert!(!duty.deletion_deferred());
    }

    #[test]
    fn a_clean_on_after_protected_defaults_loads_the_file() {
        let (_scratch, store) = seeded("protected-on", &["one"]);
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store.clone()));
        duty.apply(&settings(false), true);
        duty.apply(&settings(true), false);
        assert!(!duty.deletion_deferred());
        heard(&log, "worker entries=1");
        assert_eq!(said(&log)[0], "spawn load=true generation=2 store=true");
        duty.shutdown();
    }

    #[test]
    fn the_shells_own_exit_keeps_the_file() {
        let (_scratch, store) = seeded("exit", &["one"]);
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), Some(store.clone()));
        duty.apply(&settings(true), false);
        heard(&log, "worker entries=1");
        duty.shutdown();
        assert!(!duty.running());
        assert!(store.path().exists(), "only the duty being off deletes");
        assert!(said(&log).contains(&"worker stopped".to_owned()));
    }

    #[test]
    fn a_worker_that_does_not_start_leaves_the_duty_not_running() {
        let (mut fake, _log) = Fake::new();
        fake.fail_to_start = true;
        let duty = ClipboardDuty::with_spawner(Box::new(fake), None);
        duty.apply(&settings(true), false);
        assert!(!duty.running());
        assert_eq!(
            duty.command(ClipboardVerb::Clear),
            Err(Refused::DutyOff),
            "refused while no worker runs; the next apply starts one again"
        );
    }

    #[test]
    fn verbs_reach_the_worker_only_while_the_duty_is_on() {
        let (fake, log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), None);
        assert_eq!(
            duty.command(ClipboardVerb::Select(0)),
            Err(Refused::DutyOff)
        );
        duty.apply(&settings(true), false);
        for verb in [
            ClipboardVerb::Select(3),
            ClipboardVerb::SelectShown {
                index: 3,
                snapshot: 9,
            },
            ClipboardVerb::Remove(4),
            ClipboardVerb::RemoveShown {
                index: 4,
                snapshot: 9,
            },
            ClipboardVerb::Clear,
            ClipboardVerb::Pause,
            ClipboardVerb::Resume,
        ] {
            assert_eq!(duty.command(verb), Ok(()));
        }
        assert_eq!(
            said(&log)[1..],
            [
                "select 3 None",
                "select 3 Some(9)",
                "remove 4 None",
                "remove 4 Some(9)",
                "clear",
                "pause",
                "resume"
            ]
        );
        duty.apply(&settings(false), false);
        assert_eq!(duty.command(ClipboardVerb::Clear), Err(Refused::DutyOff));
    }

    #[test]
    fn a_worker_that_refuses_a_verb_is_reported() {
        let (mut fake, _log) = Fake::new();
        fake.refuse = true;
        let duty = ClipboardDuty::with_spawner(Box::new(fake), None);
        duty.apply(&settings(true), false);
        assert!(matches!(
            duty.command(ClipboardVerb::Pause),
            Err(Refused::Worker(_))
        ));
    }

    #[test]
    fn the_verb_runner_answers_success_or_failure_only() {
        use crate::command::Verb;
        let (fake, _log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), None);
        let clear = Verb::Clipboard(ClipboardVerb::Clear);
        let off = verbs::run(&clear, &duty).expect("a clipboard verb");
        assert_eq!(off, verbs::Outcome::DutyOff(ClipboardVerb::Clear));
        assert!(!off.succeeded());
        duty.apply(&settings(true), false);
        let on = verbs::run(&clear, &duty).expect("a clipboard verb");
        assert!(on.succeeded());
        assert_eq!(verbs::run(&Verb::Quit, &duty), None);
    }

    #[test]
    fn the_state_follows_the_events_and_forgets_on_reset() {
        let mut state = ClipboardState::default();
        assert_eq!(state.snapshot(), 0);
        let rows = vec![Row {
            index: 0,
            preview: Preview::Text(SECRET.to_owned()),
        }];
        assert!(state.apply(ClipboardEvent::Changed {
            rows,
            total: 1,
            snapshot: 4
        }));
        assert!(state.apply(ClipboardEvent::Paused(true)));
        assert!(!state.apply(ClipboardEvent::Refused("no entry".to_owned())));
        assert_eq!(
            (
                state.rows().len(),
                state.total(),
                state.paused(),
                state.snapshot()
            ),
            (1, 1, true, 4)
        );
        assert!(state.reset());
        assert_eq!(state, ClipboardState::default());
        assert!(!state.reset());
    }

    #[test]
    fn no_debug_output_carries_content() {
        let rows = vec![Row {
            index: 0,
            preview: Preview::Text(SECRET.to_owned()),
        }];
        let event = ClipboardEvent::Changed {
            rows,
            total: 1,
            snapshot: 1,
        };
        let mut state = ClipboardState::default();
        state.apply(event.clone());
        let message = ClipboardMessage {
            generation: 1,
            event,
        };
        let (fake, _log) = Fake::new();
        let duty = ClipboardDuty::with_spawner(Box::new(fake), None);
        for text in [
            format!("{message:?}"),
            format!("{:?}", Message::Clipboard(message)),
            format!("{state:?}"),
            format!("{duty:?}"),
        ] {
            assert!(!text.contains(SECRET), "{text}");
        }
    }
}
