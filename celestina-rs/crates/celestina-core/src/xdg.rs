//! Freedesktop base directories.
//!
//! Each returns the environment variable when it holds an absolute path, and
//! otherwise the spec's `$HOME`-relative fallback. `siderita-ops` (the home
//! Trash) and the app (every config and data file it reads) both resolve these,
//! so they live here once instead of being copied into each module.

use std::path::PathBuf;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// `$XDG_CONFIG_HOME` if it is an absolute path, else `$HOME/.config`.
pub fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".config")))
}

/// `$XDG_DATA_HOME` if it is an absolute path, else `$HOME/.local/share`.
pub fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".local").join("share")))
}
