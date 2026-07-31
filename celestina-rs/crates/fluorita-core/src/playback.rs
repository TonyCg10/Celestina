//! Truthful playback state.
//!
//! Two rules decide everything in this module:
//!
//! - **A click is a request, not a state.** Pressing play records a pending
//!   request; the session keeps saying "paused" until the engine reports
//!   otherwise, so a host can show "starting…" instead of a transport that
//!   claims to be playing a file that never opened.
//! - **Every report is stamped with the generation of the selection it belongs
//!   to.** Selecting another track bumps the generation, so a report still in
//!   flight for the previous file is rejected instead of overwriting the new
//!   one's position.

use std::time::Duration;

use celestina_core::{Generation, GenerationClock, GenerationExhausted};

use crate::media::{MediaCapabilities, MediaId, MediaKind};

/// Confirmed playback state — moved only by [`PlaybackSession::apply`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlaybackState {
    /// Nothing selected.
    #[default]
    Idle,
    /// A file is selected and the engine has not reported on it yet.
    Opening,
    Playing,
    Paused,
    /// Playback reached the end of the media.
    Ended,
    /// The engine could not open or continue this file.
    Failed,
}

impl PlaybackState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Failed)
    }
}

/// Something the user asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaybackRequest {
    Play,
    Pause,
    Stop,
    Seek(Duration),
    /// Volume in `0.0..=1.0`; the value is clamped when the request is made.
    SetVolume(f64),
}

/// A request that has been issued but not confirmed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingRequest {
    pub generation: Generation,
    pub request: PlaybackRequest,
}

/// Why a request never became pending.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestRejected {
    /// Nothing is selected, so there is nothing to control.
    NoSelection,
    /// The selected kind cannot do this — seeking an image, for instance.
    Unsupported,
    /// Playback already failed; reopening is a new selection, not a request.
    NotPlayable,
}

/// What the engine says happened.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineReport {
    /// The generation of the selection this report describes.
    pub generation: Generation,
    pub kind: ReportKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReportKind {
    /// Confirmed state transition.
    State(PlaybackState),
    /// The media's total duration, once known.
    Duration(Duration),
    /// Ordinary playback progress.
    Position(Duration),
    /// The engine finished a seek and is now at this position.
    SeekCompleted(Duration),
    /// Confirmed output volume.
    Volume(f64),
    /// The file could not be opened or decoded; the message is for the user.
    Failed(String),
}

/// What [`PlaybackSession::apply`] did with a report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportOutcome {
    Applied,
    /// The report belongs to a selection that is no longer current.
    RejectedStale,
    /// Nothing is selected, so there is nothing the report could describe.
    RejectedIdle,
}

/// One host's view of one playing item.
#[derive(Debug, Default)]
pub struct PlaybackSession {
    clock: GenerationClock,
    selection: Option<Selection>,
    state: PlaybackState,
    position: Option<Duration>,
    duration: Option<Duration>,
    volume: Option<f64>,
    pending_transport: Option<PendingRequest>,
    pending_seek: Option<PendingRequest>,
    pending_volume: Option<PendingRequest>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct Selection {
    generation: Generation,
    media: MediaId,
    kind: MediaKind,
}

impl PlaybackSession {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects a new item: bumps the generation, drops every pending request and
    /// every confirmed value from the previous item, and enters
    /// [`PlaybackState::Opening`].
    pub fn select(
        &mut self,
        media: MediaId,
        kind: MediaKind,
    ) -> Result<Generation, GenerationExhausted> {
        let generation = self.clock.issue()?;
        self.selection = Some(Selection {
            generation,
            media,
            kind,
        });
        self.state = PlaybackState::Opening;
        self.position = None;
        self.duration = None;
        self.volume = None;
        self.pending_transport = None;
        self.pending_seek = None;
        self.pending_volume = None;
        self.error = None;
        Ok(generation)
    }

    /// Closes the session. Later reports for the closed item are rejected.
    pub fn clear(&mut self) {
        self.selection = None;
        self.state = PlaybackState::Idle;
        self.position = None;
        self.duration = None;
        self.volume = None;
        self.pending_transport = None;
        self.pending_seek = None;
        self.pending_volume = None;
        self.error = None;
    }

    /// Records a user action as pending. The confirmed state does not move.
    pub fn request(&mut self, request: PlaybackRequest) -> Result<PendingRequest, RequestRejected> {
        let selection = self
            .selection
            .as_ref()
            .ok_or(RequestRejected::NoSelection)?;
        if matches!(self.state, PlaybackState::Failed) {
            return Err(RequestRejected::NotPlayable);
        }

        let capabilities = selection.kind.capabilities();
        let request = match request {
            PlaybackRequest::Play | PlaybackRequest::Pause if !capabilities.timed => {
                return Err(RequestRejected::Unsupported)
            }
            PlaybackRequest::Seek(_) if !capabilities.seekable => {
                return Err(RequestRejected::Unsupported)
            }
            PlaybackRequest::SetVolume(_) if !capabilities.has_audio => {
                return Err(RequestRejected::Unsupported)
            }
            PlaybackRequest::Seek(target) => {
                PlaybackRequest::Seek(clamp_seek(target, self.duration))
            }
            PlaybackRequest::SetVolume(level) => PlaybackRequest::SetVolume(level.clamp(0.0, 1.0)),
            other => other,
        };

        let pending = PendingRequest {
            generation: selection.generation,
            request,
        };
        match request {
            PlaybackRequest::Seek(_) => self.pending_seek = Some(pending),
            PlaybackRequest::SetVolume(_) => self.pending_volume = Some(pending),
            _ => self.pending_transport = Some(pending),
        }
        Ok(pending)
    }

    /// Applies an engine report, rejecting anything that does not belong to the
    /// current selection.
    pub fn apply(&mut self, report: &EngineReport) -> ReportOutcome {
        let Some(selection) = self.selection.as_ref() else {
            return ReportOutcome::RejectedIdle;
        };
        if report.generation != selection.generation {
            return ReportOutcome::RejectedStale;
        }

        match &report.kind {
            ReportKind::State(state) => {
                self.state = *state;
                self.pending_transport = None;
                if state.is_terminal() {
                    self.pending_seek = None;
                }
            }
            ReportKind::Duration(duration) => self.duration = Some(*duration),
            ReportKind::Position(position) => self.position = Some(*position),
            ReportKind::SeekCompleted(position) => {
                self.position = Some(*position);
                self.pending_seek = None;
            }
            ReportKind::Volume(level) => {
                self.volume = Some(level.clamp(0.0, 1.0));
                self.pending_volume = None;
            }
            ReportKind::Failed(message) => {
                self.state = PlaybackState::Failed;
                self.error = Some(message.clone());
                self.pending_transport = None;
                self.pending_seek = None;
                self.pending_volume = None;
            }
        }
        ReportOutcome::Applied
    }

    /// The generation every request and report for the current item carries.
    #[must_use]
    pub fn generation(&self) -> Option<Generation> {
        self.selection.as_ref().map(|current| current.generation)
    }

    #[must_use]
    pub fn media(&self) -> Option<&MediaId> {
        self.selection.as_ref().map(|current| &current.media)
    }

    #[must_use]
    pub fn kind(&self) -> Option<MediaKind> {
        self.selection.as_ref().map(|current| current.kind)
    }

    /// What the selected kind can offer at all, before the engine narrows it.
    #[must_use]
    pub fn capabilities(&self) -> Option<MediaCapabilities> {
        self.selection
            .as_ref()
            .map(|current| current.kind.capabilities())
    }

    #[must_use]
    pub const fn state(&self) -> PlaybackState {
        self.state
    }

    /// The last reported position — never a requested one.
    #[must_use]
    pub const fn position(&self) -> Option<Duration> {
        self.position
    }

    #[must_use]
    pub const fn duration(&self) -> Option<Duration> {
        self.duration
    }

    #[must_use]
    pub const fn volume(&self) -> Option<f64> {
        self.volume
    }

    #[must_use]
    pub const fn pending_transport(&self) -> Option<PendingRequest> {
        self.pending_transport
    }

    /// The seek in flight, so a host can show "seeking" instead of claiming the
    /// playhead already moved.
    #[must_use]
    pub const fn pending_seek(&self) -> Option<PendingRequest> {
        self.pending_seek
    }

    #[must_use]
    pub const fn pending_volume(&self) -> Option<PendingRequest> {
        self.pending_volume
    }

    #[must_use]
    pub const fn is_seeking(&self) -> bool {
        self.pending_seek.is_some()
    }

    /// The engine's failure message, if it failed.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// A seek beyond a known duration is clamped rather than sent as-is; with no
/// duration reported yet the target is passed through untouched.
fn clamp_seek(target: Duration, duration: Option<Duration>) -> Duration {
    match duration {
        Some(total) if target > total => total,
        _ => target,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EngineReport, PlaybackRequest, PlaybackSession, PlaybackState, ReportKind, ReportOutcome,
        RequestRejected,
    };
    use crate::media::{MediaId, MediaKind};
    use celestina_core::Generation;
    use std::time::Duration;

    fn session(kind: MediaKind) -> (PlaybackSession, Generation) {
        let mut session = PlaybackSession::new();
        let generation = session
            .select(MediaId::filesystem(66, 1), kind)
            .expect("a fresh clock issues a generation");
        (session, generation)
    }

    #[test]
    fn a_request_stays_pending_until_the_engine_confirms() {
        let (mut session, generation) = session(MediaKind::Audio);

        let pending = session
            .request(PlaybackRequest::Play)
            .expect("audio can play");

        assert_eq!(pending.generation, generation);
        assert_eq!(
            session.state(),
            PlaybackState::Opening,
            "a click never moves confirmed state"
        );
        assert!(session.pending_transport().is_some());

        session.apply(&EngineReport {
            generation,
            kind: ReportKind::State(PlaybackState::Playing),
        });

        assert_eq!(session.state(), PlaybackState::Playing);
        assert!(session.pending_transport().is_none());
    }

    #[test]
    fn a_late_report_for_the_previous_selection_is_rejected() {
        let (mut session, first) = session(MediaKind::Audio);
        session.apply(&EngineReport {
            generation: first,
            kind: ReportKind::Position(Duration::from_secs(30)),
        });

        let second = session
            .select(MediaId::filesystem(66, 2), MediaKind::Audio)
            .expect("a second generation");

        assert_eq!(
            session.position(),
            None,
            "selection clears the old position"
        );
        assert_eq!(
            session.apply(&EngineReport {
                generation: first,
                kind: ReportKind::Position(Duration::from_secs(31)),
            }),
            ReportOutcome::RejectedStale
        );
        assert_eq!(session.position(), None);

        assert_eq!(
            session.apply(&EngineReport {
                generation: second,
                kind: ReportKind::Position(Duration::from_secs(2)),
            }),
            ReportOutcome::Applied
        );
        assert_eq!(session.position(), Some(Duration::from_secs(2)));
    }

    #[test]
    fn a_report_after_close_is_rejected() {
        let (mut session, generation) = session(MediaKind::Video);
        session.clear();

        assert_eq!(
            session.apply(&EngineReport {
                generation,
                kind: ReportKind::State(PlaybackState::Playing),
            }),
            ReportOutcome::RejectedIdle
        );
        assert_eq!(session.state(), PlaybackState::Idle);
    }

    #[test]
    fn a_seek_is_visible_as_seeking_and_does_not_move_the_playhead() {
        let (mut session, generation) = session(MediaKind::Video);
        session.apply(&EngineReport {
            generation,
            kind: ReportKind::Position(Duration::from_secs(5)),
        });

        session
            .request(PlaybackRequest::Seek(Duration::from_secs(90)))
            .expect("video seeks");

        assert!(session.is_seeking());
        assert_eq!(
            session.position(),
            Some(Duration::from_secs(5)),
            "the playhead is what the engine last reported"
        );

        session.apply(&EngineReport {
            generation,
            kind: ReportKind::SeekCompleted(Duration::from_secs(90)),
        });

        assert!(!session.is_seeking());
        assert_eq!(session.position(), Some(Duration::from_secs(90)));
    }

    #[test]
    fn a_seek_past_a_known_duration_is_clamped() {
        let (mut session, generation) = session(MediaKind::Audio);
        session.apply(&EngineReport {
            generation,
            kind: ReportKind::Duration(Duration::from_secs(60)),
        });

        let pending = session
            .request(PlaybackRequest::Seek(Duration::from_secs(600)))
            .expect("audio seeks");

        assert_eq!(
            pending.request,
            PlaybackRequest::Seek(Duration::from_secs(60))
        );
    }

    #[test]
    fn an_image_offers_no_transport_and_rejects_one() {
        let (mut session, _) = session(MediaKind::Image);

        assert_eq!(
            session.request(PlaybackRequest::Play),
            Err(RequestRejected::Unsupported)
        );
        assert_eq!(
            session.request(PlaybackRequest::Seek(Duration::ZERO)),
            Err(RequestRejected::Unsupported)
        );
        assert_eq!(
            session.request(PlaybackRequest::SetVolume(0.5)),
            Err(RequestRejected::Unsupported)
        );
        assert!(session.pending_transport().is_none());
    }

    #[test]
    fn a_video_without_selection_cannot_be_controlled() {
        let mut session = PlaybackSession::new();

        assert_eq!(
            session.request(PlaybackRequest::Play),
            Err(RequestRejected::NoSelection)
        );
        assert_eq!(session.generation(), None);
        assert!(session.capabilities().is_none());
    }

    #[test]
    fn a_failure_is_reported_with_its_message_and_drops_pending_work() {
        let (mut session, generation) = session(MediaKind::Video);
        session
            .request(PlaybackRequest::Seek(Duration::from_secs(3)))
            .expect("video seeks");

        session.apply(&EngineReport {
            generation,
            kind: ReportKind::Failed("no decoder for this stream".to_owned()),
        });

        assert_eq!(session.state(), PlaybackState::Failed);
        assert_eq!(session.error(), Some("no decoder for this stream"));
        assert!(!session.is_seeking());
        assert_eq!(
            session.request(PlaybackRequest::Play),
            Err(RequestRejected::NotPlayable),
            "reopening a failed file is a new selection, not a transport click"
        );
    }

    #[test]
    fn volume_is_clamped_and_confirmed_by_the_engine() {
        let (mut session, generation) = session(MediaKind::Audio);

        let pending = session
            .request(PlaybackRequest::SetVolume(1.4))
            .expect("audio has volume");
        assert_eq!(pending.request, PlaybackRequest::SetVolume(1.0));
        assert_eq!(session.volume(), None, "requested is not confirmed");
        assert!(session.pending_volume().is_some());

        session.apply(&EngineReport {
            generation,
            kind: ReportKind::Volume(0.8),
        });

        assert_eq!(session.volume(), Some(0.8));
        assert!(session.pending_volume().is_none());
    }

    #[test]
    fn reaching_the_end_is_a_terminal_confirmed_state() {
        let (mut session, generation) = session(MediaKind::Audio);
        session.apply(&EngineReport {
            generation,
            kind: ReportKind::State(PlaybackState::Ended),
        });

        assert_eq!(session.state(), PlaybackState::Ended);
        assert!(PlaybackState::Ended.is_terminal());
        assert!(!PlaybackState::Paused.is_terminal());
    }
}
