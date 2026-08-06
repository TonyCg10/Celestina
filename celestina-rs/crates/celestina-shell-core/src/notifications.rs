//! What the session's applications are trying to tell the person, and for how
//! long.
//!
//! This is the whole freedesktop notification server except the bus: identity
//! and replacement, when something stops being shown, which actions exist, what
//! the server admits it can do, and how much of any of it is kept. A server
//! around this only marshals a method call into [`Incoming`] and a decision back
//! out.
//!
//! Two things shape every rule here.
//!
//! A notification is **untrusted input from any application on the session**.
//! Text arrives from a program that may be broken or hostile, so summary, body,
//! action labels, icon and application names are bounded before anything can
//! show them, and an image hint is a *description to be checked* rather than
//! bytes to be believed. Nothing here decodes an image.
//!
//! And a notification is **a claim about time**. The specification lets a
//! producer ask for a timeout, ask for the server's default, or ask to stay
//! forever; a person needs the last of those to be rare and the critical ones to
//! survive. Time arrives as a millisecond stamp, so every expiry rule is
//! testable without a clock.

use crate::bounded;
use crate::snapshot::MAX_TEXT_UNITS;

/// How many notifications may be shown at once. A stack taller than this is a
/// wall, and what it hides is the newest thing — the opposite of the point.
pub const MAX_VISIBLE: usize = 5;
/// How many are remembered after they stop being shown. History is a
/// convenience, not an archive: this shell does not persist it across sessions.
pub const MAX_HISTORY: usize = 50;
pub const MAX_APP_NAME_CHARS: usize = 64;
pub const MAX_SUMMARY_CHARS: usize = 120;
/// A body is published inside a notification row, so it is bounded by what a
/// row field may carry. It was once longer than that: the producer's body then
/// passed this bound, failed the host's, and the host discards a whole frame
/// rather than one field — one long notification froze every provider on the
/// panel. Deriving it from the field bound is what keeps the two from drifting
/// apart again.
pub const MAX_BODY_CHARS: usize = MAX_TEXT_UNITS;
pub const MAX_ICON_CHARS: usize = 256;
/// Actions are buttons on a toast. More than this is a menu nobody asked for.
pub const MAX_ACTIONS: usize = 4;
pub const MAX_ACTION_KEY_CHARS: usize = 64;
pub const MAX_ACTION_LABEL_CHARS: usize = 48;

/// How long a notification stays when its producer asked for the server's
/// default. Critical is not in this list on purpose: see [`Urgency`].
const DEFAULT_LOW_MS: u64 = 4_000;
const DEFAULT_NORMAL_MS: u64 = 6_000;
/// The longest a producer may pin something on screen. A program asking for an
/// hour is either broken or helping itself to the session's attention; it gets
/// a long time, not an unbounded one.
pub const MAX_TIMEOUT_MS: u64 = 60_000;

/// How much the person is expected to care, as the producer sees it.
///
/// Critical never expires on its own. That is the specification's convention and
/// the only part of urgency this server treats as a rule rather than a hint: a
/// critical notification is dismissed by a person or by the program that raised
/// it, never by a timer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low,
    // A producer that says nothing usable about urgency means this one.
    #[default]
    Normal,
    Critical,
}

impl Urgency {
    /// Reads the `urgency` hint's byte. Anything that is not 0, 1 or 2 is
    /// normal: an unreadable hint is not a reason to shout or to whisper.
    #[must_use]
    pub fn from_hint(value: u8) -> Self {
        match value {
            0 => Self::Low,
            2 => Self::Critical,
            _ => Self::Normal,
        }
    }

    fn default_timeout_ms(self) -> Option<u64> {
        match self {
            Self::Low => Some(DEFAULT_LOW_MS),
            Self::Normal => Some(DEFAULT_NORMAL_MS),
            Self::Critical => None,
        }
    }
}

/// Where a notification's picture would come from, once somebody has checked it.
///
/// The specification allows raw pixels in a hint. This core never carries them:
/// it says which *kind* of reference arrived and keeps a bounded name or path,
/// leaving the decoding — and the refusal — to the layer that has bytes in its
/// hands. `None` is the honest answer for anything unrecognized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Image {
    /// A themed icon name, as `app_icon` or the `image-path` hint gives it.
    Name(String),
    /// An absolute path or `file:` URI the producer pointed at.
    Path(String),
}

impl Image {
    /// Reads one image reference, or nothing when it cannot be one.
    ///
    /// A relative path is refused: a server resolving one would be resolving it
    /// against its own working directory, which is not where the producer meant.
    #[must_use]
    pub fn read(reference: &str) -> Option<Self> {
        let reference = reference.trim();
        if reference.is_empty() || reference.chars().count() > MAX_ICON_CHARS {
            return None;
        }
        if let Some(path) = reference.strip_prefix("file://") {
            return (!path.is_empty() && path.starts_with('/'))
                .then(|| Self::Path(path.to_owned()));
        }
        if reference.starts_with('/') {
            return Some(Self::Path(reference.to_owned()));
        }
        // An icon name is a name: no separators, no traversal, nothing that
        // could be read as a path by whoever resolves it later.
        let usable = reference
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !reference.contains("..");
        usable.then(|| Self::Name(reference.to_owned()))
    }
}

/// One button a producer offered. The key is what is sent back when it is
/// pressed; the label is what a person reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub key: String,
    pub label: String,
}

/// A `Notify` call, before any of it is believed.
#[derive(Clone, Debug, Default)]
pub struct Incoming {
    pub app_name: String,
    /// The id to replace, or 0 for a new notification.
    pub replaces_id: u32,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    /// The flat `[key, label, key, label, ...]` list the bus carries.
    pub actions: Vec<String>,
    pub urgency: Urgency,
    /// The `image-path` hint, when there was one.
    pub image: Option<String>,
    /// -1 for the server's default, 0 to stay until dismissed, else
    /// milliseconds.
    pub expire_timeout: i32,
}

/// One notification as this server holds it: bounded, identified, and with its
/// deadline already decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub image: Option<Image>,
    pub actions: Vec<Action>,
    pub urgency: Urgency,
    pub posted_ms: u64,
    /// When it stops being shown, or `None` when only a person ends it.
    pub expires_at_ms: Option<u64>,
    /// Whether the person has seen it since it arrived.
    pub read: bool,
}

impl Notification {
    /// Whether this action key is one the producer actually offered.
    #[must_use]
    pub fn offers(&self, key: &str) -> bool {
        self.actions.iter().any(|action| action.key == key)
    }
}

/// Why a notification stopped being shown, in the specification's own numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    Expired = 1,
    Dismissed = 2,
    Requested = 3,
    Undefined = 4,
}

/// What the server owes the bus after one notification ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closed {
    pub id: u32,
    pub reason: CloseReason,
}

/// What this server tells `GetCapabilities`.
///
/// Every entry is a promise. `body-markup` is absent because this shell renders
/// text as text, and claiming it would invite producers to send markup that is
/// then shown raw. `persistence` is absent because history does not survive the
/// session.
#[must_use]
pub fn capabilities() -> &'static [&'static str] {
    &["actions", "body", "icon-static"]
}

fn read_actions(flat: &[String]) -> Vec<Action> {
    flat.chunks_exact(2)
        .filter_map(|pair| {
            let key = pair[0].trim();
            let label = pair[1].trim();
            // A button with no key cannot be answered, and one with no label
            // cannot be read. Neither is worth showing.
            (!key.is_empty() && !label.is_empty()).then(|| Action {
                key: bounded(key, MAX_ACTION_KEY_CHARS),
                label: bounded(label, MAX_ACTION_LABEL_CHARS),
            })
        })
        .take(MAX_ACTIONS)
        .collect()
}

fn deadline(incoming: &Incoming, urgency: Urgency, now_ms: u64) -> Option<u64> {
    let requested = match incoming.expire_timeout {
        // The server's default, which is where urgency decides.
        i32::MIN..=-1 => return urgency.default_timeout_ms().map(|ms| now_ms + ms),
        // Explicitly "until somebody ends it".
        0 => return None,
        milliseconds => u64::from(milliseconds.unsigned_abs()),
    };
    Some(now_ms + requested.min(MAX_TIMEOUT_MS))
}

/// Every notification this session currently has, and the rules about them.
///
/// Ids are never reused, so an answer about a notification can never land on a
/// later one that happens to occupy its slot.
#[derive(Debug, Default)]
pub struct Notifications {
    live: Vec<Notification>,
    history: Vec<Notification>,
    last_id: u32,
    quiet: bool,
}

impl Notifications {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether toasts are being withheld. History still records everything: not
    /// being interrupted is not the same as not being told.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    pub fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    fn next_id(&mut self) -> u32 {
        // Zero is the specification's "no notification", so it is never an id.
        self.last_id = self.last_id.checked_add(1).unwrap_or(1);
        self.last_id
    }

    fn position_of(&self, id: u32) -> Option<usize> {
        self.live.iter().position(|entry| entry.id == id)
    }

    fn remember(&mut self, entry: Notification) {
        self.history.insert(0, entry);
        self.history.truncate(MAX_HISTORY);
    }

    /// Accepts one `Notify` call and returns the id it is known by.
    ///
    /// A `replaces_id` that names a live notification keeps that id and its
    /// place in the stack, which is what makes a progress or "now playing"
    /// update one notification instead of a stream of them. A `replaces_id`
    /// that names nothing live is treated as a new notification rather than
    /// refused: the producer's own record is simply older than the session's.
    pub fn post(&mut self, incoming: &Incoming, now_ms: u64) -> u32 {
        let urgency = incoming.urgency;
        let replaced = (incoming.replaces_id != 0)
            .then(|| self.position_of(incoming.replaces_id))
            .flatten();
        let id = match replaced {
            Some(position) => self.live[position].id,
            None => self.next_id(),
        };

        let entry = Notification {
            id,
            app_name: bounded(incoming.app_name.trim(), MAX_APP_NAME_CHARS),
            summary: bounded(incoming.summary.trim(), MAX_SUMMARY_CHARS),
            body: bounded(incoming.body.trim(), MAX_BODY_CHARS),
            image: incoming
                .image
                .as_deref()
                .and_then(Image::read)
                .or_else(|| Image::read(&incoming.app_icon)),
            actions: read_actions(&incoming.actions),
            urgency,
            posted_ms: now_ms,
            expires_at_ms: deadline(incoming, urgency, now_ms),
            // A replacement is news again: a person who already looked has not
            // seen what it just became.
            read: false,
        };

        match replaced {
            Some(position) => self.live[position] = entry,
            None => {
                self.live.push(entry);
                // The oldest goes to history rather than vanishing, and a
                // critical one is never pushed out by a routine one.
                while self.live.len() > MAX_VISIBLE {
                    let position = self
                        .live
                        .iter()
                        .position(|entry| entry.urgency != Urgency::Critical)
                        .unwrap_or(0);
                    let dropped = self.live.remove(position);
                    self.remember(dropped);
                }
            }
        }
        id
    }

    /// Ends one notification, if it is live.
    pub fn close(&mut self, id: u32, reason: CloseReason) -> Option<Closed> {
        let position = self.position_of(id)?;
        let mut entry = self.live.remove(position);
        // Ending it by hand is the person having dealt with it; a timeout is
        // not, which is what keeps the unread count honest. Having looked at
        // something does not un-happen when it later times out, so this only
        // ever adds to what is known — it never resurrects a read entry.
        entry.read = entry.read || reason == CloseReason::Dismissed;
        self.remember(entry);
        Some(Closed { id, reason })
    }

    /// Ends everything whose deadline has passed, oldest first.
    pub fn expire(&mut self, now_ms: u64) -> Vec<Closed> {
        let due: Vec<u32> = self
            .live
            .iter()
            .filter(|entry| entry.expires_at_ms.is_some_and(|at| at <= now_ms))
            .map(|entry| entry.id)
            .collect();

        due.into_iter()
            .filter_map(|id| self.close(id, CloseReason::Expired))
            .collect()
    }

    /// Confirms that this action belongs to this notification.
    ///
    /// A key the producer never offered is refused: invoking one would send an
    /// application a message it has no handler for, on behalf of a person who
    /// never pressed anything.
    #[must_use]
    pub fn accepts_action(&self, id: u32, key: &str) -> bool {
        self.position_of(id)
            .is_some_and(|position| self.live[position].offers(key))
    }

    /// Marks everything currently shown as seen.
    pub fn mark_read(&mut self) {
        for entry in &mut self.live {
            entry.read = true;
        }
    }

    /// What a surface should show, newest last.
    #[must_use]
    pub fn live(&self) -> &[Notification] {
        &self.live
    }

    /// What a surface may show as toasts right now. While quiet, only critical
    /// notifications interrupt; everything else waits in history.
    #[must_use]
    pub fn toasts(&self) -> Vec<&Notification> {
        self.live
            .iter()
            .filter(|entry| !self.quiet || entry.urgency == Urgency::Critical)
            .collect()
    }

    /// What has already ended, newest first.
    #[must_use]
    pub fn history(&self) -> &[Notification] {
        &self.history
    }

    /// How many the person has not seen — the panel's indicator.
    #[must_use]
    pub fn unread(&self) -> usize {
        self.live.iter().filter(|entry| !entry.read).count()
            + self.history.iter().filter(|entry| !entry.read).count()
    }

    /// Forgets what has ended. Live notifications are untouched: clearing a
    /// list is not dismissing what is still on screen.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incoming(summary: &str) -> Incoming {
        Incoming {
            app_name: "Magnetita".to_owned(),
            summary: summary.to_owned(),
            expire_timeout: -1,
            ..Incoming::default()
        }
    }

    #[test]
    fn an_id_is_never_zero_and_never_reused() {
        let mut server = Notifications::new();
        let first = server.post(&incoming("one"), 0);
        let second = server.post(&incoming("two"), 0);

        assert_ne!(first, 0);
        assert_ne!(first, second);

        server.close(first, CloseReason::Dismissed);
        assert_ne!(server.post(&incoming("three"), 0), first);
    }

    #[test]
    fn a_replacement_keeps_the_id_and_the_place() {
        let mut server = Notifications::new();
        let first = server.post(&incoming("first"), 0);
        let second = server.post(&incoming("second"), 0);

        let update = Incoming {
            replaces_id: first,
            ..incoming("first, updated")
        };
        assert_eq!(server.post(&update, 10), first);

        assert_eq!(server.live().len(), 2);
        assert_eq!(server.live()[0].id, first);
        assert_eq!(server.live()[0].summary, "first, updated");
        assert_eq!(server.live()[1].id, second);
    }

    #[test]
    fn replacing_something_that_is_gone_is_a_new_notification() {
        let mut server = Notifications::new();
        let first = server.post(&incoming("gone"), 0);
        server.close(first, CloseReason::Requested);

        let update = Incoming {
            replaces_id: first,
            ..incoming("late update")
        };
        let id = server.post(&update, 10);
        assert_ne!(id, first);
        assert_eq!(server.live().len(), 1);
    }

    #[test]
    fn the_default_timeout_comes_from_urgency() {
        let mut server = Notifications::new();
        let low = server.post(
            &Incoming {
                urgency: Urgency::Low,
                ..incoming("low")
            },
            1_000,
        );
        let critical = server.post(
            &Incoming {
                urgency: Urgency::Critical,
                ..incoming("critical")
            },
            1_000,
        );

        assert_eq!(server.live()[0].expires_at_ms, Some(1_000 + DEFAULT_LOW_MS));
        // Critical is ended by a person or by its producer, never by a timer.
        assert_eq!(server.live()[1].expires_at_ms, None);

        let closed = server.expire(1_000 + DEFAULT_LOW_MS);
        assert_eq!(
            closed,
            vec![Closed {
                id: low,
                reason: CloseReason::Expired
            }]
        );
        assert_eq!(server.live().len(), 1);
        assert_eq!(server.live()[0].id, critical);
    }

    #[test]
    fn a_producer_may_ask_to_stay_but_not_forever() {
        let mut server = Notifications::new();
        server.post(
            &Incoming {
                expire_timeout: 0,
                ..incoming("until dismissed")
            },
            0,
        );
        server.post(
            &Incoming {
                expire_timeout: i32::MAX,
                ..incoming("greedy")
            },
            0,
        );

        assert_eq!(server.live()[0].expires_at_ms, None);
        assert_eq!(server.live()[1].expires_at_ms, Some(MAX_TIMEOUT_MS));
    }

    #[test]
    fn hostile_text_is_bounded_before_anything_can_show_it() {
        let mut server = Notifications::new();
        server.post(
            &Incoming {
                app_name: "a".repeat(500),
                summary: "s".repeat(500),
                body: "b".repeat(5_000),
                ..incoming("ignored")
            },
            0,
        );

        let entry = &server.live()[0];
        assert_eq!(entry.app_name.chars().count(), MAX_APP_NAME_CHARS);
        assert_eq!(entry.summary.chars().count(), MAX_SUMMARY_CHARS);
        assert_eq!(entry.body.chars().count(), MAX_BODY_CHARS);

        // The bound the host applies is in UTF-16 code units, so a body of
        // astral characters is bounded by those units and not by how few
        // characters they happen to be. This is what a row field may carry, so
        // a body can never be the field that costs the frame.
        server.post(
            &Incoming {
                body: "😀".repeat(5_000),
                ..incoming("emoji")
            },
            0,
        );
        // Live entries keep the order they arrived in, so the emoji body is the
        // second of the two.
        let entry = &server.live()[1];
        assert!(entry.body.encode_utf16().count() <= MAX_BODY_CHARS);
        assert_eq!(entry.body.chars().count(), MAX_BODY_CHARS / 2);
    }

    #[test]
    fn actions_arrive_in_pairs_and_are_capped() {
        let mut server = Notifications::new();
        let mut flat: Vec<String> = Vec::new();
        for index in 0..10 {
            flat.push(format!("key{index}"));
            flat.push(format!("Label {index}"));
        }
        // A trailing key with no label is not a button.
        flat.push("orphan".to_owned());

        server.post(
            &Incoming {
                actions: flat,
                ..incoming("with actions")
            },
            0,
        );

        let entry = &server.live()[0];
        assert_eq!(entry.actions.len(), MAX_ACTIONS);
        assert_eq!(entry.actions[0].key, "key0");
        assert!(!entry.offers("orphan"));
    }

    #[test]
    fn an_action_nobody_offered_is_never_accepted() {
        let mut server = Notifications::new();
        let id = server.post(
            &Incoming {
                actions: vec!["open".to_owned(), "Open".to_owned()],
                ..incoming("with one action")
            },
            0,
        );

        assert!(server.accepts_action(id, "open"));
        assert!(!server.accepts_action(id, "delete-everything"));
        // Nor on a notification that is no longer live.
        server.close(id, CloseReason::Dismissed);
        assert!(!server.accepts_action(id, "open"));
    }

    #[test]
    fn an_image_reference_is_checked_rather_than_believed() {
        assert_eq!(
            Image::read("dialog-information"),
            Some(Image::Name("dialog-information".to_owned()))
        );
        assert_eq!(
            Image::read("/usr/share/icons/phone.png"),
            Some(Image::Path("/usr/share/icons/phone.png".to_owned()))
        );
        assert_eq!(
            Image::read("file:///tmp/shot.png"),
            Some(Image::Path("/tmp/shot.png".to_owned()))
        );
        // A relative path would be resolved against the server's directory,
        // which is not where the producer meant; traversal is never a name.
        assert_eq!(Image::read("../../etc/shadow"), None);
        assert_eq!(Image::read("icons/phone.png"), None);
        assert_eq!(Image::read("file://relative"), None);
        assert_eq!(Image::read(""), None);
        assert_eq!(Image::read(&"n".repeat(MAX_ICON_CHARS + 1)), None);
    }

    #[test]
    fn the_stack_is_capped_and_the_oldest_is_remembered() {
        let mut server = Notifications::new();
        for index in 0..MAX_VISIBLE + 2 {
            server.post(&incoming(&format!("notice {index}")), index as u64);
        }

        assert_eq!(server.live().len(), MAX_VISIBLE);
        assert_eq!(server.history().len(), 2);
        assert_eq!(server.history()[0].summary, "notice 1");
        assert_eq!(server.live()[0].summary, "notice 2");
    }

    #[test]
    fn a_routine_notification_never_pushes_out_a_critical_one() {
        let mut server = Notifications::new();
        let critical = server.post(
            &Incoming {
                urgency: Urgency::Critical,
                ..incoming("battery critical")
            },
            0,
        );
        for index in 0..MAX_VISIBLE + 3 {
            server.post(&incoming(&format!("chatter {index}")), index as u64);
        }

        assert!(server.live().iter().any(|entry| entry.id == critical));
    }

    #[test]
    fn history_is_capped_too() {
        let mut server = Notifications::new();
        for index in 0..MAX_HISTORY + 10 {
            let id = server.post(&incoming(&format!("notice {index}")), index as u64);
            server.close(id, CloseReason::Expired);
        }

        assert_eq!(server.history().len(), MAX_HISTORY);
        // Newest first.
        assert_eq!(
            server.history()[0].summary,
            format!("notice {}", MAX_HISTORY + 9)
        );
    }

    #[test]
    fn quiet_withholds_the_interruption_but_not_the_record() {
        let mut server = Notifications::new();
        server.set_quiet(true);
        server.post(&incoming("routine"), 0);
        server.post(
            &Incoming {
                urgency: Urgency::Critical,
                ..incoming("critical")
            },
            0,
        );

        // Only the critical one interrupts; both are still held and counted.
        assert_eq!(server.toasts().len(), 1);
        assert_eq!(server.toasts()[0].summary, "critical");
        assert_eq!(server.live().len(), 2);
        assert_eq!(server.unread(), 2);
    }

    #[test]
    fn what_counts_as_unread_is_what_nobody_looked_at() {
        let mut server = Notifications::new();
        let seen = server.post(&incoming("seen"), 0);
        server.mark_read();
        assert_eq!(server.unread(), 0);

        server.post(&incoming("new"), 10);
        assert_eq!(server.unread(), 1);

        // Timing out is not having dealt with it; dismissing it is.
        server.expire(u64::MAX);
        assert_eq!(server.unread(), 1);

        let dismissed = server.post(&incoming("dismissed"), 20);
        server.close(dismissed, CloseReason::Dismissed);
        assert_eq!(server.unread(), 1);
        assert_eq!(seen, 1);
    }

    #[test]
    fn a_replacement_is_news_again() {
        let mut server = Notifications::new();
        let id = server.post(&incoming("first"), 0);
        server.mark_read();
        assert_eq!(server.unread(), 0);

        server.post(
            &Incoming {
                replaces_id: id,
                ..incoming("changed")
            },
            10,
        );
        assert_eq!(server.unread(), 1);
    }

    #[test]
    fn clearing_history_leaves_what_is_still_on_screen() {
        let mut server = Notifications::new();
        let live = server.post(&incoming("live"), 0);
        let gone = server.post(&incoming("gone"), 0);
        server.close(gone, CloseReason::Expired);

        server.clear_history();
        assert!(server.history().is_empty());
        assert_eq!(server.live().len(), 1);
        assert_eq!(server.live()[0].id, live);
    }

    #[test]
    fn the_server_promises_only_what_it_does() {
        // Claiming markup would invite producers to send it and have it shown
        // raw; claiming persistence would promise a history that survives.
        assert!(capabilities().contains(&"actions"));
        assert!(!capabilities().contains(&"body-markup"));
        assert!(!capabilities().contains(&"persistence"));
    }

    #[test]
    fn magnetitas_call_shape_is_served_as_it_is_sent() {
        // magnetitad posts with no actions, no hints, the `phone` icon name and
        // the server's default timeout, then withdraws by id.
        let mut server = Notifications::new();
        let id = server.post(
            &Incoming {
                app_name: "Magnetita".to_owned(),
                replaces_id: 0,
                app_icon: "phone".to_owned(),
                summary: "Pixel".to_owned(),
                body: "A message arrived".to_owned(),
                actions: Vec::new(),
                urgency: Urgency::Normal,
                image: None,
                expire_timeout: -1,
            },
            0,
        );

        assert_eq!(
            server.live()[0].image,
            Some(Image::Name("phone".to_owned()))
        );
        assert_eq!(server.live()[0].expires_at_ms, Some(DEFAULT_NORMAL_MS));

        let update = Incoming {
            replaces_id: id,
            summary: "Pixel".to_owned(),
            body: "Two messages arrived".to_owned(),
            app_icon: "phone".to_owned(),
            expire_timeout: -1,
            ..Incoming::default()
        };
        assert_eq!(server.post(&update, 100), id);
        assert_eq!(server.live().len(), 1);

        assert_eq!(
            server.close(id, CloseReason::Requested),
            Some(Closed {
                id,
                reason: CloseReason::Requested
            })
        );
        assert!(server.live().is_empty());
    }
}
