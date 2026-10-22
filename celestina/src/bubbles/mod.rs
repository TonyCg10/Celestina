//! The bubbles duty (CEL-2-J): the minimised windows Melibea keeps, shown as
//! a group of bubbles and offered by the `Mod+K` selector. Off by default
//! (parity design §4.1).
//!
//! [`BubblesHandle`] holds the Melibea worker the settings ask for and
//! nothing else, like [`crate::phone::PhoneHandle`]: `duties.bubbles` on has
//! the worker, off has none, and [`BubblesHandle::apply`] moves between them
//! live. The state the pieces read is [`Bubbles`], kept by the app and updated
//! by [`reduce`]; a duty that is off, and a Melibea that is absent, refused or
//! lost, all read as no windows. Starting the worker is behind [`Spawner`] so
//! the rules are tested without a socket.
//!
//! A window whose projection names no icon gets the one of its application,
//! found by its `app_id` with the lookup the Activity page's rows use
//! ([`AppIcons`]); [`fill_icons`] does it on the worker's thread, so the
//! loop never reads a `.desktop` file and the pieces stay pure.

use std::cell::RefCell;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use celestina_shell::melibea::{BubbleAnchor, Window, WindowTransition};
use celestina_shell::settings::{Duty, Settings};
use celestina_wayland::Sender;

use crate::message::Message;
use crate::providers::duty::DutyWorker;
use crate::providers::melibea::{
    self, ActionId, BubbleAction, MelibeaConfig, MelibeaEvent, MelibeaHandle, Settlement,
};
use crate::providers::niri::AppIcons;
use crate::providers::niri::NiriModel;
use crate::providers::Worker;

/// What the Melibea worker sends to the loop: the news and the run of the
/// worker that sent it (see [`crate::providers::duty`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BubblesNews {
    pub run: u64,
    pub event: MelibeaEvent,
}

/// How far Melibea can be reached, as the pieces tell it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Link {
    /// No socket, or nobody listening on it (a normal state; also the state
    /// of a duty that is off).
    #[default]
    Absent,
    /// A socket path that is not trusted, or a subscription Melibea refused.
    Refused,
    /// The connection was lost or Melibea has no authoritative state.
    Unavailable,
    /// Subscribed: [`Bubbles::windows`] is Melibea's current projection.
    Live,
}

/// The minimised windows and whether they can be trusted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bubbles {
    /// Melibea's projection in its order; empty unless the link is live.
    pub windows: Vec<Window>,
    pub link: Link,
}

/// No windows and no link: what a duty that is off shows, and what tests
/// build a context with.
pub static NONE: Bubbles = Bubbles {
    windows: Vec::new(),
    link: Link::Absent,
};

impl Bubbles {
    /// Whether anything is worth drawing: a live link with at least one
    /// window.
    #[must_use]
    pub fn has_windows(&self) -> bool {
        self.link == Link::Live && !self.windows.is_empty()
    }
}

/// Applies one worker event to `state`. The projection replaces the windows
/// whole; anything that withdraws it empties them, so a stale bubble never
/// outlives its source. An action's answer changes no state: the next
/// projection does. Returns whether the state changed (and the bar must be
/// drawn again).
pub fn reduce(state: &mut Bubbles, event: &MelibeaEvent) -> bool {
    let next = match event {
        MelibeaEvent::Windows(windows) => Bubbles {
            windows: windows.clone(),
            link: Link::Live,
        },
        MelibeaEvent::Absent => Bubbles {
            windows: Vec::new(),
            link: Link::Absent,
        },
        MelibeaEvent::Refused(_) => Bubbles {
            windows: Vec::new(),
            link: Link::Refused,
        },
        MelibeaEvent::Unavailable => Bubbles {
            windows: Vec::new(),
            link: Link::Unavailable,
        },
        MelibeaEvent::ActionResult { .. } | MelibeaEvent::ActionSettled { .. } => {
            return false;
        }
    };
    let changed = *state != next;
    *state = next;
    changed
}

/// Gives every window of a projection that names no icon (none, or an empty
/// name) the one `lookup` finds for its `app_id`; a window without an
/// application id, or one `lookup` knows no icon for, keeps none. Any other
/// event is left alone.
pub fn fill_icons(event: &mut MelibeaEvent, mut lookup: impl FnMut(&str) -> Option<String>) {
    let MelibeaEvent::Windows(windows) = event else {
        return;
    };
    for window in windows {
        if window
            .icon_name
            .as_deref()
            .is_some_and(|name| !name.is_empty())
        {
            continue;
        }
        window.icon_name = window
            .app_id
            .as_deref()
            .filter(|app_id| !app_id.is_empty())
            .and_then(&mut lookup);
    }
}

/// Writes what the shell does not show to the journal: how Melibea answered
/// an action, and why the link is down.
pub fn log(event: &MelibeaEvent) {
    match event {
        MelibeaEvent::Windows(windows) => {
            tracing::debug!(count = windows.len(), "bubbles: windows")
        }
        MelibeaEvent::Absent => tracing::info!("bubbles: Melibea is absent"),
        MelibeaEvent::Refused(why) => tracing::warn!("bubbles: refused: {why}"),
        MelibeaEvent::Unavailable => tracing::info!("bubbles: Melibea is unavailable"),
        MelibeaEvent::ActionResult { id, status } => {
            tracing::info!(id = id.0, ?status, "bubbles: action answered");
        }
        MelibeaEvent::ActionSettled { id, settlement } => match settlement {
            Settlement::Confirmed => tracing::info!(id = id.0, "bubbles: action confirmed"),
            Settlement::Refused(status) => {
                tracing::warn!(id = id.0, ?status, "bubbles: action refused");
            }
            Settlement::Unconfirmed => {
                tracing::warn!(id = id.0, "bubbles: action not confirmed in time");
            }
            Settlement::Failed(why) => {
                tracing::warn!(id = id.0, "bubbles: action failed: {why}");
            }
        },
    }
}

/// The close requests Melibea has taken and not yet settled, by window. A
/// second Delete on a window whose close is pending asks for nothing: the
/// first is still waiting for the projection to drop the window. A request
/// ends when its action settles (confirmed, refused, failed or not confirmed
/// in time, so the author may try again), when the projection no longer holds
/// the window, or when the link is withdrawn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingCloses {
    asked: Vec<(u64, ActionId)>,
}

impl PendingCloses {
    /// Whether a close of `window_id` is already pending.
    #[must_use]
    pub fn pending(&self, window_id: u64) -> bool {
        self.asked.iter().any(|(window, _)| *window == window_id)
    }

    /// Records that `id` asks to close `window_id`.
    pub fn asked(&mut self, window_id: u64, id: ActionId) {
        if !self.pending(window_id) {
            self.asked.push((window_id, id));
        }
    }

    /// Takes one worker event into account; call it with every accepted one.
    pub fn observe(&mut self, event: &MelibeaEvent) {
        match event {
            MelibeaEvent::Windows(windows) => self
                .asked
                .retain(|(window, _)| windows.iter().any(|w| w.id == *window)),
            MelibeaEvent::ActionSettled { id, .. } => self.asked.retain(|(_, asked)| asked != id),
            MelibeaEvent::Absent | MelibeaEvent::Refused(_) | MelibeaEvent::Unavailable => {
                self.asked.clear();
            }
            MelibeaEvent::ActionResult { .. } => {}
        }
    }

    /// Forgets every pending request (the duty went off).
    pub fn clear(&mut self) {
        self.asked.clear();
    }
}

/// The action `bubbles minimize` asks for: the focused window, with no
/// animation when the appearance asks for reduced motion.
#[must_use]
pub fn minimize_action(reduced_motion: bool) -> BubbleAction {
    BubbleAction::MinimizeFocused {
        transition: reduced_motion.then_some(WindowTransition::Disabled),
    }
}

/// The action a click on a bubble asks for: restore `window_id`. Reduced
/// motion asks for no animation; otherwise the transition travels from the
/// bubble, `bubble` as (x, y, width, height) in logical px on the output
/// `output`, when that is a usable rectangle; failing both, no hint (niri's
/// ordinary local motion).
#[must_use]
pub fn restore_action(
    window_id: u64,
    output: &str,
    bubble: Option<(f32, f32, f32, f32)>,
    reduced_motion: bool,
) -> BubbleAction {
    let transition = if reduced_motion {
        Some(WindowTransition::Disabled)
    } else {
        bubble
            .and_then(|(x, y, w, h)| {
                BubbleAnchor::new(
                    output,
                    f64::from(x),
                    f64::from(y),
                    f64::from(w),
                    f64::from(h),
                )
            })
            .map(|anchor| WindowTransition::Anchored { anchor })
    };
    BubbleAction::Restore {
        window_id,
        transition,
    }
}

/// The output niri says window `window_id` is on, if the model knows both the
/// window and its workspace's output. A minimised window is usually not in
/// the model (Melibea keeps it), so `None` is the ordinary answer.
#[must_use]
pub fn window_output(model: &NiriModel, window_id: u64) -> Option<&str> {
    let workspace = model
        .windows
        .iter()
        .find(|w| w.id == window_id)?
        .workspace_id?;
    model
        .workspaces
        .iter()
        .find(|w| w.id == workspace)?
        .output
        .as_deref()
}

/// Whether the bubble's rectangle may travel with a restore: false only when
/// the model knows the window's output and it is not the bubble's. A window
/// the model does not know keeps the anchor and Melibea or niri degrade it.
#[must_use]
pub fn anchor_allowed(window_output: Option<&str>, bubble_output: &str) -> bool {
    window_output.is_none_or(|known| known == bubble_output)
}

/// The loop's side of the Melibea worker.
pub trait BubblesControl: Send {
    /// # Errors
    ///
    /// Why the worker did not take the action.
    fn request(&self, action: BubbleAction) -> Result<ActionId, String>;
}

impl BubblesControl for MelibeaHandle {
    fn request(&self, action: BubbleAction) -> Result<ActionId, String> {
        MelibeaHandle::request(self, action).map_err(|e| e.to_string())
    }
}

/// How the worker comes to exist: the seam between the rules and the socket.
pub trait Spawner {
    /// Starts the worker; its news is stamped with `run`.
    ///
    /// # Errors
    ///
    /// The failure to start the threads.
    fn melibea(
        &self,
        run: u64,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn BubblesControl>, Worker)>;
}

/// The real spawner: Melibea's socket, found from the environment.
struct Live {
    sender: Sender<Message>,
}

impl Spawner for Live {
    fn melibea(
        &self,
        run: u64,
        shutdown: &Arc<AtomicBool>,
    ) -> io::Result<(Box<dyn BubblesControl>, Worker)> {
        let sender = self.sender.clone();
        // Made on the worker's thread at its first projection, so the
        // directories are read there.
        let mut icons: Option<AppIcons> = None;
        let (handle, worker) = melibea::spawn(
            &MelibeaConfig::from_env(),
            move |mut event| {
                fill_icons(&mut event, |app_id| {
                    icons.get_or_insert_with(AppIcons::new).icon_name(app_id)
                });
                sender
                    .send(Message::Melibea(Box::new(BubblesNews { run, event })))
                    .is_ok()
            },
            shutdown,
        )?;
        Ok((Box::new(handle), worker))
    }
}

struct Inner {
    spawner: Box<dyn Spawner>,
    duty: DutyWorker<dyn BubblesControl>,
}

/// The bubbles duty: the worker, if the duty is on.
pub struct BubblesHandle {
    inner: RefCell<Inner>,
}

impl std::fmt::Debug for BubblesHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BubblesHandle")
            .field("running", &self.running())
            .finish()
    }
}

impl BubblesHandle {
    /// The handle over Melibea's real socket; nothing runs until
    /// [`BubblesHandle::apply`] finds the duty on.
    #[must_use]
    pub fn new(sender: Sender<Message>) -> Self {
        Self::with_spawner(Box::new(Live { sender }))
    }

    /// The handle over `spawner`, for tests.
    #[must_use]
    pub fn with_spawner(spawner: Box<dyn Spawner>) -> Self {
        Self {
            inner: RefCell::new(Inner {
                spawner,
                duty: DutyWorker::new("bubbles"),
            }),
        }
    }

    /// Whether the worker exists.
    #[must_use]
    pub fn running(&self) -> bool {
        self.inner.borrow().duty.is_on()
    }

    /// Whether news stamped `run` is the running worker's.
    #[must_use]
    pub fn accepts(&self, run: u64) -> bool {
        self.inner.borrow().duty.accepts(run)
    }

    /// Brings the worker to what `settings` ask for. Idempotent. Returns
    /// whether a worker started or stopped, so the caller empties the bubbles
    /// when the duty went off.
    pub fn apply(&self, settings: &Settings) -> bool {
        let mut inner = self.inner.borrow_mut();
        let Inner { spawner, duty } = &mut *inner;
        duty.set(settings.duties.bubbles == Duty::On, |run, flag| {
            spawner.melibea(run, flag)
        })
    }

    /// Queues `action` for the running worker.
    ///
    /// # Errors
    ///
    /// Why it was not queued: the duty is off or the queue is full.
    pub fn request(&self, action: BubbleAction) -> Result<ActionId, String> {
        match self.inner.borrow().duty.control() {
            Some(control) => control.request(action),
            None => Err("the bubbles duty is off".to_owned()),
        }
    }

    /// Stops the worker and waits for it. Also what dropping the handle does.
    pub fn stop(&self) {
        self.inner.borrow_mut().duty.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    use celestina_shell::melibea::ActionStatus;

    use super::*;

    fn window(id: u64, title: &str) -> Window {
        Window {
            id,
            app_id: Some("app".to_owned()),
            title: Some(title.to_owned()),
            icon_name: None,
        }
    }

    #[test]
    fn a_restore_travels_from_the_bubble_unless_motion_is_reduced() {
        let anchored = restore_action(7, "DP-1", Some((10.0, 4.0, 28.0, 28.0)), false);
        let BubbleAction::Restore {
            window_id,
            transition: Some(WindowTransition::Anchored { anchor }),
        } = anchored
        else {
            panic!("{anchored:?}");
        };
        assert_eq!(window_id, 7);
        assert_eq!(
            anchor,
            BubbleAnchor::new("DP-1", 10.0, 4.0, 28.0, 28.0).expect("anchor")
        );
        assert_eq!(
            restore_action(7, "DP-1", Some((10.0, 4.0, 28.0, 28.0)), true),
            BubbleAction::Restore {
                window_id: 7,
                transition: Some(WindowTransition::Disabled),
            }
        );
        for unusable in [
            None,
            Some((0.0, 0.0, 0.0, 28.0)),
            Some((f32::NAN, 0.0, 1.0, 1.0)),
        ] {
            assert_eq!(
                restore_action(7, "DP-1", unusable, false),
                BubbleAction::Restore {
                    window_id: 7,
                    transition: None,
                },
                "{unusable:?}"
            );
        }
        assert_eq!(
            restore_action(7, "", Some((1.0, 1.0, 1.0, 1.0)), false),
            BubbleAction::Restore {
                window_id: 7,
                transition: None,
            },
            "no output, no anchor"
        );
    }

    #[test]
    fn a_pending_close_blocks_a_second_one_until_it_ends() {
        let settled = |id| MelibeaEvent::ActionSettled {
            id: ActionId(id),
            settlement: Settlement::Unconfirmed,
        };
        let mut closes = PendingCloses::default();
        assert!(!closes.pending(4));
        closes.asked(4, ActionId(1));
        closes.asked(4, ActionId(2));
        assert!(closes.pending(4));
        assert!(!closes.pending(5), "another window is free");

        // A projection that still holds the window keeps the close pending.
        closes.observe(&MelibeaEvent::Windows(vec![window(4, "a")]));
        assert!(closes.pending(4));
        // The answer is not the end; the settlement is, and then the author
        // may ask again.
        closes.observe(&MelibeaEvent::ActionResult {
            id: ActionId(1),
            status: ActionStatus::CloseRequested,
        });
        assert!(closes.pending(4));
        closes.observe(&settled(9));
        assert!(closes.pending(4), "another action's end");
        closes.observe(&settled(1));
        assert!(!closes.pending(4));

        // The window leaving the projection ends it too, and so does losing
        // the link or the duty.
        closes.asked(4, ActionId(3));
        closes.observe(&MelibeaEvent::Windows(vec![window(7, "b")]));
        assert!(!closes.pending(4));
        closes.asked(4, ActionId(4));
        closes.observe(&MelibeaEvent::Unavailable);
        assert!(!closes.pending(4));
        closes.asked(4, ActionId(5));
        closes.clear();
        assert!(!closes.pending(4));
    }

    #[test]
    fn the_anchor_travels_unless_the_window_is_known_to_be_on_another_output() {
        use crate::providers::niri::{Window as NiriWindow, Workspace};
        let model = NiriModel {
            workspaces: vec![Workspace {
                id: 10,
                idx: 1,
                name: None,
                output: Some("HDMI-A-1".to_owned()),
                is_active: true,
                is_focused: false,
                columns: 1,
            }],
            windows: vec![
                NiriWindow {
                    id: 3,
                    title: None,
                    app_id: None,
                    workspace_id: Some(10),
                    is_focused: false,
                    icon: None,
                },
                NiriWindow {
                    id: 4,
                    title: None,
                    app_id: None,
                    workspace_id: None,
                    is_focused: false,
                    icon: None,
                },
            ],
            outputs: vec!["HDMI-A-1".to_owned()],
        };
        assert_eq!(window_output(&model, 3), Some("HDMI-A-1"));
        assert_eq!(window_output(&model, 4), None, "no workspace");
        assert_eq!(window_output(&model, 99), None, "a window niri lacks");
        assert!(!anchor_allowed(window_output(&model, 3), "DP-1"));
        assert!(anchor_allowed(window_output(&model, 3), "HDMI-A-1"));
        assert!(anchor_allowed(window_output(&model, 99), "DP-1"));
    }

    #[test]
    fn the_projection_replaces_the_windows_and_makes_the_link_live() {
        let mut state = Bubbles::default();
        assert!(!state.has_windows());
        let first = MelibeaEvent::Windows(vec![window(1, "a"), window(2, "b")]);
        assert!(reduce(&mut state, &first));
        assert_eq!(state.link, Link::Live);
        assert_eq!(state.windows.len(), 2);
        assert!(state.has_windows());
        assert!(!reduce(&mut state, &first), "the same projection");
        let second = MelibeaEvent::Windows(vec![window(2, "b")]);
        assert!(reduce(&mut state, &second));
        assert_eq!(state.windows, vec![window(2, "b")]);
    }

    #[test]
    fn every_withdrawal_empties_the_windows_with_its_own_link() {
        for (event, link) in [
            (MelibeaEvent::Absent, Link::Absent),
            (MelibeaEvent::Refused("untrusted".to_owned()), Link::Refused),
            (MelibeaEvent::Unavailable, Link::Unavailable),
        ] {
            let mut state = Bubbles {
                windows: vec![window(1, "a")],
                link: Link::Live,
            };
            assert!(reduce(&mut state, &event), "{event:?}");
            assert_eq!(
                state,
                Bubbles {
                    windows: Vec::new(),
                    link
                },
                "{event:?}"
            );
            assert!(!state.has_windows());
            assert!(!reduce(&mut state, &event), "{event:?} again");
        }
        // A live link with no window draws nothing either.
        let empty = Bubbles {
            windows: Vec::new(),
            link: Link::Live,
        };
        assert!(!empty.has_windows());
    }

    #[test]
    fn an_actions_answer_changes_no_state() {
        let mut state = Bubbles {
            windows: vec![window(1, "a")],
            link: Link::Live,
        };
        let before = state.clone();
        for event in [
            MelibeaEvent::ActionResult {
                id: ActionId(1),
                status: ActionStatus::Applied,
            },
            MelibeaEvent::ActionSettled {
                id: ActionId(1),
                settlement: Settlement::Confirmed,
            },
            MelibeaEvent::ActionSettled {
                id: ActionId(2),
                settlement: Settlement::Failed("x".to_owned()),
            },
        ] {
            assert!(!reduce(&mut state, &event));
            log(&event);
            assert_eq!(state, before);
        }
    }

    #[test]
    fn a_window_without_an_icon_gets_its_applications() {
        let with_icon = |id, app_id: Option<&str>, icon: Option<&str>| Window {
            id,
            app_id: app_id.map(str::to_owned),
            title: None,
            icon_name: icon.map(str::to_owned),
        };
        let mut event = MelibeaEvent::Windows(vec![
            with_icon(1, Some("kitty"), Some("own-icon")),
            with_icon(2, Some("firefox"), None),
            with_icon(3, Some("firefox"), Some("")),
            with_icon(4, None, None),
            with_icon(5, Some(""), None),
            with_icon(6, Some("unknown"), None),
            with_icon(7, Some(&"x".repeat(10_000)), None),
        ]);
        let mut asked = Vec::new();
        fill_icons(&mut event, |app_id| {
            asked.push(app_id.chars().count());
            (app_id == "firefox").then(|| "firefox-icon".to_owned())
        });
        let MelibeaEvent::Windows(windows) = &event else {
            panic!("{event:?}");
        };
        let icons: Vec<_> = windows.iter().map(|w| w.icon_name.as_deref()).collect();
        assert_eq!(
            icons,
            [
                Some("own-icon"),
                Some("firefox-icon"),
                Some("firefox-icon"),
                None,
                None,
                None,
                None
            ]
        );
        // Only the windows that lacked an icon and had an id were looked up.
        assert_eq!(asked, [7, 7, 7, 10_000]);
        // Other events are left alone and ask nothing.
        let mut absent = MelibeaEvent::Absent;
        fill_icons(&mut absent, |_| panic!("no lookup"));
        assert_eq!(absent, MelibeaEvent::Absent);
    }

    #[test]
    fn minimize_asks_for_no_animation_only_under_reduced_motion() {
        assert_eq!(
            minimize_action(true),
            BubbleAction::MinimizeFocused {
                transition: Some(WindowTransition::Disabled)
            }
        );
        assert_eq!(
            minimize_action(false),
            BubbleAction::MinimizeFocused { transition: None }
        );
    }

    #[derive(Clone, Default)]
    struct Fake {
        runs: Arc<Mutex<Vec<u64>>>,
        actions: Arc<Mutex<Vec<BubbleAction>>>,
    }

    struct FakeControl(Arc<Mutex<Vec<BubbleAction>>>);

    impl BubblesControl for FakeControl {
        fn request(&self, action: BubbleAction) -> Result<ActionId, String> {
            let mut actions = self.0.lock().expect("lock");
            actions.push(action);
            Ok(ActionId(actions.len() as u64))
        }
    }

    impl Spawner for Fake {
        fn melibea(
            &self,
            run: u64,
            shutdown: &Arc<AtomicBool>,
        ) -> io::Result<(Box<dyn BubblesControl>, Worker)> {
            self.runs.lock().expect("lock").push(run);
            let flag = Arc::clone(shutdown);
            let worker = Worker::spawn("test-bubbles", shutdown, move || {
                while !flag.load(Ordering::Acquire) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })?;
            Ok((Box::new(FakeControl(Arc::clone(&self.actions))), worker))
        }
    }

    fn settings(bubbles: Duty) -> Settings {
        let mut settings = Settings::default();
        settings.duties.bubbles = bubbles;
        settings
    }

    #[test]
    fn the_worker_follows_the_duty_on_off_on() {
        let fake = Fake::default();
        let handle = BubblesHandle::with_spawner(Box::new(fake.clone()));
        assert!(!handle.apply(&settings(Duty::Off)));
        assert!(handle.apply(&settings(Duty::On)));
        assert!(handle.running() && handle.accepts(1));
        assert!(!handle.apply(&settings(Duty::On)), "idempotent");

        let id = handle.request(minimize_action(false)).expect("queued");
        assert_eq!(id, ActionId(1));

        assert!(handle.apply(&settings(Duty::Off)));
        assert!(!handle.running());
        assert!(!handle.accepts(1));
        assert!(handle.request(minimize_action(false)).is_err());

        assert!(handle.apply(&settings(Duty::On)));
        assert!(handle.accepts(2) && !handle.accepts(1));
        assert_eq!(*fake.runs.lock().expect("lock"), vec![1, 2]);
        handle.stop();
        assert!(!handle.running());
    }

    #[test]
    fn a_file_from_1_3_0_starts_no_worker() {
        let older = Settings::from_bytes(br#"{"duties":{"idle":"on","wallpaper":"on"}}"#)
            .expect("a valid older file");
        let fake = Fake::default();
        let handle = BubblesHandle::with_spawner(Box::new(fake.clone()));
        assert!(!handle.apply(&older));
        assert!(!handle.running());
        assert!(fake.runs.lock().expect("lock").is_empty());
    }
}
