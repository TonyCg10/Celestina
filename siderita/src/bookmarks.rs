use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One sidebar bookmark: a display name and the path key it points at.
///
/// The key is what ADR 0008 puts across the Qt seam, and storing the same
/// spelling means a bookmarked folder whose name is not valid UTF-8 still
/// opens. Records written before that decision hold the raw path; they are
/// migrated on load by `pathkey::normalize`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
}

/// The XDG config file bookmarks are stored in, if a config home is resolvable.
fn config_file() -> Option<PathBuf> {
    Some(
        celestina_core::xdg::config_home()?
            .join("siderita")
            .join("bookmarks.tsv"),
    )
}

/// Loads saved bookmarks. Returns an empty list if none exist or on any error;
/// bookmarks are a convenience, never a hard dependency.
pub fn load() -> Vec<Bookmark> {
    match config_file() {
        Some(path) => load_from(&path),
        None => Vec::new(),
    }
}

/// Persists bookmarks to the config file, creating the directory if needed.
///
/// This writes only Siderita's own config, never the user's files, so it does
/// not breach the read-only file-management stance of Iteration 1.
pub fn save(bookmarks: &[Bookmark]) -> io::Result<()> {
    match config_file() {
        Some(path) => save_to(&path, bookmarks),
        None => Ok(()),
    }
}

/// Moves the bookmark at `from` so it sits at `to`, shifting the rest along.
/// Returns whether the list actually changed — an out-of-range index or a move
/// onto itself leaves it untouched, so a stray drag can never lose a bookmark.
pub fn move_item(bookmarks: &mut Vec<Bookmark>, from: usize, to: usize) -> bool {
    if from == to || from >= bookmarks.len() || to >= bookmarks.len() {
        return false;
    }
    let moved = bookmarks.remove(from);
    bookmarks.insert(to, moved);
    true
}

fn load_from(path: &Path) -> Vec<Bookmark> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let name = parts.next()?.trim();
            let location = parts.next()?.trim();
            if location.is_empty() {
                return None;
            }
            Some(Bookmark {
                name: if name.is_empty() {
                    location.to_owned()
                } else {
                    name.to_owned()
                },
                path: crate::pathkey::normalize(location),
            })
        })
        .collect()
}

fn save_to(path: &Path, bookmarks: &[Bookmark]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for bookmark in bookmarks {
        let location = sanitize(&bookmark.path);
        if location.is_empty() {
            continue;
        }
        text.push_str(&sanitize(&bookmark.name));
        text.push('\t');
        // Marked, so the reader knows this is a key and does not have to infer
        // it from the codec. A bookmark is a navigation and a drop target, so
        // reading it as the wrong path is worse here than anywhere else.
        text.push_str(&crate::pathkey::persist(&location));
        text.push('\n');
    }
    celestina_core::atomic_file::replace(path, text.as_bytes())
}

fn sanitize(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "siderita-bm-{label}-{}-{nonce}/bookmarks.tsv",
            std::process::id()
        ))
    }

    #[test]
    fn save_then_load_round_trips() {
        let file = temp_file("roundtrip");
        let items = vec![
            Bookmark {
                name: "Docs".to_owned(),
                path: "/home/u/Documents".to_owned(),
            },
            Bookmark {
                name: "/".to_owned(),
                path: "/".to_owned(),
            },
        ];
        save_to(&file, &items).expect("save bookmarks");
        assert_eq!(load_from(&file), items);
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn tabs_and_newlines_in_names_do_not_corrupt_rows() {
        let file = temp_file("sanitize");
        let items = vec![Bookmark {
            name: "a\tb\nc".to_owned(),
            path: "/x/y".to_owned(),
        }];
        save_to(&file, &items).expect("save bookmarks");
        let loaded = load_from(&file);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].path, "/x/y");
        assert!(!loaded[0].name.contains('\t') && !loaded[0].name.contains('\n'));
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn move_item_reorders_and_refuses_nonsense() {
        let mk = |name: &str| Bookmark {
            name: name.to_owned(),
            path: format!("/{name}"),
        };
        let mut list = vec![mk("a"), mk("b"), mk("c")];

        assert!(move_item(&mut list, 0, 2));
        assert_eq!(names(&list), ["b", "c", "a"]);

        assert!(move_item(&mut list, 2, 0));
        assert_eq!(names(&list), ["a", "b", "c"]);

        // No-ops: onto itself, or either index past the end.
        assert!(!move_item(&mut list, 1, 1));
        assert!(!move_item(&mut list, 3, 0));
        assert!(!move_item(&mut list, 0, 9));
        assert_eq!(names(&list), ["a", "b", "c"]);
    }

    fn names(list: &[Bookmark]) -> Vec<&str> {
        list.iter().map(|b| b.name.as_str()).collect()
    }

    #[test]
    fn a_legacy_raw_path_record_is_migrated_to_a_key() {
        let file = temp_file("migrate");
        fs::create_dir_all(file.parent().expect("temp parent")).expect("create dir");
        fs::write(&file, "Mis fotos\t/home/u/mis fotos\n").expect("write");
        let loaded = load_from(&file);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].path, "/home/u/mis%20fotos");
        let _ = fs::remove_dir_all(file.parent().expect("temp parent"));
    }

    #[test]
    fn a_saved_key_holding_a_literal_percent_escape_reads_back_as_itself() {
        // `/home/u/100%20` is a folder whose name ends in the four characters
        // `%20`, so its key doubles the escape. Without the written mark, load
        // would re-encode a record it could not tell from a legacy raw path and
        // point the bookmark at `/home/u/100 ` instead — a wrong navigation and
        // a wrong paste target, not merely a forgotten mark.
        let file = temp_file("literal-escape");
        let items = vec![Bookmark {
            name: "Rebajas".to_owned(),
            path: "/home/u/100%2520".to_owned(),
        }];
        save_to(&file, &items).expect("save bookmarks");
        assert_eq!(load_from(&file), items);
        let _ = fs::remove_dir_all(file.parent().expect("temp parent"));
    }

    #[test]
    fn missing_file_loads_empty() {
        assert!(load_from(Path::new("/nonexistent/siderita/bookmarks.tsv")).is_empty());
    }
}
