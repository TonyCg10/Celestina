//! Fluorita's pure media domain: what a local media file is, how a library
//! projects into Gallery and Music, what the desktop's thumbnail cache expects,
//! and what playback is actually known to be doing.
//!
//! Nothing here touches Qt, a decoder or the filesystem. Every function takes
//! what it needs as an argument — path bytes, stat values, engine reports — and
//! returns a decision, so both hosts (the standalone Fluorita app and Siderita's
//! minimal player modal) can share one truth without sharing a toolkit.
//!
//! Two invariants run through the whole crate:
//!
//! - **Confirmed state comes only from the engine.** A user action is a pending
//!   request beside the state, never the state itself.
//! - **Every job carries a [`celestina_core::Generation`].** A report that
//!   belongs to an older selection is rejected instead of overwriting a newer
//!   one.

#![forbid(unsafe_code)]

pub mod artwork;
pub mod batch;
pub mod catalogue;
pub mod continuation;
pub mod edit;
pub mod edit_stack;
pub mod gallery;
pub mod media;
pub mod metadata;
pub mod music;
pub mod pacing;
pub mod playback;
pub mod preview;
pub mod search;
pub mod source;
pub mod streams;

pub use artwork::{
    cache_key, file_uri, large_thumbnail_path, ArtworkPublication, ArtworkValidity, ThumbnailSize,
    LARGE_THUMBNAIL_PIXELS,
};
pub use batch::{BatchOperation, BatchProgress, ItemOutcome};
pub use catalogue::{
    AbsorbSummary, Availability, Catalogue, MediaMetadata, MediaRecord, ReconcileSummary,
    SourceIdentity,
};
pub use continuation::Continuation;
pub use edit::{
    EditCapabilities, EditClass, ImageFormat, Operation, OutputFormat, SaveChoice, HIGH_QUALITY,
};
pub use edit_stack::{
    Annotation, Area, Axis, Canvas, Composition, EditDocument, EditLimits, EditRejected, Ink,
    ObjectId, Orientation, Point, Preview, Quarter, Redaction, ShapeKind, Transform,
};
pub use gallery::{gallery, GalleryFilter, GalleryItem, GalleryOrder};
pub use media::{ArtworkOrigin, MediaCapabilities, MediaId, MediaKind};
pub use metadata::{
    CoverBudget, MetadataCapabilities, MetadataFormat, MetadataRejected, PrivateFact, TagChange,
    TagField, MAX_TAG_CHARACTERS,
};
pub use music::{Album, Artist, MusicLibrary, Track};
pub use pacing::{PacingCapture, PacingSample, PacingSummary, Verdict};
pub use playback::{
    EngineReport, PendingRequest, PlaybackRequest, PlaybackSession, PlaybackState, ReportKind,
    ReportOutcome, RequestRejected,
};
pub use preview::{
    StaticArtworkRequest, TrailerBudget, TrailerHost, TrailerLease, TrailerRejected,
    TrailerRequest, MAX_TRAILERS_PER_HOST,
};
pub use search::{Query, MAX_QUERY_CHARACTERS};
pub use source::{
    KindSet, MediaSource, SourceId, SourceRejected, SourceScope, SourceSet, XdgMediaDirs,
};
pub use streams::{Speed, Stream, StreamKind, StreamSet, MAX_STREAMS};
