//! Bounded decode and scale of one wallpaper file (CEL-2-E plan, decision 5).
//!
//! Adapted, not copied, from the halted Qt shell's image validation:
//! `celestina-qt/src/provider_adapter/wallpaper.rs` (`read_validated_source`,
//! `validate_image`, `validate_reader`, `Fingerprint`) at commit
//! `c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413`. Kept: the dimension limits,
//! the `O_NONBLOCK` open followed by `fstat` (a FIFO named like an image
//! cannot block the worker), the size/mtime fingerprint that detects a file
//! changing under the decoder. Changed: the file cap is 128 MiB (was
//! 256 MiB); the stream is decoded from the open descriptor; the error is
//! typed instead of a `String`; AVIF is not accepted; the decode is budgeted
//! by memory, not only by dimensions; and the result is scaled here to the
//! output's device size, with EXIF orientation applied while sampling.
//!
//! # Bounds, in the order they are checked
//!
//! The file is hostile input. Every bound is checked before the work it
//! bounds:
//!
//! 1. the target buffer: each side at most [`MAX_TARGET_DIMENSION`] and the
//!    area at most [`MAX_TARGET_PIXELS`] (an 8K output), so the returned
//!    buffer is at most 126.6 MiB;
//! 2. the extension, before any IO;
//! 3. `stat` on the path (a device, FIFO, socket or directory is refused
//!    before it is opened), then an `O_NONBLOCK | O_NOCTTY` open and `fstat`
//!    on the descriptor: a non-empty regular file of at most
//!    [`MAX_FILE_BYTES`];
//! 4. every read goes through a cap at the size `fstat` reported, so a file
//!    that grows after the check is never read past it (the fingerprint then
//!    reports the change);
//! 5. the header: each side at most [`MAX_IMAGE_DIMENSION`], the area at most
//!    [`MAX_SOURCE_PIXELS`], one frame only (animated PNG and WebP are
//!    refused), an 8- or 16-bit colour type, and no lossless WebP coding (see
//!    below);
//! 6. the memory the decoder will need, predicted per format from the header
//!    and the file size, must fit [`MAX_DECODE_BYTES`], and the decoded image
//!    itself [`MAX_DECODED_BYTES`] — both before a pixel is decoded;
//! 7. a panic inside a decoder is caught and becomes
//!    [`DecodeError::Panicked`], so the worker survives it.
//!
//! # Peak memory
//!
//! `N` is the source area in pixels, `F` the file size, `D` the decoded image
//! (`N` × bytes per pixel), `O` the output buffer (≤ 126.6 MiB). The decoders
//! of `image` 0.25.6 need, beyond `D`:
//!
//! | Format | Decoder memory beyond `D` |
//! |---|---|
//! | PNG | ancillary chunks, a decompressed `iCCP` included, bounded by [`PNG_METADATA_BYTES`]; rows and the inflate window < 1 MiB |
//! | JPEG | the whole file (`F`) plus its ICC segments (≤ `F`), plus the coefficient store `2 B × Σ 64·hᵢ·vᵢ·mcuₓ·mcu_y` when the image is progressive or its first scan does not hold every component (≈ 3 N for 4:2:0, 6 N for 4:4:4, 8 N for four components) |
//! | WebP (lossy) | the Y/U/V planes (1.5 N, rounded to macroblocks), the partitions read twice (2 F), and an uncompressed alpha plane (N) |
//!
//! Each prediction plus [`DECODER_SLACK`] (per-row scratch) must fit
//! [`MAX_DECODE_BYTES`] = 320 MiB. After the decode the decoder's memory is
//! gone and the scaler holds `D + O` plus two rows and the filter weights
//! (< 2 MiB). So the documented worst case for any accepted file is
//! **≈ 322 MiB**: `max(320 MiB, D + O) + 2 MiB`, with `D + O` at most
//! 152.6 + 126.6 = 279.2 MiB.
//!
//! What that means at the area budget (40 MP, e.g. 8192 × 4882):
//!
//! - PNG, 8-bit RGBA: `D` = 152.6 MiB; peak ≈ 281 MiB (scaling into an 8K
//!   buffer). 16-bit PNG is accepted while `D` ≤ [`MAX_DECODED_BYTES`]
//!   (20 MP RGBA, 26.6 MP RGB), with the same peak.
//! - JPEG, baseline: `D` = 114.4 MiB (RGB) + 2 F; accepted for files up to
//!   ≈ 95 MiB. Progressive 4:2:0 adds 114.4 MiB (files up to ≈ 37 MiB);
//!   progressive 4:4:4 adds 228.9 MiB, so it is refused at 40 MP and accepted
//!   up to ≈ 33 MP for a 10 MiB file.
//! - WebP, lossy: `D` + 57.4 MiB + 2 F (+ 38.1 MiB with an uncompressed alpha
//!   plane); peak ≤ 320 MiB by the check.
//!
//! **Lossless WebP is refused.** The lossless decoder of `image-webp` 0.2.4
//! builds up to 65 536 Huffman groups of five tables each from a few bytes
//! apiece, whatever the image size: a ~3 MB file can make it allocate more
//! than 1 GiB, and no area or file budget bounds that. This covers `VP8L`
//! images and the losslessly compressed alpha (`ALPH`) most encoders write
//! for translucent lossy WebP.
//!
//! The caller runs this on the wallpaper worker, one file at a time. The
//! budget bounds memory, not time: a progressive JPEG with very many scans
//! still decodes slowly.

use std::any::Any;
use std::fs::{File, Metadata};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use celestina_shell::wallpaper::{placement, Mode};
use image::codecs::jpeg::JpegDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::error::{DecodingError, ImageFormatHint};
use image::metadata::Orientation;
use image::{ColorType, ImageDecoder, ImageError, ImageFormat, Limits};
use rustix::fs::OFlags;

/// The widest and tallest source image accepted, in pixels.
pub const MAX_IMAGE_DIMENSION: u32 = 8_192;
/// The largest source area accepted, in pixels (40 MP).
pub const MAX_SOURCE_PIXELS: u64 = 40_000_000;
/// The largest decoded image, in bytes: 40 MP of 8-bit RGBA.
pub const MAX_DECODED_BYTES: u64 = 4 * MAX_SOURCE_PIXELS;
/// The most the decode of one file may need, predicted from its header and
/// size before any pixel is decoded (the decoded image included).
pub const MAX_DECODE_BYTES: u64 = 320 * 1024 * 1024;
/// The most a PNG's ancillary chunks (a decompressed `iCCP` profile
/// included) may take; an `iCCP` bomb fails here.
pub const PNG_METADATA_BYTES: u64 = 16 * 1024 * 1024;
/// Per-row decoder scratch counted in every prediction.
pub const DECODER_SLACK: u64 = 16 * 1024 * 1024;
/// The largest wallpaper file accepted, in bytes.
pub const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
/// The largest source area [`decode_png_thumbnail`] accepts: 16 MP (a 4K
/// screenshot is 8.3 MP). A clipboard image is a selection any client may
/// offer, and its thumbnail is 64 pixels a side; it does not get the
/// wallpaper's budget.
pub const THUMBNAIL_MAX_SOURCE_PIXELS: u64 = 16 * 1024 * 1024;
/// The largest decoded image [`decode_png_thumbnail`] accepts: 64 MiB, 16 MP
/// of 8-bit RGBA (a 16-bit image gets half the area).
pub const THUMBNAIL_MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;
/// The most the decode of one clipboard image may need: the decoded image,
/// the PNG's ancillary chunks and the decoder's scratch.
pub const THUMBNAIL_MAX_DECODE_BYTES: u64 =
    THUMBNAIL_MAX_DECODED_BYTES + PNG_METADATA_BYTES + DECODER_SLACK;
/// The largest output buffer side, in device pixels.
pub const MAX_TARGET_DIMENSION: u32 = 8_192;
/// The largest output buffer area, in device pixels: an 8K output
/// (7680×4320), so the buffer is at most 126.6 MiB.
pub const MAX_TARGET_PIXELS: u64 = 7_680 * 4_320;
/// The file extensions accepted, compared without case. The content must
/// also be one of these formats; a PNG named `.jpg` is accepted, because
/// every accepted decoder is equally bounded.
pub const EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
/// The colour, opaque, of the bars [`Mode::Fit`] and [`Mode::Center`] leave
/// around the image, and of the backdrop translucent pixels are composited
/// over: black, so a letterboxed wallpaper reads as the compositor's own
/// empty background.
pub const BAR_RGB: [u8; 3] = [0, 0, 0];

/// JPEG segments read before the frame header is given up on.
const MAX_JPEG_SEGMENTS: usize = 4_096;
/// RIFF chunks of a WebP file inspected before decoding.
const MAX_WEBP_CHUNKS: usize = 64;
/// The longest panic message carried in [`DecodeError::Panicked`].
const MAX_PANIC_CHARS: usize = 256;

/// A decoded wallpaper: opaque RGBA8, row-major, exactly `width × height`
/// (the requested target), ready to upload as one texture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// `width * height * 4` bytes; every alpha byte is 255.
    pub rgba: Vec<u8>,
    /// The buffer width in device pixels (the requested target width).
    pub width: u32,
    /// The buffer height in device pixels (the requested target height).
    pub height: u32,
    /// The source's size after its EXIF orientation was applied.
    pub source_px: (u32, u32),
}

/// Why a wallpaper file could not be decoded.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// The requested buffer is empty or larger than [`MAX_TARGET_DIMENSION`]
    /// or [`MAX_TARGET_PIXELS`].
    #[error("invalid target size {width}x{height}")]
    InvalidTarget { width: u32, height: u32 },
    /// The file name's extension is not one of [`EXTENSIONS`].
    #[error("unsupported file extension: {}", path.display())]
    UnsupportedExtension { path: PathBuf },
    /// The file could not be inspected or opened.
    #[error("cannot open {}", path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The open file could not be inspected or read.
    #[error("cannot read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The path is a directory, FIFO, socket or device.
    #[error("not a regular file: {}", path.display())]
    NotRegularFile { path: PathBuf },
    /// The file is empty.
    #[error("empty file: {}", path.display())]
    Empty { path: PathBuf },
    /// The file is larger than [`MAX_FILE_BYTES`].
    #[error("file of {bytes} bytes exceeds {MAX_FILE_BYTES}: {}", path.display())]
    TooLarge { path: PathBuf, bytes: u64 },
    /// The content is not PNG, JPEG or WebP.
    #[error("unsupported image format{}: {}", format.map(|f| format!(" {f:?}")).unwrap_or_default(), path.display())]
    UnsupportedFormat {
        path: PathBuf,
        format: Option<ImageFormat>,
    },
    /// The header declares an empty side or one beyond
    /// [`MAX_IMAGE_DIMENSION`].
    #[error("image {width}x{height} exceeds {MAX_IMAGE_DIMENSION} pixels a side: {}", path.display())]
    Dimensions {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    /// The header declares more than the decode's area bound
    /// ([`MAX_SOURCE_PIXELS`] for a wallpaper, [`THUMBNAIL_MAX_SOURCE_PIXELS`]
    /// for a clipboard thumbnail).
    #[error("image {width}x{height} exceeds {limit} pixels: {}", path.display())]
    TooManyPixels {
        path: PathBuf,
        width: u32,
        height: u32,
        limit: u64,
    },
    /// Decoding would need more than the decode's budget
    /// ([`MAX_DECODE_BYTES`] for a wallpaper, [`THUMBNAIL_MAX_DECODE_BYTES`]
    /// for a clipboard thumbnail), or the decoded image more than its bound
    /// (a 16-bit image of a large area, a progressive JPEG with
    /// full-resolution chroma, ...).
    #[error("decoding needs about {needed} bytes, over the {budget}-byte budget: {}", path.display())]
    OverBudget {
        path: PathBuf,
        needed: u64,
        budget: u64,
    },
    /// The decoder produces a colour type this shell does not scale.
    #[error("unsupported colour type {color:?}: {}", path.display())]
    UnsupportedColor { path: PathBuf, color: ColorType },
    /// The file holds more than one frame (an animated PNG or WebP).
    #[error("animated images are not wallpapers: {}", path.display())]
    MultiFrame { path: PathBuf },
    /// A coding whose decoder cannot be bounded (lossless WebP).
    #[error("unsupported {feature}: {}", path.display())]
    Unsupported {
        path: PathBuf,
        feature: &'static str,
    },
    /// The decoder refused a limit (dimensions or allocation) itself.
    #[error("image exceeds the decode limits: {}", path.display())]
    Limits {
        path: PathBuf,
        #[source]
        source: ImageError,
    },
    /// The content is malformed or truncated.
    #[error("cannot decode {}", path.display())]
    Decode {
        path: PathBuf,
        #[source]
        source: ImageError,
    },
    /// The file's size or modification time changed while it was decoded.
    #[error("file changed while being decoded: {}", path.display())]
    Changed { path: PathBuf },
    /// A decoder panicked on this file; the worker carries on.
    #[error("the decoder panicked on {}: {message}", path.display())]
    Panicked { path: PathBuf, message: String },
}

/// Decodes the image at `path` (symlinks are followed once) and lays it by
/// `mode`, through the one placement rule
/// (`celestina_shell::wallpaper::placement`), into an opaque RGBA buffer of
/// exactly `target_px` (width, height) device pixels. Downscaling uses
/// Lanczos3, upscaling Catmull-Rom.
///
/// # Errors
///
/// A [`DecodeError`] naming the first bound or check the file failed; no
/// hostile file blocks, none is decoded past its header when the header is
/// out of bounds, and a decoder panic is returned as
/// [`DecodeError::Panicked`].
pub fn decode(path: &Path, target_px: (u32, u32), mode: Mode) -> Result<Decoded, DecodeError> {
    panic::catch_unwind(AssertUnwindSafe(|| decode_unguarded(path, target_px, mode)))
        .unwrap_or_else(|payload| {
            Err(DecodeError::Panicked {
                path: path.to_owned(),
                message: panic_message(payload.as_ref()),
            })
        })
}

/// What an in-memory image is called in an error: it has no path.
const IN_MEMORY: &str = "<in-memory image>";

/// Decodes the PNG in `bytes` (the clipboard's `image/png`) into an opaque
/// RGBA thumbnail that fits a square of `max_side` pixels, keeping the aspect
/// ratio and never enlarging. Every bound of this module applies: the
/// header's dimensions, the memory budget checked before a pixel is decoded,
/// one frame only, a caught decoder panic; the area and memory bounds are the
/// tighter [`THUMBNAIL_MAX_SOURCE_PIXELS`], [`THUMBNAIL_MAX_DECODED_BYTES`]
/// and [`THUMBNAIL_MAX_DECODE_BYTES`]. Anything that is not a PNG is
/// refused.
///
/// # Errors
///
/// A [`DecodeError`] naming the first bound or check the bytes failed. The
/// error carries no byte of the image.
pub fn decode_png_thumbnail(bytes: &[u8], max_side: u32) -> Result<Decoded, DecodeError> {
    let path = Path::new(IN_MEMORY);
    panic::catch_unwind(AssertUnwindSafe(|| {
        thumbnail_unguarded(path, bytes, max_side)
    }))
    .unwrap_or_else(|payload| {
        Err(DecodeError::Panicked {
            path: path.to_owned(),
            message: panic_message(payload.as_ref()),
        })
    })
}

fn thumbnail_unguarded(path: &Path, bytes: &[u8], max_side: u32) -> Result<Decoded, DecodeError> {
    if max_side == 0 || max_side > MAX_TARGET_DIMENSION {
        return Err(DecodeError::InvalidTarget {
            width: max_side,
            height: max_side,
        });
    }
    let len = bytes.len() as u64;
    if len == 0 {
        return Err(DecodeError::Empty {
            path: path.to_owned(),
        });
    }
    if len > MAX_FILE_BYTES {
        return Err(DecodeError::TooLarge {
            path: path.to_owned(),
            bytes: len,
        });
    }
    let source = read_source_from(
        path,
        io::Cursor::new(bytes),
        len,
        Formats::PngOnly,
        Budget::THUMBNAIL,
    )?;
    let (image_w, image_h) = source.oriented_size();
    let (width, height) = fit_within(image_w, image_h, max_side);
    let rgba =
        render(&source, Mode::Stretch, (width, height)).ok_or_else(|| DecodeError::OverBudget {
            path: path.to_owned(),
            needed: u64::from(width) * u64::from(height) * 4,
            budget: THUMBNAIL_MAX_DECODE_BYTES,
        })?;
    Ok(Decoded {
        rgba,
        width,
        height,
        source_px: (image_w, image_h),
    })
}

/// `width` × `height` scaled down to fit a square of `max_side`, each side at
/// least one pixel; a smaller image keeps its size.
fn fit_within(width: u32, height: u32, max_side: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_side {
        return (width, height);
    }
    let scaled = |side: u32| {
        let value =
            (u64::from(side) * u64::from(max_side) + u64::from(longest) / 2) / u64::from(longest);
        u32::try_from(value).unwrap_or(max_side).clamp(1, max_side)
    };
    (scaled(width), scaled(height))
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    let text = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic");
    text.chars().take(MAX_PANIC_CHARS).collect()
}

fn decode_unguarded(
    path: &Path,
    target_px: (u32, u32),
    mode: Mode,
) -> Result<Decoded, DecodeError> {
    let (target_w, target_h) = target_px;
    if target_w == 0
        || target_h == 0
        || target_w > MAX_TARGET_DIMENSION
        || target_h > MAX_TARGET_DIMENSION
        || u64::from(target_w) * u64::from(target_h) > MAX_TARGET_PIXELS
    {
        return Err(DecodeError::InvalidTarget {
            width: target_w,
            height: target_h,
        });
    }
    if !has_accepted_extension(path) {
        return Err(DecodeError::UnsupportedExtension {
            path: path.to_owned(),
        });
    }

    let (file, expected) = open_regular(path)?;
    let source = read_source(path, &file, expected.bytes)?;
    let after = file.metadata().map_err(|source| DecodeError::Read {
        path: path.to_owned(),
        source,
    })?;
    if Fingerprint::from_metadata(&after) != expected {
        return Err(DecodeError::Changed {
            path: path.to_owned(),
        });
    }
    drop(file);

    let rgba = render(&source, mode, target_px).ok_or_else(|| DecodeError::OverBudget {
        path: path.to_owned(),
        needed: u64::from(target_w) * u64::from(target_h) * 4,
        budget: MAX_DECODE_BYTES,
    })?;
    Ok(Decoded {
        rgba,
        width: target_w,
        height: target_h,
        source_px: source.oriented_size(),
    })
}

fn has_accepted_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| EXTENSIONS.iter().any(|ok| ok.eq_ignore_ascii_case(ext)))
}

/// Refuses anything but a regular file by `stat` before opening it, opens it
/// without blocking and without taking a controlling terminal, and checks
/// again on the descriptor (the path may have been swapped in between) that
/// it is a non-empty regular file within [`MAX_FILE_BYTES`].
pub(crate) fn open_regular(path: &Path) -> Result<(File, Fingerprint), DecodeError> {
    let before = std::fs::metadata(path).map_err(|source| DecodeError::Open {
        path: path.to_owned(),
        source,
    })?;
    if !before.is_file() {
        return Err(DecodeError::NotRegularFile {
            path: path.to_owned(),
        });
    }
    let descriptor = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY,
        rustix::fs::Mode::empty(),
    )
    .map_err(|errno| DecodeError::Open {
        path: path.to_owned(),
        source: io::Error::from(errno),
    })?;
    let file = File::from(descriptor);
    let metadata = file.metadata().map_err(|source| DecodeError::Read {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(DecodeError::NotRegularFile {
            path: path.to_owned(),
        });
    }
    let fingerprint = Fingerprint::from_metadata(&metadata);
    if fingerprint.bytes == 0 {
        return Err(DecodeError::Empty {
            path: path.to_owned(),
        });
    }
    if fingerprint.bytes > MAX_FILE_BYTES {
        return Err(DecodeError::TooLarge {
            path: path.to_owned(),
            bytes: fingerprint.bytes,
        });
    }
    Ok((file, fingerprint))
}

/// A reader that ends at `end` bytes whatever the file does later: the
/// decoders need `Seek`, which `Read::take` does not offer.
struct Capped<R> {
    inner: R,
    position: u64,
    end: u64,
}

impl<R: Read> Read for Capped<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.end.saturating_sub(self.position);
        if left == 0 {
            return Ok(0);
        }
        let len = usize::try_from(left).map_or(buf.len(), |left| left.min(buf.len()));
        let read = self.inner.read(&mut buf[..len])?;
        self.position = self.position.saturating_add(read as u64);
        Ok(read)
    }
}

impl<R: Seek> Seek for Capped<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let position = self.inner.seek(pos)?;
        self.position = position;
        Ok(position)
    }
}

type Reader<'f> = BufReader<Capped<&'f File>>;

/// A decoded source image in its stored orientation, with the EXIF
/// orientation still to apply.
struct Source {
    /// Row-major samples, native-endian for 16-bit types.
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    channels: usize,
    wide: bool,
    orientation: Orientation,
}

fn read_source(path: &Path, file: &File, file_bytes: u64) -> Result<Source, DecodeError> {
    let reader: Reader<'_> = BufReader::new(Capped {
        inner: file,
        position: 0,
        end: file_bytes,
    });
    read_source_from(path, reader, file_bytes, Formats::Any, Budget::WALLPAPER)
}

/// The area and memory one decode may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Budget {
    /// The largest source area, in pixels.
    pixels: u64,
    /// The largest decoded image, in bytes.
    decoded: u64,
    /// The most the whole decode may need, in bytes.
    total: u64,
}

impl Budget {
    /// A wallpaper file.
    const WALLPAPER: Self = Self {
        pixels: MAX_SOURCE_PIXELS,
        decoded: MAX_DECODED_BYTES,
        total: MAX_DECODE_BYTES,
    };
    /// A clipboard image made into a thumbnail.
    const THUMBNAIL: Self = Self {
        pixels: THUMBNAIL_MAX_SOURCE_PIXELS,
        decoded: THUMBNAIL_MAX_DECODED_BYTES,
        total: THUMBNAIL_MAX_DECODE_BYTES,
    };
}

/// Which formats [`read_source_from`] accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Formats {
    /// PNG, JPEG and WebP: a wallpaper file.
    Any,
    /// PNG only: the clipboard's `image/png`.
    PngOnly,
}

/// Reads the image `reader` holds, `file_bytes` long, through every bound of
/// the module; `path` only names it in errors.
fn read_source_from<R: BufRead + Seek>(
    path: &Path,
    mut reader: R,
    file_bytes: u64,
    formats: Formats,
    budget: Budget,
) -> Result<Source, DecodeError> {
    let read_error = |source: io::Error| DecodeError::Read {
        path: path.to_owned(),
        source,
    };
    let mut head = [0u8; 32];
    let mut filled = 0;
    while filled < head.len() {
        match reader.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(read_error(error)),
        }
    }
    reader.rewind().map_err(read_error)?;
    let format = image::guess_format(&head[..filled]).ok();
    let decode_error = |source: ImageError| image_error(path, source);
    match format {
        Some(format) if formats == Formats::PngOnly && format != ImageFormat::Png => {
            Err(DecodeError::UnsupportedFormat {
                path: path.to_owned(),
                format: Some(format),
            })
        }
        Some(ImageFormat::Png) => {
            let mut limits = dimension_limits();
            limits.max_alloc = Some(PNG_METADATA_BYTES);
            let decoder = PngDecoder::with_limits(reader, limits).map_err(decode_error)?;
            if decoder.is_apng().map_err(decode_error)? {
                return Err(DecodeError::MultiFrame {
                    path: path.to_owned(),
                });
            }
            read_pixels(path, decoder, PNG_METADATA_BYTES, budget)
        }
        Some(ImageFormat::Jpeg) => {
            let frame = jpeg_frame(path, &mut reader)?;
            check_dimensions(path, frame.width, frame.height, budget)?;
            reader.rewind().map_err(read_error)?;
            let mut decoder = JpegDecoder::new(reader).map_err(decode_error)?;
            decoder
                .set_limits(dimension_limits())
                .map_err(decode_error)?;
            read_pixels(
                path,
                decoder,
                file_bytes
                    .saturating_mul(2)
                    .saturating_add(frame.coefficient_bytes),
                budget,
            )
        }
        Some(ImageFormat::WebP) => {
            let header = webp_header(path, &mut reader)?;
            if header.animated {
                return Err(DecodeError::MultiFrame {
                    path: path.to_owned(),
                });
            }
            if header.lossless {
                return Err(DecodeError::Unsupported {
                    path: path.to_owned(),
                    feature: "lossless WebP coding",
                });
            }
            check_dimensions(path, header.width, header.height, budget)?;
            reader.rewind().map_err(read_error)?;
            let mut decoder = WebPDecoder::new(reader).map_err(decode_error)?;
            if decoder.has_animation() {
                return Err(DecodeError::MultiFrame {
                    path: path.to_owned(),
                });
            }
            decoder
                .set_limits(dimension_limits())
                .map_err(decode_error)?;
            let (width, height) = decoder.dimensions();
            let planes = u64::from(width.div_ceil(16) * 16) * u64::from(height.div_ceil(16) * 16);
            let alpha = if header.alpha {
                u64::from(width) * u64::from(height)
            } else {
                0
            };
            let extra = planes
                .saturating_mul(3)
                .div_ceil(2)
                .saturating_add(file_bytes.saturating_mul(2))
                .saturating_add(alpha);
            read_pixels(path, decoder, extra, budget)
        }
        format => Err(DecodeError::UnsupportedFormat {
            path: path.to_owned(),
            format,
        }),
    }
}

fn dimension_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits
}

fn image_error(path: &Path, source: ImageError) -> DecodeError {
    match source {
        ImageError::Limits(_) => DecodeError::Limits {
            path: path.to_owned(),
            source,
        },
        source => DecodeError::Decode {
            path: path.to_owned(),
            source,
        },
    }
}

/// A header the pre-decode parse could not read: the decoder would fail on
/// it too, so it is reported the same way.
fn malformed(path: &Path, format: ImageFormat, what: &'static str) -> DecodeError {
    DecodeError::Decode {
        path: path.to_owned(),
        source: ImageError::Decoding(DecodingError::new(ImageFormatHint::Exact(format), what)),
    }
}

fn check_dimensions(
    path: &Path,
    width: u32,
    height: u32,
    budget: Budget,
) -> Result<(), DecodeError> {
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(DecodeError::Dimensions {
            path: path.to_owned(),
            width,
            height,
        });
    }
    if u64::from(width) * u64::from(height) > budget.pixels {
        return Err(DecodeError::TooManyPixels {
            path: path.to_owned(),
            width,
            height,
            limit: budget.pixels,
        });
    }
    Ok(())
}

/// Checks the header against every bound, predicts the decode's memory
/// (`extra` is what the decoder needs beyond the decoded image) and only then
/// decodes the pixels into one buffer.
fn read_pixels<D: ImageDecoder>(
    path: &Path,
    mut decoder: D,
    extra: u64,
    budget: Budget,
) -> Result<Source, DecodeError> {
    let (width, height) = decoder.dimensions();
    check_dimensions(path, width, height, budget)?;
    let color = decoder.color_type();
    let (channels, wide) = match color {
        ColorType::L8 => (1, false),
        ColorType::La8 => (2, false),
        ColorType::Rgb8 => (3, false),
        ColorType::Rgba8 => (4, false),
        ColorType::L16 => (1, true),
        ColorType::La16 => (2, true),
        ColorType::Rgb16 => (3, true),
        ColorType::Rgba16 => (4, true),
        color => {
            return Err(DecodeError::UnsupportedColor {
                path: path.to_owned(),
                color,
            })
        }
    };
    let total = decoder.total_bytes();
    let needed = total.saturating_add(extra).saturating_add(DECODER_SLACK);
    let over = || DecodeError::OverBudget {
        path: path.to_owned(),
        needed,
        budget: budget.total,
    };
    if total > budget.decoded || needed > budget.total {
        return Err(over());
    }
    let len = usize::try_from(total).map_err(|_| over())?;
    let orientation = decoder
        .orientation()
        .map_err(|source| image_error(path, source))?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(len).map_err(|_| over())?;
    pixels.resize(len, 0);
    decoder
        .read_image(&mut pixels)
        .map_err(|source| image_error(path, source))?;
    Ok(Source {
        pixels,
        width,
        height,
        channels,
        wide,
        orientation,
    })
}

fn read_u8<R: Read>(reader: &mut R) -> io::Result<u8> {
    let mut byte = [0u8; 1];
    reader.read_exact(&mut byte)?;
    Ok(byte[0])
}

fn read_u16_be<R: Read>(reader: &mut R) -> io::Result<u16> {
    let mut bytes = [0u8; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_be_bytes(bytes))
}

/// What the JPEG decoder will allocate besides the file and the image.
struct JpegFrame {
    width: u32,
    height: u32,
    /// The whole-image coefficient store (`i16` per sample, padded to MCUs)
    /// when the decoder keeps one: progressive images, and baseline images
    /// whose first scan does not hold every component.
    coefficient_bytes: u64,
}

/// A JPEG start-of-frame header.
struct FrameHeader {
    width: u32,
    height: u32,
    progressive: bool,
    /// Each component's horizontal and vertical sampling factor.
    components: Vec<(u64, u64)>,
}

/// Reads the JPEG markers up to the first scan header: the frame's size and
/// sampling, whether it is progressive, and how many components the first
/// scan holds. Segment payloads other than the frame header are skipped.
fn jpeg_frame<R: BufRead + Seek>(path: &Path, reader: &mut R) -> Result<JpegFrame, DecodeError> {
    let bad = |what| malformed(path, ImageFormat::Jpeg, what);
    let io_bad = |error: io::Error| match error.kind() {
        io::ErrorKind::UnexpectedEof => bad("truncated JPEG header"),
        _ => DecodeError::Read {
            path: path.to_owned(),
            source: error,
        },
    };
    if read_u16_be(reader).map_err(io_bad)? != 0xFFD8 {
        return Err(bad("missing JPEG start of image"));
    }
    let mut frame: Option<FrameHeader> = None;
    for _ in 0..MAX_JPEG_SEGMENTS {
        if read_u8(reader).map_err(io_bad)? != 0xFF {
            return Err(bad("JPEG marker expected"));
        }
        let mut marker = read_u8(reader).map_err(io_bad)?;
        while marker == 0xFF {
            marker = read_u8(reader).map_err(io_bad)?;
        }
        match marker {
            0x01 | 0xD0..=0xD8 => continue,
            0xD9 => return Err(bad("JPEG ends before its first scan")),
            _ => {}
        }
        let length = read_u16_be(reader).map_err(io_bad)?;
        let payload = length
            .checked_sub(2)
            .ok_or_else(|| bad("JPEG segment length"))?;
        let is_frame = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_frame {
            let mut bytes = vec![0u8; usize::from(payload)];
            reader.read_exact(&mut bytes).map_err(io_bad)?;
            let [_, h0, h1, w0, w1, count, rest @ ..] = bytes.as_slice() else {
                return Err(bad("short JPEG frame header"));
            };
            let count = usize::from(*count);
            if count == 0 || rest.len() < count * 3 {
                return Err(bad("JPEG frame components"));
            }
            let components = rest
                .as_chunks::<3>()
                .0
                .iter()
                .take(count)
                .map(|component| (u64::from(component[1] >> 4), u64::from(component[1] & 0x0F)))
                .collect::<Vec<_>>();
            if components
                .iter()
                .any(|&(h, v)| !(1..=4).contains(&h) || !(1..=4).contains(&v))
            {
                return Err(bad("JPEG sampling factors"));
            }
            frame = Some(FrameHeader {
                width: u32::from(u16::from_be_bytes([*w0, *w1])),
                height: u32::from(u16::from_be_bytes([*h0, *h1])),
                progressive: matches!(marker, 0xC2 | 0xC6 | 0xCA | 0xCE),
                components,
            });
        } else if marker == 0xDA {
            let FrameHeader {
                width,
                height,
                progressive,
                components,
            } = frame.ok_or_else(|| bad("JPEG scan before its frame header"))?;
            let in_first_scan = usize::from(read_u8(reader).map_err(io_bad)?);
            let h_max = components.iter().map(|c| c.0).max().unwrap_or(1);
            let v_max = components.iter().map(|c| c.1).max().unwrap_or(1);
            let mcu_x = u64::from(width).div_ceil(8 * h_max);
            let mcu_y = u64::from(height).div_ceil(8 * v_max);
            let coefficient_bytes = if progressive || in_first_scan < components.len() {
                components
                    .iter()
                    .map(|&(h, v)| 2 * 64 * h * v * mcu_x * mcu_y)
                    .sum()
            } else {
                0
            };
            return Ok(JpegFrame {
                width,
                height,
                coefficient_bytes,
            });
        } else {
            reader.seek_relative(i64::from(payload)).map_err(io_bad)?;
        }
    }
    Err(bad("too many JPEG segments before the first scan"))
}

/// What the RIFF chunks of a WebP file say before it is decoded.
struct WebpHeader {
    width: u32,
    height: u32,
    animated: bool,
    /// A `VP8L` image or a losslessly compressed `ALPH` plane.
    lossless: bool,
    /// An uncompressed alpha plane the decoder holds whole.
    alpha: bool,
}

fn webp_header<R: BufRead + Seek>(path: &Path, reader: &mut R) -> Result<WebpHeader, DecodeError> {
    let bad = |what| malformed(path, ImageFormat::WebP, what);
    let io_bad = |error: io::Error| match error.kind() {
        io::ErrorKind::UnexpectedEof => bad("truncated WebP header"),
        _ => DecodeError::Read {
            path: path.to_owned(),
            source: error,
        },
    };
    let mut riff = [0u8; 12];
    reader.read_exact(&mut riff).map_err(io_bad)?;
    if &riff[..4] != b"RIFF" || &riff[8..] != b"WEBP" {
        return Err(bad("not a RIFF WebP file"));
    }
    let mut header = WebpHeader {
        width: 0,
        height: 0,
        animated: false,
        lossless: false,
        alpha: false,
    };
    for _ in 0..MAX_WEBP_CHUNKS {
        let mut chunk = [0u8; 8];
        match reader.read_exact(&mut chunk) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(io_bad(error)),
        }
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        let start = reader.stream_position().map_err(io_bad)?;
        match &chunk[..4] {
            b"VP8X" => {
                let mut bytes = [0u8; 10];
                reader.read_exact(&mut bytes).map_err(io_bad)?;
                header.animated |= bytes[0] & 0x02 != 0;
                header.width = 1 + u32::from_le_bytes([bytes[4], bytes[5], bytes[6], 0]);
                header.height = 1 + u32::from_le_bytes([bytes[7], bytes[8], bytes[9], 0]);
            }
            b"VP8 " => {
                let mut bytes = [0u8; 10];
                reader.read_exact(&mut bytes).map_err(io_bad)?;
                if header.width == 0 {
                    header.width = u32::from(u16::from_le_bytes([bytes[6], bytes[7]]) & 0x3FFF);
                    header.height = u32::from(u16::from_le_bytes([bytes[8], bytes[9]]) & 0x3FFF);
                }
            }
            b"VP8L" => header.lossless = true,
            b"ANIM" | b"ANMF" => header.animated = true,
            b"ALPH" => match read_u8(reader).map_err(io_bad)? & 0x03 {
                0 => header.alpha = true,
                _ => header.lossless = true,
            },
            _ => {}
        }
        let next = start
            .saturating_add(u64::from(size))
            .saturating_add(u64::from(size & 1));
        reader.seek(SeekFrom::Start(next)).map_err(io_bad)?;
    }
    Ok(header)
}

impl Source {
    fn transposed(&self) -> bool {
        matches!(
            self.orientation,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Rotate90FlipH
                | Orientation::Rotate270FlipH
        )
    }

    /// The size once the orientation is applied.
    fn oriented_size(&self) -> (u32, u32) {
        if self.transposed() {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    /// The stored pixel index of oriented `(x, y)` is `base + x·dx + y·dy`.
    fn walk(&self) -> (i64, i64, i64) {
        let w = i64::from(self.width);
        let last_row = (i64::from(self.height) - 1) * w;
        let last_col = w - 1;
        match self.orientation {
            Orientation::NoTransforms => (0, 1, w),
            Orientation::Rotate90 => (last_row, -w, 1),
            Orientation::Rotate180 => (last_row + last_col, -1, -w),
            Orientation::Rotate270 => (last_col, w, -1),
            Orientation::FlipHorizontal => (last_col, -1, w),
            Orientation::FlipVertical => (last_row, 1, -w),
            Orientation::Rotate90FlipH => (0, w, 1),
            Orientation::Rotate270FlipH => (last_row + last_col, -w, -1),
        }
    }

    /// Fills `out` with the oriented row `y` from column `x0`, composited
    /// over [`BAR_RGB`], as RGB in `0.0..=255.0`.
    fn read_row(&self, y: u32, x0: u32, out: &mut [[f32; 3]]) {
        let (base, dx, dy) = self.walk();
        let row = base + i64::from(y) * dy;
        let bytes_per_sample = if self.wide { 2 } else { 1 };
        let stride = self.channels * bytes_per_sample;
        let bar = BAR_RGB.map(f32::from);
        for (i, slot) in out.iter_mut().enumerate() {
            let x = i64::from(x0) + i as i64;
            let start = usize::try_from(row + x * dx).unwrap_or(usize::MAX);
            let Some(pixel) = start
                .checked_mul(stride)
                .and_then(|at| self.pixels.get(at..at + stride))
            else {
                *slot = bar;
                continue;
            };
            let sample = |k: usize| -> f32 {
                if self.wide {
                    f32::from(u16::from_ne_bytes([pixel[2 * k], pixel[2 * k + 1]]))
                        * (255.0 / 65_535.0)
                } else {
                    f32::from(pixel[k])
                }
            };
            let (rgb, alpha) = match self.channels {
                1 => ([sample(0); 3], 255.0),
                2 => ([sample(0); 3], sample(1)),
                3 => ([sample(0), sample(1), sample(2)], 255.0),
                _ => ([sample(0), sample(1), sample(2)], sample(3)),
            };
            let a = alpha / 255.0;
            *slot = [
                rgb[0] * a + bar[0] * (1.0 - a),
                rgb[1] * a + bar[1] * (1.0 - a),
                rgb[2] * a + bar[2] * (1.0 - a),
            ];
        }
    }
}

/// The source samples one destination pixel reads along one axis.
#[derive(Debug, Clone, PartialEq)]
struct Taps {
    first: u32,
    weights: Vec<f32>,
}

fn lanczos3(x: f64) -> f64 {
    let sinc = |t: f64| {
        if t == 0.0 {
            1.0
        } else {
            let pt = std::f64::consts::PI * t;
            pt.sin() / pt
        }
    };
    if x.abs() < 3.0 {
        sinc(x) * sinc(x / 3.0)
    } else {
        0.0
    }
}

/// The cubic with B = 0, C = 0.5.
fn catmull_rom(x: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 {
        1.5 * x * x * x - 2.5 * x * x + 1.0
    } else if x < 2.0 {
        -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
    } else {
        0.0
    }
}

/// Rounds a placement edge to whole device pixels: the start to the nearest
/// pixel, the length to at least one.
fn span(start: f32, length: f32) -> (i64, u32) {
    (start.round() as i64, length.round().max(1.0) as u32)
}

/// For each of `dst` destination pixels, the source taps when the pixel lies
/// inside the image's span (`span_start`, `span_len`) and `None` (a bar)
/// otherwise. The total number of weights is about `6·src + 5·dst`.
fn axis(src: u32, span_start: i64, span_len: u32, dst: u32) -> Vec<Option<Taps>> {
    let downscale = span_len < src;
    let (kernel, radius): (fn(f64) -> f64, f64) = if downscale {
        (lanczos3, 3.0)
    } else {
        (catmull_rom, 2.0)
    };
    let ratio = f64::from(src) / f64::from(span_len.max(1));
    let scale = ratio.max(1.0);
    let support = radius * scale;
    let last = f64::from(src.max(1) - 1);
    (0..dst)
        .map(|t| {
            let rel = i64::from(t) - span_start;
            if rel < 0 || rel >= i64::from(span_len) {
                return None;
            }
            let centre = (rel as f64 + 0.5) * ratio - 0.5;
            let lo = (centre - support).ceil().clamp(0.0, last);
            let hi = (centre + support).floor().clamp(lo, last);
            let mut weights = Vec::with_capacity((hi - lo) as usize + 1);
            let mut index = lo;
            while index <= hi {
                weights.push(kernel((index - centre) / scale));
                index += 1.0;
            }
            let sum: f64 = weights.iter().sum();
            if sum.abs() < 1e-9 {
                return Some(Taps {
                    first: centre.round().clamp(0.0, last) as u32,
                    weights: vec![1.0],
                });
            }
            Some(Taps {
                first: lo as u32,
                weights: weights.into_iter().map(|w| (w / sum) as f32).collect(),
            })
        })
        .collect()
}

fn to_byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

/// Lays the oriented source onto a `target_px` buffer where [`placement`]
/// puts it, sampling row by row: one vertically filtered row of the visible
/// source columns at a time, so nothing but the output, two rows and the
/// weights is ever allocated. `None` when the output buffer cannot be
/// allocated.
fn render(source: &Source, mode: Mode, target_px: (u32, u32)) -> Option<Vec<u8>> {
    let (target_w, target_h) = target_px;
    let (image_w, image_h) = source.oriented_size();
    let rect = placement(mode, (image_w, image_h), target_px);
    let (x0, width) = span(rect.x, rect.w);
    let (y0, height) = span(rect.y, rect.h);
    let columns = axis(image_w, x0, width, target_w);
    let rows = axis(image_h, y0, height, target_h);

    let first = columns.iter().flatten().map(|t| t.first).min();
    let last = columns
        .iter()
        .flatten()
        .map(|t| t.first + t.weights.len() as u32 - 1)
        .max();
    let len = usize::try_from(u64::from(target_w) * u64::from(target_h) * 4).ok()?;
    let mut out = Vec::new();
    out.try_reserve_exact(len).ok()?;
    let [bar_r, bar_g, bar_b] = BAR_RGB;
    let bar = [bar_r, bar_g, bar_b, u8::MAX];
    let (Some(c0), Some(c1)) = (first, last) else {
        for _ in 0..u64::from(target_w) * u64::from(target_h) {
            out.extend_from_slice(&bar);
        }
        return Some(out);
    };
    let visible = (c1 - c0 + 1) as usize;
    let mut line = vec![[0f32; 3]; visible];
    let mut row = vec![[0f32; 3]; visible];
    for vertical in &rows {
        let Some(vertical) = vertical else {
            for _ in 0..target_w {
                out.extend_from_slice(&bar);
            }
            continue;
        };
        line.iter_mut().for_each(|p| *p = [0.0; 3]);
        for (k, &weight) in vertical.weights.iter().enumerate() {
            source.read_row(vertical.first + k as u32, c0, &mut row);
            for (acc, px) in line.iter_mut().zip(&row) {
                acc[0] += weight * px[0];
                acc[1] += weight * px[1];
                acc[2] += weight * px[2];
            }
        }
        for horizontal in &columns {
            let Some(horizontal) = horizontal else {
                out.extend_from_slice(&bar);
                continue;
            };
            let start = (horizontal.first - c0) as usize;
            let mut rgb = [0f32; 3];
            for (px, &weight) in line[start..].iter().zip(&horizontal.weights) {
                rgb[0] += weight * px[0];
                rgb[1] += weight * px[1];
                rgb[2] += weight * px[2];
            }
            out.extend_from_slice(&[to_byte(rgb[0]), to_byte(rgb[1]), to_byte(rgb[2]), u8::MAX]);
        }
    }
    Some(out)
}

/// The size and modification time of a file, compared before and after the
/// decode to detect a file replaced or rewritten under the decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fingerprint {
    pub(crate) bytes: u64,
    modified_ns: Option<u128>,
}

impl Fingerprint {
    fn from_metadata(metadata: &Metadata) -> Self {
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos());
        Self {
            bytes: metadata.len(),
            modified_ns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [Mode; 4] = [Mode::Fill, Mode::Fit, Mode::Stretch, Mode::Center];

    /// An 8-bit RGBA source from a pixel function, stored as given.
    fn rgba(w: u32, h: u32, orientation: Orientation, f: impl Fn(u32, u32) -> [u8; 4]) -> Source {
        let mut pixels = Vec::new();
        for y in 0..h {
            for x in 0..w {
                pixels.extend_from_slice(&f(x, y));
            }
        }
        Source {
            pixels,
            width: w,
            height: h,
            channels: 4,
            wide: false,
            orientation,
        }
    }

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Source {
        rgba(w, h, Orientation::NoTransforms, |_, _| px)
    }

    fn at(out: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [out[i], out[i + 1], out[i + 2], out[i + 3]]
    }

    #[test]
    fn extensions_are_compared_without_case_and_need_a_dot() {
        assert!(has_accepted_extension(Path::new("/w/a.PNG")));
        assert!(has_accepted_extension(Path::new("/w/a.jpeg")));
        assert!(has_accepted_extension(Path::new("/w/a.WebP")));
        assert!(!has_accepted_extension(Path::new("/w/png")));
        assert!(!has_accepted_extension(Path::new("/w/a.gif")));
        assert!(!has_accepted_extension(Path::new("/w/a.avif")));
    }

    #[test]
    fn translucent_pixels_are_composited_over_the_bar_colour() {
        let out = render(&solid(1, 1, [255, 255, 255, 0]), Mode::Center, (1, 1)).expect("render");
        assert_eq!(at(&out, 1, 0, 0), [0, 0, 0, 255]);
        let out = render(&solid(1, 1, [255, 255, 255, 128]), Mode::Center, (1, 1)).expect("render");
        assert_eq!(at(&out, 1, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn every_mode_returns_exactly_the_target() {
        let shapes = [(40, 20), (20, 40), (30, 30), (8, 1), (1, 8)];
        let targets = [(16, 9), (9, 16), (10, 10), (1, 1)];
        for (sw, sh) in shapes {
            let source = solid(sw, sh, [200, 10, 10, 255]);
            for target in targets {
                for mode in MODES {
                    let out = render(&source, mode, target).expect("render");
                    assert_eq!(out.len(), (target.0 * target.1 * 4) as usize);
                    assert!(out.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
                }
            }
        }
    }

    #[test]
    fn fit_leaves_bars_and_fill_does_not() {
        let source = solid(40, 20, [200, 10, 10, 255]);
        let fitted = render(&source, Mode::Fit, (20, 20)).expect("render");
        assert_eq!(at(&fitted, 20, 10, 0), [0, 0, 0, 255]);
        assert_eq!(at(&fitted, 20, 10, 10)[0], 200);
        let filled = render(&source, Mode::Fill, (20, 20)).expect("render");
        assert!(filled.as_chunks::<4>().0.iter().all(|p| p[0] > 150));
    }

    #[test]
    fn center_copies_one_to_one_and_clips() {
        let source = rgba(4, 4, Orientation::NoTransforms, |x, y| {
            [x as u8 * 10, y as u8 * 10, 9, 255]
        });
        let out = render(&source, Mode::Center, (8, 2)).expect("render");
        assert_eq!(at(&out, 8, 0, 0), [0, 0, 0, 255]);
        // Offset (2, -1): output (3, 1) is source (1, 2), unblurred.
        assert_eq!(at(&out, 8, 3, 1), [10, 20, 9, 255]);
        assert_eq!(at(&out, 8, 6, 1), [0, 0, 0, 255]);
    }

    #[test]
    fn center_rounds_a_half_pixel_offset_instead_of_blurring() {
        // (4 - 3) / 2 = 0.5: rounded to a whole pixel, so the copy is exact.
        let source = rgba(3, 1, Orientation::NoTransforms, |x, _| {
            [x as u8 * 100, 0, 0, 255]
        });
        let out = render(&source, Mode::Center, (4, 1)).expect("render");
        let reds: Vec<u8> = out.as_chunks::<4>().0.iter().map(|p| p[0]).collect();
        assert_eq!(reds, vec![0, 0, 100, 200]);
    }

    #[test]
    fn every_orientation_matches_the_reference_transform() {
        // A 3x2 image whose pixels are all distinct, against image's own
        // `apply_orientation`.
        let f = |x: u32, y: u32| [(x * 40) as u8, (y * 90) as u8, 7, 255];
        for orientation in [
            Orientation::NoTransforms,
            Orientation::Rotate90,
            Orientation::Rotate180,
            Orientation::Rotate270,
            Orientation::FlipHorizontal,
            Orientation::FlipVertical,
            Orientation::Rotate90FlipH,
            Orientation::Rotate270FlipH,
        ] {
            let source = rgba(3, 2, orientation, f);
            let mut reference =
                image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(3, 2, |x, y| {
                    image::Rgba(f(x, y))
                }));
            reference.apply_orientation(orientation);
            let (w, h) = source.oriented_size();
            assert_eq!(
                (w, h),
                (reference.width(), reference.height()),
                "{orientation:?}"
            );
            let out = render(&source, Mode::Stretch, (w, h)).expect("render");
            assert_eq!(out, reference.to_rgba8().into_raw(), "{orientation:?}");
        }
    }

    #[test]
    fn a_rotated_translucent_source_is_flattened_and_turned() {
        // Left column opaque red, right column half-transparent white;
        // rotated 90° clockwise the left column becomes the top row.
        let source = rgba(2, 1, Orientation::Rotate90, |x, _| {
            if x == 0 {
                [255, 0, 0, 255]
            } else {
                [255, 255, 255, 128]
            }
        });
        assert_eq!(source.oriented_size(), (1, 2));
        let out = render(&source, Mode::Stretch, (1, 2)).expect("render");
        assert_eq!(at(&out, 1, 0, 0), [255, 0, 0, 255]);
        assert_eq!(at(&out, 1, 0, 1), [128, 128, 128, 255]);
    }

    #[test]
    fn sixteen_bit_samples_are_scaled_to_eight() {
        let mut pixels = Vec::new();
        for value in [65_535u16, 0, 32_896] {
            pixels.extend_from_slice(&value.to_ne_bytes());
        }
        let source = Source {
            pixels,
            width: 1,
            height: 1,
            channels: 3,
            wide: true,
            orientation: Orientation::NoTransforms,
        };
        let out = render(&source, Mode::Stretch, (1, 1)).expect("render");
        assert_eq!(at(&out, 1, 0, 0), [255, 0, 128, 255]);
    }

    #[test]
    fn the_weights_stay_bounded_for_extreme_ratios() {
        for (src, span_len, dst) in [(8_192, 1, 1), (1, 8_192, 8_192), (8_192, 16_384, 8_192)] {
            let taps = axis(src, 0, span_len, dst);
            let total: usize = taps.iter().flatten().map(|t| t.weights.len()).sum();
            assert!(
                total <= 6 * src as usize + 5 * dst as usize + 8,
                "{src} {span_len} {dst}: {total}"
            );
            for t in taps.iter().flatten() {
                let sum: f32 = t.weights.iter().sum();
                assert!((sum - 1.0).abs() < 1e-3);
                assert!(t.first as usize + t.weights.len() <= src as usize);
            }
        }
    }

    #[test]
    fn a_panic_payload_becomes_a_bounded_message() {
        let payload: Box<dyn Any + Send> = Box::new("x".repeat(10_000));
        assert_eq!(
            panic_message(payload.as_ref()).chars().count(),
            MAX_PANIC_CHARS
        );
        let payload: Box<dyn Any + Send> = Box::new("boom");
        assert_eq!(panic_message(payload.as_ref()), "boom");
        let payload: Box<dyn Any + Send> = Box::new(7u8);
        assert_eq!(panic_message(payload.as_ref()), "unknown panic");
    }

    // ---- In-memory PNG thumbnails (the clipboard's images).

    fn png_of(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(w, h, image::Rgba(px));
        let mut out = io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut out, ImageFormat::Png)
            .expect("encode a png");
        out.into_inner()
    }

    #[test]
    fn a_large_png_becomes_a_thumbnail_that_keeps_its_aspect() {
        let thumb = decode_png_thumbnail(&png_of(400, 200, [10, 20, 30, 255]), 64).expect("decode");
        assert_eq!((thumb.width, thumb.height), (64, 32));
        assert_eq!(thumb.source_px, (400, 200));
        assert_eq!(thumb.rgba.len(), 64 * 32 * 4);
        assert_eq!(at(&thumb.rgba, thumb.width, 5, 5), [10, 20, 30, 255]);
        let tall = decode_png_thumbnail(&png_of(100, 1000, [1, 2, 3, 255]), 64).expect("decode");
        assert_eq!((tall.width, tall.height), (6, 64));
    }

    #[test]
    fn a_small_png_is_not_enlarged() {
        let thumb = decode_png_thumbnail(&png_of(10, 7, [9, 9, 9, 255]), 64).expect("decode");
        assert_eq!((thumb.width, thumb.height), (10, 7));
    }

    #[test]
    fn fit_within_never_returns_an_empty_or_oversized_side() {
        for (w, h) in [(1, 8_192), (8_192, 1), (8_192, 8_192), (65, 64), (64, 65)] {
            let (fw, fh) = fit_within(w, h, 64);
            assert!((1..=64).contains(&fw) && (1..=64).contains(&fh), "{w}x{h}");
        }
    }

    #[test]
    fn the_thumbnail_entry_refuses_what_is_not_a_bounded_png() {
        let png = png_of(8, 8, [0, 0, 0, 255]);
        assert!(matches!(
            decode_png_thumbnail(&png, 0),
            Err(DecodeError::InvalidTarget { .. })
        ));
        assert!(matches!(
            decode_png_thumbnail(&[], 64),
            Err(DecodeError::Empty { .. })
        ));
        assert!(decode_png_thumbnail(b"GIF89a not an image", 64).is_err());
        // A PNG cut short is malformed, not a panic and not a hang.
        assert!(decode_png_thumbnail(&png[..png.len() / 2], 64).is_err());
        // A JPEG decodes elsewhere but is not an `image/png`.
        let mut jpeg = io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3])))
            .write_to(&mut jpeg, ImageFormat::Jpeg)
            .expect("encode a jpeg");
        assert!(matches!(
            decode_png_thumbnail(&jpeg.into_inner(), 64),
            Err(DecodeError::UnsupportedFormat { .. })
        ));
    }

    /// The CRC-32 of the PNG specification (ISO 3309, polynomial
    /// 0xEDB88320), so a test can rewrite a chunk and keep it valid.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFF_u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    /// A real PNG whose IHDR is rewritten to claim `width` x `height` at
    /// `depth` bits per channel, with a correct CRC, so the decoder reads the
    /// header as valid and only a bound can refuse it. The pixel data that
    /// would follow is not there.
    fn png_claiming(width: u32, height: u32, depth: u8) -> Vec<u8> {
        let mut png = png_of(2, 2, [0, 0, 0, 255]);
        // Signature (8), IHDR length (4) and type (4), then the 13 bytes of
        // data and the CRC over type and data.
        assert_eq!(&png[12..16], b"IHDR");
        png[16..20].copy_from_slice(&width.to_be_bytes());
        png[20..24].copy_from_slice(&height.to_be_bytes());
        png[24] = depth;
        let crc = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
        png
    }

    #[test]
    fn the_test_crc_is_the_png_one() {
        // The IHDR of the unmodified image carries the encoder's CRC.
        let png = png_of(2, 2, [0, 0, 0, 255]);
        assert_eq!(crc32(&png[12..29]).to_be_bytes(), png[29..33]);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn a_png_header_over_the_dimension_bound_is_refused_before_decoding() {
        let error =
            decode_png_thumbnail(&png_claiming(60_000, 60_000, 8), 64).expect_err("refused");
        assert!(
            matches!(
                error,
                DecodeError::Dimensions { .. } | DecodeError::Limits { .. }
            ),
            "a bound, not a decode failure: {error:?}"
        );
    }

    #[test]
    fn a_clipboard_png_gets_the_thumbnail_budget_not_the_wallpapers() {
        // 4097 x 4097 is 16.8 MP: within a wallpaper's 40 MP, over 16 MP.
        let error = decode_png_thumbnail(&png_claiming(4_097, 4_097, 8), 64).expect_err("refused");
        assert!(
            matches!(
                error,
                DecodeError::TooManyPixels {
                    limit: THUMBNAIL_MAX_SOURCE_PIXELS,
                    ..
                }
            ),
            "{error:?}"
        );
        // 3000 x 3000 at 16 bits is 72 MB decoded: within the area, over
        // the 64 MiB the thumbnail may decode.
        let error = decode_png_thumbnail(&png_claiming(3_000, 3_000, 16), 64).expect_err("refused");
        assert!(
            matches!(
                error,
                DecodeError::OverBudget {
                    budget: THUMBNAIL_MAX_DECODE_BYTES,
                    ..
                }
            ),
            "{error:?}"
        );
        assert_eq!(THUMBNAIL_MAX_DECODED_BYTES, 4 * THUMBNAIL_MAX_SOURCE_PIXELS);
    }
}
