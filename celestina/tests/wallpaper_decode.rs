//! Hostile and valid wallpaper files through `celestina::wallpaper::decode`.
//! Every fixture is generated in a private temporary directory (the few
//! lossy WebP and progressive JPEG files the `image` encoders cannot write
//! are embedded below as bytes); every hostile case must fail with its typed
//! error within [`SLOW`].

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use celestina::wallpaper::decode::{
    decode, DecodeError, Decoded, MAX_FILE_BYTES, MAX_SOURCE_PIXELS, PNG_METADATA_BYTES,
};
use celestina_shell::wallpaper::Mode;
use image::{ImageBuffer, ImageFormat, Rgb, Rgba, RgbaImage};

/// Generous on purpose: the bound is "does not hang", not a benchmark.
const SLOW: Duration = Duration::from_secs(5);
const MODES: [Mode; 4] = [Mode::Fill, Mode::Fit, Mode::Stretch, Mode::Center];

/// A 64×32 lossy WebP (`VP8 `), left half red and right half blue, written
/// by libwebp through Pillow 12.3 (`lossy=True, quality=90`).
const HALVES_WEBP: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0x68, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x20,
    0x5c, 0x00, 0x00, 0x00, 0x10, 0x05, 0x00, 0x9d, 0x01, 0x2a, 0x40, 0x00, 0x20, 0x00, 0x3e, 0x31,
    0x16, 0x89, 0x43, 0x22, 0x21, 0x21, 0x16, 0x04, 0x00, 0x20, 0x03, 0x04, 0xb1, 0x80, 0x66, 0x84,
    0xe0, 0x7f, 0x00, 0xfc, 0x40, 0xd8, 0x0c, 0xe0, 0x1f, 0x80, 0x1f, 0xb0, 0x1f, 0xd0, 0x00, 0x32,
    0x8c, 0x34, 0x09, 0x62, 0xc0, 0x00, 0xfe, 0xf7, 0xa2, 0x50, 0x15, 0x27, 0x22, 0x3e, 0xe5, 0xdc,
    0xfe, 0xea, 0xab, 0x8d, 0xff, 0xc2, 0x73, 0x47, 0xdc, 0x9a, 0x4b, 0x5a, 0xf4, 0x95, 0x35, 0xff,
    0x85, 0x87, 0xf0, 0x1b, 0xf1, 0xb4, 0xe8, 0x01, 0x0d, 0x7c, 0x84, 0xa0, 0xe0, 0x00, 0x00, 0x00,
];

/// A 16×16 translucent lossy WebP whose `ALPH` plane is losslessly
/// compressed (libwebp's default), through Pillow 12.3.
const LOSSLESS_ALPHA_WEBP: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0x4a, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x58,
    0x0a, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x0f, 0x00, 0x00, 0x0f, 0x00, 0x00, 0x41, 0x4c,
    0x50, 0x48, 0x09, 0x00, 0x00, 0x00, 0x01, 0x07, 0x10, 0x11, 0x11, 0x88, 0x88, 0xfe, 0x07, 0x00,
    0x56, 0x50, 0x38, 0x20, 0x1a, 0x00, 0x00, 0x00, 0x30, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x10, 0x00,
    0x10, 0x00, 0x00, 0xc0, 0x12, 0x25, 0xa4, 0x00, 0x03, 0x70, 0x00, 0xfe, 0xfe, 0xdd, 0x78, 0x00,
    0x00, 0x00,
];

/// A two-frame 8×8 animated lossy WebP (red, then green), through Pillow 12.3.
const ANIMATED_WEBP: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0xca, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x58,
    0x0a, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x07, 0x00, 0x00, 0x41, 0x4e,
    0x49, 0x4d, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x41, 0x4e, 0x4d, 0x46,
    0x4a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x07, 0x00, 0x00,
    0x64, 0x00, 0x00, 0x02, 0x56, 0x50, 0x38, 0x20, 0x32, 0x00, 0x00, 0x00, 0x30, 0x01, 0x00, 0x9d,
    0x01, 0x2a, 0x08, 0x00, 0x08, 0x00, 0x01, 0x40, 0x26, 0x25, 0xa0, 0x00, 0x03, 0x70, 0x00, 0xfe,
    0xf2, 0xeb, 0x7f, 0xff, 0xf9, 0xb0, 0x3f, 0xf6, 0xf3, 0xff, 0x04, 0x7a, 0x01, 0xff, 0xff, 0xd2,
    0xe0, 0xff, 0xfe, 0x97, 0x07, 0xff, 0xf4, 0xb8, 0x3f, 0xf4, 0xa4, 0x00, 0x00, 0x00, 0x41, 0x4e,
    0x4d, 0x46, 0x4c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x07,
    0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0x56, 0x50, 0x38, 0x20, 0x34, 0x00, 0x00, 0x00, 0x34, 0x01,
    0x00, 0x9d, 0x01, 0x2a, 0x08, 0x00, 0x08, 0x00, 0x00, 0x00, 0x26, 0x25, 0xa0, 0x00, 0x03, 0x70,
    0x00, 0xfe, 0xe9, 0x22, 0x1f, 0xff, 0xf7, 0x9f, 0x3f, 0xff, 0xb9, 0xf3, 0xff, 0xfb, 0x9f, 0x3f,
    0xe8, 0xcf, 0xff, 0xff, 0x29, 0xfb, 0xff, 0xf2, 0x38, 0xff, 0xff, 0x23, 0x8f, 0xfe, 0x50, 0x20,
    0x00, 0x00,
];

/// A 64×32 progressive (`SOF2`) 4:2:0 JPEG, left half red and right half
/// blue, written by libjpeg through Pillow 12.3 (`progressive=True`).
const PROGRESSIVE_JPEG: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x03, 0x02, 0x02, 0x03, 0x02, 0x02, 0x03,
    0x03, 0x03, 0x03, 0x04, 0x03, 0x03, 0x04, 0x05, 0x08, 0x05, 0x05, 0x04, 0x04, 0x05, 0x0a, 0x07,
    0x07, 0x06, 0x08, 0x0c, 0x0a, 0x0c, 0x0c, 0x0b, 0x0a, 0x0b, 0x0b, 0x0d, 0x0e, 0x12, 0x10, 0x0d,
    0x0e, 0x11, 0x0e, 0x0b, 0x0b, 0x10, 0x16, 0x10, 0x11, 0x13, 0x14, 0x15, 0x15, 0x15, 0x0c, 0x0f,
    0x17, 0x18, 0x16, 0x14, 0x18, 0x12, 0x14, 0x15, 0x14, 0xff, 0xdb, 0x00, 0x43, 0x01, 0x03, 0x04,
    0x04, 0x05, 0x04, 0x05, 0x09, 0x05, 0x05, 0x09, 0x14, 0x0d, 0x0b, 0x0d, 0x14, 0x14, 0x14, 0x14,
    0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14,
    0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14,
    0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0xff, 0xc2,
    0x00, 0x11, 0x08, 0x00, 0x20, 0x00, 0x40, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11,
    0x01, 0xff, 0xc4, 0x00, 0x16, 0x00, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x07, 0xff, 0xc4, 0x00, 0x16, 0x01, 0x01, 0x01,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08,
    0x06, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x10, 0x03, 0x10, 0x00, 0x00, 0x01, 0xce,
    0x46, 0x16, 0xa9, 0x02, 0x04, 0x5c, 0x71, 0xd0, 0x17, 0xc2, 0x1c, 0xb1, 0x40, 0x81, 0x17, 0x1c,
    0x74, 0x07, 0xff, 0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x50, 0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x01,
    0x05, 0x02, 0x03, 0xff, 0xc4, 0x00, 0x14, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0xff, 0xda, 0x00, 0x08, 0x01, 0x03, 0x01,
    0x01, 0x3f, 0x01, 0x0f, 0xff, 0xc4, 0x00, 0x14, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0xff, 0xda, 0x00, 0x08, 0x01, 0x02,
    0x01, 0x01, 0x3f, 0x01, 0x0f, 0xff, 0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x50, 0xff, 0xda, 0x00, 0x08, 0x01,
    0x01, 0x00, 0x06, 0x3f, 0x02, 0x03, 0xff, 0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x50, 0xff, 0xda, 0x00, 0x08,
    0x01, 0x01, 0x00, 0x01, 0x3f, 0x21, 0x03, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x00,
    0x03, 0x00, 0x00, 0x00, 0x10, 0xff, 0x00, 0xf0, 0x82, 0xff, 0x00, 0xf0, 0x82, 0xff, 0xc4, 0x00,
    0x14, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x30, 0xff, 0xda, 0x00, 0x08, 0x01, 0x03, 0x01, 0x01, 0x3f, 0x10, 0x0f, 0xff, 0xc4,
    0x00, 0x14, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x30, 0xff, 0xda, 0x00, 0x08, 0x01, 0x02, 0x01, 0x01, 0x3f, 0x10, 0x0f, 0xff,
    0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x50, 0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x01, 0x3f, 0x10, 0x03,
    0xff, 0xd9,
];

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
            "celestina-wallpaper-decode-{}-{}-{nanos}",
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

/// Left half red, right half blue.
fn halves(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, _| {
        if x < w / 2 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 255])
        }
    })
}

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

fn timed(path: &Path, target: (u32, u32), mode: Mode) -> Result<Decoded, DecodeError> {
    let start = Instant::now();
    let result = decode(path, target, mode);
    let took = start.elapsed();
    if result.is_err() {
        assert!(took < SLOW, "{} took {took:?}", path.display());
    }
    result
}

fn pixel(decoded: &Decoded, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * decoded.width + x) * 4) as usize;
    [
        decoded.rgba[i],
        decoded.rgba[i + 1],
        decoded.rgba[i + 2],
        decoded.rgba[i + 3],
    ]
}

fn assert_shape(decoded: &Decoded, target: (u32, u32)) {
    assert_eq!((decoded.width, decoded.height), target);
    assert_eq!(decoded.rgba.len(), (target.0 * target.1 * 4) as usize);
    assert!(decoded.rgba.chunks_exact(4).all(|p| p[3] == 255));
}

fn assert_halves(decoded: &Decoded) {
    let (w, h) = (decoded.width, decoded.height);
    let left = pixel(decoded, w / 8, h / 2);
    let right = pixel(decoded, w - 1 - w / 8, h / 2);
    assert!(left[0] > 200 && left[2] < 60, "left {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "right {right:?}");
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

/// A PNG built by hand: signature, `IHDR`, the `extra` chunks, then one
/// `IDAT` and `IEND`.
fn png(width: u32, height: u32, depth: u8, color: u8, extra: &[Vec<u8>], idat: &[u8]) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = width.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[depth, color, 0, 0, 0]);
    bytes.extend(png_chunk(b"IHDR", &ihdr));
    for chunk in extra {
        bytes.extend_from_slice(chunk);
    }
    bytes.extend(png_chunk(b"IDAT", idat));
    bytes.extend(png_chunk(b"IEND", &[]));
    bytes
}

/// Inserts `chunk` right after the `IHDR` of an encoded PNG.
fn with_chunk_after_ihdr(png: &[u8], chunk: &[u8]) -> Vec<u8> {
    let after_ihdr = 8 + 8 + 13 + 4;
    let mut bytes = png[..after_ihdr].to_vec();
    bytes.extend_from_slice(chunk);
    bytes.extend_from_slice(&png[after_ihdr..]);
    bytes
}

/// A zlib stream of `len` zero bytes, by fixed-Huffman back-references:
/// a 120 MB image is under a megabyte of fixture.
fn zlib_zeros(len: usize) -> Vec<u8> {
    struct Bits {
        out: Vec<u8>,
        acc: u32,
        count: u32,
    }
    impl Bits {
        fn push(&mut self, value: u32, count: u32) {
            for i in 0..count {
                self.acc |= ((value >> i) & 1) << self.count;
                self.count += 1;
                if self.count == 8 {
                    self.out.push(self.acc as u8);
                    self.acc = 0;
                    self.count = 0;
                }
            }
        }
        /// Huffman codes are packed most significant bit first.
        fn code(&mut self, code: u32, count: u32) {
            for i in (0..count).rev() {
                self.push((code >> i) & 1, 1);
            }
        }
    }
    let mut bits = Bits {
        out: vec![0x78, 0x01],
        acc: 0,
        count: 0,
    };
    bits.push(1, 1); // final block
    bits.push(1, 2); // fixed Huffman
    let mut left = len;
    if left > 0 {
        bits.code(0x30, 8); // literal 0
        left -= 1;
    }
    while left >= 258 {
        bits.code(0xC5, 8); // length 258 (symbol 285)
        bits.code(0, 5); // distance 1
        left -= 258;
    }
    for _ in 0..left {
        bits.code(0x30, 8);
    }
    bits.code(0, 7); // end of block
    if bits.count > 0 {
        bits.push(0, 8 - bits.count);
    }
    let adler = ((len % 65_521) as u32) << 16 | 1;
    bits.out.extend_from_slice(&adler.to_be_bytes());
    bits.out
}

/// A big-endian TIFF with one Orientation entry.
fn tiff_orientation(orientation: u16) -> Vec<u8> {
    let mut tiff = b"MM\x00\x2a\x00\x00\x00\x08".to_vec();
    tiff.extend_from_slice(&1u16.to_be_bytes()); // one entry
    tiff.extend_from_slice(&0x0112u16.to_be_bytes()); // Orientation
    tiff.extend_from_slice(&3u16.to_be_bytes()); // SHORT
    tiff.extend_from_slice(&1u32.to_be_bytes()); // count
    tiff.extend_from_slice(&orientation.to_be_bytes());
    tiff.extend_from_slice(&[0, 0]); // value padding
    tiff.extend_from_slice(&0u32.to_be_bytes()); // no next IFD
    tiff
}

/// An APP1 Exif segment with one Orientation entry.
fn exif_orientation_segment(orientation: u16) -> Vec<u8> {
    let mut payload = b"Exif\x00\x00".to_vec();
    payload.extend(tiff_orientation(orientation));
    let mut segment = vec![0xFF, 0xE1];
    segment.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    segment.extend(payload);
    segment
}

/// A lossy WebP whose `VP8 ` frame header claims `width × height` and whose
/// partitions are missing.
fn truncated_vp8(width: u16, height: u16) -> Vec<u8> {
    let mut vp8 = vec![0x10, 0x05, 0x00, 0x9d, 0x01, 0x2a];
    vp8.extend_from_slice(&width.to_le_bytes());
    vp8.extend_from_slice(&height.to_le_bytes());
    vp8.extend_from_slice(&[0u8; 16]);
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&((4 + 8 + vp8.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WEBPVP8 ");
    bytes.extend_from_slice(&(vp8.len() as u32).to_le_bytes());
    bytes.extend(vp8);
    bytes
}

#[test]
fn valid_png_jpeg_and_webp_decode_in_every_mode() {
    let scratch = Scratch::new();
    let source = halves(64, 32);
    for (name, bytes) in [
        ("a.png", encoded(&source, ImageFormat::Png)),
        ("a.jpg", encoded(&source, ImageFormat::Jpeg)),
        ("a.jpeg", encoded(&source, ImageFormat::Jpeg)),
        ("a.webp", HALVES_WEBP.to_vec()),
        ("p.jpg", PROGRESSIVE_JPEG.to_vec()),
    ] {
        let path = scratch.write(name, &bytes);
        for mode in MODES {
            for target in [(32, 32), (128, 48), (16, 40)] {
                let decoded = decode(&path, target, mode).expect(name);
                assert_shape(&decoded, target);
                assert_eq!(decoded.source_px, (64, 32));
            }
        }
        assert_halves(&decode(&path, (64, 32), Mode::Stretch).expect(name));
    }
}

#[test]
fn fit_bars_are_black_and_stretch_keeps_the_halves() {
    let scratch = Scratch::new();
    let path = scratch.write("a.png", &encoded(&halves(64, 32), ImageFormat::Png));
    let fitted = decode(&path, (32, 32), Mode::Fit).expect("fit");
    assert_eq!(pixel(&fitted, 16, 0), [0, 0, 0, 255]);
    let stretched = decode(&path, (16, 16), Mode::Stretch).expect("stretch");
    assert!(pixel(&stretched, 1, 8)[0] > 200);
    assert!(pixel(&stretched, 14, 8)[2] > 200);
    let centred = decode(&path, (128, 64), Mode::Center).expect("center");
    assert_eq!(pixel(&centred, 0, 0), [0, 0, 0, 255]);
    assert_eq!(pixel(&centred, 40, 32), [255, 0, 0, 255]);
}

#[test]
fn translucent_pixels_become_opaque_over_black() {
    let scratch = Scratch::new();
    let image = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 0]));
    let path = scratch.write("t.png", &encoded(&image, ImageFormat::Png));
    let decoded = decode(&path, (4, 4), Mode::Center).expect("decode");
    assert_eq!(pixel(&decoded, 1, 1), [0, 0, 0, 255]);
}

#[test]
fn a_png_header_claiming_60000_squared_is_refused_before_its_pixels() {
    let scratch = Scratch::new();
    let mut bytes = png(60_000, 60_000, 8, 6, &[], &[0u8; 120]);
    bytes.resize(bytes.len().max(200), 0);
    let path = scratch.write("huge.png", &bytes);
    let err = timed(&path, (64, 64), Mode::Fill).expect_err("refused");
    assert!(
        matches!(
            err,
            DecodeError::Limits { .. } | DecodeError::Dimensions { .. }
        ),
        "{err:?}"
    );
}

#[test]
fn an_area_over_the_budget_is_refused_by_its_header_in_every_format() {
    let scratch = Scratch::new();
    // 8192 × 4884 is 40 009 728 pixels: within each side, over the area.
    const { assert!(8_192 * 4_884 > MAX_SOURCE_PIXELS) };
    let path = scratch.write("wide.png", &png(8_192, 4_884, 8, 2, &[], &[0u8; 64]));
    let err = timed(&path, (64, 64), Mode::Fill).expect_err("png");
    assert!(matches!(err, DecodeError::TooManyPixels { .. }), "{err:?}");

    let path = scratch.write("wide.webp", &truncated_vp8(8_192, 4_884));
    let err = timed(&path, (64, 64), Mode::Fill).expect_err("webp");
    assert!(matches!(err, DecodeError::TooManyPixels { .. }), "{err:?}");

    // The progressive fixture with its frame size patched to 8192 × 4884.
    let mut jpeg = PROGRESSIVE_JPEG.to_vec();
    let sof = jpeg
        .windows(2)
        .position(|w| w == [0xFF, 0xC2])
        .expect("SOF2");
    jpeg[sof + 5..sof + 7].copy_from_slice(&4_884u16.to_be_bytes());
    jpeg[sof + 7..sof + 9].copy_from_slice(&8_192u16.to_be_bytes());
    let path = scratch.write("wide.jpg", &jpeg);
    let err = timed(&path, (64, 64), Mode::Fill).expect_err("jpeg");
    assert!(matches!(err, DecodeError::TooManyPixels { .. }), "{err:?}");
}

#[test]
fn a_truncated_file_fails_to_decode() {
    let scratch = Scratch::new();
    for (name, full) in [
        ("t.png", encoded(&halves(64, 64), ImageFormat::Png)),
        ("t.jpg", encoded(&halves(64, 64), ImageFormat::Jpeg)),
        ("t.webp", HALVES_WEBP.to_vec()),
        ("p.jpg", PROGRESSIVE_JPEG.to_vec()),
    ] {
        let path = scratch.write(name, &full[..full.len() / 2]);
        let err = timed(&path, (32, 32), Mode::Fill).expect_err(name);
        assert!(matches!(err, DecodeError::Decode { .. }), "{name}: {err:?}");
    }
}

#[test]
fn a_wrong_extension_is_refused_without_reading() {
    let scratch = Scratch::new();
    let png = encoded(&halves(8, 8), ImageFormat::Png);
    for name in ["a.gif", "a.txt", "a.avif", "noextension"] {
        let path = scratch.write(name, &png);
        let err = timed(&path, (8, 8), Mode::Fill).expect_err(name);
        assert!(
            matches!(err, DecodeError::UnsupportedExtension { .. }),
            "{name}: {err:?}"
        );
    }
}

#[test]
fn content_outside_the_allowed_formats_is_refused() {
    let scratch = Scratch::new();
    let path = scratch.write("fake.png", b"GIF89a\x01\x00\x01\x00\x00\x00\x00;");
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("gif");
    assert!(
        matches!(err, DecodeError::UnsupportedFormat { .. }),
        "{err:?}"
    );
    let path = scratch.write("noise.webp", b"this is not an image at all");
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("noise");
    assert!(
        matches!(err, DecodeError::UnsupportedFormat { format: None, .. }),
        "{err:?}"
    );
}

#[test]
fn a_zero_byte_file_is_empty() {
    let scratch = Scratch::new();
    let path = scratch.write("zero.png", b"");
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("empty");
    assert!(matches!(err, DecodeError::Empty { .. }), "{err:?}");
}

#[test]
fn a_fifo_does_not_block_and_is_not_a_regular_file() {
    let scratch = Scratch::new();
    let path = scratch.path("pipe.png");
    let status = Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("run mkfifo");
    assert!(status.success());
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("fifo");
    assert!(matches!(err, DecodeError::NotRegularFile { .. }), "{err:?}");
}

#[test]
fn a_device_is_refused_before_it_is_opened() {
    // `stat` sees a character device through the link; nothing opens it.
    let scratch = Scratch::new();
    let link = scratch.path("null.png");
    std::os::unix::fs::symlink("/dev/null", &link).expect("symlink");
    let err = timed(&link, (8, 8), Mode::Fill).expect_err("device");
    assert!(matches!(err, DecodeError::NotRegularFile { .. }), "{err:?}");
}

#[test]
fn a_directory_named_like_an_image_is_not_a_regular_file() {
    let scratch = Scratch::new();
    let path = scratch.path("x.png");
    fs::create_dir(&path).expect("mkdir");
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("dir");
    assert!(matches!(err, DecodeError::NotRegularFile { .. }), "{err:?}");
}

#[test]
fn a_symlink_to_a_regular_file_elsewhere_is_followed_once() {
    let outside = Scratch::new();
    let folder = Scratch::new();
    let target = outside.write("real.png", &encoded(&halves(16, 8), ImageFormat::Png));
    let link = folder.path("link.png");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");
    let decoded = decode(&link, (16, 8), Mode::Center).expect("followed");
    assert_shape(&decoded, (16, 8));
    assert_eq!(decoded.source_px, (16, 8));

    let dangling = folder.path("dangling.png");
    std::os::unix::fs::symlink(outside.path("missing.png"), &dangling).expect("symlink");
    let err = timed(&dangling, (8, 8), Mode::Fill).expect_err("dangling");
    assert!(matches!(err, DecodeError::Open { .. }), "{err:?}");
}

#[test]
fn a_sparse_file_over_the_cap_is_refused_by_its_size() {
    let scratch = Scratch::new();
    let path = scratch.path("big.png");
    let file = fs::File::create(&path).expect("create");
    file.set_len(MAX_FILE_BYTES + 1024 * 1024).expect("sparse");
    drop(file);
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("big");
    assert!(
        matches!(err, DecodeError::TooLarge { bytes, .. } if bytes > MAX_FILE_BYTES),
        "{err:?}"
    );
}

#[test]
fn a_missing_file_fails_to_open() {
    let scratch = Scratch::new();
    let err = timed(&scratch.path("gone.png"), (8, 8), Mode::Fill).expect_err("missing");
    assert!(matches!(err, DecodeError::Open { .. }), "{err:?}");
}

#[test]
fn an_invalid_target_is_refused() {
    let scratch = Scratch::new();
    let path = scratch.write("a.png", &encoded(&halves(8, 8), ImageFormat::Png));
    for target in [(0, 8), (8, 0), (8193, 8), (8, 100_000), (7_680, 4_321)] {
        let err = timed(&path, target, Mode::Fill).expect_err("target");
        assert!(matches!(err, DecodeError::InvalidTarget { .. }), "{err:?}");
    }
}

#[test]
fn an_exif_rotated_jpeg_is_turned_upright() {
    let scratch = Scratch::new();
    let plain = encoded(&halves(40, 20), ImageFormat::Jpeg);
    assert_eq!(&plain[..2], &[0xFF, 0xD8]);
    let mut rotated = plain[..2].to_vec();
    rotated.extend(exif_orientation_segment(6)); // rotate 90° clockwise
    rotated.extend_from_slice(&plain[2..]);
    let path = scratch.write("rotated.jpg", &rotated);
    let decoded = decode(&path, (20, 40), Mode::Center).expect("decode");
    assert_eq!(decoded.source_px, (20, 40));
    // Rotating clockwise brings the left (red) half to the top.
    let top = pixel(&decoded, 10, 5);
    let bottom = pixel(&decoded, 10, 35);
    assert!(top[0] > 200 && top[2] < 60, "top {top:?}");
    assert!(bottom[2] > 200 && bottom[0] < 60, "bottom {bottom:?}");
}

#[test]
fn an_exif_tagged_translucent_png_is_flattened_and_left_as_stored() {
    // `image` 0.25.6 does not read a PNG's `eXIf` chunk, so the orientation
    // stays as stored; the alpha is still composited over black.
    let scratch = Scratch::new();
    let image = RgbaImage::from_fn(40, 20, |x, _| {
        if x < 20 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([255, 255, 255, 128])
        }
    });
    let bytes = with_chunk_after_ihdr(
        &encoded(&image, ImageFormat::Png),
        &png_chunk(b"eXIf", &tiff_orientation(6)),
    );
    let path = scratch.write("rotated.png", &bytes);
    let decoded = decode(&path, (40, 20), Mode::Center).expect("decode");
    assert_eq!(decoded.source_px, (40, 20));
    assert_eq!(pixel(&decoded, 5, 10), [255, 0, 0, 255]);
    assert_eq!(pixel(&decoded, 35, 10), [128, 128, 128, 255]);
}

#[test]
fn a_progressive_jpeg_decodes() {
    let scratch = Scratch::new();
    let path = scratch.write("p.jpg", PROGRESSIVE_JPEG);
    let decoded = decode(&path, (64, 32), Mode::Stretch).expect("progressive");
    assert_eq!(decoded.source_px, (64, 32));
    assert_halves(&decoded);
}

#[test]
fn an_animated_webp_or_png_is_refused() {
    let scratch = Scratch::new();
    let path = scratch.write("anim.webp", ANIMATED_WEBP);
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("webp");
    assert!(matches!(err, DecodeError::MultiFrame { .. }), "{err:?}");

    // acTL: two frames, played forever.
    let mut actl = 2u32.to_be_bytes().to_vec();
    actl.extend_from_slice(&0u32.to_be_bytes());
    let bytes = with_chunk_after_ihdr(
        &encoded(&halves(8, 8), ImageFormat::Png),
        &png_chunk(b"acTL", &actl),
    );
    let path = scratch.write("anim.png", &bytes);
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("apng");
    assert!(matches!(err, DecodeError::MultiFrame { .. }), "{err:?}");
}

#[test]
fn lossless_webp_is_refused_before_its_decoder_runs() {
    let scratch = Scratch::new();
    let path = scratch.write("l.webp", &encoded(&halves(16, 16), ImageFormat::WebP));
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("vp8l");
    assert!(matches!(err, DecodeError::Unsupported { .. }), "{err:?}");
    let path = scratch.write("alpha.webp", LOSSLESS_ALPHA_WEBP);
    let err = timed(&path, (8, 8), Mode::Fill).expect_err("alph");
    assert!(matches!(err, DecodeError::Unsupported { .. }), "{err:?}");
}

#[test]
fn a_small_sixteen_bit_png_decodes_to_eight_bits() {
    let scratch = Scratch::new();
    let image: ImageBuffer<Rgb<u16>, Vec<u16>> = ImageBuffer::from_fn(2, 1, |x, _| {
        if x == 0 {
            Rgb([65_535, 0, 0])
        } else {
            Rgb([0, 0, 65_535])
        }
    });
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Png)
        .expect("encode");
    let path = scratch.write("deep.png", &bytes.into_inner());
    let decoded = decode(&path, (2, 1), Mode::Center).expect("decode");
    assert_eq!(pixel(&decoded, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&decoded, 1, 0), [0, 0, 255, 255]);
}

#[test]
fn a_large_sixteen_bit_png_is_over_the_memory_budget_not_the_dimensions() {
    // 5000 × 5000 is within every side and the area, but 16-bit RGBA is
    // 200 MB decoded.
    let scratch = Scratch::new();
    let path = scratch.write("deep.png", &png(5_000, 5_000, 16, 6, &[], &[0u8; 64]));
    let err = timed(&path, (64, 64), Mode::Fill).expect_err("deep");
    assert!(matches!(err, DecodeError::OverBudget { .. }), "{err:?}");
}

#[test]
fn an_iccp_bomb_png_is_cut_at_its_metadata_limit() {
    // 32 MiB of profile from about 130 KB of chunk. The PNG decoder inflates
    // it only up to `PNG_METADATA_BYTES`, then drops the profile (an invalid
    // `iCCP` is ignored, as libpng does) and decodes the pixels.
    let scratch = Scratch::new();
    let bomb_len = 2 * PNG_METADATA_BYTES as usize;
    let mut iccp = b"bomb\x00\x00".to_vec();
    iccp.extend(zlib_zeros(bomb_len));
    let bytes = with_chunk_after_ihdr(
        &encoded(&halves(8, 8), ImageFormat::Png),
        &png_chunk(b"iCCP", &iccp),
    );
    let path = scratch.write("bomb.png", &bytes);
    let start = Instant::now();
    let decoded = decode(&path, (8, 8), Mode::Stretch).expect("bomb");
    assert!(start.elapsed() < SLOW);
    assert_halves(&decoded);
}

#[test]
fn a_largest_accepted_png_decodes() {
    // 8192 × 4882 (39.99 MP) 8-bit RGB, all black: the decoded image is at
    // the area budget and the file is under a megabyte.
    let (w, h) = (8_192u32, 4_882u32);
    assert!(u64::from(w) * u64::from(h) <= MAX_SOURCE_PIXELS);
    let raw = h as usize * (1 + w as usize * 3);
    let scratch = Scratch::new();
    let path = scratch.write("max.png", &png(w, h, 8, 2, &[], &zlib_zeros(raw)));
    let decoded = decode(&path, (16, 16), Mode::Center).expect("max png");
    assert_eq!(decoded.source_px, (w, h));
    assert!(decoded.rgba.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
}

#[test]
#[ignore = "encodes and decodes a 40 MP JPEG; several seconds in a debug build"]
fn a_largest_accepted_jpeg_decodes() {
    let (w, h) = (8_192u32, 4_882u32);
    let scratch = Scratch::new();
    let image = image::RgbImage::from_pixel(w, h, Rgb([40, 80, 120]));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageFormat::Jpeg)
        .expect("encode");
    let path = scratch.write("max.jpg", &bytes.into_inner());
    let decoded = decode(&path, (16, 16), Mode::Fill).expect("max jpeg");
    assert_eq!(decoded.source_px, (w, h));
}

#[test]
fn a_largest_webp_header_passes_every_bound_before_the_decoder() {
    // No encoder here writes a 40 MP lossy WebP; a frame header claiming
    // 8192 × 4882 must pass the dimension, area and memory checks and fail
    // only in the decoder, on its missing partitions.
    let scratch = Scratch::new();
    let path = scratch.write("max.webp", &truncated_vp8(8_192, 4_882));
    let err = timed(&path, (16, 16), Mode::Fill).expect_err("truncated");
    assert!(matches!(err, DecodeError::Decode { .. }), "{err:?}");
}
