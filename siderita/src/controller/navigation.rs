//! Navigation and the tab lifecycle: starting a tab (resolving its initial
//! location from argv/HOME or a given path), installing the scan executor,
//! seeding history, and the back / forward / up / home / open-a-path moves. Each
//! move stages a `PendingNav` whose history change only commits once its scan
//! succeeds, so a failed navigation never strands the path on an unreadable
//! folder.

use core::pin::Pin;
use std::path::{Path, PathBuf};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use siderita_core::{NavigationHistory, ScanExecutor};

use super::display::display_name;
use super::qobject;
use super::{launch_argument, PendingNav};
use crate::pathkey;

impl qobject::SideritaController {
    pub fn start(self: Pin<&mut Self>) {
        let initial = initial_location();
        self.start_common(initial);
    }

    /// Starts a tab directly at the folder `location` keys, without the
    /// argv/HOME detour `start` uses. New tabs open on the folder that spawned
    /// them, not the first tab's initial location. A key that names nothing —
    /// including the empty one a fresh tab carries — falls back to the same
    /// initial location `start` would have chosen.
    pub fn start_at(self: Pin<&mut Self>, location: &QString) {
        let initial = pathkey::decode(location).unwrap_or_else(|_| initial_location());
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

    /// Navigates to the folder `key` names. This is what every part of the
    /// interface that already holds a location uses — a crumb, a place, a
    /// bookmark, a starred folder, a row being activated — because none of them
    /// has to spell a path to say which folder it means.
    pub fn open_key(mut self: Pin<&mut Self>, key: &QString) {
        let Some(destination) = self.as_mut().accept_key(key) else {
            return;
        };
        self.as_mut().request_nav_scan(PendingNav::To(destination));
    }

    /// The breadcrumbs for the folder being shown, as `name\tkey` lines.
    pub fn path_segments(&self) -> QStringList {
        let Some(location) = self.rust().history.current() else {
            return QStringList::default();
        };
        crumbs(location, &self.rust().phones)
            .iter()
            .map(|(name, path)| {
                QString::from(format!("{name}\t{}", pathkey::encode(path)).as_str())
            })
            .collect()
    }

    /// The key for `name` inside the folder being shown.
    pub fn child_key(&self, name: &QString) -> QString {
        let name = name.to_string();
        // A separator would leave the chosen folder, and `.` / `..` name a
        // directory rather than a file to write.
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            return QString::default();
        }
        self.rust()
            .history
            .current()
            .map(|current| pathkey::publish(&current.join(&name)))
            .unwrap_or_default()
    }
}

/// The crumb trail for `location`: one `(display name, path)` pair per level.
///
/// A Magnetita mount is an implementation path, not navigation context, so
/// `/run/user/.../magnetita/<id>` collapses into a single device crumb and only
/// real folders appear beneath it. Composed here rather than in QML because
/// joining path components is a path operation, and a crumb has to carry the
/// exact bytes its click will navigate to.
fn crumbs(location: &Path, phones: &[crate::devices::Device]) -> Vec<(String, PathBuf)> {
    if let Some(device) = phones.iter().find(|phone| {
        !phone.mount_path.is_empty() && !phone.name.is_empty() && {
            let mount = Path::new(&phone.mount_path);
            location == mount || location.starts_with(mount)
        }
    }) {
        let mount = PathBuf::from(&device.mount_path);
        let mut trail = vec![(device.name.clone(), mount.clone())];
        if let Ok(relative) = location.strip_prefix(&mount) {
            let mut walked = mount;
            for part in relative.components() {
                walked = walked.join(part);
                trail.push((
                    part.as_os_str().to_string_lossy().into_owned(),
                    walked.clone(),
                ));
            }
        }
        return trail;
    }

    let mut trail = Vec::new();
    let mut walked = PathBuf::new();
    for part in location.components() {
        walked = walked.join(part);
        trail.push((display_name(&walked), walked.clone()));
    }
    trail
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
    use super::{crumbs, resolve_location};
    use crate::devices::Device;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    #[test]
    fn crumbs_walk_every_level_and_keep_the_bytes() {
        let location = PathBuf::from(OsStr::from_bytes(b"/home/u/na\xffme"));
        let trail = crumbs(&location, &[]);
        let names: Vec<&str> = trail.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["/", "home", "u", "na\u{fffd}me"]);
        // The last crumb navigates to the real directory, not to its display.
        assert_eq!(trail[3].1, location);
    }

    #[test]
    fn a_phone_mount_collapses_into_one_device_crumb() {
        let phones = vec![Device {
            name: "Galaxy S25 Ultra".to_owned(),
            mount_path: "/run/user/1000/magnetita/689da02afffe4b12".to_owned(),
            ..Device::default()
        }];
        let location = PathBuf::from("/run/user/1000/magnetita/689da02afffe4b12/DCIM");
        let trail = crumbs(&location, &phones);
        assert_eq!(
            trail
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["Galaxy S25 Ultra", "DCIM"]
        );
        assert_eq!(trail[1].1, location);
    }

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
