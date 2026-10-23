//! Workspace previews for the overview (CEL-2-L, design §4): one
//! `wlr-screencopy` frame of the output the panel opens on, taken only when
//! the person asks, and shown whole as the preview of the workspace active on
//! that output. niri gives no on-screen position for a tiled window
//! (`tile_pos_in_workspace_view` is null), so windows are never cropped out:
//! their tiles stay geometry only.
//!
//! The rules this module keeps:
//!
//! - **Permission.** A capture is the direct result of a verb. Nothing here
//!   captures on its own, on a timer or in the background. The worker owns a
//!   Wayland connection only for the length of one request; between requests
//!   it holds no protocol object at all. The app asks for the one output its
//!   panel opens on, never the others (data minimisation).
//! - **Rate.** Exactly one frame per output per request, without the cursor
//!   (`overlay_cursor = 0`), with no damage tracking and no refresh. One
//!   request is in flight at a time in the whole process
//!   ([`worker::IN_FLIGHT`]), also while a given-up capture is still
//!   draining; a second one is refused with [`Busy::InFlight`], never
//!   queued.
//! - **Bounds.** The compositor's `buffer` event is hostile input and is
//!   checked by [`check_buffer`] before a byte is allocated: one frame is at
//!   most [`MAX_FRAME_SIDE`] square (256 MiB), and on top of that a
//!   request's peak memory, the storage of its frames plus the copy of one
//!   frame while it is read back, never passes [`MAX_FRAME_BYTES_IN_FLIGHT`]
//!   (128 MiB: one 4K or 5K output fits, an 8K frame, about 127 MiB and twice
//!   that while read back, is refused before it is allocated). The whole
//!   request ends at [`CAPTURE_TIMEOUT`]; at most
//!   [`MAX_PREVIEWS`] previews leave, one per output asked for, each at most
//!   [`PREVIEW_MAX_SIDE`] on its longer side (1600 KiB, about 1.6 MiB).
//!   The frame's storage is released before the reply is sent.
//! - **One reply per opening.** A request carries the overview's opening
//!   number and its [`CaptureReply`] carries it back, so the app drops a reply
//!   that belongs to an opening it already gave up.
//! - **Pixels stay in memory.** Thumbnails are never logged, written to disk
//!   or sent over the bus. [`CaptureResult`]'s `Debug` prints sizes, not
//!   bytes; the only numbers about a picture that may be logged are
//!   [`mean_luma`] and [`half_lumas`], at debug level, which the nested smoke
//!   uses to tell a painted window from a blacked-out one.
//!
//! Starting the worker is behind the [`Spawner`] seam, as the energy duties
//! do, so the request rules are tested without a compositor.

pub mod worker;

use std::fmt;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use celestina_shell::thumbnail::{crop_scale, Frame, PixelFormat, Rect, Thumb, ThumbError};
use celestina_wayland::Sender;

use crate::message::Message;
use crate::providers::Worker;

pub use celestina_shell::thumbnail::{MAX_FRAME_SIDE, PREVIEW_MAX_SIDE};
pub use worker::{Busy, CaptureHandle};

/// How long one request may take, from its arrival at the worker to its
/// reply, connection and every output's frame included. What has not arrived
/// by then is left out of the reply.
pub const CAPTURE_TIMEOUT: Duration = Duration::from_millis(400);
/// How many outputs one request may name; more are ignored.
pub const MAX_OUTPUTS: usize = 16;
/// How many previews one reply may carry: one per output.
pub const MAX_PREVIEWS: usize = MAX_OUTPUTS;
/// The largest output scale a request may carry.
const MAX_SCALE: f64 = 16.0;
/// How far, in buffer pixels, the compositor's frame may be from the size the
/// request's logical size and scale predict before the output is refused.
const SIZE_TOLERANCE: f64 = 1.0;
/// How far below the announced scale the real one may be: `wl_output`
/// announces the ceiling of a fractional scale (1.25 arrives as 2).
const SCALE_BELOW: f64 = 1.0;
/// How far above the announced scale the real one may be (rounding slack).
const SCALE_ABOVE: f64 = 0.5;
/// The most bytes the frames of one request may hold at once, counting the
/// copy of a frame while it is read back ([`peak_bytes`]).
pub const MAX_FRAME_BYTES_IN_FLIGHT: u64 = 128 * 1024 * 1024;
/// How far the frame's aspect ratio may differ from the output's, relative.
const ASPECT_TOLERANCE: f64 = 0.01;

/// One output to capture, and the workspace its picture is the preview of.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputRequest {
    /// The output's connector name (`DP-1`), as `wl_output.name` says it.
    pub name: String,
    /// The output's logical width.
    pub logical_width: u32,
    /// The output's logical height.
    pub logical_height: u32,
    /// The output's scale: buffer pixels per logical pixel.
    pub scale: f64,
    /// The niri id of the workspace active on this output; the output's
    /// picture is that workspace's preview.
    pub workspace_id: u64,
}

/// What one capture asks for: the outputs whose pictures are wanted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaptureRequest {
    /// The overview's opening this request serves, echoed in the reply.
    pub generation: u64,
    /// At most [`MAX_OUTPUTS`] are looked at. The app names only the output
    /// its panel opens on.
    pub outputs: Vec<OutputRequest>,
}

/// One request's answer, with the opening it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureReply {
    /// The [`CaptureRequest::generation`] this answers.
    pub generation: u64,
    /// What was captured.
    pub result: CaptureResult,
}

/// The reply to one request.
#[derive(Clone, PartialEq)]
pub enum CaptureResult {
    /// The previews that could be made, keyed by workspace id: one per
    /// captured output, at most [`MAX_PREVIEWS`]. Possibly fewer than were
    /// asked for (an output that failed, a deadline), possibly none.
    Done { previews: Vec<(u64, Thumb)> },
    /// No output could be captured, with the reason. The panel draws its
    /// workspaces without pictures.
    Unavailable(String),
}

impl fmt::Debug for CaptureResult {
    /// Sizes only: a preview's bytes never reach a log line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Done { previews } => {
                let mut list = f.debug_list();
                for (id, thumb) in previews {
                    list.entry(&format_args!("{id}: {}x{}", thumb.width, thumb.height));
                }
                list.finish()
            }
            Self::Unavailable(reason) => f.debug_tuple("Unavailable").field(reason).finish(),
        }
    }
}

/// The mean luminance of `thumb`, 0 (black) to 255 (white), with the
/// Rec. 709 weights. With [`half_lumas`], the only figures about a picture
/// the shell may log.
#[must_use]
pub fn mean_luma(thumb: &Thumb) -> u8 {
    columns_luma(thumb, 0, thumb.width)
}

/// The mean luminance of the left and the right half of `thumb`, as
/// [`mean_luma`] measures it. For an odd width the middle column counts in
/// both halves.
#[must_use]
pub fn half_lumas(thumb: &Thumb) -> (u8, u8) {
    let width = thumb.width;
    (
        columns_luma(thumb, 0, width.div_ceil(2)),
        columns_luma(thumb, width / 2, width),
    )
}

/// The mean luminance of columns `[from, to)` of `thumb`; 0 when they hold
/// no pixel. Rows past the bytes `thumb` holds are not counted.
fn columns_luma(thumb: &Thumb, from: u32, to: u32) -> u8 {
    let Ok(row_len) = usize::try_from(u64::from(thumb.width) * 4) else {
        return 0;
    };
    let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
        return 0;
    };
    if row_len == 0 || to <= from {
        return 0;
    }
    let mut sum = 0_u64;
    let mut count = 0_u64;
    for row in thumb.rgba.chunks_exact(row_len) {
        let Some(span) = row.get(from * 4..to * 4) else {
            continue;
        };
        for pixel in span.as_chunks::<4>().0 {
            let (r, g, b) = (
                u64::from(pixel[0]),
                u64::from(pixel[1]),
                u64::from(pixel[2]),
            );
            sum += (54 * r + 183 * g + 19 * b) >> 8;
            count += 1;
        }
    }
    if count == 0 {
        return 0;
    }
    u8::try_from(sum / count).unwrap_or(u8::MAX)
}

/// A `buffer` event that passed [`check_buffer`]: safe to allocate for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferSpec {
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

impl BufferSpec {
    /// The bytes the buffer holds: at most `8192 * 4 * 8192`, so it fits the
    /// protocol's `int` pool size.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        u64::from(self.stride) * u64::from(self.height)
    }
}

/// The compositor's announced shm buffer, checked before anything is
/// allocated: a format other than Argb8888/Xrgb8888, a side of zero or above
/// [`MAX_FRAME_SIDE`], or a stride that cannot hold a row, is not a whole
/// number of pixels, or is wider than the widest accepted row is refused.
///
/// # Errors
///
/// The refusal, as [`ThumbError`].
pub fn check_buffer(
    format: u32,
    width: u32,
    height: u32,
    stride: u32,
) -> Result<BufferSpec, ThumbError> {
    let format = PixelFormat::from_wl_code(format)?;
    if width == 0 || height == 0 || width > MAX_FRAME_SIDE || height > MAX_FRAME_SIDE {
        return Err(ThumbError::BadDimensions { width, height });
    }
    let row = u64::from(width) * 4;
    let stride_ok = u64::from(stride) >= row
        && stride.is_multiple_of(4)
        && u64::from(stride) <= u64::from(MAX_FRAME_SIDE) * 4;
    if !stride_ok {
        return Err(ThumbError::BadStride { stride, width });
    }
    Ok(BufferSpec {
        format,
        width,
        height,
        stride,
    })
}

/// Whether a frame of `width` by `height` buffer pixels plausibly is
/// `output`'s whole picture. `wl_output` announces the ceiling of the real
/// scale (1.25, 1.5 and 1.75 all arrive as 2), so the exact frame size cannot
/// be predicted: the real scale is taken to lie in
/// `[announced - 1, announced + 0.5]`. The frame must also keep the output's
/// aspect ratio within [`ASPECT_TOLERANCE`], so a rotated output (sides
/// swapped) or a frame of another shape is refused. The frame is shown whole
/// and never cropped, so the size only has to be sane.
#[must_use]
pub fn frame_matches(output: &OutputRequest, width: u32, height: u32) -> bool {
    let scale = output.scale;
    if !scale.is_finite() || scale <= 0.0 || scale > MAX_SCALE {
        return false;
    }
    if output.logical_width == 0 || output.logical_height == 0 || width == 0 || height == 0 {
        return false;
    }
    let low = (scale - SCALE_BELOW).max(0.0);
    let high = scale + SCALE_ABOVE;
    let within = |logical: u32, actual: u32| {
        let actual = f64::from(actual);
        let logical = f64::from(logical);
        actual >= (logical * low).round() - SIZE_TOLERANCE
            && actual <= (logical * high).round() + SIZE_TOLERANCE
    };
    let (lw, lh) = (
        f64::from(output.logical_width),
        f64::from(output.logical_height),
    );
    let (w, h) = (f64::from(width), f64::from(height));
    let aspect = (w * lh - h * lw).abs() <= ASPECT_TOLERANCE * (w * lh).max(h * lw);
    aspect && within(output.logical_width, width) && within(output.logical_height, height)
}

/// The most bytes a request holds at once once a frame of `next` bytes is
/// allocated beside frames of the sizes in `held`: every frame's storage,
/// plus the copy the worker reads back from the largest one (frames are read
/// back one at a time, each into a buffer of its own size). `None` past
/// `u64`.
#[must_use]
pub fn peak_bytes(held: &[u64], next: u64) -> Option<u64> {
    let storage = held
        .iter()
        .try_fold(next, |sum, bytes| sum.checked_add(*bytes))?;
    let largest = held.iter().copied().fold(next, u64::max);
    storage.checked_add(largest)
}

/// Whether a frame of `next` bytes may be allocated beside frames of the
/// sizes in `held`: its [`peak_bytes`] stay within
/// [`MAX_FRAME_BYTES_IN_FLIGHT`].
#[must_use]
pub fn fits_budget(held: &[u64], next: u64) -> bool {
    peak_bytes(held, next).is_some_and(|peak| peak <= MAX_FRAME_BYTES_IN_FLIGHT)
}

/// Why an output's frame gave no preview.
#[derive(Debug, thiserror::Error)]
pub enum CropError {
    /// The frame's size is not the one the output's geometry predicts.
    #[error("the frame is {width}x{height}, not what the output's size and scale predict")]
    Mismatch { width: u32, height: u32 },
    /// The frame's bytes could not be read back.
    #[error("cannot read the frame back")]
    Read(#[source] io::Error),
    /// The bytes read back could not be scaled (fewer than the frame needs).
    #[error("cannot scale the frame")]
    Scale(#[source] ThumbError),
}

/// The preview of `output`: its whole frame, described by `spec`, scaled so
/// its longer side is at most [`PREVIEW_MAX_SIDE`]. `read(offset, len)`
/// returns `len` bytes of the frame from `offset`; the whole frame is read
/// once. A frame the compositor marked `y_invert` is turned upright.
///
/// # Errors
///
/// [`CropError`] when the frame does not match the output (nothing is read),
/// cannot be read, or holds too few bytes.
pub fn preview_output(
    spec: &BufferSpec,
    y_invert: bool,
    output: &OutputRequest,
    read: impl FnOnce(u64, usize) -> io::Result<Vec<u8>>,
) -> Result<Thumb, CropError> {
    if !frame_matches(output, spec.width, spec.height) {
        return Err(CropError::Mismatch {
            width: spec.width,
            height: spec.height,
        });
    }
    let len = usize::try_from(spec.bytes())
        .map_err(|_| CropError::Read(io::Error::other("frame too large")))?;
    let data = read(0, len).map_err(CropError::Read)?;
    let frame = Frame {
        width: spec.width,
        height: spec.height,
        stride: spec.stride,
        format: spec.format,
        data: &data,
    };
    let whole = Rect::new(0, 0, spec.width, spec.height);
    let mut thumb = crop_scale(&frame, whole, PREVIEW_MAX_SIDE).map_err(CropError::Scale)?;
    if y_invert {
        flip_rows(&mut thumb);
    }
    Ok(thumb)
}

/// Turns `thumb` upside down in place.
fn flip_rows(thumb: &mut Thumb) {
    let Ok(row_len) = usize::try_from(u64::from(thumb.width) * 4) else {
        return;
    };
    if row_len == 0 {
        return;
    }
    let rows = thumb.rgba.len() / row_len;
    for top in 0..rows / 2 {
        let bottom = rows - 1 - top;
        let (upper, lower) = thumb.rgba.split_at_mut(bottom * row_len);
        upper[top * row_len..(top + 1) * row_len].swap_with_slice(&mut lower[..row_len]);
    }
}

/// How the capture worker comes to exist: the seam between the request
/// rules and the compositor.
pub trait Spawner {
    /// Starts a capture worker. `shutdown` must be a flag of the worker's
    /// own: [`Worker`] raises it when joined.
    ///
    /// # Errors
    ///
    /// The failure to start the thread.
    fn spawn(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(CaptureHandle, Worker)>;
}

/// The real spawner: the session's compositor, replies into the loop.
#[derive(Clone)]
pub struct Live {
    sender: Sender<Message>,
}

impl Live {
    /// Replies are sent to `sender` as [`Message::Captured`].
    #[must_use]
    pub fn new(sender: Sender<Message>) -> Self {
        Self { sender }
    }
}

impl Spawner for Live {
    fn spawn(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(CaptureHandle, Worker)> {
        let sender = self.sender.clone();
        worker::spawn(
            worker::WaylandCapturer,
            move |reply| sender.send(Message::Captured(reply)).is_ok(),
            shutdown,
            &worker::IN_FLIGHT,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{self, Receiver};
    use std::sync::Mutex;
    use std::time::Instant;

    fn output(lw: u32, lh: u32, scale: f64) -> OutputRequest {
        OutputRequest {
            name: "DP-1".to_owned(),
            logical_width: lw,
            logical_height: lh,
            scale,
            workspace_id: 5,
        }
    }

    #[test]
    fn fractional_scales_announced_as_their_ceiling_are_accepted() {
        // (physical size, real scale): the logical size is physical / real,
        // and `wl_output` announces the real scale's ceiling.
        for ((w, h), real) in [
            ((1920_u32, 1080_u32), 1.1_f64),
            ((2560, 1440), 1.25),
            ((3840, 2160), 1.5),
            ((3840, 2160), 1.75),
            ((3840, 2160), 2.25),
        ] {
            let logical = |side: u32| (f64::from(side) / real).round() as u32;
            let out = output(logical(w), logical(h), real.ceil());
            assert!(frame_matches(&out, w, h), "{w}x{h} at {real}");
            assert!(!frame_matches(&out, h, w), "{h}x{w} rotated at {real}");
        }
    }

    #[test]
    fn a_frame_that_does_not_match_the_geometry_is_refused() {
        let out = output(2560, 1440, 2.0);
        assert!(frame_matches(&out, 3840, 2160));
        assert!(frame_matches(&out, 3841, 2159));
        assert!(frame_matches(&out, 5120, 2880));
        assert!(!frame_matches(&out, 2160, 3840), "a rotated frame");
        assert!(!frame_matches(&out, 1280, 720), "too small for the scale");
        assert!(!frame_matches(&out, 7000, 3938), "too large for the scale");
        assert!(!frame_matches(&out, 3840, 1440), "another shape");
        assert!(
            !frame_matches(&out, 3840, 2200),
            "two percent off the shape"
        );
        assert!(!frame_matches(&output(2560, 1440, f64::NAN), 2560, 1440));
        assert!(!frame_matches(&output(2560, 1440, 0.0), 0, 0));
        assert!(!frame_matches(&output(2560, 1440, 17.0), 43520, 24480));
        assert!(!frame_matches(&output(0, 1440, 1.0), 0, 1440));
    }

    #[test]
    fn a_frame_counts_twice_while_it_is_read_back() {
        let four_k = 3840 * 4 * 2160;
        let five_k = 5120 * 4 * 2880;
        let eight_k = 7680 * 4 * 4320;
        assert_eq!(peak_bytes(&[], four_k), Some(2 * four_k));
        assert_eq!(peak_bytes(&[five_k], four_k), Some(2 * five_k + four_k));
        assert!(fits_budget(&[], four_k), "one 4K output fits");
        assert!(fits_budget(&[], five_k), "one 5K output fits");
        assert!(
            !fits_budget(&[], eight_k),
            "an 8K frame and its copy are over 128 MiB"
        );
        assert!(
            fits_budget(&[four_k, four_k], four_k),
            "three 4K frames fit"
        );
        assert!(
            !fits_budget(&[four_k, four_k, four_k], four_k),
            "a fourth does not"
        );
        assert!(!fits_budget(&[], 8192 * 4 * 8192));
        assert!(!fits_budget(&[u64::MAX], 1));
        assert!(!fits_budget(&[u64::MAX / 2 + 1], 0));
    }

    #[test]
    fn frames_in_flight_stay_within_the_byte_budget() {
        let half = MAX_FRAME_BYTES_IN_FLIGHT / 2;
        assert!(fits_budget(&[], half), "a frame and its copy at the budget");
        assert!(!fits_budget(&[], half + 1));
        // A small frame beside a large one: the large one's copy is the one
        // that counts.
        let quarter = MAX_FRAME_BYTES_IN_FLIGHT / 4;
        assert!(fits_budget(&[half - quarter], quarter));
        assert!(!fits_budget(&[half], 1));
    }

    #[test]
    fn hostile_buffer_parameters_are_refused_before_allocation() {
        assert_eq!(
            check_buffer(1, 1920, 1080, 7680),
            Ok(BufferSpec {
                format: PixelFormat::Xrgb8888,
                width: 1920,
                height: 1080,
                stride: 7680,
            })
        );
        assert!(check_buffer(0, 8192, 8192, 8192 * 4).is_ok());
        assert_eq!(
            check_buffer(0x3432_5241, 10, 10, 40),
            Err(ThumbError::UnsupportedFormat(0x3432_5241))
        );
        assert!(matches!(
            check_buffer(0, 8193, 10, 8193 * 4),
            Err(ThumbError::BadDimensions { .. })
        ));
        assert!(matches!(
            check_buffer(0, 10, 0, 40),
            Err(ThumbError::BadDimensions { .. })
        ));
        assert!(matches!(
            check_buffer(0, u32::MAX, u32::MAX, u32::MAX),
            Err(ThumbError::BadDimensions { .. })
        ));
        assert!(matches!(
            check_buffer(0, 10, 10, 39),
            Err(ThumbError::BadStride { .. })
        ));
        assert!(matches!(
            check_buffer(0, 10, 10, 42),
            Err(ThumbError::BadStride { .. })
        ));
        assert!(matches!(
            check_buffer(0, 10, 8192, u32::MAX - 3),
            Err(ThumbError::BadStride { .. })
        ));
        // The largest accepted buffer fits the protocol's `int` pool size.
        let largest = check_buffer(0, 8192, 8192, 8192 * 4).map(|spec| spec.bytes());
        assert!(largest.is_ok_and(|bytes| i32::try_from(bytes).is_ok()));
    }

    /// A frame of `w` by `h` Xrgb pixels, each grey `f(x, y)`.
    fn frame_bytes(w: u32, h: u32, stride: u32, f: impl Fn(u32, u32) -> u8) -> Vec<u8> {
        let mut data = vec![0_u8; usize::try_from(stride * h).unwrap_or(0)];
        for y in 0..h {
            for x in 0..w {
                let at = usize::try_from(y * stride + x * 4).unwrap_or(0);
                let v = f(x, y);
                data[at..at + 4].copy_from_slice(&[v, v, v, 0]);
            }
        }
        data
    }

    fn read_from(data: &[u8]) -> impl FnOnce(u64, usize) -> io::Result<Vec<u8>> + '_ {
        move |offset, len| {
            let start = usize::try_from(offset).unwrap_or(usize::MAX);
            data.get(start..start.saturating_add(len))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))
        }
    }

    #[test]
    fn the_whole_frame_is_read_once_and_becomes_the_preview() {
        let out = output(100, 50, 1.0);
        let spec = check_buffer(1, 100, 50, 408).expect("a valid buffer");
        // Black on the left half, white on the right.
        let data = frame_bytes(100, 50, 408, |x, _| if x < 50 { 0 } else { 255 });
        let asked = Mutex::new(Vec::new());
        let preview = preview_output(&spec, false, &out, |offset, len| {
            asked.lock().expect("unpoisoned").push((offset, len));
            read_from(&data)(offset, len)
        })
        .expect("a matching frame");

        assert_eq!(*asked.lock().expect("unpoisoned"), vec![(0, 408 * 50)]);
        assert_eq!((preview.width, preview.height), (100, 50));
        assert_eq!(mean_luma(&preview), 127);
        assert_eq!(half_lumas(&preview), (0, 255));
    }

    #[test]
    fn a_large_frame_is_shrunk_to_the_preview_cap() {
        // A 1280x720 output at scale 1.5 gives a 1920x1080 frame.
        let out = output(1280, 720, 2.0);
        let spec = check_buffer(1, 1920, 1080, 1920 * 4).expect("a valid buffer");
        let data = frame_bytes(1920, 1080, 1920 * 4, |_, _| 128);
        let preview =
            preview_output(&spec, false, &out, read_from(&data)).expect("a matching frame");
        assert_eq!((preview.width, preview.height), (PREVIEW_MAX_SIDE, 360));
        assert_eq!(mean_luma(&preview), 128);
    }

    #[test]
    fn a_y_inverted_frame_is_turned_upright() {
        let out = output(4, 4, 1.0);
        let spec = check_buffer(1, 4, 4, 16).expect("a valid buffer");
        // The bottom row is white: upside down, the picture's top row.
        let data = frame_bytes(4, 4, 16, |_, y| if y == 3 { 255 } else { 0 });
        let upright =
            preview_output(&spec, true, &out, read_from(&data)).expect("a matching frame");
        assert_eq!(&upright.rgba[..16], &[255, 255, 255, 255].repeat(4)[..]);
        assert!(upright.rgba[16..].chunks(4).all(|p| p == [0, 0, 0, 255]));
        let as_is = preview_output(&spec, false, &out, read_from(&data)).expect("a matching frame");
        assert_eq!(&as_is.rgba[48..], &[255, 255, 255, 255].repeat(4)[..]);
    }

    #[test]
    fn a_mismatched_frame_is_never_read() {
        let spec = check_buffer(0, 10, 10, 40).expect("a valid buffer");
        let outcome = preview_output(&spec, false, &output(10, 10, 3.0), |_, _| {
            Err(io::Error::other("a mismatched frame is never read"))
        });
        assert!(matches!(outcome, Err(CropError::Mismatch { .. })));
    }

    #[test]
    fn a_short_read_is_an_error_not_a_panic() {
        let out = output(10, 10, 1.0);
        let spec = check_buffer(0, 10, 10, 40).expect("a valid buffer");
        // A compositor that truncated the memory file: fewer bytes than asked.
        let outcome = preview_output(&spec, false, &out, |_, _| {
            Err(io::Error::from(io::ErrorKind::UnexpectedEof))
        });
        assert!(matches!(outcome, Err(CropError::Read(_))));
        // And a reader that returns too little is caught by the scale itself.
        let outcome = preview_output(&spec, false, &out, |_, len| Ok(vec![0; len / 2]));
        assert!(matches!(
            outcome,
            Err(CropError::Scale(ThumbError::ShortData { .. }))
        ));
    }

    #[test]
    fn the_debug_form_of_a_result_carries_no_pixels() {
        let result = CaptureResult::Done {
            previews: vec![(
                3,
                Thumb {
                    width: 1,
                    height: 1,
                    rgba: vec![11, 22, 33, 44],
                },
            )],
        };
        let text = format!("{result:?}");
        assert_eq!(text, "[3: 1x1]");
        assert!(!text.contains("22"));
        let message = format!(
            "{:?}",
            Message::Captured(CaptureReply {
                generation: 1,
                result
            })
        );
        assert!(
            !message.contains("rgba") && !message.contains("22"),
            "{message}"
        );
    }

    #[test]
    fn mean_luma_spans_black_to_white() {
        let thumb = |rgba: Vec<u8>| Thumb {
            width: 1,
            height: 1,
            rgba,
        };
        assert_eq!(mean_luma(&thumb(vec![0, 0, 0, 255])), 0);
        assert_eq!(mean_luma(&thumb(vec![255, 255, 255, 255])), 255);
        assert_eq!(mean_luma(&thumb(Vec::new())), 0);
    }

    #[test]
    fn half_lumas_split_the_columns() {
        let row = |values: &[u8]| Thumb {
            width: u32::try_from(values.len()).unwrap_or(0),
            height: 1,
            rgba: values.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        };
        assert_eq!(half_lumas(&row(&[0, 0, 255, 255])), (0, 255));
        // An odd width: the middle column is in both halves.
        assert_eq!(half_lumas(&row(&[0, 255, 255])), (127, 255));
        assert_eq!(half_lumas(&row(&[200])), (200, 200));
        assert_eq!(half_lumas(&row(&[])), (0, 0));
        // Bytes that do not fill a row are not counted.
        let ragged = Thumb {
            width: 2,
            height: 2,
            rgba: vec![255; 12],
        };
        assert_eq!(half_lumas(&ragged), (255, 255));
    }

    #[test]
    fn every_preview_is_within_its_side_and_byte_bound() {
        // Frames of every shape the check lets through: 4K, a very wide and a
        // very tall output at the widest accepted side, a square, and a tiny
        // one, each scaled as the worker scales it.
        let cap = usize::try_from(PREVIEW_MAX_SIDE).unwrap_or(usize::MAX);
        for (w, h) in [(3840, 2160), (8192, 64), (64, 8192), (1000, 1000), (3, 2)] {
            let spec = check_buffer(1, w, h, w * 4).expect("a valid buffer");
            let data = vec![200_u8; usize::try_from(spec.bytes()).unwrap_or(0)];
            let preview = preview_output(&spec, false, &output(w, h, 1.0), read_from(&data))
                .expect("a matching frame");
            assert!(
                preview.width.max(preview.height) <= PREVIEW_MAX_SIDE,
                "{w}x{h}"
            );
            assert!(preview.rgba.len() <= cap * cap * 4, "{w}x{h}");
            assert_eq!(
                preview.rgba.len(),
                usize::try_from(preview.width * preview.height * 4).unwrap_or(0)
            );
        }
    }

    // The request rules, through the seam.

    /// A capturer that waits for the test to let each request go.
    struct Gated {
        seen: mpsc::Sender<CaptureRequest>,
        release: Arc<Mutex<Receiver<CaptureResult>>>,
    }

    impl worker::Capturer for Gated {
        fn capture(
            &mut self,
            request: &CaptureRequest,
            deadline: Instant,
            shutdown: &AtomicBool,
        ) -> CaptureResult {
            let _ = self.seen.send(request.clone());
            let release = self.release.lock().expect("unpoisoned");
            loop {
                if shutdown.load(std::sync::atomic::Ordering::Acquire) {
                    return CaptureResult::Unavailable("stopped".to_owned());
                }
                if let Ok(result) = release.recv_timeout(Duration::from_millis(5)) {
                    return result;
                }
                if Instant::now() >= deadline {
                    return CaptureResult::Unavailable("timed out".to_owned());
                }
            }
        }
    }

    struct TestSpawner {
        seen: mpsc::Sender<CaptureRequest>,
        release: Arc<Mutex<Receiver<CaptureResult>>>,
        replies: Mutex<Option<mpsc::Sender<CaptureReply>>>,
    }

    impl Spawner for TestSpawner {
        fn spawn(&self, shutdown: &Arc<AtomicBool>) -> io::Result<(CaptureHandle, Worker)> {
            let replies = self
                .replies
                .lock()
                .expect("unpoisoned")
                .take()
                .ok_or_else(|| io::Error::other("spawned twice"))?;
            worker::spawn(
                Gated {
                    seen: self.seen.clone(),
                    release: Arc::clone(&self.release),
                },
                move |reply| replies.send(reply).is_ok(),
                shutdown,
                own_mark(),
            )
        }
    }

    /// An in-flight mark of a test's own, so tests that run in parallel never
    /// refuse each other's requests through [`worker::IN_FLIGHT`].
    pub(crate) fn own_mark() -> &'static AtomicBool {
        Box::leak(Box::new(AtomicBool::new(false)))
    }

    struct Rig {
        handle: CaptureHandle,
        worker: Worker,
        seen: Receiver<CaptureRequest>,
        release: mpsc::Sender<CaptureResult>,
        replies: Receiver<CaptureReply>,
    }

    fn rig() -> Rig {
        let (seen_tx, seen) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let (replies_tx, replies) = mpsc::channel();
        let spawner = TestSpawner {
            seen: seen_tx,
            release: Arc::new(Mutex::new(release_rx)),
            replies: Mutex::new(Some(replies_tx)),
        };
        let shutdown = Arc::new(AtomicBool::new(false));
        let (handle, worker) = spawner.spawn(&shutdown).expect("the worker starts");
        Rig {
            handle,
            worker,
            seen,
            release,
            replies,
        }
    }

    fn one_output() -> CaptureRequest {
        CaptureRequest {
            generation: 7,
            outputs: vec![output(100, 100, 1.0)],
        }
    }

    fn reply(result: CaptureResult) -> CaptureReply {
        CaptureReply {
            generation: 7,
            result,
        }
    }

    const WAIT: Duration = Duration::from_secs(2);

    #[test]
    fn a_second_request_while_one_is_in_flight_is_busy_not_queued() {
        let rig = rig();
        assert_eq!(rig.handle.request(one_output()), Ok(()));
        assert!(rig.seen.recv_timeout(WAIT).is_ok());

        assert_eq!(rig.handle.request(one_output()), Err(Busy::InFlight));
        assert_eq!(
            rig.handle.clone().request(one_output()),
            Err(Busy::InFlight)
        );

        let done = CaptureResult::Done {
            previews: Vec::new(),
        };
        rig.release
            .send(done.clone())
            .expect("the capturer listens");
        assert_eq!(rig.replies.recv_timeout(WAIT), Ok(reply(done)));
        // The refused ones were never queued: nothing else was captured.
        assert!(rig.seen.recv_timeout(Duration::from_millis(50)).is_err());

        // Once the reply is out, the next request is taken.
        assert_eq!(rig.handle.request(one_output()), Ok(()));
        assert!(rig.seen.recv_timeout(WAIT).is_ok());
        rig.worker.join();
    }

    #[test]
    fn a_request_that_outlives_the_timeout_is_answered_unavailable() {
        let rig = rig();
        let started = Instant::now();
        assert_eq!(rig.handle.request(one_output()), Ok(()));
        let reply = rig.replies.recv_timeout(WAIT);
        assert_eq!(
            reply,
            Ok(super::tests::reply(CaptureResult::Unavailable(
                "timed out".to_owned()
            )))
        );
        assert!(started.elapsed() >= CAPTURE_TIMEOUT);
        assert!(started.elapsed() < WAIT);
        rig.worker.join();
    }

    #[test]
    fn joining_stops_a_capture_in_flight_and_publishes_nothing() {
        let rig = rig();
        assert_eq!(rig.handle.request(one_output()), Ok(()));
        assert!(rig.seen.recv_timeout(WAIT).is_ok());
        let started = Instant::now();
        rig.worker.join();
        assert!(started.elapsed() < WAIT);
        assert!(rig.replies.try_recv().is_err());
        assert_eq!(rig.handle.request(one_output()), Err(Busy::Gone));
    }

    #[test]
    fn a_stopped_worker_refuses_as_gone_and_frees_the_slot() {
        let rig = rig();
        rig.worker.join();
        assert_eq!(rig.handle.request(one_output()), Err(Busy::Gone));
        // Still gone, not stuck as in flight.
        assert_eq!(rig.handle.request(one_output()), Err(Busy::Gone));
    }

    #[test]
    fn an_empty_request_is_done_at_once_without_the_compositor() {
        let mut capturer = worker::WaylandCapturer;
        let shutdown = AtomicBool::new(false);
        let result = worker::Capturer::capture(
            &mut capturer,
            &CaptureRequest::default(),
            Instant::now() + CAPTURE_TIMEOUT,
            &shutdown,
        );
        assert_eq!(
            result,
            CaptureResult::Done {
                previews: Vec::new()
            }
        );
    }
}
