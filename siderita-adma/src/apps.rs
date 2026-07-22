//! Desktop-application discovery for the "Abrir con…" chooser and default-app
//! management.
//!
//! MIME classification and the default-app database are delegated to the
//! desktop's own `xdg-mime` (integration via freedesktop, not a reimplemented
//! shared-mime-info), while the candidate-app list is built by parsing the
//! `.desktop` files under the XDG application directories — the one part worth
//! doing here, and the part that is unit-testable without a session.

use std::path::Path;
use std::process::{Command, Stdio};

/// A launchable desktop application: its `.desktop` id and display name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopApp {
    /// The `.desktop` file name, e.g. `firefox.desktop` — the id `xdg-mime` and
    /// `gtk-launch` expect.
    pub id: String,
    /// The user-facing `Name=`.
    pub name: String,
}

/// The fields of a `[Desktop Entry]` group this module cares about.
struct ParsedEntry {
    name: Option<String>,
    is_application: bool,
    hidden: bool,
    no_display: bool,
    mimetypes: Vec<String>,
}

/// Parses the `[Desktop Entry]` group of a `.desktop` file body. Only that first
/// group is read; later action groups are ignored. Returns `None` if there is no
/// `[Desktop Entry]` group at all.
fn parse_desktop_entry(content: &str) -> Option<ParsedEntry> {
    let mut in_group = false;
    let mut entry = ParsedEntry {
        name: None,
        is_application: false,
        hidden: false,
        no_display: false,
        mimetypes: Vec::new(),
    };
    let mut seen_group = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_group = line == "[Desktop Entry]";
            if in_group {
                seen_group = true;
            }
            continue;
        }
        if !in_group || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            // Prefer the unlocalized Name; ignore Name[xx] variants.
            "Name" => entry.name = Some(value.to_owned()),
            "Type" => entry.is_application = value == "Application",
            "Hidden" => entry.hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => entry.no_display = value.eq_ignore_ascii_case("true"),
            "MimeType" => {
                entry.mimetypes = value
                    .split(';')
                    .filter(|mime| !mime.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            _ => {}
        }
    }

    seen_group.then_some(entry)
}

/// Whether a parsed entry is a visible application that declares `mime`.
fn entry_handles(entry: &ParsedEntry, mime: &str) -> bool {
    entry.is_application
        && !entry.hidden
        && !entry.no_display
        && entry.mimetypes.iter().any(|declared| declared == mime)
}

/// The XDG application directories, most-specific (user) first, so a user
/// override of a system `.desktop` id wins.
fn application_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();

    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".local").join("share"))
        });
    if let Some(data_home) = data_home {
        dirs.push(data_home.join("applications"));
    }

    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .map(|raw| raw.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    for dir in data_dirs.split(':').filter(|part| !part.is_empty()) {
        dirs.push(std::path::Path::new(dir).join("applications"));
    }

    dirs
}

/// The visible applications that declare support for `mime`, de-duplicated by id
/// (a user `.desktop` shadows a system one of the same name) and sorted by name.
pub fn apps_for_mime(mime: &str) -> Vec<DesktopApp> {
    let mut seen = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for dir in application_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
                continue;
            }
            let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if seen.contains(id) {
                continue; // a more specific dir already provided this id
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(parsed) = parse_desktop_entry(&content) else {
                continue;
            };
            seen.insert(id.to_owned());
            if entry_handles(&parsed, mime) {
                apps.push(DesktopApp {
                    id: id.to_owned(),
                    name: parsed.name.unwrap_or_else(|| id.to_owned()),
                });
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Classifies `path`'s MIME type via `xdg-mime query filetype`, the desktop's
/// own database. Returns `None` if the tool is missing or gives nothing.
pub fn detect_mime(path: &Path) -> Option<String> {
    let output = Command::new("xdg-mime")
        .args(["query", "filetype"])
        .arg(path.as_os_str())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mime = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!mime.is_empty()).then_some(mime)
}

/// The default application id registered for `mime`, via `xdg-mime query
/// default`, or `None` if there is none.
pub fn default_app_id(mime: &str) -> Option<String> {
    let output = Command::new("xdg-mime")
        .args(["query", "default", mime])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!id.is_empty()).then_some(id)
}

/// Registers `id` as the default application for `mime` via `xdg-mime default`.
pub fn set_default_app(mime: &str, id: &str) -> Result<(), String> {
    let status = Command::new("xdg-mime")
        .args(["default", id, mime])
        .status()
        .map_err(|error| format!("No se pudo ejecutar «xdg-mime»: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("«xdg-mime» no pudo fijar la aplicación predeterminada".to_owned())
    }
}

/// Launches `path` with the application `id`, detached and reaped on a throwaway
/// thread, via `gtk-launch` (which applies the `.desktop` Exec field codes).
pub fn launch_with(id: &str, path: &Path) -> Result<(), String> {
    let child = Command::new("gtk-launch")
        .arg(id)
        .arg(path.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match child {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("No se encontró «gtk-launch» para abrir el archivo".to_owned())
        }
        Err(error) => Err(format!("No se pudo abrir el archivo: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{entry_handles, parse_desktop_entry};

    const FIREFOX: &str = "\
[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox %u
MimeType=text/html;text/xml;x-scheme-handler/http;
";

    #[test]
    fn parses_name_type_and_mimetypes() {
        let entry = parse_desktop_entry(FIREFOX).expect("entry");
        assert_eq!(entry.name.as_deref(), Some("Firefox"));
        assert!(entry.is_application);
        assert!(entry.mimetypes.iter().any(|mime| mime == "text/html"));
    }

    #[test]
    fn handles_only_a_declared_mime() {
        let entry = parse_desktop_entry(FIREFOX).expect("entry");
        assert!(entry_handles(&entry, "text/html"));
        assert!(!entry_handles(&entry, "image/png"));
    }

    #[test]
    fn a_hidden_or_nodisplay_entry_never_handles() {
        let hidden = parse_desktop_entry(
            "[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\nMimeType=text/html;\n",
        )
        .expect("entry");
        assert!(!entry_handles(&hidden, "text/html"));
    }

    #[test]
    fn only_the_desktop_entry_group_is_read() {
        // A later action group with its own Name must not override the entry.
        let content = "\
[Desktop Entry]
Type=Application
Name=Real
MimeType=text/plain;

[Desktop Action new]
Name=Ventana nueva
";
        let entry = parse_desktop_entry(content).expect("entry");
        assert_eq!(entry.name.as_deref(), Some("Real"));
    }

    #[test]
    fn a_body_without_the_group_is_none() {
        assert!(parse_desktop_entry("just some text\n").is_none());
    }
}
