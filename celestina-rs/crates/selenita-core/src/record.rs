//! The screen recording: the GStreamer pipeline `gst-launch-1.0` runs, the
//! state a recording walks through and the stop file `selenita --stop`
//! touches when no running instance answers on the bus.
//!
//! The pipeline reads the ScreenCast portal's PipeWire node (`pipewiresrc`),
//! encodes it to H.264 (`vah264enc` on a VA-API render node, else `x264enc`),
//! both at a constant quality rather than a bitrate (a screen is sharp text
//! that a 2 Mbit/s budget smears), and muxes it into an MP4 ([`MUXER`], from
//! `gst-plugins-good`), with the system's sound as a second branch when
//! asked: `pipewiresrc` capturing the default sink's monitor, mixed over a
//! silent live bed ([`AUDIO_BED`] into an `audiomixer` that ignores a pad
//! with nothing on it) and encoded to AAC ([`AUDIO_ENCODER`]). The bed keeps
//! the audio branch flowing when the monitor hands over nothing, which is
//! what makes the stop finish: the launcher runs with `-e`, so the SIGINT
//! the worker sends becomes an EOS and the muxer writes its index before the
//! child exits, and a muxer pad that never saw a buffer never reaches that
//! EOS (measured in the SEL-1-D evidence).

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

/// The muxer, from `gst-plugins-good`; its absence is said at start.
pub const MUXER: &str = "mp4mux";
/// The AAC encoder of `gst-libav`.
pub const AUDIO_ENCODER: &str = "avenc_aac";
/// The silent live source under the sound branch: the mixer always has one
/// pad producing, so the branch reaches EOS whatever the monitor does.
pub const AUDIO_BED: &str = "audiotestsrc";
/// The mixer the monitor and the bed meet in.
pub const AUDIO_MIXER: &str = "audiomixer";
/// The keyframe interval, in frames: one a second at 60 fps.
pub const KEY_INTERVAL: u32 = 60;
/// The child that runs the pipeline.
pub const LAUNCHER: &str = "gst-launch-1.0";
/// The tool that says whether an element exists (`--exists`, status 0 or 1).
pub const INSPECTOR: &str = "gst-inspect-1.0";

/// The H.264 encoder a recording uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VideoEncoder {
    /// Software, always available with `gst-plugins-ugly`.
    X264,
    /// VA-API, when a render node and the element exist.
    VaH264,
}

impl VideoEncoder {
    /// The element name.
    #[must_use]
    pub fn element(self) -> &'static str {
        match self {
            Self::X264 => "x264enc",
            Self::VaH264 => "vah264enc",
        }
    }

    /// `VaH264` only with a `/dev/dri/renderD*` node and the element
    /// installed; else the software encoder.
    #[must_use]
    pub fn choose(render_node: bool, va_element: bool) -> Self {
        if render_node && va_element {
            Self::VaH264
        } else {
            Self::X264
        }
    }

    /// The encoder's own settings: a live screen with little latency, at a
    /// constant quality. `x264enc` defaults to 2048 kbit/s ABR and
    /// `vah264enc` to CBR at a bitrate it works out itself; both are replaced
    /// by a quantizer around 20 (CRF for x264, CQP for VA), the usual range
    /// for text that must stay legible.
    fn settings(self) -> &'static [&'static str] {
        match self {
            Self::X264 => &[
                "tune=zerolatency",
                "speed-preset=veryfast",
                "pass=qual",
                "quantizer=21",
                "key-int-max=60",
            ],
            Self::VaH264 => &["rate-control=cqp", "qpi=20", "qpp=22", "key-int-max=60"],
        }
    }
}

/// Where the video comes from: the portal's PipeWire node, and the remote's
/// file descriptor as the child sees it (the worker hands it over on the
/// child's standard input, so it is 0). Without one `pipewiresrc` connects
/// to the session's daemon on its own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub node: u32,
    pub fd: Option<i32>,
}

/// One recording's pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pipeline {
    pub source: Source,
    pub encoder: VideoEncoder,
    /// The system's sound as a second branch.
    pub audio: bool,
    /// The MP4 written; its folder exists.
    pub out: PathBuf,
}

impl Pipeline {
    /// The description as `gst-launch-1.0` tokens: the muxer and its file
    /// first, then the video branch and, when asked, the audio branch, both
    /// linking to `mux.`.
    #[must_use]
    pub fn description(&self) -> Vec<String> {
        let mut tokens: Vec<String> = vec![
            MUXER.to_owned(),
            "name=mux".to_owned(),
            "!".to_owned(),
            "filesink".to_owned(),
            format!("location={}", quoted(&self.out)),
        ];
        tokens.push("pipewiresrc".to_owned());
        if let Some(fd) = self.source.fd {
            tokens.push(format!("fd={fd}"));
        }
        tokens.push(format!("path={}", self.source.node));
        tokens.extend(
            [
                "do-timestamp=true",
                "!",
                "videoconvert",
                "!",
                self.encoder.element(),
            ]
            .into_iter()
            .map(str::to_owned),
        );
        tokens.extend(
            self.encoder
                .settings()
                .iter()
                .map(|word| (*word).to_owned()),
        );
        tokens.extend(
            ["!", "h264parse", "!", "queue", "!", "mux."]
                .into_iter()
                .map(str::to_owned),
        );
        if self.audio {
            // The bed first, so its stereo 48 kHz is the mixer's format and
            // the monitor is converted to it rather than the other way
            // round; the monitor joins the mixer by name.
            tokens.extend(
                [
                    AUDIO_BED,
                    "wave=silence",
                    "is-live=true",
                    "!",
                    "audio/x-raw,format=S16LE,rate=48000,channels=2",
                    "!",
                    AUDIO_MIXER,
                    "name=mix",
                    "ignore-inactive-pads=true",
                    "!",
                    "audioconvert",
                    "!",
                    "audioresample",
                    "!",
                    AUDIO_ENCODER,
                    "!",
                    "queue",
                    "!",
                    "mux.",
                    "pipewiresrc",
                    "stream-properties=\"props,stream.capture.sink=true\"",
                    "do-timestamp=true",
                    "!",
                    "audio/x-raw",
                    "!",
                    "audioconvert",
                    "!",
                    "audioresample",
                    "!",
                    "mix.",
                ]
                .into_iter()
                .map(str::to_owned),
            );
        }
        tokens
    }

    /// The whole argv: the launcher, `-e` (EOS on SIGINT, so the muxer
    /// finishes the file), `-q` (no progress on stdout) and the description.
    #[must_use]
    pub fn launch_argv(&self) -> Vec<OsString> {
        let mut argv: Vec<OsString> = vec![LAUNCHER.into(), "-e".into(), "-q".into()];
        argv.extend(self.description().into_iter().map(OsString::from));
        argv
    }
}

/// `gst-inspect-1.0 --exists <element>`: status 0 when installed.
#[must_use]
pub fn inspect_argv(element: &str) -> Vec<OsString> {
    vec![INSPECTOR.into(), "--exists".into(), element.into()]
}

/// `path` as one double-quoted token of the launcher's language, with its
/// backslashes and quotes escaped; a path with spaces stays one value.
#[must_use]
pub fn quoted(path: &Path) -> String {
    let mut text = String::from("\"");
    for character in path.to_string_lossy().chars() {
        if character == '\\' || character == '"' {
            text.push('\\');
        }
        text.push(character);
    }
    text.push('"');
    text
}

/// Where a recording is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Idle,
    /// The portal is asked for a session (the person may be choosing the
    /// output in its dialog).
    Preparing,
    Recording,
    /// The child was told to finish the file.
    Stopping,
    /// Something went wrong; the notice said what.
    Failed,
}

/// What moves a recording from one state to the next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Start,
    /// The portal answered and the child runs.
    Prepared,
    Stop,
    /// The child exited after the stop.
    Stopped,
    Failure,
    /// The failure was shown; back to idle.
    Acknowledged,
}

/// An event this state has no transition for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidTransition {
    pub state: State,
    pub event: Event,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "no transition for {:?} while {}",
            self.event,
            self.state.as_str()
        )
    }
}

impl std::error::Error for InvalidTransition {}

impl State {
    /// The word QML reads.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Preparing => "preparing",
            Self::Recording => "recording",
            Self::Stopping => "stopping",
            Self::Failed => "failed",
        }
    }

    /// Whether a recording is under way in any of its steps.
    #[must_use]
    pub fn is_busy(self) -> bool {
        matches!(self, Self::Preparing | Self::Recording | Self::Stopping)
    }

    /// The state after `event`.
    ///
    /// # Errors
    ///
    /// [`InvalidTransition`] when this state has none for the event.
    pub fn next(self, event: Event) -> Result<Self, InvalidTransition> {
        match (self, event) {
            (Self::Idle | Self::Failed, Event::Start) => Ok(Self::Preparing),
            (Self::Preparing, Event::Prepared) => Ok(Self::Recording),
            (Self::Recording, Event::Stop) => Ok(Self::Stopping),
            (Self::Stopping, Event::Stopped) => Ok(Self::Idle),
            (Self::Preparing | Self::Recording | Self::Stopping, Event::Failure) => {
                Ok(Self::Failed)
            }
            (Self::Failed, Event::Acknowledged) => Ok(Self::Idle),
            (state, event) => Err(InvalidTransition { state, event }),
        }
    }
}

/// The stop file, `runtime_dir/selenita/stop`: `selenita --stop` touches it
/// when no running instance answers on the bus, and the recording worker
/// takes it while it records.
#[must_use]
pub fn stop_file(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("selenita").join("stop")
}

/// Touches the stop file, making its folder.
///
/// # Errors
///
/// The folder or the file could not be made.
pub fn request_stop(file: &Path) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file, b"")
}

/// Removes the stop file; whether there was one (a stop requested).
#[must_use]
pub fn take_stop_request(file: &Path) -> bool {
    std::fs::remove_file(file).is_ok()
}
