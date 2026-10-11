//! Trimming a video's duration, frame-accurately, by an `ffmpeg` child.
//!
//! [ADR 0009](../../../../docs/decisions/0009-editing-without-an-encoder.md)
//! keeps every encoder out of the dependency closure and, since `PRV-1`,
//! makes one exception: a video's duration trim. The cut is re-encoded, so
//! it lands on the frame the person chose rather than on the keyframe before
//! it — a recording with a keyframe a second would otherwise be cut a whole
//! second wrong — and it is *raster*-class: a new file is produced, and the
//! surface says so. The encoder is `/usr/bin/ffmpeg`, run as a child process
//! and never linked: it ships in the `ffmpeg` package libmpv already needs.
//!
//! This module is the pure half: the span a person chose ([`Span`]), the
//! encoder the suite's rule picks ([`VideoEncoder`]), the exact argument
//! list ([`trim_argv`]), what the child's `-progress` and `-encoders`
//! output say ([`Progress`], [`Encoders`]), and what `ffprobe` says of the
//! film before a save ([`probe_argv`], [`SourceFacts`]): its frame, and the
//! streams a trim leaves out. A trim always writes MP4 with the film's main
//! video and one audio stream; everything else is dropped, and said so. Running the child, cancelling it
//! and placing its file are the host's; nothing here spawns, reads a clock
//! or touches a file.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod copy;

/// The program the host runs, found on `PATH` as any child is.
pub const PROGRAM: &str = "ffmpeg";
/// The program that describes a film before it is trimmed; it ships with
/// [`PROGRAM`], in the same package.
pub const PROBE_PROGRAM: &str = "ffprobe";

/// The frame assumed when the film's own rate is unknown: one frame at 30
/// frames a second.
///
/// [`SourceFacts::frame`] gives the real one when `ffprobe` can tell it. A
/// film whose frame is longer than the one assumed can still come out
/// empty; the host refuses a result the child reports no frame for
/// ([`Progress::frames`]), so that is an error the person reads, never an
/// empty file beside theirs.
pub const MIN_SPAN: Duration = Duration::from_nanos(33_333_334);

/// The shortest and longest frame believed: a rate between 1 and 1000
/// frames a second. A container's claim outside them is no rate at all.
const SHORTEST_FRAME: Duration = Duration::from_millis(1);
const LONGEST_FRAME: Duration = Duration::from_secs(1);

/// The part of a film that is kept: from `start` up to, not including,
/// `end`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: Duration,
    pub end: Duration,
}

impl Span {
    /// A span of a film `length` long whose rate is unknown: at least
    /// [`MIN_SPAN`].
    ///
    /// # Errors
    ///
    /// As [`Span::framed`].
    pub fn new(start: Duration, end: Duration, length: Duration) -> Result<Self, TrimError> {
        Self::framed(start, end, length, MIN_SPAN)
    }

    /// A span of a film `length` long whose frames last `frame`.
    ///
    /// # Errors
    ///
    /// [`TrimError::Reversed`] unless `start < end`, [`TrimError::PastTheEnd`]
    /// when `end` is past `length`, and [`TrimError::TooShort`] when the span
    /// is shorter than one frame.
    pub fn framed(
        start: Duration,
        end: Duration,
        length: Duration,
        frame: Duration,
    ) -> Result<Self, TrimError> {
        if start >= end {
            return Err(TrimError::Reversed);
        }
        if end > length {
            return Err(TrimError::PastTheEnd);
        }
        if end - start < frame {
            return Err(TrimError::TooShort);
        }
        Ok(Self { start, end })
    }

    /// Whether this span is the whole film, which a trim has nothing to save
    /// for.
    #[must_use]
    pub fn is_whole(self, length: Duration) -> bool {
        self.start.is_zero() && self.end >= length
    }

    /// How long the result is.
    #[must_use]
    pub fn length(self) -> Duration {
        self.end - self.start
    }
}

/// The H.264 encoder a trim uses: the rule Selenita's recordings follow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VideoEncoder {
    /// VA-API on this render node, at a constant quantizer.
    Vaapi { device: PathBuf },
    /// Software, at a constant rate factor.
    X264,
}

impl VideoEncoder {
    /// VA-API only with a `/dev/dri/renderD*` node and `h264_vaapi` in the
    /// child's encoder list; else `libx264`.
    #[must_use]
    pub fn choose(render_node: Option<PathBuf>, has_h264_vaapi: bool) -> Self {
        match render_node {
            Some(device) if has_h264_vaapi => Self::Vaapi { device },
            _ => Self::X264,
        }
    }

    /// The encoder's own tokens. Both aim at the same quality as Selenita's
    /// recordings: a quantizer of 20 for VA (CQP), a rate factor of 21 for
    /// x264, fast enough for a person waiting on it.
    fn tokens(&self) -> Vec<OsString> {
        match self {
            Self::Vaapi { device } => {
                let mut tokens = vec![OsString::from("-vaapi_device"), device.clone().into()];
                tokens.extend(
                    [
                        "-vf",
                        "format=nv12,hwupload",
                        "-c:v",
                        "h264_vaapi",
                        "-qp",
                        "20",
                    ]
                    .map(OsString::from),
                );
                tokens
            }
            // 4:2:0, which every player decodes: x264 would otherwise keep a
            // 4:4:4 or 10-bit source's format and write a film phones refuse.
            Self::X264 => [
                "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "21", "-preset", "veryfast",
            ]
            .map(OsString::from)
            .to_vec(),
        }
    }
}

/// The child's whole command line, program first.
///
/// The seek and the end are input options, so the decoder starts at the
/// keyframe before `start` and drops every frame before it: the cut is exact.
/// The audio is re-encoded to AAC too — copying it would cut on its own
/// packets, not on the chosen frame. Only the main video and one audio
/// stream are kept: `-sn -dn` leaves subtitles and data out, so what the
/// result holds never depends on which subtitle MP4 can carry. `-nostdin`
/// (and the host's null stdin) keeps the child from reading a terminal;
/// `-nostats` keeps its standard error to messages, so a failure's last
/// line is the error; `-progress pipe:1` is what the host reads into
/// [`Progress`]. The paths are passed as they are, byte for
/// byte, each as one token.
#[must_use]
pub fn trim_argv(input: &Path, output: &Path, span: Span, encoder: &VideoEncoder) -> Vec<OsString> {
    let mut argv: Vec<OsString> = [PROGRAM, "-hide_banner", "-nostdin", "-nostats", "-y", "-ss"]
        .map(OsString::from)
        .to_vec();
    argv.push(seconds(span.start).into());
    argv.push("-to".into());
    argv.push(seconds(span.end).into());
    argv.push("-i".into());
    argv.push(input.as_os_str().to_os_string());
    argv.extend(encoder.tokens());
    argv.extend(
        [
            "-c:a",
            "aac",
            "-b:a",
            "160k",
            "-sn",
            "-dn",
            "-movflags",
            "+faststart",
            "-progress",
            "pipe:1",
        ]
        .map(OsString::from),
    );
    argv.push(output.as_os_str().to_os_string());
    argv
}

/// A time as `ffmpeg` reads it, to the millisecond and cut down, never
/// rounded up: a handle on a frame's first instant must not move past it.
fn seconds(time: Duration) -> String {
    format!("{}.{:03}", time.as_secs(), time.subsec_millis())
}

/// What the child's `-progress pipe:1` lines say, folded as they arrive.
#[derive(Clone, Debug)]
pub struct Progress {
    span: Duration,
    frames: u64,
    ended: bool,
}

impl Progress {
    #[must_use]
    pub fn new(span: Span) -> Self {
        Self {
            span: span.length(),
            frames: 0,
            ended: false,
        }
    }

    /// Reads one `key=value` line. Returns the share of the span written so
    /// far, `0.0..=1.0`, when the line moved it; `None` for every other line,
    /// including a time the child could not state (`N/A`).
    pub fn read(&mut self, line: &str) -> Option<f64> {
        let (key, value) = line.trim().split_once('=')?;
        match key {
            "out_time_us" => {
                let written: i64 = value.trim().parse().ok()?;
                let total = self.span.as_micros() as f64;
                if total <= 0.0 {
                    return None;
                }
                Some((written as f64 / total).clamp(0.0, 1.0))
            }
            "frame" => {
                if let Ok(frames) = value.trim().parse() {
                    self.frames = frames;
                }
                None
            }
            "progress" => {
                self.ended = value.trim() == "end";
                None
            }
            _ => None,
        }
    }

    /// How many frames the child says it wrote.
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    /// Whether the child reported its last progress block.
    #[must_use]
    pub const fn ended(&self) -> bool {
        self.ended
    }
}

/// The H.264 encoders `ffmpeg -hide_banner -encoders` lists.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Encoders {
    pub h264_vaapi: bool,
    pub libx264: bool,
}

impl Encoders {
    /// Reads the listing: one encoder a line, its flags then its name. A
    /// name counts only whole (`libx264rgb` is not `libx264`).
    #[must_use]
    pub fn parse(listing: &str) -> Self {
        let mut found = Self::default();
        for line in listing.lines() {
            let mut fields = line.split_whitespace();
            let (Some(_flags), Some(name)) = (fields.next(), fields.next()) else {
                continue;
            };
            match name {
                "h264_vaapi" => found.h264_vaapi = true,
                "libx264" => found.libx264 = true,
                _ => {}
            }
        }
        found
    }
}

/// The `ffprobe` command line that describes `input`: each stream's kind,
/// rates and whether it is a cover picture, and the container, one
/// `key=value|…` line each.
#[must_use]
pub fn probe_argv(input: &Path) -> Vec<OsString> {
    let mut argv: Vec<OsString> = [
        PROBE_PROGRAM,
        "-hide_banner",
        "-v",
        "error",
        "-show_entries",
        "stream=codec_type,avg_frame_rate,r_frame_rate:stream_disposition=attached_pic:format=format_name",
        "-of",
        "compact=p=0",
    ]
    .map(OsString::from)
    .to_vec();
    argv.push(input.as_os_str().to_os_string());
    argv
}

/// What `ffprobe` said of a film, as far as a trim needs it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceFacts {
    frame: Option<Duration>,
    videos: usize,
    audios: usize,
    others: usize,
}

impl SourceFacts {
    /// Reads [`probe_argv`]'s output. Lines it does not understand are
    /// skipped; a film it understands nothing of has no known frame and
    /// leaves nothing out.
    #[must_use]
    pub fn parse(output: &str) -> Self {
        let mut facts = Self::default();
        for line in output.lines() {
            let fields: Vec<(&str, &str)> = line
                .split('|')
                .filter_map(|field| field.trim().split_once('='))
                .collect();
            let value = |key: &str| {
                fields
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, value)| *value)
            };
            let Some(kind) = value("codec_type") else {
                continue;
            };
            let cover = value("disposition:attached_pic") == Some("1");
            match kind {
                "video" if !cover => {
                    if facts.videos == 0 {
                        facts.frame = value("avg_frame_rate")
                            .and_then(frame_of)
                            .or_else(|| value("r_frame_rate").and_then(frame_of));
                    }
                    facts.videos += 1;
                }
                "audio" => facts.audios += 1,
                _ => facts.others += 1,
            }
        }
        facts
    }

    /// How long one frame of the main video lasts, when the container says
    /// so plausibly.
    #[must_use]
    pub const fn frame(&self) -> Option<Duration> {
        self.frame
    }

    /// How many streams a trim leaves out: every one but the main video and
    /// the first audio — further audio tracks, subtitles, cover pictures,
    /// data.
    #[must_use]
    pub fn left_out(&self) -> usize {
        self.videos.saturating_sub(1) + self.audios.saturating_sub(1) + self.others
    }
}

/// One frame of a rate written `num/den`, rounded to the nanosecond; `None`
/// for `0/0` and for any rate outside 1 to 1000 frames a second.
fn frame_of(rate: &str) -> Option<Duration> {
    let (num, den) = rate.trim().split_once('/')?;
    let num: u128 = num.parse().ok()?;
    let den: u128 = den.parse().ok()?;
    if num == 0 || den == 0 {
        return None;
    }
    let nanos = (den * 1_000_000_000 + num / 2) / num;
    let frame = Duration::from_nanos(u64::try_from(nanos).ok()?);
    (SHORTEST_FRAME..=LONGEST_FRAME)
        .contains(&frame)
        .then_some(frame)
}

/// Why a trim was refused or did not land. The original is never touched by
/// any of them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrimError {
    /// The span does not run forward.
    Reversed,
    /// The span ends after the film does.
    PastTheEnd,
    /// The span is shorter than [`MIN_SPAN`].
    TooShort,
    /// The span is the whole film: there is nothing to save.
    Unchanged,
    /// The file is not a video.
    NotAVideo,
    /// No `ffmpeg` on `PATH`.
    ToolMissing,
    /// `ffmpeg` lists neither H.264 encoder this trim can use.
    EncoderMissing,
    /// The child exited unsuccessfully: its status, `None` for a signal, and
    /// the last line it wrote on its standard error.
    Failed { code: Option<i32>, detail: String },
    /// The child succeeded and wrote no frame.
    NoFrames,
    /// Cancelled before the result was published.
    Cancelled,
    /// The child or its worker could not be started.
    Unstarted { detail: String },
}

impl fmt::Display for TrimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reversed => formatter.write_str("the span does not run forward"),
            Self::PastTheEnd => formatter.write_str("the span ends after the film"),
            Self::TooShort => formatter.write_str("the span is shorter than one frame"),
            Self::Unchanged => formatter.write_str("the span is the whole film"),
            Self::NotAVideo => formatter.write_str("the file is not a video"),
            Self::ToolMissing => write!(formatter, "{PROGRAM} is not installed"),
            Self::EncoderMissing => {
                write!(formatter, "{PROGRAM} lists neither h264_vaapi nor libx264")
            }
            Self::Failed { code, detail } => match code {
                Some(code) => write!(formatter, "{PROGRAM} exited with {code}: {detail}"),
                None => write!(formatter, "{PROGRAM} was killed: {detail}"),
            },
            Self::NoFrames => write!(formatter, "{PROGRAM} wrote no frame"),
            Self::Cancelled => formatter.write_str("the trim was cancelled"),
            Self::Unstarted { detail } => write!(formatter, "{PROGRAM} did not start: {detail}"),
        }
    }
}

impl std::error::Error for TrimError {}
