//! IME text input through `zwp_text_input_v3` (text-input-unstable-v3):
//! enable and disable follow the surfaces that asked for it and the
//! text-input focus, the caret rectangle is sent with `set_cursor_rectangle`,
//! and `preedit_string`/`commit_string`/`done` become
//! `InputEvent::{Preedit, CommitText}` for the focused surface.
//!
//! The protocol is double-buffered: requests apply on `commit`, whose count
//! is the serial the compositor echoes in `done`; events apply on `done`.
//! [`TextInputState`] keeps that bookkeeping pure; [`TextInput`] owns the
//! protocol objects. Without the manager global the feature is off.
#![forbid(unsafe_code)]

use smithay_client_toolkit::dispatch2::Dispatch2;
use wayland_client::{
    globals::GlobalList, protocol::wl_seat::WlSeat, Connection, Dispatch, QueueHandle,
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{self, ZwpTextInputV3},
};

use crate::{
    geometry::{to_pixel_rects, PixelRect, Rect},
    input::{InputEvent, SeatRequest},
    runtime::SurfaceId,
    surface::NoEvents,
};

/// The longest preedit or commit text taken from the compositor, in bytes;
/// longer text is cut at a character boundary. A field holds far less.
pub(crate) const MAX_IME_BYTES: usize = 1024;

/// The serial and double-buffer bookkeeping of one `zwp_text_input_v3`.
#[derive(Debug, Default)]
pub(crate) struct TextInputState {
    /// `commit` requests issued so far (the serial `done` echoes).
    commits: u32,
    /// The commit count right after the commit that carried the current
    /// `enable`; `None` while disabled.
    enabled_at: Option<u32>,
    pending_preedit: Option<(String, usize)>,
    pending_commit: Option<String>,
    /// A non-empty composition is shown by the surface.
    showing_preedit: bool,
}

impl TextInputState {
    /// Records one `commit` request.
    pub(crate) fn committed(&mut self) {
        self.commits = self.commits.wrapping_add(1);
    }

    /// The `commit` that carried an `enable` was just recorded: events from
    /// before it belong to the previous state and are dropped.
    pub(crate) fn enabled(&mut self) {
        self.enabled_at = Some(self.commits);
        self.pending_preedit = None;
        self.pending_commit = None;
    }

    /// Text input went off (a committed `disable`, a `leave`, the object
    /// gone): pending text is dropped and a shown composition is cleared.
    pub(crate) fn disabled(&mut self) -> Option<InputEvent> {
        self.enabled_at = None;
        self.pending_preedit = None;
        self.pending_commit = None;
        self.clear_preedit()
    }

    /// `preedit_string`: bounded; a caret outside the text (or hidden, -1)
    /// goes to its end.
    pub(crate) fn preedit_string(&mut self, text: Option<String>, cursor_begin: i32) {
        let text = bounded(text.unwrap_or_default());
        let cursor = usize::try_from(cursor_begin)
            .ok()
            .filter(|&c| c <= text.len())
            .unwrap_or(text.len());
        self.pending_preedit = Some((text, cursor));
    }

    /// `commit_string`, bounded.
    pub(crate) fn commit_string(&mut self, text: Option<String>) {
        self.pending_commit = Some(bounded(text.unwrap_or_default()));
    }

    /// `done(serial)`: replaces the current state with the pending one and
    /// returns what the surface applies, in the protocol's order (the old
    /// composition goes, the commit is inserted, the new composition is
    /// shown). A `done` whose serial predates the current `enable`, or one
    /// while disabled, carries text for an earlier state and applies
    /// nothing. A later serial that lags behind our own commits still
    /// applies, as the protocol requires; it only means the compositor has
    /// not seen our newest state yet.
    pub(crate) fn done(&mut self, serial: u32) -> Vec<InputEvent> {
        let preedit = self.pending_preedit.take();
        let commit = self.pending_commit.take();
        let current = self
            .enabled_at
            .is_some_and(|at| serial.wrapping_sub(at) <= self.commits.wrapping_sub(at));
        if !current {
            tracing::debug!(
                serial,
                commits = self.commits,
                "text input: stale done dropped"
            );
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Some(text) = commit.filter(|t| !t.is_empty()) {
            out.push(InputEvent::CommitText(text));
            self.showing_preedit = false;
        }
        match preedit.filter(|(t, _)| !t.is_empty()) {
            Some((text, cursor)) => {
                out.push(InputEvent::Preedit { text, cursor });
                self.showing_preedit = true;
            }
            None => out.extend(self.clear_preedit()),
        }
        out
    }

    /// Whether the compositor has seen every commit we sent.
    pub(crate) fn in_sync(&self, serial: u32) -> bool {
        serial == self.commits
    }

    fn clear_preedit(&mut self) -> Option<InputEvent> {
        std::mem::take(&mut self.showing_preedit).then(|| InputEvent::Preedit {
            text: String::new(),
            cursor: 0,
        })
    }
}

/// `text` cut to [`MAX_IME_BYTES`] at a character boundary.
fn bounded(mut text: String) -> String {
    if text.len() > MAX_IME_BYTES {
        let mut end = MAX_IME_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

/// User data of the `zwp_text_input_v3` object.
pub(crate) struct TextInputData;

/// The runtime side of the text-input events.
pub(crate) trait TextInputSink {
    fn text_input_event(&mut self, event: zwp_text_input_v3::Event);
}

impl<D: TextInputSink> Dispatch2<ZwpTextInputV3, D> for TextInputData {
    fn event(
        &self,
        runtime: &mut D,
        _: &ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _: &Connection,
        _: &QueueHandle<D>,
    ) {
        runtime.text_input_event(event);
    }
}

/// A surface that asked for text input, with its caret rectangle.
#[derive(Debug)]
struct Wanted {
    surface: SurfaceId,
    rect: Option<PixelRect>,
}

/// The seat's text input: the manager (if the compositor has one), the
/// per-seat object and which surface it is enabled on.
#[derive(Default)]
pub(crate) struct TextInput {
    manager: Option<ZwpTextInputManagerV3>,
    object: Option<ZwpTextInputV3>,
    /// The surface of the last `enter` (the text-input focus).
    focus: Option<SurfaceId>,
    /// The surface a committed `enable` is current for.
    enabled_on: Option<SurfaceId>,
    /// The caret rectangle last committed for `enabled_on`.
    sent_rect: Option<PixelRect>,
    wanted: Vec<Wanted>,
    state: TextInputState,
}

impl TextInput {
    /// Binds the manager; without it text input stays off.
    pub(crate) fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Self
    where
        D: Dispatch<ZwpTextInputManagerV3, NoEvents> + 'static,
    {
        let manager = globals
            .bind::<ZwpTextInputManagerV3, _, _>(qh, 1..=1, NoEvents)
            .inspect_err(
                |err| tracing::info!(%err, "no zwp_text_input_manager_v3: IME input is off"),
            )
            .ok();
        Self {
            manager,
            ..Self::default()
        }
    }

    /// The seat gained a keyboard: one text-input object for it.
    pub(crate) fn seat_added<D>(&mut self, seat: &WlSeat, qh: &QueueHandle<D>)
    where
        D: Dispatch<ZwpTextInputV3, TextInputData> + 'static,
    {
        if let (Some(manager), None) = (&self.manager, &self.object) {
            self.object = Some(manager.get_text_input(seat, qh, TextInputData));
        }
    }

    /// The keyboard went away: the object is destroyed (which disables it);
    /// a shown composition is cleared.
    pub(crate) fn seat_removed(&mut self) -> Vec<(SurfaceId, InputEvent)> {
        let mut out = Vec::new();
        if let Some(object) = self.object.take() {
            object.destroy();
            out.extend(self.off());
        }
        self.focus = None;
        out
    }

    /// Records a shell's request for surface `id`.
    pub(crate) fn request(&mut self, id: SurfaceId, request: SeatRequest) {
        let pos = self.wanted.iter().position(|w| w.surface == id);
        match (request, pos) {
            (SeatRequest::TextInput(true), None) => self.wanted.push(Wanted {
                surface: id,
                rect: None,
            }),
            (SeatRequest::TextInput(false), Some(i)) => {
                self.wanted.remove(i);
            }
            (SeatRequest::CursorRect(rect), Some(i)) => {
                let caret = Rect {
                    w: rect.w.max(1.0),
                    h: rect.h.max(1.0),
                    ..rect
                };
                self.wanted[i].rect = to_pixel_rects(&[caret]).first().copied();
            }
            _ => {}
        }
    }

    /// Surface `id` is gone.
    pub(crate) fn forget(&mut self, id: SurfaceId) {
        self.wanted.retain(|w| w.surface != id);
        if self.focus == Some(id) {
            self.focus = None;
        }
    }

    /// Sends what changed since the last call: `enable` with the caret on
    /// the focused surface that wants text input, `disable` when none does,
    /// or the moved caret. Returns the events turning text input off
    /// produced (a cleared composition).
    pub(crate) fn sync(&mut self) -> Vec<(SurfaceId, InputEvent)> {
        let Some(object) = self.object.clone() else {
            return Vec::new();
        };
        let desired = self
            .focus
            .and_then(|f| self.wanted.iter().find(|w| w.surface == f));
        let rect = desired.and_then(|w| w.rect);
        let desired = desired.map(|w| w.surface);
        let mut out = Vec::new();
        if desired != self.enabled_on {
            if desired.is_none() {
                object.disable();
                object.commit();
                self.state.committed();
                out.extend(self.off());
                return out;
            }
            // The protocol wants the old input disabled before another is
            // enabled; `enable` itself resets every earlier state.
            out.extend(self.off());
            object.enable();
            if let Some(r) = rect {
                object.set_cursor_rectangle(r.x, r.y, r.w, r.h);
            }
            object.commit();
            self.state.committed();
            self.state.enabled();
            self.enabled_on = desired;
            self.sent_rect = rect;
            tracing::debug!(surface = ?desired, "text input enabled");
        } else if desired.is_some() && rect != self.sent_rect {
            if let Some(r) = rect {
                object.set_cursor_rectangle(r.x, r.y, r.w, r.h);
                object.commit();
                self.state.committed();
            }
            self.sent_rect = rect;
        }
        out
    }

    /// A text-input event; `surface` is the live surface an `enter` or
    /// `leave` names. Returns the events for the shells.
    pub(crate) fn event(
        &mut self,
        event: zwp_text_input_v3::Event,
        surface: Option<SurfaceId>,
    ) -> Vec<(SurfaceId, InputEvent)> {
        use zwp_text_input_v3::Event;
        match event {
            Event::Enter { .. } => {
                // After an enter nothing is enabled: `sync` re-enables.
                let out = self.off();
                self.focus = surface;
                out
            }
            Event::Leave { .. } => {
                let out = self.off();
                self.focus = None;
                out
            }
            Event::PreeditString {
                text, cursor_begin, ..
            } => {
                self.state.preedit_string(text, cursor_begin);
                Vec::new()
            }
            Event::CommitString { text } => {
                self.state.commit_string(text);
                Vec::new()
            }
            Event::Done { serial } => {
                if !self.state.in_sync(serial) {
                    tracing::trace!(serial, "text input: done before our latest commit");
                }
                self.done(serial)
            }
            // No surrounding text is ever sent, so there is nothing the
            // input method could ask to delete.
            other => {
                tracing::debug!(event = ?other, "text input: event ignored");
                Vec::new()
            }
        }
    }

    /// `done`: the text goes to the surface text input is enabled on.
    fn done(&mut self, serial: u32) -> Vec<(SurfaceId, InputEvent)> {
        match self.enabled_on {
            Some(id) => self
                .state
                .done(serial)
                .into_iter()
                .map(|ev| (id, ev))
                .collect(),
            None => {
                self.state.done(serial);
                Vec::new()
            }
        }
    }

    /// Destroys the object and the manager at exit.
    pub(crate) fn destroy(&mut self) {
        if let Some(object) = self.object.take() {
            object.destroy();
        }
        if let Some(manager) = self.manager.take() {
            manager.destroy();
        }
    }

    /// Text input is no longer enabled anywhere.
    fn off(&mut self) -> Vec<(SurfaceId, InputEvent)> {
        let was = self.enabled_on.take();
        self.sent_rect = None;
        let cleared = self.state.disabled();
        was.zip(cleared).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preedit(text: &str, cursor: usize) -> InputEvent {
        InputEvent::Preedit {
            text: text.into(),
            cursor,
        }
    }

    fn cleared() -> InputEvent {
        preedit("", 0)
    }

    /// A state with one committed `enable` (serial 1).
    fn enabled() -> TextInputState {
        let mut s = TextInputState::default();
        s.committed();
        s.enabled();
        s
    }

    #[test]
    fn done_applies_the_commit_then_the_preedit_and_resets_the_pending_state() {
        let mut s = enabled();
        s.preedit_string(Some("\u{4f60}".into()), 3);
        assert_eq!(s.done(1), vec![preedit("\u{4f60}", 3)]);
        s.commit_string(Some("\u{4f60}\u{597d}".into()));
        s.preedit_string(Some("x".into()), 1);
        assert_eq!(
            s.done(1),
            vec![
                InputEvent::CommitText("\u{4f60}\u{597d}".into()),
                preedit("x", 1)
            ]
        );
        // Nothing pending: the next done clears the shown composition once.
        assert_eq!(s.done(1), vec![cleared()]);
        assert_eq!(s.done(1), Vec::new());
    }

    #[test]
    fn a_commit_alone_replaces_the_composition_without_a_separate_clear() {
        let mut s = enabled();
        s.preedit_string(Some("ni".into()), 2);
        s.done(1);
        s.commit_string(Some("\u{4f60}".into()));
        assert_eq!(s.done(1), vec![InputEvent::CommitText("\u{4f60}".into())]);
        assert_eq!(s.done(1), Vec::new());
    }

    #[test]
    fn a_done_from_before_the_current_enable_applies_nothing() {
        let mut s = enabled();
        // A caret move (serial 2), then a second enable for another field
        // (serial 3): text the compositor sent for serials 1 and 2 belonged
        // to the earlier state.
        s.committed();
        s.committed();
        s.enabled();
        s.commit_string(Some("old".into()));
        assert_eq!(s.done(2), Vec::new());
        // The pending text went with it.
        assert_eq!(s.done(3), Vec::new());
        s.commit_string(Some("new".into()));
        assert_eq!(s.done(3), vec![InputEvent::CommitText("new".into())]);
    }

    #[test]
    fn a_lagging_serial_inside_the_current_enable_still_applies() {
        let mut s = enabled();
        // Two caret moves the compositor has not answered yet.
        s.committed();
        s.committed();
        assert!(!s.in_sync(1));
        assert!(s.in_sync(3));
        s.commit_string(Some("a".into()));
        assert_eq!(s.done(1), vec![InputEvent::CommitText("a".into())]);
        // A serial from the future (a confused compositor) does not.
        s.commit_string(Some("b".into()));
        assert_eq!(s.done(9), Vec::new());
    }

    #[test]
    fn nothing_applies_while_disabled_and_disabling_clears_a_composition() {
        let mut s = TextInputState::default();
        s.commit_string(Some("x".into()));
        assert_eq!(s.done(0), Vec::new());
        let mut s = enabled();
        s.preedit_string(Some("ka".into()), 2);
        s.done(1);
        s.preedit_string(Some("kan".into()), 3);
        assert_eq!(s.disabled(), Some(cleared()));
        assert_eq!(s.disabled(), None);
        assert_eq!(s.done(1), Vec::new());
    }

    #[test]
    fn serials_wrap() {
        let mut s = TextInputState {
            commits: u32::MAX,
            ..TextInputState::default()
        };
        s.committed();
        s.enabled();
        s.commit_string(Some("w".into()));
        assert_eq!(s.done(0), vec![InputEvent::CommitText("w".into())]);
        s.commit_string(Some("v".into()));
        assert_eq!(s.done(u32::MAX), Vec::new());
    }

    #[test]
    fn hostile_text_is_bounded_and_the_caret_clamped() {
        let mut s = enabled();
        // 1500 three-byte characters: cut to whole characters within 1024.
        s.commit_string(Some("\u{4f60}".repeat(1500)));
        s.preedit_string(Some("abc".into()), -1);
        let out = s.done(1);
        let InputEvent::CommitText(commit) = &out[0] else {
            panic!("{out:?}");
        };
        assert_eq!(commit.len(), 1023);
        assert_eq!(out[1], preedit("abc", 3));
        s.preedit_string(Some("abc".into()), 99);
        assert_eq!(s.done(1), vec![preedit("abc", 3)]);
        s.preedit_string(None, 0);
        s.commit_string(None);
        assert_eq!(s.done(1), vec![cleared()]);
    }
}
