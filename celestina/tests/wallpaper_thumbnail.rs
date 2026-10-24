//! The wallpaper's blurred thumbnail for the session lock (CEL-2-H-b): its
//! bounds, its blur against a naive reference, and the store the lock's
//! bundle is made from.

use celestina::lock::bundle::sealed;
use celestina::wallpaper::decode::Decoded;
use celestina::wallpaper::thumbnail::{
    box_blur, derive, Thumbnail, ThumbnailStore, BLUR_PASSES, BLUR_RADIUS, MAX_BYTES, MAX_SIDE,
};
use celestina_lock_proto::{Bundle, MAX_FILE_BYTES, MAX_OUTPUTS};
use rustix::fs::fstat;

fn picture(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> Decoded {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let [r, g, b] = pixel(x, y);
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Decoded {
        rgba,
        width,
        height,
        source_px: (width, height),
    }
}

fn gradient(x: u32, y: u32) -> [u8; 3] {
    [(x % 251) as u8, (y % 241) as u8, ((x ^ y) % 239) as u8]
}

#[test]
fn thumbnail_is_at_most_640_px_a_side_and_1_6_mib() {
    for (w, h) in [
        (3840, 2160),
        (2160, 3840),
        (641, 641),
        (5120, 1440),
        (640, 640),
    ] {
        let thumb = derive(&picture(w, h, gradient)).expect("a thumbnail");
        let (tw, th) = (u32::from(thumb.width()), u32::from(thumb.height()));
        assert!(tw <= MAX_SIDE && th <= MAX_SIDE, "{w}x{h} -> {tw}x{th}");
        assert_eq!(tw.max(th), MAX_SIDE, "the longest side is 640");
        assert_eq!(thumb.rgba().len(), tw as usize * th as usize * 4);
        assert!(thumb.rgba().len() <= MAX_BYTES);
        // The aspect survives the rounding to within one pixel.
        let expected = f64::from(h) * f64::from(tw) / f64::from(w);
        assert!(
            (expected - f64::from(th)).abs() <= 1.0,
            "{w}x{h} -> {tw}x{th}"
        );
    }
}

/// The definition, written the slow obvious way: per pass a horizontal then a
/// vertical mean over `2r+1` pixels, edges extended, rounded to nearest.
fn reference_blur(pixels: &[u8], (w, h): (usize, usize), radius: usize, passes: usize) -> Vec<u8> {
    let mut current = pixels.to_vec();
    let r = radius as i64;
    for _ in 0..passes {
        for horizontal in [true, false] {
            let mut next = current.clone();
            for y in 0..h {
                for x in 0..w {
                    for c in 0..4 {
                        let mut sum = 0i64;
                        for d in -r..=r {
                            let (sx, sy) = if horizontal {
                                ((x as i64 + d).clamp(0, w as i64 - 1) as usize, y)
                            } else {
                                (x, (y as i64 + d).clamp(0, h as i64 - 1) as usize)
                            };
                            sum += i64::from(current[(sy * w + sx) * 4 + c]);
                        }
                        let window = 2 * r + 1;
                        next[(y * w + x) * 4 + c] = ((sum + window / 2) / window) as u8;
                    }
                }
            }
            current = next;
        }
    }
    current
}

#[test]
fn three_pass_box_blur_matches_a_reference_on_a_fixture() {
    for (w, h, radius) in [(23, 17, 3), (9, 5, 6), (1, 12, 2), (30, 1, 4)] {
        let fixture = picture(w, h, |x, y| {
            [
                ((x * 37 + y * 11) % 256) as u8,
                if (x + y) % 5 == 0 { 255 } else { 0 },
                ((x * y) % 256) as u8,
            ]
        });
        let mut fast = fixture.rgba.clone();
        box_blur(&mut fast, (w as usize, h as usize), radius, 3);
        let slow = reference_blur(&fixture.rgba, (w as usize, h as usize), radius, 3);
        assert_eq!(fast, slow, "{w}x{h} radius {radius}");
    }
    // The shipped thumbnail is exactly that blur, with the shipped
    // parameters, applied to the downscaled picture (here already small).
    let source = picture(50, 40, gradient);
    let mut expected = source.rgba.clone();
    box_blur(&mut expected, (50, 40), BLUR_RADIUS, BLUR_PASSES);
    assert_eq!(derive(&source).expect("a thumbnail").rgba(), expected);
}

#[test]
fn the_blur_hides_a_sharp_edge() {
    let source = picture(
        64,
        64,
        |x, _| if x < 32 { [0, 0, 0] } else { [255, 255, 255] },
    );
    let thumb = derive(&source).expect("a thumbnail");
    let row = &thumb.rgba()[..64 * 4];
    let near_edge = [row[31 * 4], row[32 * 4]];
    assert!(near_edge[0] > 0 && near_edge[1] < 255, "{near_edge:?}");
}

fn maximal(shade: u8) -> Thumbnail {
    derive(&picture(1280, 1280, |_, _| [shade, shade, shade])).expect("a thumbnail")
}

#[test]
fn eight_maximal_outputs_fit_16_mib() {
    let store = ThumbnailStore::default();
    for index in 0..MAX_OUTPUTS + 3 {
        store.set(
            &format!("{index:0>64}"),
            maximal(u8::try_from(index).unwrap_or(0)),
        );
    }
    let blocks = store.blocks();
    assert_eq!(blocks.len(), MAX_OUTPUTS);
    let bundle = Bundle {
        prior_failures: 0,
        offset_known: false,
        utc_offset_s: 0,
        outputs: blocks,
    };
    let fd = sealed(&bundle).expect("it fits");
    let size = u64::try_from(fstat(&fd).expect("fstat").st_size).expect("size");
    assert!(size <= MAX_FILE_BYTES, "{size}");
    assert_eq!(
        size,
        13 + 8 * (1 + 64 + 4 + 640 * 640 * 4) as u64,
        "the format's size, no padding"
    );
}
