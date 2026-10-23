//! Hostile and valid `mpris:artUrl` values through
//! `celestina::media::cover::load`. Every fixture is generated in a private
//! temporary directory; every hostile case must fail with its typed error
//! within [`SLOW`].

use std::error::Error as _;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use celestina::media::cover::{
    load, Cover, CoverError, COVER_SIZE, MAX_COVER_PIXELS, MAX_COVER_SIDE,
};
use celestina::wallpaper::decode::DecodeError;
use celestina_core::image::MAX_ARTWORK_BYTES;
use image::{ImageFormat, Rgba, RgbaImage};

/// Generous on purpose: the bound is "does not hang", not a benchmark.
const SLOW: Duration = Duration::from_secs(5);

/// A unique directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "celestina-media-cover-{}-{}-{nanos}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).expect("create scratch dir");
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, bytes).expect("write fixture");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn url(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn timed(art_url: &str) -> Result<Cover, CoverError> {
    let start = Instant::now();
    let result = load(art_url);
    let took = start.elapsed();
    assert!(took < SLOW, "{art_url} took {took:?}");
    result
}

/// Vertical bands, `bands[i]` over the `i`-th third of the width.
fn bands_wide(w: u32, h: u32, bands: [[u8; 3]; 3]) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, _| {
        let [r, g, b] = bands[(x * 3 / w) as usize];
        Rgba([r, g, b, 255])
    })
}

/// Horizontal bands, `bands[i]` over the `i`-th third of the height.
fn bands_tall(w: u32, h: u32, bands: [[u8; 3]; 3]) -> RgbaImage {
    RgbaImage::from_fn(w, h, |_, y| {
        let [r, g, b] = bands[(y * 3 / h) as usize];
        Rgba([r, g, b, 255])
    })
}

const GREEN: [u8; 3] = [0, 255, 0];
const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];

/// PNG, baseline JPEG (opaque) or lossless WebP, by the `image` encoders.
fn encoded(image: &RgbaImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    if format == ImageFormat::Jpeg {
        image::DynamicImage::ImageRgba8(image.clone())
            .to_rgb8()
            .write_to(&mut bytes, format)
            .expect("encode jpeg");
    } else {
        image.write_to(&mut bytes, format).expect("encode");
    }
    bytes.into_inner()
}

fn pixel(cover: &Cover, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * cover.width + x) * 4) as usize;
    [
        cover.rgba[i],
        cover.rgba[i + 1],
        cover.rgba[i + 2],
        cover.rgba[i + 3],
    ]
}

fn assert_square(cover: &Cover) {
    assert_eq!((cover.width, cover.height), (COVER_SIZE, COVER_SIZE));
    assert_eq!(
        cover.rgba.len(),
        (COVER_SIZE * COVER_SIZE * 4) as usize,
        "RGBA8, row-major"
    );
    assert!(cover.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
}

/// Every pixel away from the crop's edges is red: the green and blue
/// outer bands were cut, not squeezed in.
fn assert_center_band_only(cover: &Cover) {
    for y in 8..COVER_SIZE - 8 {
        for x in 8..COVER_SIZE - 8 {
            let [r, g, b, _] = pixel(cover, x, y);
            assert!(r > 200 && g < 60 && b < 60, "({x}, {y}) = {r},{g},{b}");
        }
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    chunk
}

/// A PNG built by hand: signature, an 8-bit `IHDR` of `color`, one `IDAT`
/// and `IEND`.
fn png_header_only(width: u32, height: u32, color: u8) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = width.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, color, 0, 0, 0]);
    bytes.extend(png_chunk(b"IHDR", &ihdr));
    bytes.extend(png_chunk(b"IDAT", &[0u8; 120]));
    bytes.extend(png_chunk(b"IEND", &[]));
    bytes
}

#[test]
fn a_valid_png_becomes_a_square_rgba_cover() {
    let scratch = Scratch::new();
    let path = scratch.write(
        "cover.png",
        &encoded(&bands_wide(96, 96, [RED; 3]), ImageFormat::Png),
    );
    let cover = timed(&url(&path)).expect("valid png");
    assert_square(&cover);
    assert_center_band_only(&cover);
}

#[test]
fn a_percent_encoded_path_is_decoded() {
    let scratch = Scratch::new();
    let path = scratch.write(
        "my cover.png",
        &encoded(&bands_wide(32, 32, [RED; 3]), ImageFormat::Png),
    );
    let encoded_url = url(&path).replace(' ', "%20");
    assert!(encoded_url.contains("%20"));
    assert_square(&timed(&encoded_url).expect("percent-encoded"));
}

#[test]
fn a_wide_png_is_cropped_to_its_center_square() {
    let scratch = Scratch::new();
    let path = scratch.write(
        "wide.png",
        &encoded(&bands_wide(384, 128, [GREEN, RED, BLUE]), ImageFormat::Png),
    );
    let cover = timed(&url(&path)).expect("wide png");
    assert_square(&cover);
    assert_center_band_only(&cover);
}

#[test]
fn a_tall_jpeg_is_cropped_to_its_center_square() {
    let scratch = Scratch::new();
    let path = scratch.write(
        "tall.jpg",
        &encoded(&bands_tall(200, 600, [GREEN, RED, BLUE]), ImageFormat::Jpeg),
    );
    let cover = timed(&url(&path)).expect("tall jpeg");
    assert_square(&cover);
    assert_center_band_only(&cover);
}

#[test]
fn a_remote_or_relative_url_is_not_local() {
    for art_url in [
        "http://example.com/cover.png",
        "https://example.com/cover.png",
        "data:image/png;base64,iVBORw0KGgo=",
        "cover.png",
        "file://cover.png",
        "file://host/tmp/cover.png",
        "file:///tmp/../etc/cover.png",
        "file:///tmp/a%00.png",
        "file:///tmp/a\0.png",
        "",
    ] {
        let err = timed(art_url).expect_err(art_url);
        assert!(
            matches!(err, CoverError::NotLocal { .. }),
            "{art_url}: {err:?}"
        );
    }
}

#[test]
fn a_not_local_error_bounds_the_url_it_names() {
    let long = format!("http://{}", "a".repeat(10_000));
    let err = timed(&long).expect_err("remote");
    let CoverError::NotLocal { url } = err else {
        panic!("not local expected");
    };
    assert!(url.len() <= celestina_shell::media::MAX_TEXT_BYTES);
}

#[test]
fn a_fifo_named_like_a_png_does_not_block() {
    let scratch = Scratch::new();
    let path = scratch.path("x.png");
    let status = Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("run mkfifo");
    assert!(status.success());
    let err = timed(&url(&path)).expect_err("fifo");
    assert!(
        matches!(
            err,
            CoverError::Unreadable {
                source: DecodeError::NotRegularFile { .. },
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_symlink_to_a_directory_is_not_a_regular_file() {
    let scratch = Scratch::new();
    let dir = scratch.path("albums");
    fs::create_dir(&dir).expect("dir");
    let link = scratch.path("cover.png");
    std::os::unix::fs::symlink(&dir, &link).expect("symlink");
    let err = timed(&url(&link)).expect_err("dir link");
    assert!(
        matches!(
            err,
            CoverError::Unreadable {
                source: DecodeError::NotRegularFile { .. },
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_missing_file_is_unreadable() {
    let scratch = Scratch::new();
    let err = timed(&url(&scratch.path("gone.png"))).expect_err("missing");
    assert!(
        matches!(
            err,
            CoverError::Unreadable {
                source: DecodeError::Open { .. },
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.source().is_some(), "the cause is kept");
}

#[test]
fn a_file_over_the_artwork_cap_is_refused_by_its_size() {
    let scratch = Scratch::new();
    let path = scratch.path("big.png");
    // Sparse, with a real PNG signature: only the size refuses it.
    let mut bytes = encoded(&bands_wide(8, 8, [RED; 3]), ImageFormat::Png);
    bytes.truncate(16);
    fs::write(&path, &bytes).expect("write");
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open");
    file.set_len(9 * 1024 * 1024).expect("grow");
    drop(file);
    const { assert!(9 * 1024 * 1024 > MAX_ARTWORK_BYTES) };
    let err = timed(&url(&path)).expect_err("9 MiB");
    assert!(
        matches!(err, CoverError::TooLarge { bytes, .. } if bytes == 9 * 1024 * 1024),
        "{err:?}"
    );
}

#[test]
fn a_file_whose_header_is_not_an_image_is_refused_before_decoding() {
    let scratch = Scratch::new();
    for (name, bytes) in [
        ("gif.png", b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".as_slice()),
        ("text.png", b"this is not an image at all".as_slice()),
        ("short.png", b"\x89PN".as_slice()),
    ] {
        let path = scratch.write(name, bytes);
        let err = timed(&url(&path)).expect_err(name);
        assert!(
            matches!(err, CoverError::NotAnImage { .. }),
            "{name}: {err:?}"
        );
    }
}

#[test]
fn an_empty_file_is_unreadable() {
    let scratch = Scratch::new();
    let path = scratch.write("zero.png", b"");
    let err = timed(&url(&path)).expect_err("empty");
    assert!(
        matches!(
            err,
            CoverError::Unreadable {
                source: DecodeError::Empty { .. },
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_truncated_png_fails_to_decode() {
    let scratch = Scratch::new();
    let full = encoded(&bands_wide(64, 64, [GREEN, RED, BLUE]), ImageFormat::Png);
    let path = scratch.write("t.png", &full[..full.len() / 2]);
    let err = timed(&url(&path)).expect_err("truncated");
    assert!(
        matches!(
            err,
            CoverError::Decode {
                source: DecodeError::Decode { .. },
                ..
            }
        ),
        "{err:?}"
    );
}

fn assert_oversized(err: &CoverError, size: (u32, u32)) {
    assert!(
        matches!(err, CoverError::Oversized { width, height, .. } if (*width, *height) == size),
        "{err:?}"
    );
}

#[test]
fn decompression_bomb_dimensions_are_refused_by_the_header() {
    let scratch = Scratch::new();
    let path = scratch.write("huge.png", &png_header_only(60_000, 60_000, 6));
    let err = timed(&url(&path)).expect_err("60000 squared");
    assert_oversized(&err, (60_000, 60_000));
    // Within the wallpaper's sides, over the cover's.
    let path = scratch.write("wide.png", &png_header_only(8_192, 4_884, 2));
    let err = timed(&url(&path)).expect_err("over the side");
    assert_oversized(&err, (8_192, 4_884));
}

#[test]
fn the_cover_limits_are_far_below_the_wallpapers() {
    let scratch = Scratch::new();
    // One side past MAX_COVER_SIDE, a tiny area.
    let path = scratch.write("strip.png", &png_header_only(MAX_COVER_SIDE + 1, 1, 2));
    let err = timed(&url(&path)).expect_err("one side over");
    assert_oversized(&err, (MAX_COVER_SIDE + 1, 1));
    // Both sides at the bound: over MAX_COVER_PIXELS in all.
    const { assert!(MAX_COVER_SIDE as u64 * MAX_COVER_SIDE as u64 > MAX_COVER_PIXELS) };
    let path = scratch.write(
        "square.png",
        &png_header_only(MAX_COVER_SIDE, MAX_COVER_SIDE, 2),
    );
    let err = timed(&url(&path)).expect_err("over the area");
    assert_oversized(&err, (MAX_COVER_SIDE, MAX_COVER_SIDE));
    // Within both: the header passes and the (bogus) data fails to decode.
    let path = scratch.write("fine.png", &png_header_only(64, 64, 2));
    let err = timed(&url(&path)).expect_err("bogus data");
    assert!(matches!(err, CoverError::Decode { .. }), "{err:?}");
}

#[test]
fn a_url_longer_than_the_text_bound_is_loaded_whole() {
    let scratch = Scratch::new();
    let mut dir = scratch.0.clone();
    while dir.as_os_str().len() < 300 {
        dir.push("nested-directory-of-covers");
    }
    fs::create_dir_all(&dir).expect("nested dirs");
    let path = dir.join("cover.png");
    fs::write(
        &path,
        encoded(&bands_wide(32, 32, [RED; 3]), ImageFormat::Png),
    )
    .expect("write");
    let art_url = url(&path);
    assert!(art_url.len() > celestina_shell::media::MAX_TEXT_BYTES);
    assert!(art_url.len() <= celestina::media::mpris::MAX_ART_URL_BYTES);
    assert_square(&timed(&art_url).expect("a long url decodes"));
}

#[test]
fn lossless_webp_is_refused() {
    let scratch = Scratch::new();
    let path = scratch.write(
        "l.webp",
        &encoded(&bands_wide(16, 16, [RED; 3]), ImageFormat::WebP),
    );
    let err = timed(&url(&path)).expect_err("vp8l");
    assert!(
        matches!(
            err,
            CoverError::Decode {
                source: DecodeError::Unsupported { .. },
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_cover_without_an_image_extension_is_refused_by_the_shared_decoder() {
    // The wallpaper decoder owns the extension rule; a cover file a player
    // writes without one (Chromium's temporary files) is "no cover".
    let scratch = Scratch::new();
    let path = scratch.write(
        "cover",
        &encoded(&bands_wide(8, 8, [RED; 3]), ImageFormat::Png),
    );
    let err = timed(&url(&path)).expect_err("no extension");
    assert!(
        matches!(
            err,
            CoverError::Decode {
                source: DecodeError::UnsupportedExtension { .. },
                ..
            }
        ),
        "{err:?}"
    );
}
