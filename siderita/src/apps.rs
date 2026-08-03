//! Desktop-application discovery for the "Abrir con…" chooser and default-app
//! management.
//!
//! MIME classification and the default-app database are delegated to the
//! desktop's own `xdg-mime` (integration via freedesktop, not a reimplemented
//! shared-mime-info), while the candidate-app list is built by parsing the
//! `.desktop` files under the XDG application directories — the one part worth
//! doing here, and the part that is unit-testable without a session.

use std::path::Path;

use celestina_core::desktop_entry;
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
/// The visible applications that declare support for `mime`, de-duplicated by id
/// (a user `.desktop` shadows a system one of the same name) and sorted by name.
pub fn apps_for_mime(mime: &str) -> Vec<DesktopApp> {
    let mut seen = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for dir in desktop_entry::application_dirs() {
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
            let Some(parsed) = desktop_entry::parse(id, &content) else {
                continue;
            };
            // The id is claimed whether or not this entry handles the type: a
            // user override shadows the system one either way.
            seen.insert(id.to_owned());
            if parsed.is_application && !parsed.hidden && !parsed.no_display && parsed.handles(mime)
            {
                apps.push(DesktopApp {
                    id: id.to_owned(),
                    // An entry with no name is still a launchable application;
                    // its id is what to call it.
                    name: if parsed.name.is_empty() {
                        id.to_owned()
                    } else {
                        parsed.name
                    },
                });
            }
        }
    }

    apps.sort_by_key(|a| a.name.to_lowercase());
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
    use celestina_core::desktop_entry::{self, DesktopEntry};

    // The suite reads the file; this asks the question this module asks.
    fn parse(content: &str) -> Option<DesktopEntry> {
        desktop_entry::parse("test.desktop", content)
    }

    fn handles(entry: &DesktopEntry, mime: &str) -> bool {
        entry.is_application && !entry.hidden && !entry.no_display && entry.handles(mime)
    }

    const FIREFOX: &str = "\
[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox %u
MimeType=text/html;text/xml;x-scheme-handler/http;
";

    #[test]
    fn parses_name_type_and_mimetypes() {
        let entry = parse(FIREFOX).expect("entry");
        assert_eq!(entry.name, "Firefox");
        assert!(entry.is_application);
        assert!(entry.mimetypes.iter().any(|mime| mime == "text/html"));
    }

    #[test]
    fn handles_only_a_declared_mime() {
        let entry = parse(FIREFOX).expect("entry");
        assert!(handles(&entry, "text/html"));
        assert!(!handles(&entry, "image/png"));
    }

    #[test]
    fn a_hidden_or_nodisplay_entry_never_handles() {
        let hidden = parse(
            "[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\nMimeType=text/html;\n",
        )
        .expect("entry");
        assert!(!handles(&hidden, "text/html"));
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
        let entry = parse(content).expect("entry");
        assert_eq!(entry.name, "Real");
    }

    #[test]
    fn a_body_without_the_group_is_none() {
        assert!(parse("just some text\n").is_none());
    }
}
