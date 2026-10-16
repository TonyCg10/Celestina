//! The app's side of the notification server: the latest snapshot the
//! worker published, who holds the bus name when it is not the shell, and
//! the way back to the worker. The banners and the Notifications page read
//! the snapshot; the verbs and the menu send commands through it.

pub mod banners;
pub mod history;

use std::{cell::OnceCell, rc::Rc};

use crate::{
    command::{Page, Verb},
    island::{clock, pages::HistoryRow},
    providers::notifications::{
        NotificationsCommand, NotificationsEvent, NotificationsProvider, NotificationsSnapshot,
    },
};

/// Whether an event changed what a surface shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum Applied {
    /// The event repeated what the state already held.
    Unchanged,
    /// The snapshot or the owner is new: the surfaces redraw.
    Changed,
}

impl Applied {
    /// Whether the surfaces have something new to draw.
    #[must_use]
    pub fn is_changed(self) -> bool {
        self == Applied::Changed
    }
}

/// The latest snapshot, the holder of the name and the command queue.
#[derive(Debug)]
pub struct NotificationsState {
    snapshot: NotificationsSnapshot,
    /// The Notifications page's rows for `snapshot`, built once per change.
    rows: Vec<HistoryRow>,
    /// The description of the program holding the notifications name when
    /// it is not the shell.
    owner: Option<String>,
    /// The worker's handle, set once it runs.
    provider: Rc<OnceCell<NotificationsProvider>>,
    /// A command was refused already: the next refusals are not logged,
    /// they fail for the same reason.
    refusal_logged: bool,
}

impl NotificationsState {
    /// Empty state over the slot the worker's handle is put in.
    pub fn new(provider: Rc<OnceCell<NotificationsProvider>>) -> NotificationsState {
        NotificationsState {
            snapshot: NotificationsSnapshot::default(),
            rows: Vec::new(),
            owner: None,
            provider,
            refusal_logged: false,
        }
    }

    /// The latest snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &NotificationsSnapshot {
        &self.snapshot
    }

    /// The rows of the Notifications page: live first, then ended.
    #[must_use]
    pub fn rows(&self) -> &[HistoryRow] {
        &self.rows
    }

    /// Who holds the notifications name, when not the shell.
    #[must_use]
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    /// Takes in an event of the worker.
    pub fn apply(&mut self, event: NotificationsEvent) -> Applied {
        match event {
            NotificationsEvent::Owner(owner) => {
                if owner == self.owner {
                    return Applied::Unchanged;
                }
                self.owner = owner;
                Applied::Changed
            }
            NotificationsEvent::Snapshot(snapshot) => {
                if snapshot == self.snapshot {
                    return Applied::Unchanged;
                }
                let (secs, _) = clock::unix_now();
                self.rows = history::history_rows(
                    &snapshot,
                    secs.saturating_mul(1000),
                    clock::utc_offset(),
                );
                self.snapshot = snapshot;
                Applied::Changed
            }
        }
    }

    /// Queues `command` for the worker without waiting; whether it was
    /// queued. A refusal (full queue, worker stopped or never started, or
    /// serving nothing because another program holds the name) is logged
    /// once per run.
    pub fn send(&mut self, command: NotificationsCommand) -> bool {
        let Some(provider) = self.provider.get() else {
            tracing::debug!(?command, "no notification worker: the command is dropped");
            return false;
        };
        match provider.send(command) {
            Ok(()) => true,
            Err(refused) => {
                if !self.refusal_logged {
                    self.refusal_logged = true;
                    tracing::warn!(
                        command = ?refused,
                        "the notification worker refused a command (its queue is full or it serves nothing); later refusals are not logged"
                    );
                }
                false
            }
        }
    }
}

/// The most characters of a body a banner or a page row carries: more
/// than two wrapped lines of the banner hold, far less than the server's
/// bound, so a long body is never shaped or copied whole.
pub const EXCERPT_CHARS: usize = 240;

/// The first [`EXCERPT_CHARS`] characters of `text`.
#[must_use]
pub fn excerpt(text: &str) -> &str {
    match text.char_indices().nth(EXCERPT_CHARS) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

/// The Island verb `notifications toggle` runs on an Island whose open
/// page is `open`: close it when the Notifications page is the open one,
/// open it otherwise.
#[must_use]
pub fn toggle_verb(open: Option<Page>) -> Verb {
    if open == Some(Page::Notifications) {
        Verb::IslandClose
    } else {
        Verb::IslandOpen(Page::Notifications)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;

    use super::*;

    fn state_with_worker() -> (NotificationsState, Receiver<NotificationsCommand>) {
        let slot = Rc::new(OnceCell::new());
        let (provider, queue) = NotificationsProvider::channel();
        assert!(slot.set(provider).is_ok());
        (NotificationsState::new(slot), queue)
    }

    fn snapshot(unread: usize) -> NotificationsSnapshot {
        NotificationsSnapshot {
            unread,
            ..NotificationsSnapshot::default()
        }
    }

    #[test]
    fn apply_keeps_the_latest_snapshot_and_ignores_a_repeat() {
        let (mut state, _queue) = state_with_worker();
        assert_eq!(state.snapshot(), &NotificationsSnapshot::default());
        assert_eq!(
            state.apply(NotificationsEvent::Snapshot(snapshot(0))),
            Applied::Unchanged
        );
        assert!(state
            .apply(NotificationsEvent::Snapshot(snapshot(2)))
            .is_changed());
        assert_eq!(state.snapshot().unread, 2);
        assert_eq!(
            state.apply(NotificationsEvent::Snapshot(snapshot(2))),
            Applied::Unchanged
        );
        assert!(state
            .apply(NotificationsEvent::Snapshot(snapshot(3)))
            .is_changed());
        assert_eq!(state.snapshot().unread, 3);
    }

    #[test]
    fn the_rows_follow_the_snapshot() {
        let (mut state, _queue) = state_with_worker();
        assert!(state.rows().is_empty());
        let mut machine = celestina_shell::notifications::Notifications::new();
        machine.post(&celestina_shell::notifications::Incoming::default(), 0);
        let snapshot = crate::providers::notifications::snapshot(&machine);
        assert!(state
            .apply(NotificationsEvent::Snapshot(snapshot))
            .is_changed());
        assert_eq!(state.rows().len(), 1);
        assert!(state.rows()[0].live);
    }

    #[test]
    fn the_owner_follows_the_worker_and_leaves_the_snapshot_alone() {
        let (mut state, _queue) = state_with_worker();
        assert!(state
            .apply(NotificationsEvent::Snapshot(snapshot(4)))
            .is_changed());
        assert_eq!(state.owner(), None);
        let held = || NotificationsEvent::Owner(Some(":1.42 (noctalia)".into()));
        assert!(state.apply(held()).is_changed());
        assert_eq!(state.owner(), Some(":1.42 (noctalia)"));
        assert_eq!(state.apply(held()), Applied::Unchanged);
        assert_eq!(state.snapshot().unread, 4);
        assert!(state.apply(NotificationsEvent::Owner(None)).is_changed());
        assert_eq!(state.owner(), None);
        assert_eq!(
            state.apply(NotificationsEvent::Owner(None)),
            Applied::Unchanged
        );
    }

    #[test]
    fn a_command_reaches_the_worker_queue() {
        let (mut state, queue) = state_with_worker();
        assert!(state.send(NotificationsCommand::DismissAll));
        assert!(state.send(NotificationsCommand::SetQuiet(true)));
        assert_eq!(queue.try_recv(), Ok(NotificationsCommand::DismissAll));
        assert_eq!(queue.try_recv(), Ok(NotificationsCommand::SetQuiet(true)));
    }

    #[test]
    fn a_full_queue_reports_instead_of_blocking() {
        let (mut state, queue) = state_with_worker();
        for _ in 0..crate::providers::notifications::COMMAND_QUEUE {
            assert!(state.send(NotificationsCommand::MarkRead));
        }
        assert!(!state.send(NotificationsCommand::DismissAll));
        assert!(state.refusal_logged);
        assert!(!state.send(NotificationsCommand::DismissAll));
        drop(queue);
    }

    #[test]
    fn a_stopped_or_missing_worker_refuses_without_panicking() {
        let (mut state, queue) = state_with_worker();
        drop(queue);
        assert!(!state.send(NotificationsCommand::DismissAll));
        let mut none = NotificationsState::new(Rc::new(OnceCell::new()));
        assert!(!none.send(NotificationsCommand::DismissAll));
    }

    #[test]
    fn the_toggle_opens_the_page_then_closes_it() {
        assert_eq!(toggle_verb(None), Verb::IslandOpen(Page::Notifications));
        assert_eq!(toggle_verb(Some(Page::Notifications)), Verb::IslandClose);
        // Another open page slides to this one.
        assert_eq!(
            toggle_verb(Some(Page::Activity)),
            Verb::IslandOpen(Page::Notifications)
        );
        assert_eq!(
            toggle_verb(Some(Page::Tray)),
            Verb::IslandOpen(Page::Notifications)
        );
    }
}
