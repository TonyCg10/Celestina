//! Fluorita's media engine: the side of the contract that touches real files.
//!
//! `fluorita-core` owns what is true — media identity, the library, the
//! playback model, the thumbnail contract. This crate is what makes those
//! truths come from a real decoder: it probes files, extracts static artwork
//! and runs playback sessions over **libmpv**, the backend the author chose on
//! 2026-07-30 from the measured spike in `fluorita/spikes/`.
//!
//! Three boundaries hold the design together:
//!
//! - **The backend is behind [`backend::MediaEngine`].** Hosts depend on that
//!   trait and on core types, never on libmpv, so replacing the backend costs
//!   an implementation rather than an application.
//! - **Nothing is asserted that the backend did not report.** A request returns
//!   as soon as the backend accepts it; confirmed state arrives later as an
//!   [`fluorita_core::EngineReport`] stamped with the session's generation.
//! - **Images never come through here.** The toolkit already decodes them, and
//!   routing them through the media backend would start a decoder for a
//!   thumbnail that never needed one.
//!
//! No Qt, no QML, no window. The Qt Quick surface arrives at the host
//! milestone and will use libmpv's render API; nothing in this crate
//! initializes it.

#![forbid(unsafe_code)]

pub mod artwork;
pub mod backend;
pub mod catalogue_store;
pub mod engine;
pub mod error;
pub mod instance;
pub mod library;
pub mod probe;
pub mod session;
pub mod source;
pub mod trailer;
pub mod watch;
pub mod worker;

pub use artwork::{pending as pending_artwork, PendingArtwork};
pub use backend::{
    ArtworkJob, AudioOutput, EngineSession, MediaEngine, ProbeBudget, ProbeReport, SessionRequest,
    TrailerJob, TrailerOutcome,
};
pub use catalogue_store::{load as load_catalogue, save as save_catalogue, LoadOutcome};
pub use engine::MpvEngine;
pub use error::{EngineError, EngineResult};
pub use library::{scan, ScanLimits, ScanOutcome};
pub use session::MpvSession;
pub use source::SourceHandle;
pub use watch::{LibraryChange, LibraryWatcher, ResyncReason};
pub use worker::{EngineWorker, Job, JobOutcome};
