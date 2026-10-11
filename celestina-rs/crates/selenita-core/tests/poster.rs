use selenita_core::poster::{clock_text, mp4_duration, png_size, poster_argv};
use std::ffi::OsString;
use std::io::Cursor;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::time::Duration;

/// The whole argv, one token per word, each path a single token with its
/// spaces kept: `gst-launch-1.0` escapes a space inside one argument itself.
#[test]
fn the_poster_argv_takes_one_frame_into_a_png() {
    let argv = poster_argv(Path::new("/tmp/a b.mp4"), Path::new("/run/x.png"));
    let expected: Vec<OsString> = [
        "gst-launch-1.0",
        "-q",
        "filesrc",
        "location=/tmp/a b.mp4",
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
        "location=/run/x.png",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    assert_eq!(argv, expected);
}

/// A path that is not UTF-8 crosses byte for byte.
#[test]
fn the_poster_argv_keeps_the_paths_bytes() {
    let video = OsString::from_vec(b"/tmp/clip-\xf3.mp4".to_vec());
    let argv = poster_argv(Path::new(&video), Path::new("/run/x.png"));
    assert_eq!(
        argv[3],
        OsString::from_vec(b"location=/tmp/clip-\xf3.mp4".to_vec())
    );
}

/// The smallest valid PNG, one pixel: the fake's every picture.
const PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89,
];

#[test]
fn a_pngs_size_is_read_from_its_header() {
    assert_eq!(png_size(PIXEL_PNG), Some((1, 1)));
    let mut wide = PIXEL_PNG.to_vec();
    wide[16..20].copy_from_slice(&1920u32.to_be_bytes());
    wide[20..24].copy_from_slice(&1080u32.to_be_bytes());
    assert_eq!(png_size(&wide), Some((1920, 1080)));
    assert_eq!(png_size(&wide[..20]), None, "a cut header");
    assert_eq!(png_size(b"fake mp4 finished"), None, "not a PNG");
    let mut empty = wide.clone();
    empty[16..20].copy_from_slice(&0u32.to_be_bytes());
    assert_eq!(png_size(&empty), None, "no width");
}

/// One ISO box: its size, its four-letter type and its body.
fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = u32::try_from(body.len() + 8)
        .expect("small")
        .to_be_bytes()
        .to_vec();
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(body);
    bytes
}

/// A movie header of version 0: timescale and duration in 32 bits.
fn mvhd_v0(timescale: u32, duration: u32) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0];
    body.extend_from_slice(&[0; 8]);
    body.extend_from_slice(&timescale.to_be_bytes());
    body.extend_from_slice(&duration.to_be_bytes());
    body.extend_from_slice(&[0; 80]);
    mp4_box(b"mvhd", &body)
}

/// mp4mux writes `ftyp`, then `mdat`, then `moov` at the end; the movie
/// header inside `moov` holds the length.
#[test]
fn an_mp4s_length_is_its_movie_headers() {
    let mut file = mp4_box(b"ftyp", b"mp42\0\0\0\0mp42isom");
    file.extend(mp4_box(b"mdat", &[7; 300]));
    let mut moov = mvhd_v0(1000, 7400);
    moov.extend(mp4_box(b"trak", &[0; 40]));
    file.extend(mp4_box(b"moov", &moov));
    assert_eq!(
        mp4_duration(Cursor::new(&file)),
        Some(Duration::from_millis(7400))
    );
}

#[test]
fn a_version_one_movie_header_has_a_64_bit_length() {
    let mut body = vec![1, 0, 0, 0];
    body.extend_from_slice(&[0; 16]);
    body.extend_from_slice(&90_000u32.to_be_bytes());
    body.extend_from_slice(&(90_000u64 * 125).to_be_bytes());
    body.extend_from_slice(&[0; 80]);
    let file = mp4_box(b"moov", &mp4_box(b"mvhd", &body));
    assert_eq!(
        mp4_duration(Cursor::new(&file)),
        Some(Duration::from_secs(125))
    );
}

#[test]
fn a_file_without_a_readable_movie_header_has_no_length() {
    assert_eq!(mp4_duration(Cursor::new(b"fake mp4 finished")), None);
    assert_eq!(mp4_duration(Cursor::new(Vec::<u8>::new())), None);
    // A box that claims less than its own header.
    let mut broken = mp4_box(b"ftyp", b"");
    broken[3] = 4;
    assert_eq!(mp4_duration(Cursor::new(&broken)), None);
    // A movie header cut short, and one with no timescale.
    let cut = mp4_box(b"moov", &mvhd_v0(1000, 7400)[..20]);
    assert_eq!(mp4_duration(Cursor::new(&cut)), None);
    let timeless = mp4_box(b"moov", &mvhd_v0(0, 7400));
    assert_eq!(mp4_duration(Cursor::new(&timeless)), None);
    // A `moov` without a movie header.
    let empty = mp4_box(b"moov", &mp4_box(b"trak", &[0; 8]));
    assert_eq!(mp4_duration(Cursor::new(&empty)), None);
}

/// As the recording card's clock reads: `m:ss`, the hours only once there
/// are some, whole seconds.
#[test]
fn a_length_reads_as_the_recording_clock_does() {
    assert_eq!(clock_text(Duration::ZERO), "0:00");
    assert_eq!(clock_text(Duration::from_millis(7900)), "0:07");
    assert_eq!(clock_text(Duration::from_secs(65)), "1:05");
    assert_eq!(
        clock_text(Duration::from_secs(3600 + 2 * 60 + 3)),
        "1:02:03"
    );
}

/// A box whose size is 0 runs to the end of its parent: mp4mux may leave
/// the last box that way, and `moov` is often the last one.
#[test]
fn a_size_zero_box_runs_to_its_parents_end() {
    let mut file = mp4_box(b"ftyp", b"mp42");
    let mut moov = mp4_box(b"moov", &mvhd_v0(600, 1500));
    moov[..4].copy_from_slice(&0u32.to_be_bytes());
    file.extend(moov);
    assert_eq!(
        mp4_duration(Cursor::new(&file)),
        Some(Duration::from_millis(2500))
    );
    // Inside `moov` too: a size-0 `mvhd` ends where `moov` ends.
    let mut mvhd = mvhd_v0(1000, 4000);
    mvhd[..4].copy_from_slice(&0u32.to_be_bytes());
    let mut inner = mp4_box(b"trak", &[0; 8]);
    inner.extend(mvhd);
    let file = mp4_box(b"moov", &inner);
    assert_eq!(
        mp4_duration(Cursor::new(&file)),
        Some(Duration::from_secs(4))
    );
}

/// A size of 1 means a 64-bit size follows the type (a large `mdat`).
#[test]
fn a_64_bit_size_box_is_stepped_over() {
    let body = [9u8; 40];
    let mut large = 1u32.to_be_bytes().to_vec();
    large.extend_from_slice(b"mdat");
    large.extend_from_slice(&(16u64 + 40).to_be_bytes());
    large.extend_from_slice(&body);
    let mut file = mp4_box(b"ftyp", b"mp42");
    file.extend(large);
    file.extend(mp4_box(b"moov", &mvhd_v0(1000, 1250)));
    assert_eq!(
        mp4_duration(Cursor::new(&file)),
        Some(Duration::from_millis(1250))
    );
    // A 64-bit size smaller than its own 16-byte header is damage.
    let mut broken = 1u32.to_be_bytes().to_vec();
    broken.extend_from_slice(b"mdat");
    broken.extend_from_slice(&8u64.to_be_bytes());
    broken.extend(mp4_box(b"moov", &mvhd_v0(1000, 1250)));
    assert_eq!(mp4_duration(Cursor::new(&broken)), None);
}
