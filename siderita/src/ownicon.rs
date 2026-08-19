//! The picture a file carries about itself.
//!
//! Some entries are not "a kind of file" — they are a specific thing with a
//! face: a launcher for a game, an application, a shortcut. Drawing the generic
//! page for those throws away the one piece of information the folder had.
//!
//! This module answers only what it can answer honestly and cheaply: a
//! `.desktop` entry names its icon, and that name resolves to a file on disk
//! through the same directories every desktop searches. An entry that names no
//! icon, or names one that is not installed, gets nothing back and keeps the
//! glyph its extension earned.
//!
//! A file that carries its picture *inside* it — a program, a song, a package —
//! is answered differently: those go through the thumbnail provider, which
//! already caches, runs off the UI thread and knows how to decode. This module
//! only says that they should, and `siderita-embedded` does the reading.

use std::path::{Path, PathBuf};

/// The image file a launcher names as its own icon, if any.
pub(crate) fn own_icon(path: &Path) -> Option<PathBuf> {
    if path.extension()? != "desktop" {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let entry = celestina_core::desktop_entry::parse("", &content)?;
    if entry.icon.is_empty() {
        return None;
    }
    resolve_icon(&entry.icon)
}

/// Turns an `Icon=` value into a file on disk.
///
/// The value is either an absolute path — which some launchers write, and which
/// is used as-is — or a theme name, which is searched for in the directories
/// the icon-theme spec lists, largest size first so a grid cell gets something
/// worth scaling down rather than a 16-pixel stamp scaled up.
fn resolve_icon(icon: &str) -> Option<PathBuf> {
    let direct = Path::new(icon);
    if direct.is_absolute() {
        return direct.is_file().then(|| direct.to_path_buf());
    }

    let mut roots = Vec::new();
    if let Some(data_home) = celestina_core::xdg::data_home() {
        roots.push(data_home.join("icons"));
    }
    roots.push(PathBuf::from("/usr/local/share/icons"));
    roots.push(PathBuf::from("/usr/share/icons"));

    for root in &roots {
        for theme in ["hicolor", "Adwaita", "breeze"] {
            for size in [
                "scalable", "512x512", "256x256", "192x192", "128x128", "96x96", "64x64", "48x48",
                "32x32",
            ] {
                for extension in ["svg", "png"] {
                    let candidate = root
                        .join(theme)
                        .join(size)
                        .join("apps")
                        .join(format!("{icon}.{extension}"));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    // The flat directory that predates the theme spec, still used by plenty of
    // installers — including the ones that write a launcher for a game.
    for extension in ["png", "svg", "xpm"] {
        let candidate = PathBuf::from("/usr/share/pixmaps").join(format!("{icon}.{extension}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{own_icon, resolve_icon};
    use std::path::Path;

    #[test]
    fn only_a_desktop_entry_carries_an_icon_of_its_own() {
        assert_eq!(own_icon(Path::new("/tmp/imagen.png")), None);
        assert_eq!(own_icon(Path::new("/tmp/no-existe.desktop")), None);
    }

    #[test]
    fn an_absolute_icon_is_taken_as_written_when_it_exists() {
        assert_eq!(resolve_icon("/definitivamente/no/existe.png"), None);
        // Whatever this machine has, a resolved icon is a real file.
        if let Some(found) = resolve_icon("firefox") {
            assert!(found.is_file());
        }
    }
}
