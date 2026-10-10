//! Desktop-application discovery for the "Abrir con…" chooser and default-app
//! management.
//!
//! MIME classification and the default-app database are delegated to the
//! desktop's own `xdg-mime` (integration via freedesktop, not a reimplemented
//! shared-mime-info), while the candidate-app list comes from the suite's one
//! application scan, `celestina_core::desktop_entry::scan`. Everything here
//! blocks — processes and a walk of every application directory — and runs on
//! a worker thread.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use celestina_core::activation::{self, ActivationError, ActivationName, HAND_OFF_TIMEOUT};

use celestina_core::{desktop_entry, CancellationToken};

/// A launchable desktop application: its `.desktop` id and display name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopApp {
    /// The `.desktop` file name, e.g. `firefox.desktop` — the id `xdg-mime` and
    /// `gtk-launch` expect.
    pub id: String,
    /// The user-facing `Name=`.
    pub name: String,
}

/// The most desktop-file ids one chooser scan reads. A desktop has a few
/// hundred; the bound is what keeps a directory stuffed with entries from
/// turning "Abrir con…" into a long read.
const MAX_ENTRIES: usize = 4096;

/// What the "Abrir con…" chooser offers for one file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Choices {
    /// The file's MIME type, as the desktop classifies it.
    pub mime: String,
    /// The visible applications that declare it, sorted by name.
    pub apps: Vec<DesktopApp>,
    /// Which of them is the current default, if any is.
    pub default_index: Option<usize>,
}

/// Everything the chooser needs for `path`, or `None` when its type cannot be
/// classified. Runs `xdg-mime` twice and reads every `.desktop` file on the
/// system, so it belongs on a worker thread — it used to run inside the Qt
/// thread's `open_with` invokable, where a FIFO named `x.desktop` froze the
/// window for good.
pub fn choices_for(path: &Path) -> Option<Choices> {
    let mime = detect_mime(path)?;
    let apps = apps_for_mime(&mime, &CancellationToken::new()).unwrap_or_default();
    let default_index =
        default_app_id(&mime).and_then(|id| apps.iter().position(|app| app.id == id));
    Some(Choices {
        mime,
        apps,
        default_index,
    })
}

/// The visible applications that declare support for `mime`, sorted by name.
///
/// The walk is `celestina_core::desktop_entry::scan`'s — one shadowing rule
/// for the whole suite (a user `.desktop` shadows a system one of the same
/// id, whether or not it reads), every file read bounded and only when it is a
/// regular file — and "visible" is the entry's own `is_listable`.
pub fn apps_for_mime(
    mime: &str,
    cancellation: &CancellationToken,
) -> Result<Vec<DesktopApp>, desktop_entry::ScanCancelled> {
    apps_in(
        &desktop_entry::application_search_dirs(),
        mime,
        cancellation,
    )
}

/// [`apps_for_mime`] over the given application directories.
fn apps_in(
    dirs: &[std::path::PathBuf],
    mime: &str,
    cancellation: &CancellationToken,
) -> Result<Vec<DesktopApp>, desktop_entry::ScanCancelled> {
    let scanned = desktop_entry::scan(dirs, cancellation, MAX_ENTRIES)?;
    let mut apps: Vec<DesktopApp> = scanned
        .entries
        .into_iter()
        .filter(|entry| entry.is_listable() && entry.handles(mime))
        .map(|entry| DesktopApp {
            id: entry.id,
            name: entry.name,
        })
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    Ok(apps)
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

/// Grafita, Fluorita, Calcita and Hematita, as Siderita reaches a running
/// instance.
pub use celestina_core::activation::{CALCITA, FLUORITA, GRAFITA, HEMATITA};

/// Opens `paths` in the application that owns `name`, on a worker thread:
/// through the bus when an instance is running (one window, a new tab or
/// view there), else by `spawn`, which hands the program the same paths so
/// the new instance opens them itself. `done` hears the outcome on that
/// worker; it queues whatever the Qt side should learn.
pub fn open_in(
    name: ActivationName,
    paths: Vec<PathBuf>,
    spawn: impl FnOnce(&[PathBuf]) -> Result<(), String> + Send + 'static,
    done: impl FnOnce(Result<(), OpenFailure>) + Send + 'static,
) {
    let worker = std::thread::Builder::new()
        .name("siderita-open-in".to_owned())
        .spawn(move || {
            let answer = activation::open_in(name, &paths, HAND_OFF_TIMEOUT);
            done(settle(name, answer, || spawn(&paths)));
        });
    if let Err(error) = worker {
        eprintln!("siderita: cannot start the open-in worker: {error}");
    }
}

/// What one selected entry is, as far as the «Abrir en» section asks: the
/// kind is decided by the folder model and by name, never by reading the file,
/// so the menu can ask it on the Qt thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    /// A folder, or a symlink to one.
    Folder,
    /// An image, a video or a sound, by name (`fluorita_core::MediaKind`).
    Media,
    /// A PDF, by name (`.pdf`, in any case): the one type Calcita's desktop
    /// entry declares, `application/pdf`, which the generic chooser below
    /// the section reaches through `xdg-mime` and this section by name.
    Pdf,
    /// Any other file.
    File,
}

/// One entry of the «Abrir en» section: a suite application reached through
/// [`open_in`], or the send-to-phone verb through Magnetita.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Grafita,
    Fluorita,
    Calcita,
    Hematita,
    Phone,
}

impl Target {
    /// The applications, in the order the menu shows them.
    pub const APPLICATIONS: [Self; 4] =
        [Self::Grafita, Self::Fluorita, Self::Calcita, Self::Hematita];

    /// The id QML carries for this target.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Grafita => "grafita",
            Self::Fluorita => "fluorita",
            Self::Calcita => "calcita",
            Self::Hematita => "hematita",
            Self::Phone => "phone",
        }
    }

    /// The application target a QML id names; the phone has its own verb.
    #[must_use]
    pub fn application(key: &str) -> Option<Self> {
        Self::APPLICATIONS
            .into_iter()
            .find(|target| target.key() == key)
    }

    /// The activation name, which is also the desktop-file id's basename;
    /// `None` for the phone, which is not an application of its own.
    #[must_use]
    pub const fn activation_name(self) -> Option<ActivationName> {
        match self {
            Self::Grafita => Some(GRAFITA),
            Self::Fluorita => Some(FLUORITA),
            Self::Calcita => Some(CALCITA),
            Self::Hematita => Some(HEMATITA),
            Self::Phone => None,
        }
    }
}

/// The targets that apply to a selection, before asking what is installed:
/// Grafita takes any file (it decides by bytes and says so when one is not
/// text); Fluorita takes media, or one folder as a source; Calcita takes
/// PDFs; Hematita takes one folder, to see its usage; the phone takes any
/// file or files. A selection that mixes folders and files gets no
/// application, and an empty one none.
#[must_use]
pub fn suite_targets(selection: &[ItemKind]) -> Vec<Target> {
    if selection.is_empty() {
        return Vec::new();
    }
    let all_files = selection.iter().all(|kind| *kind != ItemKind::Folder);
    let all_media = selection.iter().all(|kind| *kind == ItemKind::Media);
    let all_pdf = selection.iter().all(|kind| *kind == ItemKind::Pdf);
    let one_folder = selection == [ItemKind::Folder];
    let mut targets = Vec::new();
    if all_files {
        targets.push(Target::Grafita);
    }
    if all_media || one_folder {
        targets.push(Target::Fluorita);
    }
    if all_pdf {
        targets.push(Target::Calcita);
    }
    if one_folder {
        targets.push(Target::Hematita);
    }
    if all_files {
        targets.push(Target::Phone);
    }
    targets
}

/// Which suite applications have a desktop entry, once [`probe_installed`]
/// has looked; `None` until then.
static INSTALLED: std::sync::OnceLock<Vec<Target>> = std::sync::OnceLock::new();

/// Looks up each suite application's desktop entry on a worker thread, once:
/// the walk reads the application directories, which the Qt thread never
/// does. Until it answers, the menu offers no application.
pub fn probe_installed() {
    if INSTALLED.get().is_some() {
        return;
    }
    let worker = std::thread::Builder::new()
        .name("siderita-suite-probe".to_owned())
        .spawn(|| {
            let dirs = desktop_entry::application_search_dirs();
            let found = installed_in(&dirs);
            let _ = INSTALLED.set(found);
        });
    if let Err(error) = worker {
        eprintln!("siderita: cannot start the suite probe: {error}");
    }
}

/// The application targets whose desktop entry `dirs` hold, under the scan's
/// own shadowing rule (`desktop_entry::find`).
fn installed_in(dirs: &[PathBuf]) -> Vec<Target> {
    Target::APPLICATIONS
        .into_iter()
        .filter(|target| {
            target.activation_name().is_some_and(|name| {
                desktop_entry::find(dirs, &format!("{}.desktop", name.0)).is_some()
            })
        })
        .collect()
}

/// `targets` without the applications that are not installed; the phone
/// stays, its own condition is a connected device.
#[must_use]
pub fn only_installed(targets: Vec<Target>) -> Vec<Target> {
    let installed = INSTALLED.get().map_or(&[][..], Vec::as_slice);
    retain_installed(targets, installed)
}

fn retain_installed(mut targets: Vec<Target>, installed: &[Target]) -> Vec<Target> {
    targets.retain(|target| *target == Target::Phone || installed.contains(target));
    targets
}

/// Why an `open_in` did not open anything.
#[derive(Debug, PartialEq, Eq)]
pub enum OpenFailure {
    /// An instance owns the name and refused the paths (said on stderr).
    Unanswered,
    /// Nobody owned the name and the program could not be started; the
    /// spawn's own message, already worded for the person.
    Spawn(String),
}

/// What an `open_in` answer means: delivered; start the program when nobody
/// owns the name or the bus cannot settle it (no session bus, a timeout), so
/// the launch carries on standalone as `claim` does; or a failure when an
/// owner answered and refused.
fn settle(
    name: ActivationName,
    answer: Result<bool, ActivationError>,
    spawn: impl FnOnce() -> Result<(), String>,
) -> Result<(), OpenFailure> {
    match answer {
        Ok(true) => Ok(()),
        Ok(false) => spawn().map_err(OpenFailure::Spawn),
        Err(error @ ActivationError::Refused(_)) => {
            eprintln!("siderita: {} refused the paths: {error}", name.0);
            Err(OpenFailure::Unanswered)
        }
        Err(error) => {
            eprintln!("siderita: cannot reach {}, starting it: {error}", name.0);
            spawn().map_err(OpenFailure::Spawn)
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_core::activation::ActivationError;
    use celestina_core::desktop_entry::{self, DesktopEntry};
    use std::cell::Cell;

    use super::{retain_installed, suite_targets, ItemKind, Target};

    #[test]
    fn a_text_file_goes_to_grafita_and_the_phone() {
        assert_eq!(
            suite_targets(&[ItemKind::File]),
            vec![Target::Grafita, Target::Phone]
        );
    }

    #[test]
    fn an_image_goes_to_grafita_fluorita_and_the_phone() {
        assert_eq!(
            suite_targets(&[ItemKind::Media]),
            vec![Target::Grafita, Target::Fluorita, Target::Phone]
        );
    }

    #[test]
    fn a_pdf_goes_to_grafita_calcita_and_the_phone() {
        assert_eq!(
            suite_targets(&[ItemKind::Pdf]),
            vec![Target::Grafita, Target::Calcita, Target::Phone]
        );
        assert_eq!(
            suite_targets(&[ItemKind::Pdf, ItemKind::Pdf]),
            vec![Target::Grafita, Target::Calcita, Target::Phone]
        );
    }

    #[test]
    fn one_folder_goes_to_fluorita_and_hematita_only() {
        assert_eq!(
            suite_targets(&[ItemKind::Folder]),
            vec![Target::Fluorita, Target::Hematita]
        );
        assert_eq!(suite_targets(&[ItemKind::Folder, ItemKind::Folder]), vec![]);
    }

    #[test]
    fn a_mixed_selection_keeps_only_what_applies_to_every_entry() {
        assert_eq!(
            suite_targets(&[ItemKind::File, ItemKind::Media]),
            vec![Target::Grafita, Target::Phone]
        );
        assert_eq!(
            suite_targets(&[ItemKind::Media, ItemKind::Media]),
            vec![Target::Grafita, Target::Fluorita, Target::Phone]
        );
        assert_eq!(
            suite_targets(&[ItemKind::Pdf, ItemKind::File]),
            vec![Target::Grafita, Target::Phone]
        );
        assert_eq!(
            suite_targets(&[ItemKind::Pdf, ItemKind::Media]),
            vec![Target::Grafita, Target::Phone]
        );
        assert_eq!(suite_targets(&[ItemKind::File, ItemKind::Folder]), vec![]);
        assert_eq!(suite_targets(&[]), vec![]);
    }

    #[test]
    fn an_application_that_is_not_installed_is_hidden_but_the_phone_stays() {
        let all = suite_targets(&[ItemKind::Media]);
        assert_eq!(
            retain_installed(all.clone(), &[Target::Fluorita]),
            vec![Target::Fluorita, Target::Phone]
        );
        assert_eq!(retain_installed(all, &[]), vec![Target::Phone]);
    }

    #[test]
    fn only_a_desktop_entry_marks_an_application_installed() {
        let dir = std::env::temp_dir().join(format!("siderita-suite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{}.desktop", super::GRAFITA.0)),
            "[Desktop Entry]\nType=Application\nName=Grafita\nExec=grafita %F\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("{}.desktop", super::CALCITA.0)),
            "[Desktop Entry]\nType=Application\nName=Calcita\nExec=calcita %U\n",
        )
        .unwrap();
        let found = super::installed_in(std::slice::from_ref(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(found, vec![Target::Grafita, Target::Calcita]);
    }

    #[test]
    fn a_qml_key_names_only_an_application() {
        assert_eq!(Target::application("hematita"), Some(Target::Hematita));
        assert_eq!(Target::application("calcita"), Some(Target::Calcita));
        assert_eq!(Target::application("phone"), None);
        assert_eq!(Target::application(""), None);
    }

    #[test]
    fn a_running_instance_takes_the_paths_and_nothing_is_spawned() {
        let spawned = Cell::new(false);
        let outcome = super::settle(super::GRAFITA, Ok(true), || {
            spawned.set(true);
            Ok(())
        });
        assert_eq!(outcome, Ok(()));
        assert!(!spawned.get());
    }

    #[test]
    fn the_program_is_spawned_only_when_nobody_owns_the_name() {
        let spawned = Cell::new(false);
        let outcome = super::settle(super::FLUORITA, Ok(false), || {
            spawned.set(true);
            Ok(())
        });
        assert_eq!(outcome, Ok(()));
        assert!(spawned.get());

        let spawned = Cell::new(false);
        let outcome = super::settle(
            super::HEMATITA,
            Err(ActivationError::Refused("no".to_owned())),
            || {
                spawned.set(true);
                Ok(())
            },
        );
        assert_eq!(outcome, Err(super::OpenFailure::Unanswered));
        assert!(!spawned.get(), "an owner that refused is not shadowed");
    }

    #[test]
    fn a_bus_that_cannot_settle_it_starts_the_program_standalone() {
        for answer in [
            Err(ActivationError::Bus("no session bus".to_owned())),
            Err(ActivationError::Timeout),
        ] {
            let spawned = Cell::new(false);
            let outcome = super::settle(super::GRAFITA, answer, || {
                spawned.set(true);
                Ok(())
            });
            assert_eq!(outcome, Ok(()));
            assert!(spawned.get());
        }
    }

    // The suite reads the file; this asks the question this module asks.
    fn parse(content: &str) -> Option<DesktopEntry> {
        desktop_entry::parse("test.desktop", content)
    }

    fn handles(entry: &DesktopEntry, mime: &str) -> bool {
        entry.is_listable() && entry.handles(mime)
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

    /// RS-4: a FIFO named `x.desktop` in an application directory used to
    /// block the chooser — on the Qt thread — for good. It is skipped, and the
    /// real entry beside it is still offered.
    #[test]
    fn a_fifo_named_like_an_entry_neither_blocks_nor_hides_the_others() {
        let dir = std::env::temp_dir().join(format!("siderita-apps-fifo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mk dir");
        std::fs::write(dir.join("firefox.desktop"), FIREFOX).expect("write entry");
        let made = std::process::Command::new("mkfifo")
            .arg(dir.join("trampa.desktop"))
            .status()
            .expect("run mkfifo");
        assert!(made.success());

        let apps = super::apps_in(
            std::slice::from_ref(&dir),
            "text/html",
            &celestina_core::CancellationToken::new(),
        )
        .expect("not cancelled");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            apps,
            vec![super::DesktopApp {
                id: "firefox.desktop".to_owned(),
                name: "Firefox".to_owned(),
            }]
        );
    }

    #[test]
    fn a_body_without_the_group_is_none() {
        assert!(parse("just some text\n").is_none());
    }
}
