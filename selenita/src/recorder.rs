//! The recorders the recording worker drives (`record.rs`): the real one
//! (the ScreenCast portal, `gst-launch-1.0` children and the `ffmpeg` join)
//! and the fake `SELENITA_FAKE=1` asks for. Every method blocks and runs on
//! the worker.
//!
//! A recording is one portal session and one or more children: a pause
//! finishes the current child's segment and keeps the session ([`Recorder::
//! finish`]); a resume asks the same session for a fresh PipeWire remote
//! ([`Recorder::reopen`]) and starts the next child; the stop finishes the
//! last child, closes the session ([`Recorder::release`]) and, when there
//! is more than one segment, joins them ([`Recorder::join`]).

use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use selenita_core::niri::{self, NiriError};
use selenita_core::pick;
use selenita_core::record::{
    inspect_argv, Pipeline, Source, VideoEncoder, INSPECTOR, LAUNCHER, MUXER,
};
use selenita_core::runner::{self, Exit, RunError, Running};
use selenita_core::segments::{concat_list, join_argv, join_deadline};
use selenita_core::tools;

use crate::portal;
use crate::record::{RecordError, STOP_DEADLINE};

/// How long `gst-inspect-1.0` may take per element.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);
/// How long a finished child gets to reach its EOS before an empty file
/// says it never received a buffer: such a child holds the interrupt until
/// [`STOP_DEADLINE`], and nothing of it would be lost by killing it.
pub const EOS_GRACE: Duration = Duration::from_secs(1);
/// How often a finishing child is looked at.
const FINISH_POLL: Duration = Duration::from_millis(25);
/// A render node VA-API can use.
const RENDER_NODE: &str = "/dev/dri/renderD128";

/// What the recorder found installed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Probe {
    /// `gst-inspect-1.0`, which the other answers come from.
    pub inspector: bool,
    pub launcher: bool,
    pub muxer: bool,
    /// A VA-API render node exists.
    pub render_node: bool,
    /// `vah264enc` is installed.
    pub va_element: bool,
}

impl Probe {
    /// The first thing a recording needs and the host lacks.
    #[must_use]
    pub fn missing(self) -> Option<&'static str> {
        if !self.inspector {
            Some(INSPECTOR)
        } else if !self.launcher {
            Some(LAUNCHER)
        } else if !self.muxer {
            Some(MUXER)
        } else {
            None
        }
    }
}

/// The source the recorder prepared, and whatever keeps it open.
pub struct Prepared {
    pub source: Source,
    pub fd: Option<OwnedFd>,
}

/// The session as the recording worker sees it. Every method blocks.
pub trait Recorder: Send {
    /// What is installed; run once at start.
    fn probe(&mut self) -> Probe;
    /// The name of the output niri has focused: a focused start's monitor.
    fn focused_output(&mut self) -> Result<String, NiriError>;
    /// Asks for the monitor (the person may be choosing). `pick` is where a
    /// focused start left its request: the real portal's chooser reads it
    /// itself (`selenita --pick-output`); the fake reads it as that chooser
    /// would.
    fn prepare(&mut self, pick: Option<&Path>) -> Result<Prepared, RecordError>;
    /// The remote for the next segment, on the session `prepare` opened and
    /// without asking the person again: a fresh one when the first segment
    /// had one, `None` when it had none.
    fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError>;
    /// Starts the pipeline into `pipeline.out`, handing `fd` to the child.
    fn start(&mut self, pipeline: &Pipeline, fd: Option<OwnedFd>) -> Result<(), RecordError>;
    /// `Some` once the child exited on its own.
    fn exited(&mut self) -> Option<Exit>;
    /// Tells the child to finish its file `out` (SIGINT, EOS) and waits for
    /// it; the portal's session stays open for a resume. A child still
    /// running after [`EOS_GRACE`] with `out` still empty never received a
    /// buffer: it is killed and the answer is [`RecordError::NoSignal`].
    /// Otherwise it gets [`STOP_DEADLINE`], after which it is killed.
    fn finish(&mut self, out: &Path) -> Result<(), RecordError>;
    /// Closes the portal's session: the compositor stops casting.
    fn release(&mut self);
    /// Joins `segments` (whose concat list is the file `list`) into `out`,
    /// within `segments::join_deadline` of their size.
    fn join(&mut self, list: &Path, segments: &[PathBuf], out: &Path) -> Result<(), RecordError>;
    /// Kills whatever runs and releases the session.
    fn abandon(&mut self);
}

/// The recorder `fake` asks for.
pub fn make(fake: bool) -> Box<dyn Recorder> {
    if fake {
        Box::new(Fake::default())
    } else {
        Box::new(Real {
            tools_dir: std::env::var_os("SELENITA_TOOLS_DIR")
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
            session: None,
            remote: false,
            child: None,
        })
    }
}

struct Real {
    tools_dir: Option<PathBuf>,
    session: Option<portal::Session>,
    /// The first segment read through the portal's own remote.
    remote: bool,
    child: Option<Running>,
}

impl Real {
    fn has_element(&self, element: &str) -> bool {
        let argv = tools::resolve(inspect_argv(element), self.tools_dir.as_deref());
        runner::run(&argv, None, PROBE_DEADLINE).is_ok()
    }
}

impl Recorder for Real {
    fn probe(&mut self) -> Probe {
        let installed = |argv: Vec<std::ffi::OsString>| {
            let argv = tools::resolve(argv, self.tools_dir.as_deref());
            !matches!(
                runner::run(&argv, None, PROBE_DEADLINE),
                Err(RunError::Missing(_))
            )
        };
        let inspector = installed(inspect_argv(MUXER));
        let launcher = installed(vec![LAUNCHER.into(), "--version".into()]);
        Probe {
            inspector,
            launcher,
            muxer: inspector && self.has_element(MUXER),
            render_node: Path::new(RENDER_NODE).exists(),
            va_element: inspector && self.has_element(VideoEncoder::VaH264.element()),
        }
    }

    fn focused_output(&mut self) -> Result<String, NiriError> {
        let socket = niri::socket()?;
        niri::focused_output(&socket).map(|output| output.name)
    }

    fn prepare(&mut self, _pick: Option<&Path>) -> Result<Prepared, RecordError> {
        let (session, stream) = portal::open_monitor().map_err(RecordError::Portal)?;
        self.session = Some(session);
        self.remote = stream.fd.is_some();
        // The one line a live diagnosis needs: the node the `Start` response
        // named and whether the remote came with it.
        eprintln!(
            "selenita: recording: portal stream node {}, remote fd {}",
            stream.node,
            if stream.fd.is_some() { "yes" } else { "no" }
        );
        let fd_number = stream.fd.as_ref().map(|_| 0);
        Ok(Prepared {
            source: Source {
                node: stream.node,
                fd: fd_number,
            },
            fd: stream.fd,
        })
    }

    fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError> {
        let session = self.session.as_ref().ok_or(RecordError::NotRecording)?;
        if !self.remote {
            return Ok(None);
        }
        let fd = session.remote().map_err(RecordError::Portal)?;
        eprintln!("selenita: recording: a fresh remote for the next segment");
        Ok(Some(fd))
    }

    fn start(&mut self, pipeline: &Pipeline, fd: Option<OwnedFd>) -> Result<(), RecordError> {
        let argv = tools::resolve(pipeline.launch_argv(), self.tools_dir.as_deref());
        let child = runner::start(&argv, fd).map_err(RecordError::Tool)?;
        self.child = Some(child);
        Ok(())
    }

    fn exited(&mut self) -> Option<Exit> {
        let exit = self.child.as_mut()?.poll()?;
        self.child = None;
        Some(exit)
    }

    fn finish(&mut self, out: &Path) -> Result<(), RecordError> {
        let mut child = self.child.take().ok_or(RecordError::NotRecording)?;
        if let Err(error) = child.interrupt() {
            let _ = child.kill();
            return Err(RecordError::Tool(error));
        }
        let asked = Instant::now();
        loop {
            if let Some(exit) = child.poll() {
                return if exit.code == Some(0) {
                    Ok(())
                } else {
                    Err(RecordError::Exited(exit))
                };
            }
            let waited = asked.elapsed();
            let written = std::fs::metadata(out).map_or(0, |meta| meta.len());
            if waited >= EOS_GRACE && written == 0 {
                let _ = child.kill();
                return Err(RecordError::NoSignal);
            }
            if waited >= STOP_DEADLINE {
                return Err(RecordError::Exited(child.kill()));
            }
            std::thread::sleep(FINISH_POLL);
        }
    }

    fn release(&mut self) {
        // Closing the session tells the compositor to stop casting.
        self.session = None;
        self.remote = false;
    }

    fn join(&mut self, list: &Path, segments: &[PathBuf], out: &Path) -> Result<(), RecordError> {
        let argv = tools::resolve(join_argv(list, out), self.tools_dir.as_deref());
        let bytes = segments
            .iter()
            .filter_map(|segment| std::fs::metadata(segment).ok())
            .map(|meta| meta.len())
            .sum();
        runner::run(&argv, None, join_deadline(bytes))
            .map(|_| ())
            .map_err(|error| RecordError::Join(error.to_string()))
    }

    fn abandon(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
        self.release();
    }
}

/// `SELENITA_FAKE=1`: no portal, no child, no `ffmpeg`. A start writes a
/// few bytes to the segment and the finish a few more, so the recording
/// lands in the history like a real one; the join checks that the list it
/// is handed names the segments as `concat_list` writes them and joins
/// their bytes in that order (the fakes never run a tool: a byte join keeps
/// the smoke free of GStreamer, which the host lacked when the recording
/// was built, and the real list and argv are run against `ffmpeg` in the
/// crate's tests). Nothing leaves the process but a line on stderr. Its
/// niri has `FAKE-1` focused (a test sets `focused` to `None` for a niri
/// that cannot say), and its portal consumes a pick request as `selenita
/// --pick-output` would, keeping the output it named in `chosen`.
pub struct Fake {
    out: Option<PathBuf>,
    pub started: Vec<Pipeline>,
    /// Every call, in order: `focused_output`, `prepare`, `reopen`,
    /// `start`, `finish` (and `kill` when its grace went by), `release`,
    /// `join`, `abandon`.
    pub calls: Vec<&'static str>,
    pub focused: Option<String>,
    /// The output the last `prepare` took from a pick request, if any.
    pub chosen: Option<String>,
    /// The portal's session is gone: a resume cannot reopen it.
    pub reopen_fails: bool,
    /// `ffmpeg` fails the join.
    pub join_fails: bool,
    /// The next children write their file only at their EOS, as `mp4mux`
    /// does when the frames are few: empty until the finish.
    pub writes_at_eos: bool,
    /// The next children never write and never reach their EOS (their node
    /// never sends a buffer): the finish kills them after the grace.
    pub never_exits: bool,
    /// The next finish fails as a child that exits unsuccessfully.
    pub finish_fails: bool,
    /// Runs inside the finish, as something the window does meanwhile.
    pub on_finish: Option<Box<dyn FnMut() + Send>>,
    /// Runs inside the join, as something the window does meanwhile.
    pub on_join: Option<Box<dyn FnMut() + Send>>,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            out: None,
            started: Vec::new(),
            calls: Vec::new(),
            focused: Some(FAKE_OUTPUT.to_owned()),
            chosen: None,
            reopen_fails: false,
            join_fails: false,
            writes_at_eos: false,
            never_exits: false,
            finish_fails: false,
            on_finish: None,
            on_join: None,
        }
    }
}

/// The node the fake portal answers with.
pub const FAKE_NODE: u32 = 1;
/// The output the fake niri has focused, as the capture backend's fake.
pub const FAKE_OUTPUT: &str = "FAKE-1";

impl Recorder for Fake {
    fn probe(&mut self) -> Probe {
        Probe {
            inspector: true,
            launcher: true,
            muxer: true,
            render_node: false,
            va_element: false,
        }
    }

    fn focused_output(&mut self) -> Result<String, NiriError> {
        self.calls.push("focused_output");
        self.focused.clone().ok_or(NiriError::NoFocusedOutput)
    }

    fn prepare(&mut self, pick: Option<&Path>) -> Result<Prepared, RecordError> {
        self.calls.push("prepare");
        self.chosen = match pick.map(|file| pick::take_request(file, SystemTime::now())) {
            Some(Ok(pick::Taken::Fresh(request))) => Some(request.output().to_owned()),
            _ => None,
        };
        match &self.chosen {
            Some(output) => {
                eprintln!("selenita-fake: the chooser took {output} without its dialog");
            }
            None => eprintln!("selenita-fake: the chooser's dialog asks"),
        }
        Ok(Prepared {
            source: Source {
                node: FAKE_NODE,
                fd: None,
            },
            fd: None,
        })
    }

    fn reopen(&mut self) -> Result<Option<OwnedFd>, RecordError> {
        self.calls.push("reopen");
        if self.reopen_fails {
            return Err(RecordError::Portal(portal::PortalError::Bus(
                "the session is gone".to_owned(),
            )));
        }
        Ok(None)
    }

    fn start(&mut self, pipeline: &Pipeline, _fd: Option<OwnedFd>) -> Result<(), RecordError> {
        self.calls.push("start");
        eprintln!("selenita-fake: record to {}", pipeline.out.display());
        let first: &[u8] = if self.writes_at_eos || self.never_exits {
            b""
        } else {
            b"fake mp4 "
        };
        std::fs::write(&pipeline.out, first)
            .map_err(|error| RecordError::Write(error.to_string()))?;
        self.out = Some(pipeline.out.clone());
        self.started.push(pipeline.clone());
        Ok(())
    }

    fn exited(&mut self) -> Option<Exit> {
        None
    }

    fn finish(&mut self, _out: &Path) -> Result<(), RecordError> {
        self.calls.push("finish");
        if let Some(meanwhile) = self.on_finish.as_mut() {
            meanwhile();
        }
        let out = self.out.take().ok_or(RecordError::NotRecording)?;
        let written = std::fs::metadata(&out).map_or(0, |meta| meta.len());
        if self.never_exits && written == 0 {
            // The grace went by with nothing written.
            self.calls.push("kill");
            return Err(RecordError::NoSignal);
        }
        if self.writes_at_eos && written == 0 {
            return std::fs::write(&out, b"fake mp4 finished")
                .map_err(|error| RecordError::Write(error.to_string()));
        }
        if self.finish_fails {
            return Err(RecordError::Exited(Exit {
                code: Some(1),
                stderr: "fake: the child failed".to_owned(),
            }));
        }
        let mut bytes =
            std::fs::read(&out).map_err(|error| RecordError::Write(error.to_string()))?;
        bytes.extend_from_slice(b"finished");
        std::fs::write(&out, bytes).map_err(|error| RecordError::Write(error.to_string()))
    }

    fn release(&mut self) {
        self.calls.push("release");
    }

    fn join(&mut self, list: &Path, segments: &[PathBuf], out: &Path) -> Result<(), RecordError> {
        self.calls.push("join");
        if let Some(meanwhile) = self.on_join.as_mut() {
            meanwhile();
        }
        let join = |detail: String| RecordError::Join(detail);
        if self.join_fails {
            return Err(join("ffmpeg: exit 1".to_owned()));
        }
        let written = std::fs::read(list).map_err(|error| join(error.to_string()))?;
        let expected = concat_list(segments).map_err(|error| join(error.to_string()))?;
        if written != expected {
            return Err(join("the list does not name the segments".to_owned()));
        }
        let mut bytes = Vec::new();
        for segment in segments {
            bytes.extend(std::fs::read(segment).map_err(|error| join(error.to_string()))?);
        }
        std::fs::write(out, bytes).map_err(|error| join(error.to_string()))?;
        eprintln!(
            "selenita-fake: join {} segments to {}",
            segments.len(),
            out.display()
        );
        Ok(())
    }

    fn abandon(&mut self) {
        self.calls.push("abandon");
        self.out = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{Real, Recorder, EOS_GRACE};
    use crate::record::RecordError;
    use selenita_core::runner;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// A real recorder whose child is `script` run by `sh` with the segment
    /// as `$1`, as `gst-launch-1.0 -e` would hold it.
    fn real_with(script: &str, out: &std::path::Path) -> Real {
        let argv: Vec<OsString> = ["sh", "-c", script, "sh"]
            .into_iter()
            .map(OsString::from)
            .chain([out.as_os_str().to_os_string()])
            .collect();
        Real {
            tools_dir: None,
            session: None,
            remote: false,
            child: Some(runner::start(&argv, None).expect("a child")),
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("selenita-finish-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    /// A child that writes its file only when the interrupt asks for its
    /// EOS is waited for and kept, however empty the file was before.
    #[test]
    fn a_child_that_writes_at_its_eos_is_waited_for() {
        let dir = scratch("eos");
        let out = dir.join("segment.mp4");
        std::fs::write(&out, b"").expect("empty");
        let mut real = real_with(
            "trap 'printf finished > \"$1\"; exit 0' INT; while :; do sleep 0.05; done",
            &out,
        );
        // The trap is set before the interrupt arrives.
        std::thread::sleep(Duration::from_millis(200));
        real.finish(&out).expect("finished");
        assert_eq!(std::fs::read(&out).expect("file"), b"finished");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A child that ignores the interrupt with nothing written is killed
    /// once the grace went by, not at the stop's deadline.
    #[test]
    fn a_child_that_never_reaches_its_eos_with_nothing_written_is_killed_after_the_grace() {
        let dir = scratch("never");
        let out = dir.join("segment.mp4");
        std::fs::write(&out, b"").expect("empty");
        let mut real = real_with("trap '' INT; exec sleep 30", &out);
        std::thread::sleep(Duration::from_millis(200));
        let asked = Instant::now();
        let result = real.finish(&out);
        let waited = asked.elapsed();
        assert!(matches!(result, Err(RecordError::NoSignal)), "{result:?}");
        assert!(waited >= EOS_GRACE, "{waited:?}");
        assert!(waited < EOS_GRACE + Duration::from_secs(2), "{waited:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
