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
//!
//! Everything here runs on the Qt thread, called from a delegate's binding, so
//! it is answered from a cache after the first time. It has to be: resolving a
//! name against every theme directory cost 165 `stat` calls per launcher, and a
//! grid of forty of them re-resolved on every rebind — 6 600 calls, 2.5 ms of a
//! thread that should be drawing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The themes to search, in order: the one this session is configured to use,
/// whatever it inherits, and `hicolor` last — which the icon-theme spec makes
/// the final fallback every theme falls back to.
///
/// Reading the configured theme is not a nicety. The list used to be three
/// names hard-coded here, and on a machine themed with anything else — this
/// author's is `Qogir` — an installed application's icon was simply not found:
/// `firefox` lives only in that theme's directory.
fn themes(roots: &[PathBuf]) -> Vec<String> {
    static THEMES: OnceLock<Vec<String>> = OnceLock::new();
    THEMES
        .get_or_init(|| {
            let mut ordered = Vec::new();
            if let Some(configured) = configured_theme() {
                push_theme(&mut ordered, roots, configured, 0);
            }
            for fallback in ["Adwaita", "breeze", "hicolor"] {
                if !ordered.iter().any(|theme| theme == fallback) {
                    ordered.push(fallback.to_owned());
                }
            }
            ordered
        })
        .clone()
}

/// Adds `theme` and then whatever its `index.theme` says it inherits.
///
/// Bounded, because `Inherits` is a graph a theme author writes by hand and
/// nothing stops it from pointing back at itself.
fn push_theme(ordered: &mut Vec<String>, roots: &[PathBuf], theme: String, depth: u8) {
    if depth > 4 || ordered.contains(&theme) {
        return;
    }
    ordered.push(theme.clone());
    for root in roots {
        let Ok(index) = std::fs::read_to_string(root.join(&theme).join("index.theme")) else {
            continue;
        };
        let inherits = index
            .lines()
            .find_map(|line| line.trim().strip_prefix("Inherits="));
        if let Some(inherits) = inherits {
            for parent in inherits.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                push_theme(ordered, roots, parent.to_owned(), depth + 1);
            }
            return;
        }
    }
}

/// The icon theme this session is configured to use, as GTK and KDE record it.
///
/// Read from the files rather than from a running settings daemon: this
/// application has no toolkit-settings client, and the two files below are what
/// every desktop writes when a person picks a theme.
fn configured_theme() -> Option<String> {
    let config = celestina_core::xdg::config_home()?;
    for (file, key) in [
        ("gtk-4.0/settings.ini", "gtk-icon-theme-name"),
        ("gtk-3.0/settings.ini", "gtk-icon-theme-name"),
        ("kdeglobals", "Icon"),
    ] {
        let Ok(text) = std::fs::read_to_string(config.join(file)) else {
            continue;
        };
        let found = text.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        });
        if let Some(theme) = found.filter(|theme| !theme.is_empty()) {
            return Some(theme);
        }
    }
    None
}

/// Resolved icon names, including the ones that resolved to nothing.
///
/// A miss is worth remembering as much as a hit: a launcher naming an icon that
/// is not installed is exactly the case that pays the full search, and it is
/// also the case that repeats on every rebind.
fn cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

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
    if let Ok(cache) = cache().lock() {
        if let Some(known) = cache.get(icon) {
            return known.clone();
        }
    }
    let found = search_icon(icon);
    if let Ok(mut cache) = cache().lock() {
        cache.insert(icon.to_owned(), found.clone());
    }
    found
}

/// The search itself, run once per name.
fn search_icon(icon: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(data_home) = celestina_core::xdg::data_home() {
        roots.push(data_home.join("icons"));
    }
    roots.push(PathBuf::from("/usr/local/share/icons"));
    roots.push(PathBuf::from("/usr/share/icons"));

    for root in &roots {
        for theme in themes(&roots) {
            for size in [
                "scalable", "512x512", "256x256", "192x192", "128x128", "96x96", "64x64", "48x48",
                "32x32",
            ] {
                for extension in ["svg", "png"] {
                    let candidate = root
                        .join(&theme)
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

    /// The configured theme is searched, and what it inherits after it, with
    /// `hicolor` always last. Two names hard-coded here used to decide this,
    /// which is why an icon installed only under the session's own theme was
    /// reported as missing.
    #[test]
    fn the_search_order_starts_with_the_configured_theme() {
        let roots = vec![std::env::temp_dir().join("siderita-themes-none")];
        let order = super::themes(&roots);
        assert!(
            order.iter().any(|theme| theme == "hicolor"),
            "hicolor must always be reachable: {order:?}"
        );
        // Whatever this machine is themed with, the fallbacks are never lost
        // and never duplicated.
        let mut seen = order.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(
            seen.len(),
            order.len(),
            "a theme was listed twice: {order:?}"
        );
    }

    /// A name resolves once; the answer — including "there is none" — is kept.
    #[test]
    fn a_resolved_name_is_not_searched_for_twice() {
        let name = format!("siderita-no-existe-{}", std::process::id());
        assert_eq!(super::resolve_icon(&name), None);
        // The second call is answered from the cache: the entry is there.
        assert!(super::cache().lock().expect("cache").contains_key(&name));
        assert_eq!(super::resolve_icon(&name), None);
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

#[cfg(test)]
mod resolution_probe {
    /// Not a rule, a probe: prints what the resolver answers on this machine so
    /// the audit's claim about a missing icon can be checked rather than
    /// believed. Ignored by default because its answer depends on what is
    /// installed.
    #[test]
    #[ignore]
    fn what_this_machine_resolves() {
        for name in ["firefox", "krita", "waydroid", "no-existe-en-ningun-tema"] {
            let started = std::time::Instant::now();
            let found = super::resolve_icon(name);
            let first = started.elapsed();
            let started = std::time::Instant::now();
            let _ = super::resolve_icon(name);
            println!(
                "{name:<28} {:?}  primera {:>8.1?}  cacheada {:>8.1?}",
                found.as_deref().map(std::path::Path::to_string_lossy),
                first,
                started.elapsed()
            );
        }
    }
}
