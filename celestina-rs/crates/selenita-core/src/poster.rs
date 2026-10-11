//! What the corner preview shows of a result: a recording's poster (its
//! first frame as a PNG, in place of the video), a picture's size (the
//! preview takes its shape) and a recording's length on the clock the
//! recording card shows.
//!
//! For the poster `gst-launch-1.0` decodes the MP4 and `pngenc
//! snapshot=true` ends the pipeline after one picture, so the child exits on
//! its own; the caller runs it through the [`crate::runner`] with a deadline
//! and resolves the program like any other tool ([`crate::tools::resolve`]).
//! The length is the MP4's own movie header, read without decoding.

use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use crate::record::LAUNCHER;

/// The argv that writes `video`'s first frame to `out_png`:
/// `gst-launch-1.0 -q filesrc location=<video> ! qtdemux ! decodebin !
/// videoconvert ! pngenc snapshot=true ! filesink location=<out_png>`.
///
/// Each `location=` is one argument built from the path's bytes, spaces
/// and all: the launcher escapes a space inside one argument before it
/// parses the description, so no quoting is added here and nothing is
/// lost to a lossy conversion.
#[must_use]
pub fn poster_argv(video: &Path, out_png: &Path) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec![LAUNCHER.into(), "-q".into(), "filesrc".into()];
    argv.push(location(video));
    argv.extend(
        [
            "!",
            "qtdemux",
            "!",
            "decodebin",
            "!",
            "videoconvert",
            "!",
            "pngenc",
            "snapshot=true",
            "!",
            "filesink",
        ]
        .into_iter()
        .map(OsString::from),
    );
    argv.push(location(out_png));
    argv
}

/// `location=<path>`, byte-exact.
fn location(path: &Path) -> OsString {
    let mut token = OsString::from("location=");
    token.push(path.as_os_str());
    token
}

/// The eight bytes every PNG starts with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A PNG's width and height from its header (the `IHDR` chunk, which the
/// format puts first); `None` for anything else or a zero side. Only the
/// first 24 bytes are read.
#[must_use]
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let header = bytes.get(..24)?;
    if &header[..8] != PNG_SIGNATURE || &header[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(header[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(header[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// An MP4's length, from the movie header (`moov` → `mvhd`): its duration
/// over its timescale. `None` when the file has no readable header or a
/// zero timescale or duration. Only box headers and the movie header are
/// read, wherever `moov` lies in the file; every step moves forward, so a
/// damaged file ends the walk instead of looping.
pub fn mp4_duration<R: Read + Seek>(mut file: R) -> Option<Duration> {
    let end = file.seek(SeekFrom::End(0)).ok()?;
    let (moov, moov_end) = find_box(&mut file, 0, end, b"moov")?;
    let (mvhd, mvhd_end) = find_box(&mut file, moov, moov_end, b"mvhd")?;
    file.seek(SeekFrom::Start(mvhd)).ok()?;
    let version = read_array::<_, 4>(&mut file)?[0];
    // Version 1 widens the times and the duration to 64 bits.
    let (skip, wide) = if version == 1 { (16, true) } else { (8, false) };
    let fields = 4 + skip + 4 + if wide { 8 } else { 4 };
    if mvhd + fields > mvhd_end {
        return None;
    }
    file.seek(SeekFrom::Current(i64::try_from(skip).ok()?))
        .ok()?;
    let timescale = u64::from(u32::from_be_bytes(read_array(&mut file)?));
    let duration = if wide {
        u64::from_be_bytes(read_array(&mut file)?)
    } else {
        u64::from(u32::from_be_bytes(read_array(&mut file)?))
    };
    if timescale == 0 || duration == 0 {
        return None;
    }
    let nanos = (duration % timescale) * 1_000_000_000 / timescale;
    Some(Duration::new(
        duration / timescale,
        u32::try_from(nanos).ok()?,
    ))
}

/// The body of the first `kind` box between `start` and `end`, as its first
/// and past-the-last offsets.
fn find_box<R: Read + Seek>(
    file: &mut R,
    start: u64,
    end: u64,
    kind: &[u8; 4],
) -> Option<(u64, u64)> {
    let mut at = start;
    while at.checked_add(8)? <= end {
        file.seek(SeekFrom::Start(at)).ok()?;
        let header: [u8; 8] = read_array(file)?;
        let declared = u64::from(u32::from_be_bytes(header[..4].try_into().ok()?));
        let (size, header_len) = match declared {
            // The box runs to the end of its parent.
            0 => (end - at, 8),
            // A 64-bit size follows the type.
            1 => (u64::from_be_bytes(read_array(file)?), 16),
            size => (size, 8),
        };
        if size < header_len || at.checked_add(size)? > end {
            return None;
        }
        if &header[4..] == kind {
            return Some((at + header_len, at + size));
        }
        at += size;
    }
    None
}

fn read_array<R: Read, const N: usize>(file: &mut R) -> Option<[u8; N]> {
    let mut bytes = [0; N];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

/// `m:ss`, the hours only once there are some (`h:mm:ss`), in whole
/// seconds: the recording card's clock, so a recording's length reads the
/// same in the preview as it did while it recorded.
#[must_use]
pub fn clock_text(length: Duration) -> String {
    let seconds = length.as_secs();
    let (hours, minutes, rest) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{rest:02}")
    } else {
        format!("{minutes}:{rest:02}")
    }
}
