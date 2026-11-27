//! The app's side of the notification server: the latest snapshot the
//! worker published, who holds the bus name when it is not the shell, and
//! the way back to the worker. The banners and the Notifications page read
//! the snapshot; the verbs and the menu send commands through it.

pub mod banners;
pub mod handover;
pub mod history;
pub mod island_notice;

use std::{cell::OnceCell, rc::Rc};

use crate::{
    command::Verb,
    island::{clock, pages::HistoryRow},
    pieces::PageId,
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
    /// The worker has not yet answered the `MarkRead` that [`Self::mark_viewed`]
    /// queued: its snapshot repeats the one held, yet every surface still
    /// has to draw it.
    seen_unanswered: bool,
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
            seen_unanswered: false,
        }
    }

    /// The latest snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &NotificationsSnapshot {
        &self.snapshot
    }

    /// Marks everything read in the held snapshot and rows, ahead of the
    /// worker's own snapshot, so a notification that arrives while the page
    /// is shown is never drawn unread for a frame. The worker's snapshot
    /// that follows is then equal to this one. Whether anything changed.
    pub fn see_all(&mut self) -> bool {
        if self.snapshot.unread == 0 {
            return false;
        }
        let snapshot = &mut self.snapshot;
        for entry in snapshot
            .toasts
            .iter_mut()
            .chain(&mut snapshot.live)
            .chain(&mut snapshot.history)
        {
            entry.read = true;
        }
        snapshot.unread = 0;
        for row in &mut self.rows {
            row.read = true;
        }
        true
    }

    /// What is on the Notifications page counts as seen: asks the worker to
    /// mark everything read and, only if it was queued, marks the held state
    /// read ahead of it ([`Self::see_all`]). The worker's answer then redraws
    /// every surface once, though it repeats the held snapshot. Whether
    /// anything was marked.
    pub fn mark_viewed(&mut self) -> bool {
        if self.snapshot.unread == 0 || !self.send(NotificationsCommand::MarkRead) {
            return false;
        }
        self.seen_unanswered = self.see_all();
        self.seen_unanswered
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
                let answered = std::mem::take(&mut self.seen_unanswered);
                if snapshot == self.snapshot {
                    return if answered {
                        Applied::Changed
                    } else {
                        Applied::Unchanged
                    };
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
pub fn toggle_verb(open: Option<PageId>) -> Verb {
    if open == Some(PageId::NOTIFICATIONS) {
        Verb::IslandClose
    } else {
        Verb::IslandOpen(PageId::NOTIFICATIONS)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;

    use celestina_shell::notifications::{Notification, Urgency};

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
    fn see_all_clears_the_unread_count_and_the_rows_ahead_of_the_worker() {
        let (mut state, _queue) = state_with_worker();
        let notification = Notification {
            id: 1,
            app_name: "Magnetita".to_owned(),
            summary: "hello".to_owned(),
            body: String::new(),
            image: None,
            actions: Vec::new(),
            urgency: Urgency::Normal,
            posted_ms: 0,
            expires_at_ms: None,
            read: false,
        };
        let unread = NotificationsSnapshot {
            live: vec![notification.clone()],
            toasts: vec![notification],
            unread: 1,
            ..NotificationsSnapshot::default()
        };
        assert!(state
            .apply(NotificationsEvent::Snapshot(unread.clone()))
            .is_changed());
        assert!(state.see_all());
        assert_eq!(state.snapshot().unread, 0);
        assert!(state.rows().iter().all(|row| row.read));
        assert!(!state.see_all(), "nothing left: no loop");

        // The worker's snapshot after its MarkRead is what is held already.
        let mut seen = unread;
        seen.unread = 0;
        seen.live[0].read = true;
        seen.toasts[0].read = true;
        assert_eq!(
            state.apply(NotificationsEvent::Snapshot(seen)),
            Applied::Unchanged
        );
    }

    #[test]
    fn the_answer_to_a_viewed_mark_redraws_every_surface_once() {
        let (mut state, queue) = state_with_worker();
        let notification = Notification {
            id: 1,
            app_name: "app".to_owned(),
            summary: "hello".to_owned(),
            body: String::new(),
            image: None,
            actions: Vec::new(),
            urgency: Urgency::Normal,
            posted_ms: 0,
            expires_at_ms: None,
            read: false,
        };
        let unread = NotificationsSnapshot {
            history: vec![notification],
            unread: 1,
            ..NotificationsSnapshot::default()
        };
        let _ = state.apply(NotificationsEvent::Snapshot(unread.clone()));
        assert!(state.mark_viewed());
        assert_eq!(queue.try_recv(), Ok(NotificationsCommand::MarkRead));
        let mut seen = unread;
        seen.unread = 0;
        seen.history[0].read = true;
        assert_eq!(
            state.apply(NotificationsEvent::Snapshot(seen.clone())),
            Applied::Changed,
            "the other outputs still draw the dot"
        );
        assert_eq!(
            state.apply(NotificationsEvent::Snapshot(seen)),
            Applied::Unchanged,
            "once"
        );
    }

    #[test]
    fn nothing_is_marked_seen_when_the_command_is_refused() {
        let (mut state, queue) = state_with_worker();
        let unread = NotificationsSnapshot {
            unread: 1,
            ..NotificationsSnapshot::default()
        };
        let _ = state.apply(NotificationsEvent::Snapshot(unread));
        drop(queue);
        assert!(!state.mark_viewed());
        assert_eq!(state.snapshot().unread, 1, "still unread");
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
        assert_eq!(toggle_verb(None), Verb::IslandOpen(PageId::NOTIFICATIONS));
        assert_eq!(toggle_verb(Some(PageId::NOTIFICATIONS)), Verb::IslandClose);
        // Another open page slides to this one.
        assert_eq!(
            toggle_verb(Some(PageId::ACTIVITY)),
            Verb::IslandOpen(PageId::NOTIFICATIONS)
        );
        assert_eq!(
            toggle_verb(Some(PageId::TRAY)),
            Verb::IslandOpen(PageId::NOTIFICATIONS)
        );
    }
}
