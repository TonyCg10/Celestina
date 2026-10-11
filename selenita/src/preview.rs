//! The corner preview's work on the capture worker: what the preview shows
//! of a result (the picture's shape, a recording's poster and length) and
//! the hand-off of the file to Fluorita's editor when the preview is
//! clicked.
//!
//! The poster is the recording's first frame, written by
//! `gst-launch-1.0` ([`selenita_core::poster::poster_argv`]) into the
//! runtime folder's private `selenita/poster-<n>.png` within
//! [`POSTER_DEADLINE`]; without a runtime folder there is no poster (never
//! a predictable name in a shared folder), and when it fails the preview
//! shows the film glyph alone. The hand-off is
//! ADR 0012's PRV-1 contract: `org.celestina.Fluorita1.Edit(key)` on the
//! owner of `org.celestina.Fluorita` when there is one, else
//! `fluorita --edit <path>` started detached with no standard streams.
//! [`Editor`] is the seam the tests use to tell the call from the spawn.

use std::fs::File;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use celestina_core::activation::{self, FLUORITA};
use celestina_core::{pathkey, xdg};
use selenita_core::poster::{mp4_duration, png_size, poster_argv};
use selenita_core::runner::{self, RunError};
use selenita_core::{tools, Entry, EntryKind};

use crate::backend::Backend;
use crate::capture::CaptureError;

/// Fluorita's own interface, served beside the shared activation one at
/// Fluorita's activation object path (ADR 0012, PRV-1).
const EDIT_INTERFACE: &str = "org.celestina.Fluorita1";
/// How long the poster's `gst-launch-1.0` may take.
pub const POSTER_DEADLINE: Duration = Duration::from_secs(3);

/// What the preview shows of a published result.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    /// The file itself: what a drag offers and a click edits.
    pub path: PathBuf,
    pub kind: EntryKind,
    /// A recording's first frame; `None` for a picture, or when the frame
    /// could not be taken.
    pub poster: Option<PathBuf>,
    /// Height over width of what the preview shows; `None` when unknown.
    pub aspect: Option<f64>,
    /// A recording's length; `None` for a picture or when unknown.
    pub length: Option<Duration>,
}

static NEXT_POSTER: AtomicU64 = AtomicU64::new(1);

/// Where the next poster goes: `poster-<n>.png` in `staging`.
fn poster_file(staging: &Path) -> PathBuf {
    staging.join(format!(
        "poster-{}.png",
        NEXT_POSTER.fetch_add(1, Ordering::Relaxed)
    ))
}

/// The private folder posters go to: the runtime folder's `selenita`;
/// `None` without a runtime folder (the recording then has no poster).
/// Blocking.
pub fn poster_folder() -> Option<PathBuf> {
    xdg::runtime_dir()
        .ok()
        .map(|runtime| runtime.join("selenita"))
}

/// The preview of `entry`, just published. A recording's poster goes to
/// `staging`, made private (`0700`) when needed; with no `staging` there is
/// none. `previous`, the last preview's poster, is removed, as the preview
/// it belonged to is replaced, and becomes this one's. Blocking: it reads
/// the file's head and, for a recording, runs the poster's child.
pub fn of(
    backend: &mut dyn Backend,
    entry: &Entry,
    staging: Option<&Path>,
    previous: &mut Option<PathBuf>,
) -> Preview {
    if let Some(old) = previous.take() {
        let _ = std::fs::remove_file(old);
    }
    match entry.kind {
        EntryKind::Screenshot => Preview {
            path: entry.path.clone(),
            kind: entry.kind,
            poster: None,
            aspect: picture_aspect(&entry.path),
            length: None,
        },
        EntryKind::Recording => {
            let poster = staging.and_then(|staging| take_poster(backend, &entry.path, staging));
            previous.clone_from(&poster);
            Preview {
                path: entry.path.clone(),
                kind: entry.kind,
                aspect: poster.as_deref().and_then(picture_aspect),
                poster,
                length: File::open(&entry.path).ok().and_then(mp4_duration),
            }
        }
    }
}

/// The recording's first frame in a new file in `staging`; `None` (and no
/// file) when it could not be taken.
fn take_poster(backend: &mut dyn Backend, video: &Path, staging: &Path) -> Option<PathBuf> {
    if let Err(error) = xdg::ensure_private_dir(staging) {
        eprintln!("selenita: poster folder: {error}");
        return None;
    }
    let out = poster_file(staging);
    match backend.poster(video, &out) {
        Ok(()) if picture_aspect(&out).is_some() => Some(out),
        Ok(()) => {
            eprintln!("selenita: poster: no picture in {}", out.display());
            let _ = std::fs::remove_file(&out);
            None
        }
        Err(error) => {
            eprintln!("selenita: poster: {error}");
            let _ = std::fs::remove_file(&out);
            None
        }
    }
}

/// Height over width of the PNG at `path`, from its header.
fn picture_aspect(path: &Path) -> Option<f64> {
    let mut head = [0u8; 24];
    File::open(path).ok()?.read_exact(&mut head).ok()?;
    let (width, height) = png_size(&head)?;
    Some(f64::from(height) / f64::from(width))
}

/// Runs the poster's `gst-launch-1.0` (from `tools_dir` when given, the
/// test seam) under [`POSTER_DEADLINE`].
///
/// # Errors
///
/// The runner's: a missing launcher, the deadline or a failed pipeline.
pub fn extract_poster(tools_dir: Option<&Path>, video: &Path, out: &Path) -> Result<(), RunError> {
    let argv = tools::resolve(poster_argv(video, out), tools_dir);
    runner::run(&argv, None, POSTER_DEADLINE).map(|_| ())
}

/// How a file reached Fluorita.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    /// A running Fluorita took it through `Edit`.
    Called,
    /// None ran: `fluorita --edit` was started.
    Spawned,
}

/// What the hand-off needs from the session.
pub trait Editor {
    /// Whether `org.celestina.Fluorita` has an owner.
    fn running(&mut self) -> Result<bool, String>;
    /// `org.celestina.Fluorita1.Edit(key)` on that owner.
    fn edit(&mut self, key: &str) -> Result<(), String>;
    /// `fluorita --edit <path>`, detached.
    fn spawn(&mut self, path: &Path) -> Result<(), String>;
}

/// Hands `path` to Fluorita's editor: `Edit` with its key when Fluorita
/// runs, else a new `fluorita --edit`. A bus that cannot be asked counts as
/// nobody running: the new Fluorita then finds its own way.
///
/// # Errors
///
/// [`CaptureError::Edit`] when the call or the start failed; the file is
/// untouched.
pub fn hand_off(editor: &mut dyn Editor, path: &Path) -> Result<Route, CaptureError> {
    let running = editor.running().unwrap_or_else(|error| {
        eprintln!("selenita: cannot ask the bus for Fluorita: {error}");
        false
    });
    if running {
        editor
            .edit(&pathkey::encode(path))
            .map(|()| Route::Called)
            .map_err(CaptureError::Edit)
    } else {
        editor
            .spawn(path)
            .map(|()| Route::Spawned)
            .map_err(CaptureError::Edit)
    }
}

/// The session's Fluorita: one connection for the question and the call.
#[derive(Default)]
pub struct Session {
    connection: Option<zbus::blocking::Connection>,
}

impl Session {
    fn connection(&mut self) -> Result<&zbus::blocking::Connection, String> {
        if self.connection.is_none() {
            let connection = zbus::blocking::connection::Builder::session()
                .map(|builder| builder.method_timeout(activation::HAND_OFF_TIMEOUT))
                .and_then(zbus::blocking::connection::Builder::build)
                .map_err(|error| error.to_string())?;
            self.connection = Some(connection);
        }
        self.connection
            .as_ref()
            .ok_or_else(|| "no session bus".to_owned())
    }
}

impl Editor for Session {
    fn running(&mut self) -> Result<bool, String> {
        let connection = self.connection()?;
        let bus =
            zbus::blocking::fdo::DBusProxy::new(connection).map_err(|error| error.to_string())?;
        let name = zbus::names::BusName::try_from(FLUORITA.0).map_err(|error| error.to_string())?;
        bus.name_has_owner(name).map_err(|error| error.to_string())
    }

    fn edit(&mut self, key: &str) -> Result<(), String> {
        let connection = self.connection()?;
        let path = activation::object_path(&FLUORITA);
        let proxy =
            zbus::blocking::Proxy::new(connection, FLUORITA.0, path.as_str(), EDIT_INTERFACE)
                .map_err(|error| error.to_string())?;
        proxy
            .call::<_, _, ()>("Edit", &(key,))
            .map_err(|error| error.to_string())
    }

    fn spawn(&mut self, path: &Path) -> Result<(), String> {
        // Its own process group and no stream of ours: the editor outlives
        // Selenita and a signal to Selenita's group never reaches it.
        let mut child = Command::new("fluorita")
            .arg("--edit")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|error| error.to_string())?;
        // Reaped on a thread of its own so it never lingers as a zombie;
        // without that thread the editor still runs, and only its exit
        // status waits for Selenita's.
        let reaper = std::thread::Builder::new()
            .name("selenita-fluorita-reaper".to_owned())
            .spawn(move || {
                let _ = child.wait();
            });
        if let Err(error) = reaper {
            eprintln!("selenita: Fluorita runs unreaped: {error}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{hand_off, of, Editor, Route};
    use crate::backend::{self, PIXEL_PNG};
    use crate::capture::CaptureError;
    use selenita_core::{Entry, EntryKind};
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    /// A Fluorita that runs or not, recording what it was asked.
    struct Tally {
        running: Result<bool, String>,
        fails: bool,
        seen: Vec<String>,
    }

    impl Tally {
        fn new(running: Result<bool, String>) -> Self {
            Self {
                running,
                fails: false,
                seen: Vec::new(),
            }
        }
    }

    impl Editor for Tally {
        fn running(&mut self) -> Result<bool, String> {
            self.running.clone()
        }

        fn edit(&mut self, key: &str) -> Result<(), String> {
            self.seen.push(format!("Edit({key})"));
            if self.fails {
                Err("UnknownMethod".to_owned())
            } else {
                Ok(())
            }
        }

        fn spawn(&mut self, path: &Path) -> Result<(), String> {
            self.seen.push(format!("spawn --edit {}", path.display()));
            if self.fails {
                Err("No such file or directory".to_owned())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn a_running_fluorita_is_asked_with_the_files_key() {
        let mut fluorita = Tally::new(Ok(true));
        let path = Path::new("/pictures/Capturas/Captura 1.png");
        assert_eq!(hand_off(&mut fluorita, path).ok(), Some(Route::Called));
        assert_eq!(
            fluorita.seen,
            [format!("Edit({})", celestina_core::pathkey::encode(path))]
        );
    }

    #[test]
    fn without_fluorita_or_a_bus_a_new_one_is_started() {
        for running in [Ok(false), Err("no bus".to_owned())] {
            let mut fluorita = Tally::new(running);
            let path = Path::new("/videos/Recordings/Clip 1.mp4");
            assert_eq!(hand_off(&mut fluorita, path).ok(), Some(Route::Spawned));
            assert_eq!(
                fluorita.seen,
                ["spawn --edit /videos/Recordings/Clip 1.mp4"]
            );
        }
    }

    #[test]
    fn a_failed_hand_off_is_an_edit_error_in_spanish() {
        for running in [Ok(true), Ok(false)] {
            let mut fluorita = Tally::new(running);
            fluorita.fails = true;
            let error = hand_off(&mut fluorita, Path::new("/x.png")).expect_err("fails");
            assert!(matches!(error, CaptureError::Edit(_)));
            assert!(error.message_es().contains("Fluorita"));
        }
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("selenita-preview-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn entry(path: PathBuf, kind: EntryKind) -> Entry {
        Entry {
            path,
            kind,
            taken_at: SystemTime::now(),
            size: 1,
        }
    }

    #[test]
    fn a_picture_is_its_own_preview_in_its_own_shape() {
        let scratch = Scratch::new("picture");
        let shot = scratch.0.join("Captura 1.png");
        std::fs::write(&shot, PIXEL_PNG).expect("shot");
        let mut fake = backend::make(true);
        let mut previous = None;
        let preview = of(
            fake.as_mut(),
            &entry(shot.clone(), EntryKind::Screenshot),
            Some(&scratch.0.join("run")),
            &mut previous,
        );
        assert_eq!(preview.path, shot);
        assert_eq!(preview.poster, None);
        assert_eq!(preview.aspect, Some(1.0));
        assert_eq!(preview.length, None);
        assert!(!scratch.0.join("run").exists(), "no poster for a picture");
    }

    /// The fake's poster is its 1×1 PNG; the next recording's poster
    /// replaces it, file and all.
    #[test]
    fn a_recording_gets_a_poster_that_the_next_one_replaces() {
        let scratch = Scratch::new("recording");
        let clip = scratch.0.join("Clip 1.mp4");
        std::fs::write(&clip, b"fake mp4 finished").expect("clip");
        let staging = scratch.0.join("run");
        let mut fake = backend::make(true);
        let mut previous = None;
        let first = of(
            fake.as_mut(),
            &entry(clip.clone(), EntryKind::Recording),
            Some(&staging),
            &mut previous,
        );
        let poster = first.poster.clone().expect("a poster");
        let mode = std::os::unix::fs::PermissionsExt::mode(
            &std::fs::metadata(&staging).expect("staging").permissions(),
        );
        assert_eq!(mode & 0o777, 0o700, "the poster folder is private");
        assert_eq!(poster.parent(), Some(staging.as_path()));
        let name = poster
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        assert!(name.is_some_and(|name| name.starts_with("poster-") && name.ends_with(".png")));
        assert_eq!(std::fs::read(&poster).expect("png"), PIXEL_PNG);
        assert_eq!(first.aspect, Some(1.0));
        assert_eq!(first.length, None, "the fake's file has no movie header");
        assert_eq!(previous.as_ref(), Some(&poster));

        let second = of(
            fake.as_mut(),
            &entry(clip.clone(), EntryKind::Recording),
            Some(&staging),
            &mut previous,
        );
        assert!(!poster.exists(), "the replaced poster is gone");
        let second = second.poster.expect("a second poster");
        assert!(second != poster && second.exists());

        // Without a runtime folder: no poster at all, the film glyph.
        let none = of(
            fake.as_mut(),
            &entry(clip, EntryKind::Recording),
            None,
            &mut previous,
        );
        assert_eq!(none.poster, None);
        assert_eq!(none.aspect, None);
        assert!(!second.exists(), "the replaced poster is gone");
    }

    /// A poster that cannot be taken leaves no file and no picture: the
    /// preview shows the film glyph alone.
    #[test]
    fn a_failed_poster_leaves_the_film_glyph_alone() {
        struct Broken;
        impl crate::backend::Backend for Broken {
            fn outputs(&mut self) -> Result<Vec<selenita_core::niri::Output>, CaptureError> {
                Ok(Vec::new())
            }
            fn windows(&mut self) -> Result<Vec<selenita_core::niri::Window>, CaptureError> {
                Ok(Vec::new())
            }
            fn select_region(
                &mut self,
                _: &str,
                _: &str,
            ) -> Result<Option<selenita_core::Geometry>, CaptureError> {
                Ok(None)
            }
            fn grab(&mut self, _: &selenita_core::Target, _: &Path) -> Result<(), CaptureError> {
                Ok(())
            }
            fn copy_png(&mut self, _: &[u8]) -> Result<(), CaptureError> {
                Ok(())
            }
            fn open_in_fluorita(&mut self, _: &Path) -> Result<(), CaptureError> {
                Ok(())
            }
            fn edit_in_fluorita(&mut self, _: &Path) -> Result<(), CaptureError> {
                Ok(())
            }
            fn show_in_siderita(&mut self, _: &Path) -> Result<(), CaptureError> {
                Ok(())
            }
            fn trash(&mut self, _: &Path) -> Result<(), CaptureError> {
                Ok(())
            }
            fn poster(&mut self, _: &Path, out: &Path) -> Result<(), CaptureError> {
                // A child that wrote half a file and failed.
                std::fs::write(out, b"\x89PNG").map_err(|e| CaptureError::Write(e.to_string()))?;
                Err(CaptureError::Tool(
                    selenita_core::runner::RunError::Deadline("gst-launch-1.0".to_owned()),
                ))
            }
        }
        let scratch = Scratch::new("broken");
        let staging = scratch.0.join("run");
        let mut previous = None;
        let preview = of(
            &mut Broken,
            &entry(scratch.0.join("Clip 1.mp4"), EntryKind::Recording),
            Some(&staging),
            &mut previous,
        );
        assert_eq!(preview.poster, None);
        assert_eq!(preview.aspect, None);
        assert_eq!(previous, None);
        let left = std::fs::read_dir(&staging)
            .map(Iterator::count)
            .unwrap_or(0);
        assert_eq!(left, 0, "no half-written poster stays");
    }
}
