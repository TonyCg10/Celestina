//! Grafita's shared document core: what a text file is, how it is edited, and
//! how it is written back without losing anything.
//!
//! Both of Grafita's surfaces — the standalone application and the editor
//! embedded in Siderita — drive this crate and add only presentation of their
//! own. Nothing here knows Qt, QML or which host is calling.
//!
//! Three rules shape the whole crate:
//!
//! - **Content decides, never a name.** A dotfile, a `.rs`, a `.kdl` and a file
//!   with no extension take the same path; [`probe`] looks at bytes only.
//! - **The user's bytes survive.** Lines keep their own terminators and only
//!   reversible encodings are offered for editing, so an untouched open/save
//!   cycle is byte-identical by construction.
//! - **A save refuses rather than destroys.** Every refusal before the rename
//!   leaves the original file exactly as it was.
//!
//! IO lives in [`open`] and [`save`] as plain blocking functions. They are
//! meant to run on a host-owned worker; the crate embeds no runtime and starts
//! no threads of its own.

#![forbid(unsafe_code)]

pub mod buffer;
pub mod display;
pub mod document;
pub mod encoding;
pub mod highlight;
pub mod history;
pub mod indent;
pub mod metadata;
pub mod newline;
pub mod open;
pub mod position;
pub mod probe;
pub mod save;
pub mod search;
pub mod session;
pub mod target;
pub mod worker;

#[cfg(test)]
mod testing;

pub use buffer::{Fragment, Line, Replacement, TextBuffer};
pub use document::{Conflict, Document, EditOutcome, Freshness, SaveApplication};
pub use encoding::{DecodeError, Encoding};
pub use highlight::{Language, LineState, Span as HighlightSpan, Token};
pub use history::{History, Revision};
pub use indent::Indentation;
pub use newline::{Newline, NewlineCounts, Terminator};
pub use open::{Limits, OpenRefusal, OpenedFile, ProbeOutcome};
pub use position::{Position, PositionError, Span};
pub use probe::{BinaryReason, Classification};
pub use save::{Durability, SaveRefusal, SaveReport, SaveRequest};
pub use search::{Match, Query};
pub use session::{DocumentSession, Event, Failure, Outcome, SessionState};
pub use target::{FileIdentity, Ownership, Target};
pub use worker::{Completion, DocumentWorker, Job, WorkerStopped};
