//! A paused recording's segments and their join (SEL-4-B).
//!
//! `gst-launch-1.0` cannot be paused, so a pause finishes the current
//! child's MP4 and a resume starts the next one: a recording paused `n`
//! times is `n + 1` hidden segment files beside the hidden file the
//! recording publishes ([`segment_file`]). At the stop they are joined
//! losslessly by `ffmpeg`'s concat demuxer ([`join_argv`] over the list
//! [`concat_list`] writes), and the joined file is published as one
//! recording. When the join fails nothing recorded is lost: each segment
//! is published on its own, named by [`part_name`].

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The tool that joins the segments; installed with libmpv's `ffmpeg`.
pub const JOINER: &str = "ffmpeg";

/// The hidden file of segment `part` (from 1) of the recording whose hidden
/// file is `base`: `-part<n>` before the extension, in the same folder, so
/// `.selenita-12-1.mp4` has `.selenita-12-1-part2.mp4`.
#[must_use]
pub fn segment_file(base: &Path, part: u32) -> PathBuf {
    let stem = base
        .file_stem()
        .map_or_else(OsString::new, OsStr::to_os_string);
    let mut name = stem;
    name.push(format!("-part{part}"));
    if let Some(extension) = base.extension() {
        name.push(".");
        name.push(extension);
    }
    base.with_file_name(name)
}

/// The published name of segment `part` (from 1) when the join failed:
/// ` (<part>)` before the extension of the recording's own `name`, the
/// first segment included, so `Recording 2026-10-10 14.32.05.mp4` gives
/// `Recording 2026-10-10 14.32.05 (1).mp4`, `(2)`…
#[must_use]
pub fn part_name(name: &str, part: u32) -> String {
    match name.rsplit_once('.') {
        Some((base, extension)) if !base.is_empty() => format!("{base} ({part}).{extension}"),
        _ => format!("{name} ({part})"),
    }
}

/// A segment the concat list cannot name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentError {
    /// No segment at all.
    Empty,
    /// The demuxer reads a relative path from the list's own folder.
    NotAbsolute(PathBuf),
    /// The list is one path a line.
    LineBreak(PathBuf),
}

impl fmt::Display for SegmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("no segment to join"),
            Self::NotAbsolute(path) => write!(formatter, "not absolute: {}", path.display()),
            Self::LineBreak(path) => {
                write!(formatter, "a line break in {}", path.display())
            }
        }
    }
}

impl std::error::Error for SegmentError {}

/// The concat demuxer's list for `segments`, in order: one
/// `file '<path>'` line each, the path single-quoted with each `'` written
/// `'\''` (ffmpeg's own quoting), its bytes otherwise as they are.
///
/// # Errors
///
/// [`SegmentError`] for no segment, a relative path or one with a line
/// break.
pub fn concat_list(segments: &[PathBuf]) -> Result<Vec<u8>, SegmentError> {
    if segments.is_empty() {
        return Err(SegmentError::Empty);
    }
    let mut list = Vec::new();
    for segment in segments {
        if !segment.is_absolute() {
            return Err(SegmentError::NotAbsolute(segment.clone()));
        }
        let bytes = segment.as_os_str().as_bytes();
        if bytes.iter().any(|byte| *byte == b'\n' || *byte == b'\r') {
            return Err(SegmentError::LineBreak(segment.clone()));
        }
        list.extend_from_slice(b"file '");
        for byte in bytes {
            if *byte == b'\'' {
                list.extend_from_slice(b"'\\''");
            } else {
                list.push(*byte);
            }
        }
        list.extend_from_slice(b"'\n");
    }
    Ok(list)
}

/// The join: `ffmpeg -hide_banner -nostdin -y -f concat -safe 0 -i <list>
/// -c copy <out>`. The streams are copied, not encoded again (every segment
/// comes from the same pipeline). The index stays at the end, as in the
/// single-segment file `mp4mux` writes: the published file is played and
/// sent whole, never streamed, and `+faststart` would rewrite the whole
/// file a second time, doubling the join's time.
#[must_use]
pub fn join_argv(list: &Path, out: &Path) -> Vec<OsString> {
    let mut argv: Vec<OsString> = [
        JOINER,
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    argv.push(list.as_os_str().to_os_string());
    argv.extend(["-c", "copy"].into_iter().map(OsString::from));
    argv.push(out.as_os_str().to_os_string());
    argv
}

/// How long the join of `bytes` of segments may take: [`JOIN_BASE`] and
/// [`JOIN_PER_GIB`] for each GiB begun, at most [`JOIN_MAX`]. A copy runs at
/// disk speed, so this is generous; past it the child is killed and the
/// segments are published one by one, so nothing is lost.
#[must_use]
pub fn join_deadline(bytes: u64) -> Duration {
    let gibs = bytes.div_ceil(GIB);
    let per = JOIN_PER_GIB.saturating_mul(u32::try_from(gibs).unwrap_or(u32::MAX));
    JOIN_BASE.saturating_add(per).min(JOIN_MAX)
}

/// The join's deadline for any size: a minute.
pub const JOIN_BASE: Duration = Duration::from_secs(60);
/// What each GiB of segments adds to it.
pub const JOIN_PER_GIB: Duration = Duration::from_secs(10);
/// The longest join waited for: five minutes (24 GiB of segments).
pub const JOIN_MAX: Duration = Duration::from_secs(300);
const GIB: u64 = 1 << 30;

/// The concat list's file for the join tagged `tag` in the runtime folder
/// `runtime_dir`: `runtime_dir/selenita/join-<tag>.txt`, beside the pick
/// request, in the private folder.
#[must_use]
pub fn list_file(runtime_dir: &Path, tag: &str) -> PathBuf {
    runtime_dir.join("selenita").join(format!("join-{tag}.txt"))
}
