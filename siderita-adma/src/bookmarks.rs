use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One sidebar bookmark: a display name and the location it points at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
}

/// The XDG config file bookmarks are stored in, if a config home is resolvable.
fn config_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|value| value.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("siderita").join("bookmarks.tsv"))
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

/// Derives a default bookmark name from a path's final component.
pub fn name_for(path: &str) -> String {
    if path == "/" {
        return "/".to_owned();
    }
    match path.trim_end_matches('/').rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => path.to_owned(),
    }
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
                path: location.to_owned(),
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
        text.push_str(&location);
        text.push('\n');
    }
    fs::write(path, text)
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
    fn name_for_uses_the_last_path_component() {
        assert_eq!(name_for("/home/user/Downloads"), "Downloads");
        assert_eq!(name_for("/home/user/Downloads/"), "Downloads");
        assert_eq!(name_for("/"), "/");
    }

    #[test]
    fn missing_file_loads_empty() {
        assert!(load_from(Path::new("/nonexistent/siderita/bookmarks.tsv")).is_empty());
    }
}
