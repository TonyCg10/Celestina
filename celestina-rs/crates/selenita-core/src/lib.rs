//! Selenita's domain, without Qt: capture targets and their geometry, the
//! output file names, the history, the niri IPC client, the argv of `grim`,
//! `slurp` and `wl-copy`, the deadline process runner those tools run under,
//! and the recording pipeline with its state machine and stop file.
//!
//! Every function that touches the filesystem, a socket or a process blocks
//! and belongs on a worker thread.

pub mod geometry;
pub mod history;
pub mod names;
pub mod niri;
pub mod record;
pub mod runner;
pub mod target;
pub mod tools;

pub use geometry::Geometry;
pub use history::{Entry, EntryKind, History};
pub use names::{capture_file_name, pictures_dir, videos_dir};
pub use target::{Capture, Target, TargetKind};
