//! What the capture worker drives: the real tools, or the fakes when
//! `SELENITA_FAKE=1`.
//!
//! The real backend asks niri over `$NIRI_SOCKET`, runs `slurp`, `grim`,
//! `wl-copy` and niri's `screenshot-window` under deadlines, hands a file to
//! Fluorita through the suite's activation, asks the file manager to show one
//! (`org.freedesktop.FileManager1.ShowItems`, which Siderita serves) and moves
//! a file to the freedesktop trash through `siderita_ops`. Its tools are
//! looked up in `SELENITA_TOOLS_DIR` when that is set: a folder of shell
//! stubs named `grim`, `slurp`, `wl-copy` and `niri` (the test seam beside
//! the fakes).
//!
//! The fake takes nothing from the session: a capture writes a 1×1 PNG, the
//! clipboard and the other applications are lines on stderr, and a delete
//! removes the file. Every method blocks and runs on the capture worker.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use celestina_core::activation;
use celestina_core::{file_uri, CancellationToken};
use selenita_core::niri::{self, Output, Window};
use selenita_core::{runner, tools, Geometry, Target};

use crate::capture::CaptureError;

/// How long `grim` and niri's window action may take.
const GRAB_DEADLINE: Duration = Duration::from_secs(10);
/// How long a person may take to draw a region.
const SELECT_DEADLINE: Duration = Duration::from_secs(120);
/// How long `wl-copy` may take to take the picture.
const COPY_DEADLINE: Duration = Duration::from_secs(5);
/// niri writes the window's file after its action answered: how long the
/// worker looks for it.
const WINDOW_FILE_WAIT: Duration = Duration::from_secs(3);

/// The smallest valid PNG: one opaque pixel. The fake's every capture.
pub const PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xb0, 0x9b, 0xf8, 0xff,
    0x3f, 0x00, 0x05, 0xad, 0x02, 0xce, 0x27, 0xd5, 0xca, 0xee, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// The session as the capture worker sees it.
pub trait Backend: Send {
    /// The enabled outputs, left to right.
    fn outputs(&mut self) -> Result<Vec<Output>, CaptureError>;
    /// Every window, with when each last had the focus.
    fn windows(&mut self) -> Result<Vec<Window>, CaptureError>;
    /// Lets the person draw a region; `None` when they cancelled.
    fn select_region(
        &mut self,
        accent: &str,
        background: &str,
    ) -> Result<Option<Geometry>, CaptureError>;
    /// Writes `target` as a PNG to `out`, an absolute path that does not
    /// exist yet.
    fn grab(&mut self, target: &Target, out: &Path) -> Result<(), CaptureError>;
    /// Puts `png` on the clipboard.
    fn copy_png(&mut self, png: &[u8]) -> Result<(), CaptureError>;
    fn open_in_fluorita(&mut self, path: &Path) -> Result<(), CaptureError>;
    fn show_in_siderita(&mut self, path: &Path) -> Result<(), CaptureError>;
    /// Moves `path` to the trash.
    fn trash(&mut self, path: &Path) -> Result<(), CaptureError>;
}

/// The backend `fake` asks for.
pub fn make(fake: bool) -> Box<dyn Backend> {
    if fake {
        Box::new(Fake::default())
    } else {
        Box::new(Real {
            tools_dir: std::env::var_os("SELENITA_TOOLS_DIR")
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        })
    }
}

struct Real {
    tools_dir: Option<PathBuf>,
}

impl Real {
    fn run(
        &self,
        argv: Vec<OsString>,
        stdin: Option<&[u8]>,
        deadline: Duration,
    ) -> Result<runner::Output, CaptureError> {
        let argv = tools::resolve(argv, self.tools_dir.as_deref());
        runner::run(&argv, stdin, deadline).map_err(CaptureError::Tool)
    }
}

impl Backend for Real {
    fn outputs(&mut self) -> Result<Vec<Output>, CaptureError> {
        let socket = niri::socket().map_err(CaptureError::Niri)?;
        niri::outputs(&socket).map_err(CaptureError::Niri)
    }

    fn windows(&mut self) -> Result<Vec<Window>, CaptureError> {
        let socket = niri::socket().map_err(CaptureError::Niri)?;
        niri::windows(&socket).map_err(CaptureError::Niri)
    }

    fn select_region(
        &mut self,
        accent: &str,
        background: &str,
    ) -> Result<Option<Geometry>, CaptureError> {
        match self.run(tools::slurp_argv(accent, background), None, SELECT_DEADLINE) {
            Ok(output) => String::from_utf8_lossy(&output.stdout)
                .parse::<Geometry>()
                .map(Some)
                .map_err(|_| CaptureError::Selection),
            // slurp ends with status 1 when the person presses Escape, saying
            // "selection cancelled" or nothing; any other complaint is an
            // error the notice reports.
            Err(CaptureError::Tool(runner::RunError::Failed {
                code: Some(1),
                stderr,
                ..
            })) if slurp_cancelled(&stderr) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn grab(&mut self, target: &Target, out: &Path) -> Result<(), CaptureError> {
        if let Target::Window { id, .. } = target {
            // niri's IPC gives a tiled window no position, so grim cannot
            // be told where it is; niri takes the window itself, by id.
            self.run(tools::niri_window_argv(out, Some(*id)), None, GRAB_DEADLINE)?;
            let started = Instant::now();
            while !written(out) {
                if started.elapsed() > WINDOW_FILE_WAIT {
                    return Err(CaptureError::NoPicture);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            return Ok(());
        }
        self.run(tools::grim_argv(target, out, None), None, GRAB_DEADLINE)?;
        Ok(())
    }

    fn copy_png(&mut self, png: &[u8]) -> Result<(), CaptureError> {
        // wl-copy forks the server that keeps the clipboard: it must
        // inherit no pipe of ours.
        let argv = tools::resolve(tools::wl_copy_argv(), self.tools_dir.as_deref());
        runner::run_quiet(&argv, Some(png), COPY_DEADLINE).map_err(CaptureError::Tool)
    }

    fn open_in_fluorita(&mut self, path: &Path) -> Result<(), CaptureError> {
        let paths = [path.to_path_buf()];
        match activation::open_in(activation::FLUORITA, &paths, activation::HAND_OFF_TIMEOUT) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(error) => eprintln!("selenita: Fluorita did not take the file: {error}"),
        }
        let mut child = std::process::Command::new("fluorita")
            .arg(path)
            .spawn()
            .map_err(|error| CaptureError::Open(error.to_string()))?;
        // Reaped on a thread of its own so it never lingers as a zombie.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    fn show_in_siderita(&mut self, path: &Path) -> Result<(), CaptureError> {
        let uri = file_uri::from_path(path).ok_or_else(|| CaptureError::Show(String::new()))?;
        let show = || -> zbus::Result<()> {
            let connection = zbus::blocking::connection::Builder::session()?
                .method_timeout(activation::HAND_OFF_TIMEOUT)
                .build()?;
            let proxy = zbus::blocking::Proxy::new(
                &connection,
                "org.freedesktop.FileManager1",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1",
            )?;
            proxy.call::<_, _, ()>("ShowItems", &(vec![uri.as_str()], ""))
        };
        show().map_err(|error| CaptureError::Show(error.to_string()))
    }

    fn trash(&mut self, path: &Path) -> Result<(), CaptureError> {
        siderita_ops::trash(path, &CancellationToken::new(), &mut |_| {})
            .map(|_| ())
            .map_err(|error| CaptureError::Trash(error.to_string()))
    }
}

/// Whether slurp's stderr after status 1 means the person cancelled.
fn slurp_cancelled(stderr: &str) -> bool {
    let stderr = stderr.trim();
    stderr.is_empty() || stderr.eq_ignore_ascii_case("selection cancelled")
}

fn written(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.len() > 0)
}

/// `SELENITA_FAKE=1`: nothing leaves the process but a file in the folder
/// the test chose and a line on stderr. Selenita (window 1) has the focus
/// after a fake window (2); both are on `FAKE-1`. `grabbed` keeps every
/// target taken, for the tests.
#[derive(Default)]
pub struct Fake {
    pub grabbed: Vec<Target>,
}

impl Backend for Fake {
    fn outputs(&mut self) -> Result<Vec<Output>, CaptureError> {
        Ok(vec![Output {
            name: "FAKE-1".to_owned(),
            logical: Geometry {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
        }])
    }

    fn windows(&mut self) -> Result<Vec<Window>, CaptureError> {
        let window = |id: u64, app_id: &str, secs: u64| Window {
            id,
            app_id: app_id.to_owned(),
            title: "Fake".to_owned(),
            size: (640, 480),
            position: None,
            workspace_id: Some(1),
            focused_at: Some(Duration::from_secs(secs)),
        };
        Ok(vec![
            window(1, activation::SELENITA.0, 20),
            window(2, "fake.window", 10),
        ])
    }

    fn select_region(
        &mut self,
        _accent: &str,
        _background: &str,
    ) -> Result<Option<Geometry>, CaptureError> {
        Ok(Some(Geometry {
            x: 10,
            y: 10,
            w: 100,
            h: 100,
        }))
    }

    fn grab(&mut self, target: &Target, out: &Path) -> Result<(), CaptureError> {
        eprintln!("selenita-fake: grab {} to {}", target.kind(), out.display());
        self.grabbed.push(target.clone());
        std::fs::write(out, PIXEL_PNG).map_err(|error| CaptureError::Write(error.to_string()))
    }

    fn copy_png(&mut self, png: &[u8]) -> Result<(), CaptureError> {
        eprintln!("selenita-fake: copy {} bytes", png.len());
        Ok(())
    }

    fn open_in_fluorita(&mut self, path: &Path) -> Result<(), CaptureError> {
        eprintln!("selenita-fake: open in Fluorita {}", path.display());
        Ok(())
    }

    fn show_in_siderita(&mut self, path: &Path) -> Result<(), CaptureError> {
        eprintln!("selenita-fake: show in Siderita {}", path.display());
        Ok(())
    }

    fn trash(&mut self, path: &Path) -> Result<(), CaptureError> {
        std::fs::remove_file(path).map_err(|error| CaptureError::Trash(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::slurp_cancelled;

    #[test]
    fn only_a_quiet_or_cancelled_slurp_is_a_cancel() {
        assert!(slurp_cancelled(""));
        assert!(slurp_cancelled("selection cancelled\n"));
        assert!(!slurp_cancelled("failed to connect to the compositor"));
    }
}
