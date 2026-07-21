#![forbid(unsafe_code)]

//! Siderita's write-side filesystem domain: the loss-free create / rename /
//! copy / move / Trash operations the file manager stands on.
//!
//! Like `siderita-core`'s read side, this crate is pure, toolkit-free and
//! runnable on any executor — it takes plain paths and a [`CancellationToken`]
//! and returns a typed outcome or a typed [`OpError`], so the host can run it on
//! a worker thread and report the truth of what changed on disk.
//!
//! [`CancellationToken`]: celestina_core::CancellationToken
//!
//! ## Iteration
//!
//! Create-folder, create-file and rename are implemented. Copy, move and Trash
//! follow, each holding the guarantee that a source is never removed before its
//! destination is verified. Every verb refuses to silently overwrite an existing
//! target: a conflict is reported, never resolved by destroying data.

mod create;
mod error;
mod name;
mod rename;

pub use create::{create_directory, create_file};
pub use error::OpError;
pub use name::{validate_name, NameError};
pub use rename::{rename, Renamed};
