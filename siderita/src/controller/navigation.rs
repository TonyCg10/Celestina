//! Navigation and the tab lifecycle: starting a tab (resolving its initial
//! location from argv/HOME or a given path), installing the scan executor,
//! seeding history, and the back / forward / up / home / open-a-path moves. Each
//! move stages a `PendingNav` whose history change only commits once its scan
//! succeeds, so a failed navigation never strands the path on an unreadable
//! folder.

use core::pin::Pin;
use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use siderita_core::{NavigationHistory, ScanExecutor};

use super::qobject;
use super::{launch_argument, PendingNav};

impl qobject::SideritaController {
    pub fn start(self: Pin<&mut Self>) {
        let initial = initial_location();
        self.start_common(initial);
    }

    /// Starts a tab directly at `location`, without the argv/HOME detour `start`
    /// uses. New tabs open on the folder that spawned them, not the first tab's
    /// initial location.
    pub fn start_at(self: Pin<&mut Self>, location: &QString) {
        let initial = resolve_location(&location.to_string(), None);
        self.start_common(initial);
    }

    fn start_common(mut self: Pin<&mut Self>, initial: PathBuf) {
        if self.rust().executor.is_none() {
            let qt_thread = self.qt_thread();
            let executor = ScanExecutor::new(move |result| {
                let _ = qt_thread.queue(move |controller| {
                    controller.handle_scan_result(result);
                });
            });
            self.as_mut().rust_mut().get_mut().executor = Some(executor);
        }

        self.as_mut().reload_bookmarks();
        self.as_mut().refresh_place_props();

        if self.rust().history.current().is_none() {
            self.as_mut().rust_mut().get_mut().history = NavigationHistory::new(initial.clone());
        }

        let destination = self
            .rust()
            .history
            .current()
            .map(Path::to_path_buf)
            .unwrap_or(initial);
        self.as_mut().request_scan(destination);
    }

    /// Repaints whatever is on screen — which is not always a folder. Trash and
    /// Recientes are locations with their own listing, and a folder rescan would
    /// land in a projection that (rightly) refuses to overwrite them, so an entry
    /// deleted from one of those views stayed on screen. Each location refreshes
    /// itself instead.
    pub fn refresh(mut self: Pin<&mut Self>) {
        if self.rust().recent_active {
            self.as_mut().open_recent();
            return;
        }
        if self.rust().trash_active {
            self.as_mut().load_trash();
            self.as_mut().publish_trash();
            return;
        }
        if let Some(location) = self.rust().history.current().map(Path::to_path_buf) {
            self.as_mut().request_scan(location);
        }
    }

    pub fn go_home(mut self: Pin<&mut Self>) {
        let destination = home_location();
        self.as_mut().request_nav_scan(PendingNav::To(destination));
    }

    pub fn go_back(mut self: Pin<&mut Self>) {
        let Some(destination) = self.rust().history.peek_back().map(Path::to_path_buf) else {
            return;
        };
        self.as_mut()
            .request_nav_scan(PendingNav::Back(destination));
    }

    pub fn go_forward(mut self: Pin<&mut Self>) {
        let Some(destination) = self.rust().history.peek_forward().map(Path::to_path_buf) else {
            return;
        };
        self.as_mut()
            .request_nav_scan(PendingNav::Forward(destination));
    }

    pub fn go_up(mut self: Pin<&mut Self>) {
        let Some(destination) = self
            .rust()
            .history
            .current()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
        else {
            return;
        };
        self.as_mut().request_nav_scan(PendingNav::To(destination));
    }

    pub fn open_location(mut self: Pin<&mut Self>, location: &QString) {
        let input = location.to_string();
        if input.is_empty() {
            self.as_mut()
                .set_error_text(QString::from("Escribe una ruta local"));
            self.as_mut()
                .set_status_text(QString::from("La ubicación está vacía"));
            return;
        }

        let destination = resolve_location(&input, self.rust().history.current());
        self.as_mut().request_nav_scan(PendingNav::To(destination));
    }
}

fn initial_location() -> PathBuf {
    match launch_argument() {
        // Accept a `file://` URI argument (e.g. from a desktop "open with").
        Some(arg) => {
            let text = arg.to_string_lossy();
            if text.starts_with("file:") {
                if let Some(path) = crate::dbus::uri_to_path(&text) {
                    return path;
                }
            }
            PathBuf::from(arg)
        }
        None => home_location(),
    }
}

fn home_location() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn resolve_location(input: &str, current: Option<&Path>) -> PathBuf {
    // A local file:// URI (typed, pasted, or from another app) → its path.
    if input.starts_with("file:") {
        if let Some(path) = crate::dbus::uri_to_path(input) {
            return path;
        }
    }

    let path = if input == "~" {
        home_location()
    } else if let Some(relative) = input.strip_prefix("~/") {
        home_location().join(relative)
    } else {
        PathBuf::from(input)
    };

    if path.is_absolute() {
        path
    } else {
        current
            .map(Path::to_path_buf)
            .unwrap_or_else(home_location)
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_location;
    use std::path::{Path, PathBuf};

    #[test]
    fn absolute_location_is_preserved() {
        assert_eq!(
            resolve_location("/tmp/una carpeta", Some(Path::new("/base"))),
            PathBuf::from("/tmp/una carpeta")
        );
    }

    #[test]
    fn relative_location_uses_current_directory() {
        assert_eq!(
            resolve_location("hija", Some(Path::new("/base"))),
            PathBuf::from("/base/hija")
        );
    }

    #[test]
    fn file_uri_resolves_to_its_local_path() {
        assert_eq!(
            resolve_location("file:///tmp/una%20carpeta", Some(Path::new("/base"))),
            PathBuf::from("/tmp/una carpeta")
        );
        // A bare relative name that merely starts with "file" is not a URI.
        assert_eq!(
            resolve_location("filename.txt", Some(Path::new("/base"))),
            PathBuf::from("/base/filename.txt")
        );
    }
}
