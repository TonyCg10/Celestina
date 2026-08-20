//! The narrow contract every host talks to.
//!
//! This is the seam that keeps the backend decision reversible: hosts depend on
//! these traits and on `fluorita-core`'s types, never on libmpv. Replacing the
//! backend later costs an implementation of this file, not an application.
//!
//! The contract is deliberately small — probe, artwork, session — and every
//! method either returns a decision or an [`EngineError`]. Nothing here blocks
//! a GUI thread by contract: the caller runs it on the engine's worker.

use std::path::Path;
use std::time::Duration;

use celestina_core::{CancellationToken, Generation};
use fluorita_core::{EngineReport, MediaMetadata, PlaybackRequest};

use crate::error::EngineResult;

/// Bounds for one probe. Hostile metadata is why they exist: a file may claim
/// anything, so the engine stops looking after a fixed time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProbeBudget {
    pub deadline: Duration,
}

impl ProbeBudget {
    /// Enough for a local file on a slow disk, short enough that a catalogue
    /// scan of thousands cannot stall on one broken file.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            deadline: Duration::from_secs(10),
        }
    }
}

impl Default for ProbeBudget {
    fn default() -> Self {
        Self::conservative()
    }
}

/// What the engine learned about one file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbeReport {
    /// Tags, duration — every field optional, exactly as the catalogue stores.
    pub metadata: MediaMetadata,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_video: bool,
    pub has_audio: bool,
    /// Whether the container answered a seek request.
    pub seekable: bool,
    /// A still image carried inside an audio file (an embedded cover) rather
    /// than a real video track. It must never be treated as a video poster.
    pub video_is_attached_picture: bool,
}

impl ProbeReport {
    /// Whether this file can offer a moving picture — an attached cover cannot.
    #[must_use]
    pub const fn has_moving_video(&self) -> bool {
        self.has_video && !self.video_is_attached_picture
    }
}

/// The engine's entry points. One instance may serve many hosts; each call is
/// independent and carries its own budget and cancellation.
pub trait MediaEngine: Send + Sync {
    /// Reads metadata without rendering anything.
    fn probe(
        &self,
        path: &Path,
        budget: ProbeBudget,
        cancellation: &CancellationToken,
    ) -> EngineResult<ProbeReport>;

    /// Extracts one static PNG — a video poster or an embedded cover — and
    /// publishes it atomically into the shared freedesktop cache.
    ///
    /// Images are not this method's job: the toolkit already decodes them, and
    /// routing them here would start the media backend for a thumbnail that
    /// never needed it.
    fn publish_artwork(&self, request: &ArtworkJob) -> EngineResult<std::path::PathBuf>;

    /// Produces one bounded live preview into Fluorita's own cache.
    ///
    /// A trailer is never published as a freedesktop thumbnail: it is a
    /// different resource with a different lifetime, and the type system keeps
    /// them apart precisely so no host can confuse them.
    fn produce_trailer(&self, job: &TrailerJob) -> EngineResult<TrailerOutcome>;

    /// Opens a playback session for one item.
    fn open_session(&self, request: SessionRequest) -> EngineResult<Box<dyn EngineSession>>;
}

/// What the backend says about how the picture is actually arriving.
///
/// Only meaningful for a session that presents: an audio session has no frames
/// to drop. Every field is optional because the backend legitimately does not
/// know some of them until it has been running — and a zero it never measured
/// would read as "perfect", which is the one answer a pacing report must not
/// invent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Frames the decoder threw away to keep up.
    pub dropped: Option<i64>,
    /// Frames that were presented late.
    pub delayed: Option<i64>,
    /// The display refresh rate the backend estimates from presentation
    /// feedback. `None` until the host tells it when frames reach the screen.
    pub display_fps: Option<f64>,
    /// How irregular that feedback is, as the backend measures it.
    pub vsync_jitter: Option<f64>,
}

/// One playback session: commands in, confirmed reports out.
///
/// The session never invents state. Everything a host may believe arrives as an
/// [`EngineReport`] stamped with the generation the session was opened with, so
/// `fluorita-core` can reject anything belonging to a previous selection.
pub trait EngineSession: Send {
    /// The generation every report from this session carries.
    fn generation(&self) -> Generation;

    /// Sends a user request to the backend. Returning `Ok` means the backend
    /// accepted the request, never that it took effect.
    fn request(&mut self, request: PlaybackRequest) -> EngineResult<()>;

    /// Waits up to `timeout` for the next report. `None` means the backend had
    /// nothing to say in that window, which is not an error.
    fn poll(&mut self, timeout: Duration) -> Option<EngineReport>;

    /// Loads the media and starts the pipeline.
    ///
    /// It is separate from opening on purpose. A session that presents through
    /// a host surface has no video output until that surface has created its
    /// render context, and a file loaded before then ends immediately with
    /// "nothing to play" — the failure this split exists to prevent. The host
    /// opens, hands out the handle, waits for its surface, then starts.
    fn start(&mut self) -> EngineResult<()>;

    /// What the backend measured about presentation so far.
    fn frame_stats(&self) -> FrameStats;

    /// The backend handle a GPU surface needs, while the session is open.
    ///
    /// `None` once the session is closed — which is what stops a surface from
    /// rendering a context that is being torn down.
    fn render_handle(&self) -> Option<RenderHandle>;

    /// Stops playback and releases the backend deterministically.
    fn close(&mut self);
}

/// Everything one artwork job needs. Built from `fluorita-core`'s request so
/// the staleness rules stay in one place.
#[derive(Clone, Debug)]
pub struct ArtworkJob {
    pub source: std::path::PathBuf,
    pub cache_root: std::path::PathBuf,
    pub origin: fluorita_core::ArtworkOrigin,
    pub source_mtime: std::time::SystemTime,
    /// Distinguishes concurrent writers of the same cache entry.
    pub uniquifier: u64,
    pub deadline: Duration,
    pub cancellation: CancellationToken,
}

/// Everything one trailer job needs.
#[derive(Clone, Debug)]
pub struct TrailerJob {
    pub source: std::path::PathBuf,
    /// Fluorita's own cache root — never the freedesktop thumbnail root.
    pub cache_root: std::path::PathBuf,
    pub budget: fluorita_core::TrailerBudget,
    pub uniquifier: u64,
    pub deadline: Duration,
    pub cancellation: CancellationToken,
}

impl TrailerJob {
    /// Builds the job for a request the core already validated — only video
    /// reaches this point, and the budget and cancellation come from there.
    #[must_use]
    pub fn for_request(
        request: &fluorita_core::TrailerRequest,
        cache_root: std::path::PathBuf,
        uniquifier: u64,
        deadline: Duration,
    ) -> Self {
        Self {
            source: request.source().to_path_buf(),
            cache_root,
            budget: request.budget(),
            uniquifier,
            deadline,
            cancellation: request.cancellation().clone(),
        }
    }
}

/// A produced trailer, measured rather than assumed: these numbers come from
/// decoding the encode back, not from what was requested.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrailerOutcome {
    pub path: std::path::PathBuf,
    pub bytes: u64,
    pub duration: Duration,
    pub width: u32,
    pub height: u32,
}

/// Where a session's picture goes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VideoOutput {
    /// Decode without presenting: what audio, probing and every automated test
    /// want, and the only mode that works without a GPU context.
    #[default]
    None,
    /// Hand frames to a host surface through the backend's render API. The
    /// surface must create its render context on the thread that owns the GPU
    /// context; this crate never touches it.
    Embedded,
}

/// The backend handle, as an opaque address.
///
/// This is the single seam between the engine and a GPU surface: libmpv's
/// render API has to be driven from the host's render thread, with its GL
/// context current, which is a place Rust in this crate must never run. The
/// host's C++ surface casts this back and calls the render API itself.
///
/// It is an address, not a pointer, on purpose: nothing in Rust can dereference
/// it by accident, and it cannot outlive its session without the host noticing,
/// because a closed session reports no handle at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderHandle(u64);

impl RenderHandle {
    #[must_use]
    pub const fn from_address(address: usize) -> Self {
        Self(address as u64)
    }

    /// The address the host surface needs. Zero is never a valid handle.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Where a session's sound goes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AudioOutput {
    /// The session opens the system's audio device.
    #[default]
    System,
    /// Decode without touching an audio device — what a poster extraction, a
    /// silent preview or an automated test wants.
    Silent,
}

/// Everything one session needs to open.
#[derive(Clone, Debug)]
pub struct SessionRequest {
    pub source: std::path::PathBuf,
    pub generation: Generation,
    /// Start paused, so a host can show the first frame without committing to
    /// playing — what the minimal Siderita modal does for video.
    pub start_paused: bool,
    /// `None` leaves the backend's own default; a host that has a remembered
    /// level sets it before the first frame instead of after.
    pub initial_volume: Option<f64>,
    pub hardware_decoding: bool,
    pub audio_output: AudioOutput,
    pub video_output: VideoOutput,
    /// Where to begin. `None` starts at the beginning; a hover preview starts
    /// inside the film, because the first seconds of one are titles.
    pub start_at: Option<Duration>,
    /// Whether reaching the end starts it again. A preview that stopped after
    /// its first pass would leave a frozen frame under the pointer.
    pub looping: bool,
}

impl SessionRequest {
    #[must_use]
    pub fn new(source: std::path::PathBuf, generation: Generation) -> Self {
        Self {
            source,
            generation,
            start_paused: false,
            initial_volume: None,
            hardware_decoding: true,
            audio_output: AudioOutput::System,
            video_output: VideoOutput::None,
            start_at: None,
            looping: false,
        }
    }

    /// Presents through the host's render surface instead of decoding blind.
    #[must_use]
    pub fn embedded_video(mut self) -> Self {
        self.video_output = VideoOutput::Embedded;
        self
    }

    /// Decodes without opening an audio device.
    #[must_use]
    pub fn silent(mut self) -> Self {
        self.audio_output = AudioOutput::Silent;
        self
    }

    #[must_use]
    pub fn paused(mut self) -> Self {
        self.start_paused = true;
        self
    }

    #[must_use]
    pub fn with_volume(mut self, level: f64) -> Self {
        self.initial_volume = Some(level.clamp(0.0, 1.0));
        self
    }

    #[must_use]
    pub fn without_hardware_decoding(mut self) -> Self {
        self.hardware_decoding = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioOutput, ProbeBudget, ProbeReport, RenderHandle, SessionRequest, VideoOutput};
    use celestina_core::Generation;
    use std::path::PathBuf;

    #[test]
    fn an_embedded_cover_is_not_a_video() {
        let cover = ProbeReport {
            has_video: true,
            video_is_attached_picture: true,
            ..ProbeReport::default()
        };
        let clip = ProbeReport {
            has_video: true,
            ..ProbeReport::default()
        };

        assert!(!cover.has_moving_video());
        assert!(clip.has_moving_video());
    }

    #[test]
    fn a_session_request_clamps_the_volume_it_is_given() {
        let request = SessionRequest::new(PathBuf::from("/m/a.mp3"), Generation::INITIAL)
            .paused()
            .with_volume(4.2);

        assert_eq!(request.initial_volume, Some(1.0));
        assert!(request.start_paused);
        assert_eq!(request.audio_output, AudioOutput::System);
        assert_eq!(
            SessionRequest::new(PathBuf::from("/m/a.mp3"), Generation::INITIAL)
                .silent()
                .audio_output,
            AudioOutput::Silent
        );
        assert!(request.hardware_decoding);
        assert!(!request.without_hardware_decoding().hardware_decoding);
    }

    #[test]
    fn video_output_is_off_until_a_surface_asks_for_it() {
        let silent = SessionRequest::new(PathBuf::from("/m/a.mp3"), Generation::INITIAL);
        assert_eq!(silent.video_output, VideoOutput::None);
        assert_eq!(silent.embedded_video().video_output, VideoOutput::Embedded);
    }

    #[test]
    fn a_render_handle_is_an_address_that_round_trips() {
        let handle = RenderHandle::from_address(0x7f_00_11_22);
        assert_eq!(handle.value(), 0x7f_00_11_22);
        assert_ne!(handle.value(), 0, "cero nunca es un handle válido");
    }

    #[test]
    fn the_probe_budget_is_bounded_by_default() {
        assert_eq!(ProbeBudget::default(), ProbeBudget::conservative());
        assert!(ProbeBudget::default().deadline.as_secs() <= 10);
    }
}
