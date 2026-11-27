//! The applications launched last, as `recent.json` under the shell's
//! configuration directory (`$XDG_CONFIG_HOME/celestina/recent.json`): a JSON
//! array of at most [`MAX_RECENT`] desktop-file ids, the most recent first.
//!
//! This is now the read-only seeder of the use record (`super::usage`): the
//! record reads it once, when `usage.json` does not exist, and never writes
//! it again; the file stays in place.
//!
//! The file is read bounded, and anything in it that is not a short id is
//! dropped; an unreadable file is logged and treated as empty.

use std::path::PathBuf;

use celestina_core::atomic_file;

/// The most ids the list keeps.
pub const MAX_RECENT: usize = 50;
/// The longest id kept, in bytes; a desktop-file id is a file name.
pub const MAX_ID_BYTES: usize = 255;
/// The largest file read: the list at its longest with room to spare.
const MAX_FILE_BYTES: u64 = 64 * 1024;

/// The launched ids, most recent first, and the file they live in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentLaunches {
    path: Option<PathBuf>,
    ids: Vec<String>,
}

impl RecentLaunches {
    /// Reads `$XDG_CONFIG_HOME/celestina/recent.json` (or
    /// `~/.config/celestina/recent.json`).
    #[must_use]
    pub fn load() -> Self {
        Self::load_from(
            celestina_core::xdg::config_home()
                .map(|home| home.join("celestina").join("recent.json")),
        )
    }

    /// Reads the list at `path`; `None` keeps the list in memory only.
    #[must_use]
    pub fn load_from(path: Option<PathBuf>) -> Self {
        let ids = match path
            .as_deref()
            .map(|p| atomic_file::read_bounded(p, MAX_FILE_BYTES))
        {
            None | Some(Ok(None)) => Vec::new(),
            Some(Ok(Some(bytes))) => parse(&bytes),
            Some(Err(err)) => {
                tracing::warn!("the recent launches: {err}; starting empty");
                Vec::new()
            }
        };
        Self { path, ids }
    }

    /// The ids, most recent first.
    #[must_use]
    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

/// Whether `id` can be kept: a non-empty, short, single-line file name.
pub(super) fn acceptable(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && !id.contains('/')
        && !id.chars().any(char::is_control)
}

/// The ids of a file's bytes: a JSON array of strings, of which the first
/// [`MAX_RECENT`] acceptable and distinct ones are kept; anything else is
/// an empty list.
fn parse(bytes: &[u8]) -> Vec<String> {
    let Ok(values) = serde_json::from_slice::<Vec<serde_json::Value>>(bytes) else {
        tracing::warn!("the recent launches are not a list of ids; starting empty");
        return Vec::new();
    };
    let mut ids: Vec<String> = Vec::new();
    for value in values {
        let Some(id) = value.as_str().filter(|id| acceptable(id)) else {
            continue;
        };
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
        if ids.len() == MAX_RECENT {
            break;
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test, removed when the guard drops.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir()
                .join(format!("celestina-recent-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(&directory).expect("a test directory");
            Self(directory)
        }

        fn file(&self) -> PathBuf {
            self.0.join("celestina").join("recent.json")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_list_is_read_from_the_file_most_recent_first() {
        let scratch = Scratch::new("read");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), br#"["b.desktop","a.desktop","b.desktop"]"#)
            .expect("the file");
        let recent = RecentLaunches::load_from(Some(scratch.file()));
        assert_eq!(recent.ids(), ["b.desktop", "a.desktop"]);
        assert!(RecentLaunches::load_from(None).ids().is_empty());
    }

    #[test]
    fn a_hostile_file_keeps_only_short_distinct_ids() {
        let long = "x".repeat(MAX_ID_BYTES + 1);
        let text = format!(r#"["a.desktop", 7, "a.desktop", "../b", "{long}", "", "c.desktop"]"#);
        assert_eq!(parse(text.as_bytes()), ["a.desktop", "c.desktop"]);
        assert!(parse(b"{ half").is_empty());
        assert!(parse(b"{\"ids\": []}").is_empty());
    }

    #[test]
    fn an_oversized_file_is_refused_and_starts_empty() {
        let scratch = Scratch::new("large");
        std::fs::create_dir_all(scratch.file().parent().expect("a parent")).expect("the directory");
        std::fs::write(scratch.file(), vec![b' '; 70 * 1024]).expect("the file");
        assert!(RecentLaunches::load_from(Some(scratch.file()))
            .ids()
            .is_empty());
    }
}
