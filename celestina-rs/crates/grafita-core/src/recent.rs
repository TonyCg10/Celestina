//! The documents Grafita opened last, so a new tab is not a blank wall.
//!
//! Deliberately Grafita's own small file rather than freedesktop's
//! `recently-used.xbel`: that format is XML, and parsing it correctly would earn
//! a dependency for a list of paths. One absolute path per line is enough, and
//! a file this shape can be read by anything, including a person.
//!
//! Two rules keep it honest:
//!
//! - **A path that no longer exists is not offered.** A recent list that opens
//!   nothing is worse than a short one.
//! - **Recording is best-effort.** A history that cannot be written must never
//!   stop a document from opening; the editor's job is editing.

use std::path::{Path, PathBuf};

use celestina_core::{atomic_file, xdg};

/// How many documents are remembered. Enough to cover "the thing I was just in",
/// short enough that the list stays scannable.
const LIMIT: usize = 12;

/// The recently opened documents, newest first.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Recent {
    paths: Vec<PathBuf>,
}

impl Recent {
    /// Reads the remembered list, or an empty one.
    ///
    /// A missing or unreadable file is an empty history, never an error: there
    /// is nothing a user could do about it and nothing lost.
    #[must_use]
    pub fn load() -> Self {
        let Some(path) = storage() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    #[must_use]
    fn parse(text: &str) -> Self {
        let mut paths = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            // Only absolute paths: a relative one would mean something
            // different depending on where Grafita was launched from.
            if line.is_empty() || !line.starts_with('/') {
                continue;
            }
            let candidate = PathBuf::from(line);
            if !paths.contains(&candidate) {
                paths.push(candidate);
            }
        }
        paths.truncate(LIMIT);
        Self { paths }
    }

    /// The remembered paths, newest first.
    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Only the ones that still exist, which is what a host should offer.
    #[must_use]
    pub fn existing(&self) -> Vec<PathBuf> {
        self.paths
            .iter()
            .filter(|path| path.is_file())
            .cloned()
            .collect()
    }

    /// Moves `path` to the front, adding it if it is new.
    ///
    /// Opening a document you already had recently should reorder the list, not
    /// grow it.
    pub fn record(&mut self, path: &Path) {
        if !path.is_absolute() {
            return;
        }
        self.paths.retain(|kept| kept != path);
        self.paths.insert(0, path.to_path_buf());
        self.paths.truncate(LIMIT);
    }

    /// Forgets `path` — used when opening one turns out to fail.
    pub fn forget(&mut self, path: &Path) {
        self.paths.retain(|kept| kept != path);
    }

    /// Writes the list back. Best-effort: a history that cannot be saved is not
    /// worth failing an edit over.
    pub fn store(&self) {
        let Some(path) = storage() else {
            return;
        };
        let mut text = String::new();
        for entry in &self.paths {
            // A path that is not valid UTF-8 is skipped rather than mangled:
            // writing a lossy name would remember the wrong file.
            if let Some(line) = entry.to_str() {
                text.push_str(line);
                text.push('\n');
            }
        }
        let _ = atomic_file::replace(&path, text.as_bytes());
    }
}

/// Where the list lives. `state` in spirit, but the suite's XDG helper offers
/// data, which is the closest thing that survives a cache clear.
fn storage() -> Option<PathBuf> {
    Some(xdg::data_home()?.join("grafita").join("recent"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Recent, LIMIT};

    #[test]
    fn recording_moves_a_repeat_to_the_front_rather_than_duplicating_it() {
        let mut recent = Recent::default();
        recent.record(&PathBuf::from("/uno"));
        recent.record(&PathBuf::from("/dos"));
        recent.record(&PathBuf::from("/uno"));

        assert_eq!(
            recent.paths(),
            [PathBuf::from("/uno"), PathBuf::from("/dos")]
        );
    }

    #[test]
    fn the_list_is_bounded_and_keeps_the_newest() {
        let mut recent = Recent::default();
        for index in 0..(LIMIT + 5) {
            recent.record(&PathBuf::from(format!("/documento-{index}")));
        }

        assert_eq!(recent.paths().len(), LIMIT);
        assert_eq!(
            recent.paths()[0],
            PathBuf::from(format!("/documento-{}", LIMIT + 4))
        );
    }

    #[test]
    fn a_relative_path_is_never_remembered() {
        let mut recent = Recent::default();
        recent.record(&PathBuf::from("relativo/nota.txt"));

        assert!(
            recent.paths().is_empty(),
            "it would mean a different file elsewhere"
        );
    }

    #[test]
    fn parsing_skips_blanks_relatives_and_repeats() {
        let recent = Recent::parse("/uno\n\n  \nrelativo\n/dos\n/uno\n");

        assert_eq!(
            recent.paths(),
            [PathBuf::from("/uno"), PathBuf::from("/dos")]
        );
    }

    #[test]
    fn forgetting_removes_exactly_one_entry() {
        let mut recent = Recent::default();
        recent.record(&PathBuf::from("/uno"));
        recent.record(&PathBuf::from("/dos"));
        recent.forget(&PathBuf::from("/uno"));

        assert_eq!(recent.paths(), [PathBuf::from("/dos")]);
    }

    #[test]
    fn only_files_that_still_exist_are_offered() {
        let mut recent = Recent::default();
        // This test's own source tree is a file that certainly exists.
        let real = std::env::current_dir()
            .expect("a working directory")
            .join("Cargo.toml");
        recent.record(&PathBuf::from("/no/existe/en/ningun/sitio"));
        recent.record(&real);

        assert_eq!(recent.existing(), vec![real]);
    }
}
