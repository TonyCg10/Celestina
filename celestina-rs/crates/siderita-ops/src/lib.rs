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
//! ## Verbs
//!
//! Create-folder, create-file, rename, copy, move, send-to-Trash,
//! list-Trash, restore-from-Trash and purge-from-Trash are all implemented. The
//! loss-free verbs hold the
//! guarantee that a source is never removed before its destination is verified,
//! and none silently overwrites an existing target: a conflict is reported,
//! never resolved by destroying data.

mod copy;
mod create;
mod error;
mod name;
mod purge;
mod relocate;
mod rename;
mod restore;
mod trash;
mod trashinfo;

pub use copy::{copy, copy_as, Progress};
pub use create::{create_directory, create_file};
pub use error::OpError;
pub use name::{validate_name, NameError};
pub use purge::purge_from_trash;
pub use relocate::{move_as, move_entry, Moved};
pub use rename::{rename, Renamed};
pub use restore::{restore_from_trash, Restored};
pub use trash::{trash, Trashed};
pub use trashinfo::{list_home_trash, TrashEntry};
