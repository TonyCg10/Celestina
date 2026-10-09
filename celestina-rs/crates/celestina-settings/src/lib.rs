//! The suite's shared settings: today, the appearance file.
//!
//! `~/.config/celestina/appearance.toml` holds the two appearance choices every
//! first-party window honours: reduced motion and the text scale. This crate
//! is their one owner. It reads and writes the file ([`load`], [`save`]),
//! watches it ([`watch`]), and applies the `CELESTINA_REDUCED_MOTION`
//! development and accessibility override. It has no Qt: each application
//! runs [`watch`] on a worker and queues the values to its window, which binds
//! `CelestinaTheme.reducedMotion` and `CelestinaTheme.textScale` to them.

#![forbid(unsafe_code)]

mod appearance;
mod watch;

pub use appearance::{env_forces_reduced_motion, load, path, save, Appearance, TextScale};
pub use watch::{follow, watch, Follower, WatchHandle};

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why a settings file could not be written or watched.
#[derive(Debug)]
pub enum SettingsError {
    /// Neither `$XDG_CONFIG_HOME` nor `$HOME` is an absolute path, so there is
    /// no configuration directory to use.
    NoConfigHome,
    /// The existing file could not be read before it was rewritten.
    Read {
        path: PathBuf,
        source: celestina_core::atomic_file::ReadError,
    },
    /// The existing file is not UTF-8, so its other lines cannot be kept.
    NotText { path: PathBuf },
    /// The file or its directory could not be written.
    Write { path: PathBuf, source: io::Error },
    /// The directory could not be watched.
    Watch {
        path: PathBuf,
        source: notify_debouncer_full::notify::Error,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConfigHome => formatter.write_str(
                "no configuration directory: neither XDG_CONFIG_HOME nor HOME is absolute",
            ),
            Self::Read { path, .. } => write!(formatter, "cannot read {}", path.display()),
            Self::NotText { path } => write!(formatter, "{} is not UTF-8", path.display()),
            Self::Write { path, .. } => write!(formatter, "cannot write {}", path.display()),
            Self::Watch { path, .. } => write!(formatter, "cannot watch {}", path.display()),
        }
    }
}

impl Error for SettingsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NoConfigHome | Self::NotText { .. } => None,
            Self::Read { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
            Self::Watch { source, .. } => Some(source),
        }
    }
}
