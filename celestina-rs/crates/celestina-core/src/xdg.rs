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

/// `$XDG_CACHE_HOME` if it is an absolute path, else `$HOME/.cache`.
///
/// The shared thumbnail cache hangs off this one, so it is the base directory
/// two projects now resolve — Siderita through Qt, Fluorita through here.
pub fn cache_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".cache")))
}

/// `$XDG_DATA_HOME` if it is an absolute path, else `$HOME/.local/share`.
pub fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".local").join("share")))
}

#[cfg(test)]
mod tests {
    use super::cache_home;
    use std::path::PathBuf;

    /// The environment is process-wide, so the three cases share one test
    /// rather than racing each other under the test harness's threads.
    #[test]
    fn cache_home_prefers_the_variable_and_falls_back_to_home() {
        let previous_cache = std::env::var_os("XDG_CACHE_HOME");
        let previous_home = std::env::var_os("HOME");

        // SAFETY-equivalent discipline: restored before returning, and no other
        // test in this crate reads these variables.
        std::env::set_var("XDG_CACHE_HOME", "/tmp/celestina-cache");
        assert_eq!(cache_home(), Some(PathBuf::from("/tmp/celestina-cache")));

        // A relative value is not a base directory; the spec's fallback wins.
        std::env::set_var("XDG_CACHE_HOME", "relativa");
        std::env::set_var("HOME", "/home/prueba");
        assert_eq!(cache_home(), Some(PathBuf::from("/home/prueba/.cache")));

        std::env::remove_var("XDG_CACHE_HOME");
        assert_eq!(cache_home(), Some(PathBuf::from("/home/prueba/.cache")));

        match previous_cache {
            Some(value) => std::env::set_var("XDG_CACHE_HOME", value),
            None => std::env::remove_var("XDG_CACHE_HOME"),
        }
        match previous_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }
}
