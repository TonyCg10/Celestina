//! Desktop / OS integration: handing a file to its default handler
//! (`xdg-open`), the "Abrir con…" application chooser, and launching an external
//! terminal in the current folder. Siderita never embeds any of these — it only
//! spawns the freedesktop tools, detached, and surfaces a truthful error if one
//! cannot even be started.

use core::pin::Pin;
use std::path::{Path, PathBuf};

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QStringList};

use super::display_name;
use super::qobject;

impl qobject::SideritaController {
    /// Hands a non-directory entry to the desktop's default handler via
    /// `xdg-open`, the freedesktop way to reach a viewer/editor/player without
    /// Siderita knowing anything about the file type. The launch is fire-and-
    /// forget — `xdg-open` resolves the handler and exits — but a failure to even
    /// start it (missing binary, no handler) is surfaced truthfully as `op_error`.
    pub(crate) fn open_in_default_app(mut self: Pin<&mut Self>, path: &Path, name: &str) {
        self.as_mut().set_op_error(QString::default());
        match open_with_default(path) {
            Ok(()) => {
                let message = format!("Abriendo {name}…");
                self.as_mut()
                    .set_status_text(QString::from(message.as_str()));
            }
            Err(error) => self.as_mut().set_op_error(QString::from(error.as_str())),
        }
    }

    /// Opens the "Abrir con…" chooser for `path`: classifies its MIME type,
    /// gathers the applications that declare it (plus the current default) and
    /// publishes them for the dialog. A type that cannot be classified is
    /// reported through `op_error`.
    pub fn open_with(mut self: Pin<&mut Self>, path: &QString) {
        self.as_mut().set_op_error(QString::default());
        let path = PathBuf::from(path.to_string());
        if path.as_os_str().is_empty() {
            return;
        }

        let Some(mime) = crate::apps::detect_mime(&path) else {
            self.as_mut()
                .set_op_error(QString::from("No se pudo determinar el tipo del archivo"));
            return;
        };

        let apps = crate::apps::apps_for_mime(&mime);
        let default_id = crate::apps::default_app_id(&mime);
        let default_index = default_id
            .as_ref()
            .and_then(|id| apps.iter().position(|app| &app.id == id))
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1);

        let names: QStringList = apps
            .iter()
            .map(|app| QString::from(app.name.as_str()))
            .collect();
        let target = display_name(&path);

        {
            let state = self.as_mut().rust_mut();
            let state = state.get_mut();
            state.open_with_ids = apps.into_iter().map(|app| app.id).collect();
            state.open_with_path = path;
            state.open_with_mime = mime;
        }
        self.as_mut().set_open_with_apps(names);
        self.as_mut().set_open_with_default_index(default_index);
        self.as_mut()
            .set_open_with_target(QString::from(target.as_str()));
        self.as_mut().set_open_with_pending(true);
    }

    /// Launches the chosen application on the stored file, optionally making it
    /// the default for the file's MIME type first. Closes the chooser.
    pub fn open_with_app(mut self: Pin<&mut Self>, index: i32, set_default: bool) {
        self.as_mut().set_open_with_pending(false);
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let (id, path, mime) = {
            let state = self.rust();
            let Some(id) = state.open_with_ids.get(index).cloned() else {
                return;
            };
            (
                id,
                state.open_with_path.clone(),
                state.open_with_mime.clone(),
            )
        };

        if set_default {
            if let Err(error) = crate::apps::set_default_app(&mime, &id) {
                self.as_mut().set_op_error(QString::from(error.as_str()));
            }
        }
        match crate::apps::launch_with(&id, &path) {
            Ok(()) => {
                let message = format!("Abriendo {}…", display_name(&path));
                self.as_mut()
                    .set_status_text(QString::from(message.as_str()));
            }
            Err(error) => self.as_mut().set_op_error(QString::from(error.as_str())),
        }
    }

    pub fn cancel_open_with(mut self: Pin<&mut Self>) {
        self.as_mut().set_open_with_pending(false);
    }

    /// Launches the desktop's terminal in the current folder (an external
    /// terminal — Siderita never embeds one). A failure is surfaced truthfully.
    pub fn open_terminal(mut self: Pin<&mut Self>) {
        self.as_mut().set_op_error(QString::default());
        let Some(dir) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        if let Err(error) = open_terminal_in(&dir) {
            self.as_mut().set_op_error(QString::from(error.as_str()));
        }
    }
}

/// Launches `xdg-open PATH`, detached from Siderita's stdio, and reaps the
/// short-lived launcher on a throwaway thread so it never lingers as a zombie.
/// The opened application is reparented and outlives Siderita. Returns a
/// user-facing Spanish message if the launcher could not even be spawned.
fn open_with_default(path: &Path) -> Result<(), String> {
    spawn_opener("xdg-open", path)
}

/// Spawns `program PATH` detached from Siderita's stdio and reaps the launcher on
/// a throwaway thread. Split out from [`open_with_default`] so the spawn/error
/// contract is testable without depending on `xdg-open` being installed.
fn spawn_opener(program: &str, path: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let child = Command::new(program)
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
            Err(format!("No se encontró «{program}» para abrir el archivo"))
        }
        Err(error) => Err(format!("No se pudo abrir el archivo: {error}")),
    }
}

/// Launches an external terminal with its working directory set to `dir`.
/// Honours `$TERMINAL`, then tries a list of common emulators, spawning the
/// first that exists (they open in the inherited cwd); the launcher is detached
/// and reaped like [`spawn_opener`].
fn open_terminal_in(dir: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let mut candidates: Vec<String> = Vec::new();
    if let Some(terminal) = std::env::var_os("TERMINAL") {
        candidates.push(terminal.to_string_lossy().into_owned());
    }
    candidates.extend(
        [
            "foot",
            "alacritty",
            "kitty",
            "wezterm",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "xterm",
        ]
        .iter()
        .map(|name| (*name).to_owned()),
    );

    for program in &candidates {
        let child = Command::new(program)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Ok(());
            }
            // Not installed — try the next candidate.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("No se pudo abrir la terminal: {error}")),
        }
    }

    Err("No se encontró ninguna terminal (define $TERMINAL)".to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn spawn_opener_reports_a_missing_launcher() {
        let error =
            super::spawn_opener("siderita-no-such-launcher-xyz", Path::new("/tmp/whatever"))
                .unwrap_err();
        assert!(
            error.contains("siderita-no-such-launcher-xyz"),
            "message should name the missing launcher: {error}"
        );
    }

    #[test]
    fn spawn_opener_launches_an_existing_program() {
        // `true` ignores its argument and exits 0 — a side-effect-free stand-in
        // for xdg-open that proves the spawn path succeeds and reaps cleanly.
        super::spawn_opener("true", Path::new("/tmp/whatever"))
            .expect("spawning an existing launcher should succeed");
    }
}
