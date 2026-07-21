use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The XDG_<KEY>_DIR stems Siderita offers as sidebar places, with the English
/// fallback folder name used when `user-dirs.dirs` does not define them.
const DEFAULTS: &[(&str, &str)] = &[
    ("DESKTOP", "Desktop"),
    ("DOCUMENTS", "Documents"),
    ("DOWNLOAD", "Downloads"),
    ("MUSIC", "Music"),
    ("PICTURES", "Pictures"),
    ("VIDEOS", "Videos"),
];

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|value| value.is_absolute())
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|value| value.is_absolute())
        .or_else(|| home().map(|dir| dir.join(".config")))
}

/// Resolves the standard user directories that exist, as a KEY -> path map.
/// HOME is always included; the others only when the directory exists and is
/// not HOME itself (an unconfigured `user-dirs.dirs` often points them at HOME).
pub fn resolve() -> HashMap<String, PathBuf> {
    let mut resolved = HashMap::new();
    let Some(home) = home() else {
        return resolved;
    };
    resolved.insert("HOME".to_owned(), home.clone());

    let configured = read_user_dirs(&home);
    for &(key, fallback) in DEFAULTS {
        let path = configured
            .get(key)
            .cloned()
            .unwrap_or_else(|| home.join(fallback));
        if path != home && path.is_dir() {
            resolved.insert(key.to_owned(), path);
        }
    }
    resolved
}

fn read_user_dirs(home: &Path) -> HashMap<String, PathBuf> {
    match config_home() {
        Some(config) => match std::fs::read_to_string(config.join("user-dirs.dirs")) {
            Ok(content) => parse_user_dirs(home, &content),
            Err(_) => HashMap::new(),
        },
        None => HashMap::new(),
    }
}

/// Parses `user-dirs.dirs` content into a KEY -> path map, expanding `$HOME`.
fn parse_user_dirs(home: &Path, content: &str) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((variable, value)) = line.split_once('=') else {
            continue;
        };
        let Some(key) = variable
            .trim()
            .strip_prefix("XDG_")
            .and_then(|rest| rest.strip_suffix("_DIR"))
        else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        let path = if let Some(rest) = value.strip_prefix("$HOME/") {
            home.join(rest)
        } else if value == "$HOME" {
            home.to_path_buf()
        } else {
            PathBuf::from(value)
        };
        map.insert(key.to_owned(), path);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_localized_user_dirs() {
        let home = Path::new("/home/u");
        let content = "\
# generated
XDG_DESKTOP_DIR=\"$HOME/Escritorio\"
XDG_DOWNLOAD_DIR=\"$HOME/Descargas\"
XDG_PICTURES_DIR=\"$HOME/Imágenes\"
XDG_PROJECTS_DIR=\"$HOME/\"
";
        let map = parse_user_dirs(home, content);
        assert_eq!(map.get("DESKTOP"), Some(&home.join("Escritorio")));
        assert_eq!(map.get("DOWNLOAD"), Some(&home.join("Descargas")));
        assert_eq!(map.get("PICTURES"), Some(&home.join("Imágenes")));
        // Comments and unrelated keys are ignored; MUSIC is absent here.
        assert!(map.get("MUSIC").is_none());
    }

    #[test]
    fn ignores_malformed_lines() {
        let map = parse_user_dirs(Path::new("/home/u"), "not a var\n=nothing\nXDG_X=\"y\"\n");
        assert!(map.is_empty());
    }
}
