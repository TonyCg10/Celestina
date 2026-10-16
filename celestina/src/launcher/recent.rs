//! The applications launched last: `recent.json` under the shell's
//! configuration directory (`$XDG_CONFIG_HOME/celestina/recent.json`), a
//! JSON array of at most [`MAX_RECENT`] desktop-file ids, the most recent
//! first. The ranking lifts them (see `super::state`).
//!
//! The file is read once at start, bounded, and anything in it that is not a
//! short id is dropped; an unreadable file is logged and treated as empty.
//! After a launch the list is written whole and atomically
//! (`celestina_core::atomic_file::replace_private`). The write is a few
//! hundred bytes; a failure is logged and the launch stands.

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

    /// Puts `id` first (once), keeps at most [`MAX_RECENT`], and writes the
    /// file. Returns whether the write succeeded (always true without a file).
    pub fn record(&mut self, id: &str) -> bool {
        if !acceptable(id) {
            return false;
        }
        self.ids.retain(|known| known != id);
        self.ids.insert(0, id.to_owned());
        self.ids.truncate(MAX_RECENT);
        let Some(path) = self.path.as_deref() else {
            return true;
        };
        let bytes = match serde_json::to_vec(&self.ids) {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::warn!(%err, "the recent launches could not be encoded");
                return false;
            }
        };
        match atomic_file::replace_private(path, &bytes) {
            Ok(_) => true,
            Err(err) => {
                tracing::warn!(
                    "{} not written: {}",
                    path.display(),
                    crate::error_chain(&err)
                );
                false
            }
        }
    }
}

/// Whether `id` can be kept: a non-empty, short, single-line file name.
fn acceptable(id: &str) -> bool {
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
    fn a_launch_goes_first_once_and_survives_a_restart() {
        let scratch = Scratch::new("round-trip");
        let mut recent = RecentLaunches::load_from(Some(scratch.file()));
        assert!(recent.ids().is_empty());
        assert!(recent.record("a.desktop"));
        assert!(recent.record("b.desktop"));
        assert!(recent.record("a.desktop"));
        assert_eq!(recent.ids(), ["a.desktop", "b.desktop"]);
        let again = RecentLaunches::load_from(Some(scratch.file()));
        assert_eq!(again.ids(), ["a.desktop", "b.desktop"]);
    }

    #[test]
    fn the_list_keeps_the_last_fifty() {
        let mut recent = RecentLaunches::load_from(None);
        for i in 0..60 {
            recent.record(&format!("{i}.desktop"));
        }
        assert_eq!(recent.ids().len(), MAX_RECENT);
        assert_eq!(recent.ids()[0], "59.desktop");
        assert_eq!(recent.ids()[MAX_RECENT - 1], "10.desktop");
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
