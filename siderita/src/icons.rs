use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

/// The XDG config file per-path custom icon overrides live in, if a config home
/// is resolvable. One `path\ticon-name` line each.
fn config_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|value| value.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("siderita").join("icons.conf"))
}

/// Loads the saved icon overrides (absolute path → freedesktop icon name). Any
/// error yields an empty map — a custom icon is a convenience, never required.
pub fn load() -> HashMap<String, String> {
    let Some(path) = config_file() else {
        return HashMap::new();
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let key = parts.next()?.trim();
            let icon = parts.next()?.trim();
            if key.is_empty() || icon.is_empty() {
                None
            } else {
                Some((key.to_owned(), icon.to_owned()))
            }
        })
        .collect()
}

/// Persists the overrides, creating the config directory if needed. Writes only
/// Siderita's own config, never the user's files.
pub fn save(overrides: &HashMap<String, String>) -> io::Result<()> {
    let Some(path) = config_file() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Deterministic order so the file diffs cleanly.
    let mut entries: Vec<(&String, &String)> = overrides.iter().collect();
    entries.sort();
    let mut body = String::new();
    for (key, icon) in entries {
        body.push_str(key);
        body.push('\t');
        body.push_str(icon);
        body.push('\n');
    }
    fs::write(&path, body)
}
