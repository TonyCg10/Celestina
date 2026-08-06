use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The XDG config file starred paths live in, if a config home is resolvable.
/// One absolute path per line — a set, so the file is order-free and a repeated
/// star costs nothing.
fn config_file() -> Option<PathBuf> {
    Some(
        celestina_core::xdg::config_home()?
            .join("siderita")
            .join("favorites.conf"),
    )
}

/// Loads the starred paths. Any error yields an empty set — a star is a mark on
/// a file, never something the listing depends on.
pub fn load() -> BTreeSet<String> {
    match config_file() {
        Some(path) => load_from(&path),
        None => BTreeSet::new(),
    }
}

/// Persists the starred paths, creating the config directory if needed. Writes
/// only Siderita's own config, never the user's files.
pub fn save(paths: &BTreeSet<String>) -> io::Result<()> {
    match config_file() {
        Some(path) => save_to(&path, paths),
        None => Ok(()),
    }
}

fn load_from(path: &Path) -> BTreeSet<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return BTreeSet::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn save_to(path: &Path, paths: &BTreeSet<String>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for entry in paths {
        // A path cannot hold a newline, but a corrupt config could; dropping the
        // line is better than writing a file that reads back as two paths.
        if entry.contains(['\n', '\r']) {
            continue;
        }
        text.push_str(entry);
        text.push('\n');
    }
    celestina_core::atomic_file::replace(path, text.as_bytes())
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
            "siderita-fav-{label}-{}-{nonce}/favorites.conf",
            std::process::id()
        ))
    }

    #[test]
    fn save_then_load_round_trips() {
        let file = temp_file("roundtrip");
        let items: BTreeSet<String> = ["/home/u/Documents".to_owned(), "/etc".to_owned()]
            .into_iter()
            .collect();
        save_to(&file, &items).expect("save favorites");
        assert_eq!(load_from(&file), items);
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn blank_lines_are_ignored() {
        let file = temp_file("blanks");
        fs::create_dir_all(file.parent().unwrap()).expect("create dir");
        fs::write(&file, "\n/a\n   \n/b\n").expect("write");
        let loaded = load_from(&file);
        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains("/a") && loaded.contains("/b"));
        let _ = fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn missing_file_loads_empty() {
        assert!(load_from(Path::new("/nonexistent/siderita/favorites.conf")).is_empty());
    }
}
