//! The handshake between a playback session and the surface that renders it.
//!
//! A video surface builds its render context from the backend's handle on the
//! toolkit's render thread, where nothing on the host's side can reach it. So
//! a session may only be destroyed once its surface has confirmed that the
//! context is gone, and a handle may only be handed to a surface while the
//! session it belongs to is still the current one. Both hosts — Fluorita's
//! player and Siderita's embedded modal — obey exactly this protocol, which
//! is why it has one owner here, with no toolkit in it: the hosts only carry
//! out the steps it returns.
//!
//! The rules it keeps:
//!
//! - **Closing is decided before the handle is cleared.** A surface that holds
//!   nothing answers at once, and an answer that arrived before the host knew
//!   it was closing used to be ignored and leave the host waiting for ever.
//! - **Every close moves the generation.** A worker publishes its handle
//!   asynchronously; one that lands after the close of its own session carries
//!   an older generation and is refused, so a freed backend instance is never
//!   handed to a surface.
//! - **An acknowledgement is taken once.** A release that arrives when nothing
//!   is closing — repeated, or late — changes nothing, so it cannot stop the
//!   worker of whatever session started in the meantime.
//! - **What was asked for while closing is started, not stranded.** One open
//!   waits for the surface; a newer one replaces it; an explicit close drops it.

/// What the host must do to open an item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenStep<T> {
    /// No surface holds anything from a session: stop whatever worker is
    /// left, then start `item` as session `generation`.
    Begin { item: T, generation: u64 },
    /// A surface renders from the current session: clear its handle. The item
    /// starts when [`SurfaceHandshake::released`] says so.
    ClearHandle,
    /// A close is already waiting for the surface; the item starts after it.
    Wait,
}

/// What the host must do to close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseStep {
    /// Nothing is open, or a close is already waiting.
    Nothing,
    /// No surface ever held this session's handle: stop its worker now.
    StopNow,
    /// Clear the surface's handle and wait for it to release its context.
    ClearHandle,
}

/// What the host must do when the surface reports its context released.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseStep<T> {
    /// Nothing was closing: a repeated or late acknowledgement.
    Ignored,
    /// The surface let go: stop the worker, and then begin `next` — an open
    /// that waited for this — as the session it names.
    Stop { next: Option<(T, u64)> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    /// No session.
    Idle,
    /// A session is current; `published` once a surface was handed its handle.
    Live { published: bool },
    /// The handle was cleared and the surface has not confirmed it let go.
    Closing,
}

/// The protocol's state, for one host. `T` is what an open names — a path.
#[derive(Clone, Debug)]
pub struct SurfaceHandshake<T> {
    generation: u64,
    phase: Phase,
    parked: Option<T>,
}

impl<T> Default for SurfaceHandshake<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SurfaceHandshake<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            generation: 0,
            phase: Phase::Idle,
            parked: None,
        }
    }

    /// The current session's generation. A worker started for an older one
    /// is stale.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether a close is waiting for the surface.
    #[must_use]
    pub const fn is_closing(&self) -> bool {
        matches!(self.phase, Phase::Closing)
    }

    /// Whether a session is current.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self.phase, Phase::Live { .. })
    }

    /// Asks to open `item`, replacing whatever is open.
    pub fn open(&mut self, item: T) -> OpenStep<T> {
        match self.phase {
            Phase::Closing => {
                // The newest request is the one the person wants.
                self.parked = Some(item);
                OpenStep::Wait
            }
            Phase::Live { published: true } => {
                self.parked = Some(item);
                self.begin_close();
                OpenStep::ClearHandle
            }
            Phase::Idle | Phase::Live { published: false } => {
                let generation = self.next_session();
                OpenStep::Begin { item, generation }
            }
        }
    }

    /// Asks to close. An open that was waiting for a close in flight is
    /// dropped: closing is what was asked for last.
    pub fn close(&mut self) -> CloseStep {
        self.parked = None;
        match self.phase {
            Phase::Idle | Phase::Closing => CloseStep::Nothing,
            Phase::Live { published: true } => {
                self.begin_close();
                CloseStep::ClearHandle
            }
            Phase::Live { published: false } => {
                // Moved as well: the worker may still publish a handle for
                // the session being stopped.
                self.generation = self.generation.wrapping_add(1);
                self.phase = Phase::Idle;
                CloseStep::StopNow
            }
        }
    }

    /// The surface reports its context released.
    pub fn released(&mut self) -> ReleaseStep<T> {
        if !self.is_closing() {
            return ReleaseStep::Ignored;
        }
        self.phase = Phase::Idle;
        let next = self.parked.take().map(|item| {
            let generation = self.next_session();
            (item, generation)
        });
        ReleaseStep::Stop { next }
    }

    /// A worker asks to hand its handle to the surface. Granted only to the
    /// current session, and only while nothing is closing it.
    pub fn publishes(&mut self, generation: u64) -> bool {
        match self.phase {
            Phase::Live { .. } if generation == self.generation => {
                self.phase = Phase::Live { published: true };
                true
            }
            _ => false,
        }
    }

    fn begin_close(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.phase = Phase::Closing;
    }

    fn next_session(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.phase = Phase::Live { published: false };
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::{CloseStep, OpenStep, ReleaseStep, SurfaceHandshake};

    /// Opens `item` on an idle handshake and returns the session it began.
    fn begun(handshake: &mut SurfaceHandshake<&'static str>, item: &'static str) -> u64 {
        match handshake.open(item) {
            OpenStep::Begin {
                item: begun,
                generation,
            } => {
                assert_eq!(begun, item);
                generation
            }
            other => panic!("expected to begin, got {other:?}"),
        }
    }

    #[test]
    fn a_close_before_the_handle_is_published_refuses_the_late_handle() {
        // The worker queued its handle, and the close ran first: the session
        // is gone, and so is the backend instance the address points into.
        let mut handshake = SurfaceHandshake::new();
        let session = begun(&mut handshake, "a.mkv");

        assert_eq!(handshake.close(), CloseStep::StopNow);
        assert!(!handshake.publishes(session));
    }

    #[test]
    fn a_release_that_arrives_before_the_close_changes_nothing() {
        let mut handshake = SurfaceHandshake::new();
        let session = begun(&mut handshake, "a.mkv");
        assert!(handshake.publishes(session));

        // A stray acknowledgement while the session is live must not stop it.
        assert_eq!(handshake.released(), ReleaseStep::Ignored);
        assert!(handshake.is_live());

        assert_eq!(handshake.close(), CloseStep::ClearHandle);
        // The close is marked before the host clears the handle, so an answer
        // the surface gives from inside that write is the one that counts.
        assert!(handshake.is_closing());
        assert_eq!(handshake.released(), ReleaseStep::Stop { next: None });
        // And only once: a second answer does not reach the next session.
        assert_eq!(handshake.released(), ReleaseStep::Ignored);
    }

    #[test]
    fn opening_over_a_rendered_session_waits_for_the_surface_then_begins() {
        let mut handshake = SurfaceHandshake::new();
        let first = begun(&mut handshake, "a.mkv");
        assert!(handshake.publishes(first));

        assert_eq!(handshake.open("b.mkv"), OpenStep::ClearHandle);
        // The first session's worker may still try to publish; it cannot.
        assert!(!handshake.publishes(first));

        let ReleaseStep::Stop {
            next: Some(("b.mkv", second)),
        } = handshake.released()
        else {
            panic!("the parked open was not started");
        };
        assert_ne!(second, first);
        assert!(handshake.publishes(second));
    }

    #[test]
    fn opens_asked_for_during_a_close_keep_only_the_newest() {
        let mut handshake = SurfaceHandshake::new();
        let session = begun(&mut handshake, "a.mkv");
        assert!(handshake.publishes(session));
        assert_eq!(handshake.close(), CloseStep::ClearHandle);

        assert_eq!(handshake.open("b.mkv"), OpenStep::Wait);
        assert_eq!(handshake.open("c.mkv"), OpenStep::Wait);

        assert!(matches!(
            handshake.released(),
            ReleaseStep::Stop {
                next: Some(("c.mkv", _))
            }
        ));
    }

    #[test]
    fn an_explicit_close_drops_an_open_that_was_waiting() {
        let mut handshake = SurfaceHandshake::new();
        let session = begun(&mut handshake, "a.mkv");
        assert!(handshake.publishes(session));
        assert_eq!(handshake.open("b.mkv"), OpenStep::ClearHandle);

        assert_eq!(handshake.close(), CloseStep::Nothing);

        assert_eq!(handshake.released(), ReleaseStep::Stop { next: None });
    }

    #[test]
    fn a_session_no_surface_ever_held_is_stopped_at_once() {
        // Audio, or a film that failed before its handle was published.
        let mut handshake = SurfaceHandshake::new();
        begun(&mut handshake, "a.flac");

        assert_eq!(handshake.close(), CloseStep::StopNow);
        assert_eq!(handshake.close(), CloseStep::Nothing);
        assert_eq!(handshake.released(), ReleaseStep::Ignored);
    }

    #[test]
    fn replacing_an_unpublished_session_begins_the_next_under_a_new_generation() {
        let mut handshake = SurfaceHandshake::new();
        let first = begun(&mut handshake, "a.mkv");
        let second = begun(&mut handshake, "b.mkv");

        assert_ne!(first, second);
        assert!(!handshake.publishes(first));
        assert!(handshake.publishes(second));
    }

    #[test]
    fn the_generation_wraps_rather_than_overflowing() {
        let mut handshake: SurfaceHandshake<&str> = SurfaceHandshake {
            generation: u64::MAX,
            ..SurfaceHandshake::new()
        };
        let session = begun(&mut handshake, "a.mkv");

        assert_eq!(session, 0);
        assert!(handshake.publishes(session));
        assert!(!handshake.publishes(u64::MAX));
    }
}
