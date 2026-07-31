//! Derived-resource requests: static artwork and live trailers, kept apart.
//!
//! These are two different things and the API refuses to blur them:
//!
//! - A [`StaticArtworkRequest`] produces one PNG for the shared freedesktop
//!   cache — an image thumbnail, a video poster or an embedded cover — and is
//!   the only request type that can name a publication.
//! - A [`TrailerRequest`] produces a short, bounded, cancelable live preview
//!   that belongs to Fluorita's own cache or to nothing at all. It carries no
//!   way to publish, so a trailer cannot masquerade as a standard thumbnail.
//!
//! Both carry a generation and a cancellation token, and both validate the
//! source identity they were issued for: a result computed for the previous
//! selection, or for the file as it was before an edit, is dropped.

use std::path::{Path, PathBuf};
use std::time::Duration;

use celestina_core::{CancellationToken, Generation};

use crate::artwork::{cache_key, file_uri, ArtworkPublication};
use crate::catalogue::SourceIdentity;
use crate::media::{ArtworkOrigin, MediaId, MediaKind};

/// One job that will publish a static PNG into the shared thumbnail cache.
#[derive(Clone, Debug)]
pub struct StaticArtworkRequest {
    generation: Generation,
    media: MediaId,
    source: PathBuf,
    kind: MediaKind,
    identity: SourceIdentity,
    cancellation: CancellationToken,
}

impl StaticArtworkRequest {
    #[must_use]
    pub fn new(
        generation: Generation,
        media: MediaId,
        source: PathBuf,
        kind: MediaKind,
        identity: SourceIdentity,
    ) -> Self {
        Self {
            generation,
            media,
            source,
            kind,
            identity,
            cancellation: CancellationToken::new(),
        }
    }

    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub const fn media(&self) -> &MediaId {
        &self.media
    }

    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Where the pixels come from for this kind.
    #[must_use]
    pub fn origin(&self) -> ArtworkOrigin {
        self.kind.artwork_origin()
    }

    #[must_use]
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// The publication plan for the shared cache, or `None` for a source that
    /// has no canonical URI.
    #[must_use]
    pub fn publication(&self, cache_root: &Path, uniquifier: u64) -> Option<ArtworkPublication> {
        ArtworkPublication::prepare(cache_root, &self.source, self.identity.modified, uniquifier)
    }

    /// Whether a finished job may still publish: not cancelled, still the
    /// current generation, and describing the file as it is now.
    #[must_use]
    pub fn may_publish(&self, current_generation: Generation, current: SourceIdentity) -> bool {
        !self.cancellation.is_cancelled()
            && self.generation == current_generation
            && self.identity.still_describes(current)
    }
}

/// The ceiling a live trailer decode may not cross. A trailer exists to hint at
/// a clip, so it is short, small and bounded in bytes; the numbers are the
/// contract, the measured backend later has to fit inside them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrailerBudget {
    pub max_duration: Duration,
    pub max_pixels: u64,
    pub max_bytes: u64,
}

impl TrailerBudget {
    /// Five seconds, 720p-ish and 24 MiB — enough to recognise a clip, small
    /// enough that a grid of cards could never justify one decoder each.
    #[must_use]
    pub const fn conservative() -> Self {
        Self {
            max_duration: Duration::from_secs(5),
            max_pixels: 1_280 * 720,
            max_bytes: 24 * 1024 * 1024,
        }
    }

    #[must_use]
    pub fn accepts(&self, duration: Duration, pixels: u64, bytes: u64) -> bool {
        duration <= self.max_duration && pixels <= self.max_pixels && bytes <= self.max_bytes
    }
}

impl Default for TrailerBudget {
    fn default() -> Self {
        Self::conservative()
    }
}

/// One request for a bounded live preview of a video.
#[derive(Clone, Debug)]
pub struct TrailerRequest {
    generation: Generation,
    media: MediaId,
    source: PathBuf,
    identity: SourceIdentity,
    budget: TrailerBudget,
    cancellation: CancellationToken,
}

impl TrailerRequest {
    /// Only video has a trailer; every other kind is rejected here rather than
    /// silently producing a still that a host might publish.
    pub fn new(
        generation: Generation,
        media: MediaId,
        source: PathBuf,
        kind: MediaKind,
        identity: SourceIdentity,
        budget: TrailerBudget,
    ) -> Result<Self, TrailerRejected> {
        if kind != MediaKind::Video {
            return Err(TrailerRejected::NotVideo);
        }
        Ok(Self {
            generation,
            media,
            source,
            identity,
            budget,
            cancellation: CancellationToken::new(),
        })
    }

    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub const fn media(&self) -> &MediaId {
        &self.media
    }

    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }

    #[must_use]
    pub const fn budget(&self) -> TrailerBudget {
        self.budget
    }

    #[must_use]
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    /// Where a trailer may be kept: inside Fluorita's own bounded cache, under
    /// its own extension. Never the freedesktop `large/<key>.png` entry — a
    /// trailer is not a thumbnail and must not be mistaken for one by any
    /// consumer that scans that directory.
    #[must_use]
    pub fn trailer_cache_path(&self, fluorita_cache_root: &Path) -> Option<PathBuf> {
        let uri = file_uri(&self.source)?;
        Some(
            fluorita_cache_root
                .join("trailers")
                .join(format!("{}.trailer", cache_key(&uri))),
        )
    }

    /// Whether a finished trailer may still be shown.
    #[must_use]
    pub fn may_present(&self, current_generation: Generation, current: SourceIdentity) -> bool {
        !self.cancellation.is_cancelled()
            && self.generation == current_generation
            && self.identity.still_describes(current)
    }
}

/// Why a trailer was not started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrailerRejected {
    /// Only video has a trailer.
    NotVideo,
    /// The request was already cancelled before it could start.
    AlreadyCancelled,
}

/// A running trailer. Dropping or cancelling it cancels the decode.
#[derive(Clone, Debug)]
pub struct TrailerLease {
    generation: Generation,
    media: MediaId,
    cancellation: CancellationToken,
}

impl TrailerLease {
    #[must_use]
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    #[must_use]
    pub const fn media(&self) -> &MediaId {
        &self.media
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

/// Enforces the one-trailer-per-host limit.
///
/// Starting a trailer cancels whatever this host was decoding: hovering across a
/// grid must never leave a trail of live decoders behind.
#[derive(Debug, Default)]
pub struct TrailerHost {
    active: Option<TrailerLease>,
}

/// How many trailers one host may decode at a time.
pub const MAX_TRAILERS_PER_HOST: usize = 1;

impl TrailerHost {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts `request`, cancelling any trailer already running here.
    pub fn start(&mut self, request: &TrailerRequest) -> Result<TrailerLease, TrailerRejected> {
        if request.cancellation.is_cancelled() {
            return Err(TrailerRejected::AlreadyCancelled);
        }
        self.cancel_active();

        let lease = TrailerLease {
            generation: request.generation,
            media: request.media.clone(),
            cancellation: request.cancellation.clone(),
        };
        self.active = Some(lease.clone());
        Ok(lease)
    }

    /// Cancels and forgets the running trailer, if any.
    pub fn cancel_active(&mut self) {
        if let Some(previous) = self.active.take() {
            previous.cancel();
        }
    }

    #[must_use]
    pub fn active(&self) -> Option<&TrailerLease> {
        self.active.as_ref()
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        usize::from(self.active.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StaticArtworkRequest, TrailerBudget, TrailerHost, TrailerRejected, TrailerRequest,
        MAX_TRAILERS_PER_HOST,
    };
    use crate::catalogue::SourceIdentity;
    use crate::media::{ArtworkOrigin, MediaId, MediaKind};
    use celestina_core::{Generation, GenerationClock};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn generations() -> (Generation, Generation) {
        let mut clock = GenerationClock::default();
        let first = clock.issue().expect("first generation");
        let second = clock.issue().expect("second generation");
        (first, second)
    }

    fn identity(secs: u64) -> SourceIdentity {
        SourceIdentity::new(1_024, SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn artwork(generation: Generation, kind: MediaKind) -> StaticArtworkRequest {
        StaticArtworkRequest::new(
            generation,
            MediaId::filesystem(66, 1),
            PathBuf::from("/home/toni/clip.mp4"),
            kind,
            identity(10),
        )
    }

    fn trailer(generation: Generation) -> TrailerRequest {
        TrailerRequest::new(
            generation,
            MediaId::filesystem(66, 1),
            PathBuf::from("/home/toni/clip.mp4"),
            MediaKind::Video,
            identity(10),
            TrailerBudget::conservative(),
        )
        .expect("video has a trailer")
    }

    #[test]
    fn a_static_request_publishes_into_the_shared_cache() {
        let (generation, _) = generations();
        let request = artwork(generation, MediaKind::Video);

        let plan = request
            .publication(Path::new("/home/toni/.cache/thumbnails"), 1)
            .expect("absolute source");

        assert_eq!(request.origin(), ArtworkOrigin::VideoPoster);
        assert_eq!(
            plan.final_path,
            PathBuf::from(
                "/home/toni/.cache/thumbnails/large/053a0fcc87f42f4b9e33ebc076783935.png"
            )
        );
        assert!(request.may_publish(generation, identity(10)));
    }

    #[test]
    fn a_trailer_never_lands_on_a_freedesktop_thumbnail_path() {
        let (generation, _) = generations();
        let request = trailer(generation);

        let path = request
            .trailer_cache_path(Path::new("/home/toni/.cache/fluorita"))
            .expect("absolute source");

        assert_eq!(
            path,
            PathBuf::from(
                "/home/toni/.cache/fluorita/trailers/053a0fcc87f42f4b9e33ebc076783935.trailer"
            )
        );
        assert!(!path.starts_with("/home/toni/.cache/thumbnails"));
        assert_ne!(path.extension(), Some(std::ffi::OsStr::new("png")));
    }

    #[test]
    fn only_video_has_a_trailer() {
        let (generation, _) = generations();

        for kind in [MediaKind::Image, MediaKind::Audio] {
            assert_eq!(
                TrailerRequest::new(
                    generation,
                    MediaId::filesystem(66, 1),
                    PathBuf::from("/home/toni/a.file"),
                    kind,
                    identity(10),
                    TrailerBudget::conservative(),
                )
                .err(),
                Some(TrailerRejected::NotVideo)
            );
        }
    }

    #[test]
    fn a_host_decodes_one_trailer_and_cancels_the_previous() {
        let (first, second) = generations();
        let mut host = TrailerHost::new();
        let earlier = trailer(first);
        let later = trailer(second);

        let first_lease = host.start(&earlier).expect("first trailer starts");
        assert_eq!(host.active_count(), MAX_TRAILERS_PER_HOST);

        let second_lease = host.start(&later).expect("second trailer starts");

        assert!(first_lease.is_cancelled(), "hovering away stops the decode");
        assert!(earlier.cancellation().is_cancelled());
        assert!(!second_lease.is_cancelled());
        assert_eq!(host.active_count(), 1);
        assert_eq!(
            host.active().map(super::TrailerLease::generation),
            Some(second)
        );

        host.cancel_active();
        assert!(second_lease.is_cancelled());
        assert_eq!(host.active_count(), 0);
    }

    #[test]
    fn a_cancelled_request_never_starts() {
        let (generation, _) = generations();
        let request = trailer(generation);
        request.cancellation().cancel();

        let mut host = TrailerHost::new();

        assert_eq!(
            host.start(&request).err(),
            Some(TrailerRejected::AlreadyCancelled)
        );
        assert_eq!(host.active_count(), 0);
    }

    #[test]
    fn stale_generation_or_an_edited_source_blocks_the_result() {
        let (first, second) = generations();
        let request = artwork(first, MediaKind::Audio);

        assert!(request.may_publish(first, identity(10)));
        assert!(
            !request.may_publish(second, identity(10)),
            "the selection moved on"
        );
        assert!(
            !request.may_publish(first, identity(11)),
            "the file changed under the job"
        );

        request.cancel();
        assert!(!request.may_publish(first, identity(10)));

        let live = trailer(first);
        assert!(live.may_present(first, identity(10)));
        assert!(!live.may_present(second, identity(10)));
        assert!(!live.may_present(first, identity(11)));
    }

    #[test]
    fn the_trailer_budget_bounds_duration_pixels_and_bytes() {
        let budget = TrailerBudget::conservative();

        assert!(budget.accepts(Duration::from_secs(4), 1_280 * 720, 1_000_000));
        assert!(!budget.accepts(Duration::from_secs(6), 1_280 * 720, 1_000_000));
        assert!(!budget.accepts(Duration::from_secs(4), 3_840 * 2_160, 1_000_000));
        assert!(!budget.accepts(Duration::from_secs(4), 1_280 * 720, 64 * 1024 * 1024));
        assert_eq!(TrailerBudget::default(), budget);
    }
}
