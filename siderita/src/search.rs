//! Recursive filename search: a bounded, cancellable, non-indexed walk that is
//! truthful about the scope it covered. This is the boundary the Non-goals
//! allow — not a global indexer: it walks from one folder, on demand, stops at a
//! match cap, and never follows symlinks (so it can't loop or escape the tree).

use std::fs;
use std::path::Path;

use celestina_core::CancellationToken;

/// One entry whose name matched the query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchHit {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// The result of a search, including how far it got — so the UI never implies it
/// covered more than it did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchOutcome {
    pub hits: Vec<SearchHit>,
    /// The match cap was reached, so more may exist beyond what is listed.
    pub truncated: bool,
    /// Directories actually read (skipped-unreadable ones are not counted).
    pub dirs_scanned: usize,
    /// The walk was cancelled before it finished.
    pub cancelled: bool,
}

/// Walks `root` recursively for entries whose name contains `query`
/// (case-insensitive), collecting up to `limit` hits. Symlinks are never
/// followed. An unreadable directory is skipped, not fatal.
pub fn search(
    root: &Path,
    query: &str,
    limit: usize,
    cancellation: &CancellationToken,
) -> SearchOutcome {
    let mut outcome = SearchOutcome::default();
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return outcome;
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancellation.is_cancelled() {
            outcome.cancelled = true;
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        outcome.dirs_scanned += 1;

        for entry in entries.flatten() {
            if cancellation.is_cancelled() {
                outcome.cancelled = true;
                break;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            // Never traverse or match through a symlink.
            if file_type.is_symlink() {
                continue;
            }
            let is_dir = file_type.is_dir();
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if name.to_lowercase().contains(&needle) {
                if outcome.hits.len() >= limit {
                    outcome.truncated = true;
                    return outcome;
                }
                outcome.hits.push(SearchHit {
                    name: name.clone().into_owned(),
                    path: entry.path().to_string_lossy().into_owned(),
                    is_dir,
                });
            }

            if is_dir {
                stack.push(entry.path());
            }
        }
    }

    outcome
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use celestina_core::CancellationToken;

    use super::search;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("siderita-search-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).expect("create dir");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn live() -> CancellationToken {
        CancellationToken::new()
    }

    #[test]
    fn finds_matches_recursively_case_insensitively() {
        let dir = TestDir::new();
        fs::write(dir.0.join("Informe.txt"), b"x").unwrap();
        fs::create_dir(dir.0.join("sub")).unwrap();
        fs::write(dir.0.join("sub").join("informe-2.md"), b"x").unwrap();
        fs::write(dir.0.join("otro.txt"), b"x").unwrap();

        let outcome = search(&dir.0, "informe", 100, &live());
        assert_eq!(outcome.hits.len(), 2);
        assert!(outcome.dirs_scanned >= 2, "walked the subdir too");
        assert!(!outcome.truncated);
    }

    #[test]
    fn respects_the_match_cap_and_reports_truncation() {
        let dir = TestDir::new();
        for i in 0..10 {
            fs::write(dir.0.join(format!("match-{i}.txt")), b"x").unwrap();
        }
        let outcome = search(&dir.0, "match", 3, &live());
        assert_eq!(outcome.hits.len(), 3);
        assert!(outcome.truncated);
    }

    #[test]
    fn an_empty_query_walks_nothing() {
        let dir = TestDir::new();
        fs::write(dir.0.join("a.txt"), b"x").unwrap();
        let outcome = search(&dir.0, "   ", 100, &live());
        assert!(outcome.hits.is_empty());
        assert_eq!(outcome.dirs_scanned, 0);
    }
}
