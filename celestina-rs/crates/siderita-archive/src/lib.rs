#![forbid(unsafe_code)]

//! Siderita's archive domain: what a `.zip`, `.tar` or `.tar.gz` holds, how it
//! is extracted without ever writing outside the folder a person chose, and how
//! one is created from entries on disk.
//!
//! Pure and toolkit-free like the rest of `celestina-rs`, and process-free by
//! construction: the containers are implemented in Rust, so an extraction does
//! not depend on `unzip` being installed, cannot inherit a shell's quoting rules
//! and cannot be told what to do by a crafted file name.
//!
//! The filesystem guarantees are [`siderita_ops`]' own, reused rather than
//! restated: cancellation through [`CancellationToken`], byte and item counts
//! through [`Progress`], failures through [`OpError`], and the rule that no
//! verb overwrites an existing entry or leaves a partial result behind.
//!
//! [`CancellationToken`]: celestina_core::CancellationToken
//! [`Progress`]: siderita_ops::Progress
//! [`OpError`]: siderita_ops::OpError
//!
//! ## Verbs
//!
//! [`sniff`] identifies a container by its bytes, [`list`] reads its index,
//! [`extract`] writes it into a folder, and [`create`] packs entries into a new
//! `.zip` or `.tar.gz`.

mod create;
mod error;
mod extract;
mod format;
mod member;
mod read;
mod stamp;
mod tarname;
mod tool;

pub use create::create;
pub use error::ArchiveError;
pub use extract::{extract, measure, ExtractOptions, Extracted, SkipReason, Skipped};
pub use format::{default_stem, sniff, Format};
pub use member::Member;
pub use read::list;
pub use stamp::{Utc, Zone};
pub use tool::{can_read, reader_name};
