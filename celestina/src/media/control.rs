//! The media duty's wiring (CEL-2-D): the worker exists exactly while
//! `duties.media` is `on`, and what it published is kept here for the
//! Islands to draw.
//!
//! [`MediaControl::apply`] moves from one state to the other whenever the
//! shell's settings change, with no restart and no new connection while the
//! duty stays on. Starting the worker is behind the [`Spawner`] seam so the
//! rules are tested without a session bus. The worker gets a shutdown flag
//! of its own: joining it must not stop the shell. Turning the duty off
//! hands the worker to the one [`Reaper`] the providers share, so the loop
//! never blocks on it; the next start waits for that reap.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use celestina_shell::settings::{Duty, Settings};
use celestina_wayland::Sender;

use super::cover::Cover;
use super::covers::Covers;
use super::mpris::{self, MediaConfig};
use super::{MediaCommand, MediaEvent, MediaHandle, PlayerView};
use crate::message::Message;
use crate::providers::{Reaper, Worker};

/// How long stopping the worker waits for it.
const JOIN_WAIT: Duration = Duration::from_secs(2);

/// How the worker comes to exist: the seam between the rules and the bus.
pub trait Spawner {
    /// # Errors
    ///
    /// The failure to start the thread.
    fn spawn(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(MediaHandle, Worker)>;
}

/// The real spawner: the session bus, covers decoded on the worker.
struct Live {
    sender: Sender<Message>,
}

impl Spawner for Live {
    fn spawn(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(MediaHandle, Worker)> {
        let sender = self.sender.clone();
        mpris::spawn(
            MediaConfig::default(),
            Box::new(Covers::new()),
            move |event| sender.send(Message::Media(event)).is_ok(),
            shutdown,
        )
    }
}

/// A running worker.
struct Running {
    handle: MediaHandle,
    worker: Worker,
}

/// What the Islands draw: the players, the shown one and the covers.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// Whether the worker runs (the duty is on and it started).
    pub on: bool,
    pub players: Vec<PlayerView>,
    pub active: Option<String>,
    /// Whether the session bus cannot be followed right now: the players
    /// are the last known ones and must not be shown as live.
    pub unavailable: bool,
    /// `(bus name, art url, cover)` of every player whose art decoded.
    pub covers: Vec<(String, String, Arc<Cover>)>,
}

struct Inner {
    spawner: Box<dyn Spawner>,
    running: Option<Running>,
    players: Vec<PlayerView>,
    active: Option<String>,
    /// The decoded cover and the url it answers, by bus name.
    covers: BTreeMap<String, (String, Arc<Cover>)>,
    /// The worker said it cannot follow the bus; the next players clear it.
    unavailable: bool,
    unavailable_logged: bool,
    /// The join of the worker the duty was turned off on, still pending.
    reaper: Reaper,
    refusal_logged: bool,
    spawn_failure_logged: bool,
}

impl Inner {
    fn forget(&mut self) {
        self.players.clear();
        self.active = None;
        self.covers.clear();
        self.unavailable = false;
    }
}

/// The media duty: its worker, and the state it published.
pub struct MediaControl {
    inner: RefCell<Inner>,
}

impl std::fmt::Debug for MediaControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaControl")
            .field("running", &self.running())
            .finish()
    }
}

impl MediaControl {
    /// The control over the real session bus: the worker publishes into
    /// `sender`. Nothing runs until [`MediaControl::apply`] finds the duty on.
    #[must_use]
    pub fn new(sender: Sender<Message>) -> Self {
        Self::with_spawner(Box::new(Live { sender }))
    }

    /// The control over `spawner`, for tests.
    #[must_use]
    pub fn with_spawner(spawner: Box<dyn Spawner>) -> Self {
        Self {
            inner: RefCell::new(Inner {
                spawner,
                running: None,
                players: Vec::new(),
                active: None,
                covers: BTreeMap::new(),
                unavailable: false,
                unavailable_logged: false,
                reaper: Reaper::default(),
                refusal_logged: false,
                spawn_failure_logged: false,
            }),
        }
    }

    /// Whether the worker exists.
    #[must_use]
    pub fn running(&self) -> bool {
        self.inner.borrow().running.is_some()
    }

    /// Brings the worker to what `settings` ask for: on has one (started
    /// once, never again while it runs), off has none (joined here, bounded)
    /// and forgets the players and covers. Idempotent. A worker that cannot
    /// start degrades with a warning, once. Returns whether what the Islands
    /// draw changed.
    pub fn apply(&self, settings: &Settings) -> bool {
        let mut inner = self.inner.borrow_mut();
        if settings.duties.media == Duty::On {
            if inner.running.is_some() {
                return false;
            }
            // A duty switched off and on again must not hold two bus
            // connections while the old worker is still stopping.
            inner.reaper.wait();
            let shutdown = Arc::new(AtomicBool::new(false));
            match inner.spawner.spawn(&shutdown) {
                Ok((handle, worker)) => {
                    inner.running = Some(Running { handle, worker });
                    inner.spawn_failure_logged = false;
                    true
                }
                Err(err) => {
                    if !inner.spawn_failure_logged {
                        inner.spawn_failure_logged = true;
                        tracing::warn!(%err, "no media worker; the media piece stays empty");
                    }
                    false
                }
            }
        } else {
            inner.spawn_failure_logged = false;
            let Some(running) = inner.running.take() else {
                return false;
            };
            inner.forget();
            inner.unavailable_logged = false;
            inner.refusal_logged = false;
            let Running { handle, worker } = running;
            inner.reaper = Reaper::spawn("media", move || {
                drop(handle);
                worker.join_within(JOIN_WAIT);
            });
            true
        }
    }

    /// Stops the worker at the shell's exit, here and bounded, after any
    /// reap still pending.
    pub fn stop(&self) {
        let (running, reaper) = {
            let mut inner = self.inner.borrow_mut();
            (inner.running.take(), std::mem::take(&mut inner.reaper))
        };
        reaper.wait();
        if let Some(Running { handle, worker }) = running {
            drop(handle);
            worker.join_within(JOIN_WAIT);
        }
    }

    /// Takes the players the worker published. Returns whether they were
    /// taken: nothing is, once the duty is off (an event queued before the
    /// worker stopped).
    pub fn players(&self, players: Vec<PlayerView>, active: Option<String>) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.running.is_none() {
            return false;
        }
        inner.covers.retain(|bus, (url, _)| {
            players
                .iter()
                .any(|p| p.bus_name == *bus && p.art_url == *url)
        });
        inner.players = players;
        inner.active = active;
        inner.unavailable = false;
        inner.unavailable_logged = false;
        true
    }

    /// Takes a cover the worker published (`None`: that player has none).
    /// Returns whether it was taken, as [`MediaControl::players`] does.
    pub fn cover(&self, bus_name: &str, art_url: &str, cover: Option<Arc<Cover>>) -> bool {
        let mut inner = self.inner.borrow_mut();
        if inner.running.is_none() {
            return false;
        }
        match cover {
            Some(cover) if !art_url.is_empty() => {
                inner
                    .covers
                    .insert(bus_name.to_owned(), (art_url.to_owned(), cover));
            }
            _ => {
                inner.covers.remove(bus_name);
            }
        }
        true
    }

    /// The bus cannot be followed: logged once until players come back; the
    /// last players and covers stay but are marked unavailable for the
    /// views, until the next players. Returns whether that mark changed
    /// (nothing is marked once the duty is off).
    pub fn unavailable(&self, why: &str) -> bool {
        let mut inner = self.inner.borrow_mut();
        if !inner.unavailable_logged {
            inner.unavailable_logged = true;
            tracing::warn!(%why, "media: the session bus is not followed");
        }
        if inner.running.is_none() || inner.unavailable {
            return false;
        }
        inner.unavailable = true;
        true
    }

    /// Queues `command` for the shown player. A refusal is logged once
    /// until a command is taken again; with no worker the command is
    /// dropped.
    pub fn command(&self, command: MediaCommand) {
        let mut inner = self.inner.borrow_mut();
        let Some(running) = inner.running.as_ref() else {
            tracing::debug!(?command, "no media worker: the command is dropped");
            return;
        };
        match running.handle.command(command) {
            Ok(()) => inner.refusal_logged = false,
            Err(err) => {
                if !inner.refusal_logged {
                    inner.refusal_logged = true;
                    tracing::warn!(%err, "the media command was not queued");
                }
            }
        }
    }

    /// What the Islands draw now.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let inner = self.inner.borrow();
        Snapshot {
            on: inner.running.is_some(),
            players: inner.players.clone(),
            active: inner.active.clone(),
            unavailable: inner.unavailable,
            covers: inner
                .covers
                .iter()
                .map(|(bus, (url, cover))| (bus.clone(), url.clone(), Arc::clone(cover)))
                .collect(),
        }
    }
}

impl Drop for MediaControl {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Routes `event` into `control`; the answer says what the Islands must do.
#[must_use]
pub fn route(control: &MediaControl, event: MediaEvent) -> Routed {
    match event {
        MediaEvent::Players { players, active } => {
            if control.players(players, active) {
                Routed::Sync
            } else {
                Routed::Nothing
            }
        }
        MediaEvent::Cover {
            bus_name,
            art_url,
            cover,
        } => {
            if control.cover(&bus_name, &art_url, cover.clone()) {
                Routed::Cover {
                    bus_name,
                    art_url,
                    cover,
                }
            } else {
                Routed::Nothing
            }
        }
        MediaEvent::Unavailable(why) => {
            if control.unavailable(&why) {
                Routed::Sync
            } else {
                Routed::Nothing
            }
        }
    }
}

/// What the loop does with a routed event.
#[derive(Debug)]
pub enum Routed {
    Nothing,
    /// Give every Island the whole snapshot.
    Sync,
    /// Give every Island this one cover.
    Cover {
        bus_name: String,
        art_url: String,
        cover: Option<Arc<Cover>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{sync_channel, Receiver};
    use std::sync::Mutex;

    struct Fake {
        spawned: Rc<Cell<u32>>,
        fail: Rc<Cell<bool>>,
        queue: Rc<Mutex<Option<Receiver<crate::media::Queued>>>>,
        flags: Rc<Mutex<Vec<Arc<AtomicBool>>>>,
        stopped: Arc<AtomicBool>,
        /// While true the worker outlives the shutdown flag (a body stuck in
        /// a call), so the join takes as long as the test wants.
        hold: Arc<AtomicBool>,
    }

    impl Spawner for Fake {
        fn spawn(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(MediaHandle, Worker)> {
            if self.fail.get() {
                return Err(io::Error::other("no threads"));
            }
            self.spawned.set(self.spawned.get() + 1);
            let (tx, rx) = sync_channel(2);
            if let Ok(mut queue) = self.queue.lock() {
                *queue = Some(rx);
            }
            if let Ok(mut flags) = self.flags.lock() {
                flags.push(Arc::clone(shutdown));
            }
            let flag = Arc::clone(shutdown);
            let stopped = Arc::clone(&self.stopped);
            let hold = Arc::clone(&self.hold);
            let worker = Worker::spawn("media-test", shutdown, move || {
                while !flag.load(Ordering::Acquire) || hold.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                stopped.store(true, Ordering::Release);
            })?;
            Ok((MediaHandle::new(tx), worker))
        }
    }

    struct Rig {
        control: MediaControl,
        spawned: Rc<Cell<u32>>,
        fail: Rc<Cell<bool>>,
        queue: Rc<Mutex<Option<Receiver<crate::media::Queued>>>>,
        flags: Rc<Mutex<Vec<Arc<AtomicBool>>>>,
        stopped: Arc<AtomicBool>,
        hold: Arc<AtomicBool>,
    }

    fn rig() -> Rig {
        let spawned = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(false));
        let queue = Rc::new(Mutex::new(None));
        let flags = Rc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let hold = Arc::new(AtomicBool::new(false));
        let control = MediaControl::with_spawner(Box::new(Fake {
            spawned: Rc::clone(&spawned),
            fail: Rc::clone(&fail),
            queue: Rc::clone(&queue),
            flags: Rc::clone(&flags),
            stopped: Arc::clone(&stopped),
            hold: Arc::clone(&hold),
        }));
        Rig {
            control,
            spawned,
            fail,
            queue,
            flags,
            stopped,
            hold,
        }
    }

    fn settings(on: bool) -> Settings {
        let mut s = Settings::default();
        s.duties.media = if on { Duty::On } else { Duty::Off };
        s
    }

    fn player(bus: &str, art: &str) -> PlayerView {
        PlayerView {
            bus_name: bus.to_owned(),
            identity: "p".to_owned(),
            playing: true,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_url: art.to_owned(),
            length_ms: -1,
            position_ms: -1,
            pinned: false,
        }
    }

    fn cover() -> Arc<Cover> {
        Arc::new(Cover {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        })
    }

    #[test]
    fn off_by_default_starts_nothing() {
        let rig = rig();
        assert!(!rig.control.apply(&Settings::default()));
        assert!(!rig.control.running());
        assert_eq!(rig.spawned.get(), 0);
    }

    #[test]
    fn on_makes_a_handle_and_double_on_is_idempotent() {
        let rig = rig();
        assert!(rig.control.apply(&settings(true)));
        assert!(rig.control.running());
        assert!(!rig.control.apply(&settings(true)));
        assert!(!rig.control.apply(&settings(true)));
        assert_eq!(rig.spawned.get(), 1, "no reconnect on repeated writes");
        rig.control.command(MediaCommand::PlayPause);
        let queue = rig.queue.lock().expect("queue");
        assert_eq!(
            queue
                .as_ref()
                .expect("rx")
                .try_recv()
                .ok()
                .map(|q| q.command),
            Some(MediaCommand::PlayPause)
        );
    }

    #[test]
    fn off_reaps_the_worker_with_its_own_flag_and_forgets_everything() {
        let rig = rig();
        rig.control.apply(&settings(true));
        assert!(rig
            .control
            .players(vec![player("a", "file:///x")], Some("a".into())));
        assert!(rig.control.cover("a", "file:///x", Some(cover())));
        assert!(rig.control.apply(&settings(false)));
        assert!(!rig.control.running());
        // The reap runs off the loop; the next start waits for it.
        let snap = rig.control.snapshot();
        assert!(!snap.on && snap.players.is_empty() && snap.covers.is_empty());
        assert!(snap.active.is_none());
        assert!(!rig.control.apply(&settings(false)));
        // Late events of the stopped worker change nothing.
        assert!(!rig.control.players(vec![player("a", "")], None));
        assert!(!rig.control.cover("a", "u", Some(cover())));
        // The flag was the worker's own; a second start gets a fresh one.
        rig.control.apply(&settings(true));
        assert!(rig.stopped.load(Ordering::Acquire), "joined before restart");
        let flags = rig.flags.lock().expect("flags");
        assert_eq!(flags.len(), 2);
        assert!(flags[0].load(Ordering::Acquire));
        assert!(!flags[1].load(Ordering::Acquire));
        drop(flags);
        rig.control.stop();
    }

    #[test]
    fn turning_the_duty_off_never_waits_for_a_stuck_worker_but_a_restart_does() {
        let rig = rig();
        rig.hold.store(true, Ordering::Release);
        rig.control.apply(&settings(true));
        let started = std::time::Instant::now();
        assert!(rig.control.apply(&settings(false)));
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the loop did not block on the join"
        );
        assert!(!rig.stopped.load(Ordering::Acquire), "still stopping");
        rig.hold.store(false, Ordering::Release);
        rig.control.apply(&settings(true));
        assert!(
            rig.stopped.load(Ordering::Acquire),
            "the restart waited for the old worker"
        );
        assert_eq!(rig.spawned.get(), 2);
        rig.control.stop();
    }

    #[test]
    fn covers_follow_the_players_art() {
        let rig = rig();
        rig.control.apply(&settings(true));
        rig.control
            .players(vec![player("a", "u1")], Some("a".into()));
        rig.control.cover("a", "u1", Some(cover()));
        assert_eq!(rig.control.snapshot().covers.len(), 1);
        rig.control
            .players(vec![player("a", "u2")], Some("a".into()));
        assert!(
            rig.control.snapshot().covers.is_empty(),
            "stale art dropped"
        );
        rig.control.cover("a", "u2", Some(cover()));
        rig.control.cover("a", "", None);
        assert!(rig.control.snapshot().covers.is_empty());
    }

    #[test]
    fn unavailable_keeps_the_last_state_marks_it_and_the_next_players_clear_it() {
        let rig = rig();
        rig.control.apply(&settings(true));
        rig.control.players(vec![player("a", "u")], None);
        rig.control.cover("a", "u", Some(cover()));
        assert!(!rig.control.snapshot().unavailable);
        let routed = route(&rig.control, MediaEvent::Unavailable("gone".into()));
        assert!(matches!(routed, Routed::Sync), "the views must hear of it");
        let snap = rig.control.snapshot();
        assert_eq!((snap.players.len(), snap.covers.len()), (1, 1));
        assert!(snap.unavailable);
        // Said again: nothing new for the views.
        let routed = route(&rig.control, MediaEvent::Unavailable("gone".into()));
        assert!(matches!(routed, Routed::Nothing));
        let routed = route(
            &rig.control,
            MediaEvent::Players {
                players: vec![player("a", "u")],
                active: None,
            },
        );
        assert!(matches!(routed, Routed::Sync));
        assert!(!rig.control.snapshot().unavailable);
        // Off forgets it too.
        let _ = route(&rig.control, MediaEvent::Unavailable("gone".into()));
        rig.control.apply(&settings(false));
        assert!(!rig.control.snapshot().unavailable);
        assert!(!rig.control.unavailable("late"), "nothing runs");
        rig.control.stop();
    }

    #[test]
    fn a_failed_spawn_is_retried_by_every_apply_while_the_duty_stays_on() {
        let rig = rig();
        rig.fail.set(true);
        // The same value in force, applied again and again: each is a retry.
        assert!(!rig.control.apply(&settings(true)));
        assert!(!rig.control.apply(&settings(true)));
        assert!(!rig.control.running());
        assert_eq!(rig.spawned.get(), 0);
        rig.fail.set(false);
        assert!(rig.control.apply(&settings(true)), "the retry starts it");
        assert!(rig.control.running());
        assert!(!rig.control.apply(&settings(true)), "and then rests");
        assert_eq!(rig.spawned.get(), 1);
        rig.control.stop();
    }

    #[test]
    fn a_command_without_a_worker_is_dropped() {
        let rig = rig();
        rig.control.command(MediaCommand::Next);
        assert_eq!(rig.spawned.get(), 0);
    }
}
