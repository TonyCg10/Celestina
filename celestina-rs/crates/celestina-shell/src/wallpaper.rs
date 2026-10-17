// Copied from `celestina-rs/crates/celestina-shell-core/src/wallpaper.rs` at
// b992b14afb93d4e0c39d7f5e7d87b86385bb3a26, adapted: formats are
// png|jpg|jpeg|webp (avif dropped), the import-name helper is gone, selection
// takes the output's explicit choice first, and the display `Mode`, the pure
// `placement` math and `next` are new.

//! Which image belongs on which screen, and what to do when there is none.
//!
//! A wallpaper is the one thing on screen that nobody asks for and everybody
//! sees, so the failure that matters is not "no picture" — it is **the wrong
//! picture shown confidently**. Two ways that happens, and both are decided
//! here rather than in a surface:
//!
//! - an output falling back to another output's image, which reads as a
//!   deliberate choice and is not one;
//! - an unreadable or missing file becoming a black rectangle, which is
//!   indistinguishable from a very dark photograph.
//!
//! So selection is per output and by name, and the absence of an image is a
//! *state* the surface paints deliberately, never an image-shaped hole.
//!
//! Nothing here reads a directory. It is given the names that are there and
//! decides what they mean, so the rules are testable without a filesystem.

use crate::bounded::bounded;
use serde::{Deserialize, Serialize};

/// The longest file name this shell will carry. A wallpaper directory is the
/// author's own, but a name is still text from outside the program.
pub const MAX_NAME_CHARS: usize = 255;

/// What this shell knows how to show: the formats the decoder is built with,
/// so a wallpaper never depends on an optional codec the session may lack.
const SHOWABLE: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

fn extension_of(name: &str) -> Option<&'static str> {
    let (_, extension) = name.rsplit_once('.')?;
    SHOWABLE
        .into_iter()
        .find(|showable| extension.eq_ignore_ascii_case(showable))
}

/// Whether a file name is one this shell can show.
///
/// Extension only: reading the file to find out would mean opening every entry
/// in a directory on every start, and a mislabelled file fails at decode time
/// anyway, where the surface already has an honest fallback.
#[must_use]
pub fn is_showable(name: &str) -> bool {
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || name.starts_with('.') {
        return false;
    }
    name.rsplit_once('.')
        .is_some_and(|(stem, _)| !stem.is_empty() && extension_of(name).is_some())
}

/// Whether a showable image belongs exactly to `output` (`DP-1.png` is for
/// `DP-1`, never for `DP-10` or a session-wide `default`).
#[must_use]
pub fn belongs_to_output(output: &str, name: &str) -> bool {
    is_showable(name) && stem_of(name) == output
}

/// What an output should be showing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    /// This exact file, chosen for this output.
    Image(String),
    /// Nothing to show. The surface paints its own background and says so to
    /// assistive technology, rather than pretending a black rectangle is a
    /// photograph.
    Fallback,
}

/// Picks the image for one output.
///
/// Precedence: the `explicit` entry the author set for this output (only when
/// it is present in `available` and showable), then `<output>.<ext>`, then
/// `default.<ext>`, then [`Choice::Fallback`]. Never "the first image in the
/// directory": an output showing a picture chosen for another screen is a lie
/// about which file it is. Connector names are matched case-sensitively.
///
/// `available` is the file names present, in any order; the choice does not
/// depend on that order, so two outputs never race to a different answer.
#[must_use]
pub fn choose(output: &str, explicit: Option<&str>, available: &[String]) -> Choice {
    let chosen = explicit
        .filter(|wanted| is_showable(wanted) && available.iter().any(|name| name == wanted));
    if let Some(wanted) = chosen {
        return Choice::Image(bounded(wanted, MAX_NAME_CHARS));
    }

    let smallest = |matches: &dyn Fn(&str) -> bool| {
        available
            .iter()
            .filter(|name| is_showable(name) && matches(name))
            .min()
    };

    if let Some(named) = smallest(&|name| belongs_to_output(output, name)) {
        return Choice::Image(bounded(named, MAX_NAME_CHARS));
    }
    if let Some(shared) = smallest(&|name| stem_of(name) == "default") {
        return Choice::Image(bounded(shared, MAX_NAME_CHARS));
    }
    Choice::Fallback
}

fn stem_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// The showable names of a folder in the one order every verb agrees on.
fn ordered(names: &[String]) -> Vec<&str> {
    let mut showable: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| is_showable(name))
        .collect();
    showable.sort_unstable();
    showable.dedup();
    showable
}

/// The next (`step > 0`) or previous (`step < 0`) showable image after
/// `current`, wrapping at both ends; `None` when nothing is showable.
///
/// The folder is sorted by name first, so the answer does not depend on the
/// order the directory was read in. A `current` that is absent (or `None`)
/// starts from the first image going forward and from the last going back. A
/// zero `step` keeps the current image when it is still there.
#[must_use]
pub fn next(names: &[String], current: Option<&str>, step: i32) -> Option<String> {
    let showable = ordered(names);
    let count = i64::try_from(showable.len())
        .ok()
        .filter(|count| *count > 0)?;
    let position = current.and_then(|current| showable.iter().position(|name| *name == current));
    let target = match position {
        Some(position) => {
            let position = i64::try_from(position).ok()?;
            (position + i64::from(step)).rem_euclid(count)
        }
        None if step < 0 => count - 1,
        None => 0,
    };
    let index = usize::try_from(target).ok()?;
    showable
        .get(index)
        .map(|name| bounded(name, MAX_NAME_CHARS))
}

/// How an image is laid onto an output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Cover the output, keeping the aspect ratio and cropping the overflow.
    #[default]
    Fill,
    /// Contain the whole image, keeping the aspect ratio; the rest is bars.
    Fit,
    /// Fill the output exactly, ignoring the aspect ratio.
    Stretch,
    /// Show at 1:1, centred, clipped by the output.
    Center,
}

/// Reads a mode from a settings file without ever failing the file: a name this
/// shell does not know, or a value of another type, is the default ([`Mode::Fill`]).
///
/// # Errors
///
/// Only the deserializer's own error while reading the raw value.
pub fn lenient_mode<'de, D>(deserializer: D) -> Result<Mode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(raw).unwrap_or_default())
}

/// A rectangle in device pixels. Local on purpose: this crate does not depend
/// on any windowing library. `x`/`y` may be negative (the image overflows the
/// output and is clipped by it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Where the image lands on an output of `target_px`, in device pixels.
///
/// For non-zero inputs the result is finite and has a positive size. A zero
/// image or target dimension is degenerate (nothing to scale from or onto): it
/// yields the full target rectangle rather than NaN or infinity.
#[must_use]
pub fn placement(mode: Mode, image_px: (u32, u32), target_px: (u32, u32)) -> Rect {
    let (iw, ih) = (image_px.0 as f32, image_px.1 as f32);
    let (tw, th) = (target_px.0 as f32, target_px.1 as f32);
    let whole = Rect {
        x: 0.0,
        y: 0.0,
        w: tw,
        h: th,
    };
    if image_px.0 == 0 || image_px.1 == 0 || target_px.0 == 0 || target_px.1 == 0 {
        return whole;
    }
    let centred = |w: f32, h: f32| Rect {
        x: (tw - w) / 2.0,
        y: (th - h) / 2.0,
        w,
        h,
    };
    let (scale_x, scale_y) = (tw / iw, th / ih);
    match mode {
        Mode::Stretch => whole,
        Mode::Center => centred(iw, ih),
        Mode::Fill => {
            let scale = scale_x.max(scale_y);
            centred(iw * scale, ih * scale)
        }
        Mode::Fit => {
            let scale = scale_x.min(scale_y);
            centred(iw * scale, ih * scale)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|entry| (*entry).to_owned()).collect()
    }

    fn pick(output: &str, available: &[&str]) -> Choice {
        choose(output, None, &names(available))
    }

    #[test]
    fn only_formats_every_session_can_decode_are_showable() {
        assert!(is_showable("DP-1.png"));
        assert!(is_showable("photo.JPEG"));
        assert!(is_showable("a.webp"));
        assert!(is_showable("a.jpg"));
        assert!(!is_showable("photo.avif"));
        assert!(!is_showable("photo.svg"));
        assert!(!is_showable("photo.heic"));
        assert!(!is_showable("photo.mp4"));
        assert!(!is_showable("notes.txt"));
        assert!(!is_showable("README"));
        assert!(!is_showable(".hidden.png"));
        assert!(!is_showable(".png"));
        assert!(!is_showable(""));
        assert!(!is_showable(&format!("{}.png", "x".repeat(MAX_NAME_CHARS))));
    }

    #[test]
    fn an_output_gets_the_image_named_for_it() {
        let available = ["DP-1.png", "DP-2.jpg", "default.png"];
        assert_eq!(
            pick("DP-1", &available),
            Choice::Image("DP-1.png".to_owned())
        );
        assert_eq!(
            pick("DP-2", &available),
            Choice::Image("DP-2.jpg".to_owned())
        );
    }

    #[test]
    fn an_output_without_its_own_image_falls_back_to_the_shared_one() {
        assert_eq!(
            pick("HDMI-A-1", &["DP-1.png", "default.png"]),
            Choice::Image("default.png".to_owned())
        );
    }

    #[test]
    fn an_output_never_shows_a_picture_chosen_for_another_screen() {
        assert_eq!(pick("HDMI-A-1", &["DP-1.png"]), Choice::Fallback);
    }

    #[test]
    fn a_directory_with_nothing_showable_in_it_is_a_fallback() {
        assert_eq!(pick("DP-1", &[]), Choice::Fallback);
        assert_eq!(
            pick("DP-1", &["notes.txt", "photo.svg", ".DP-1.png"]),
            Choice::Fallback
        );
    }

    #[test]
    fn the_choice_does_not_depend_on_the_order_files_arrive_in() {
        let forwards = ["DP-1.png", "DP-1.jpg", "default.png"];
        let backwards = ["default.png", "DP-1.jpg", "DP-1.png"];
        assert_eq!(pick("DP-1", &forwards), pick("DP-1", &backwards));
        assert_eq!(
            pick("DP-1", &forwards),
            Choice::Image("DP-1.jpg".to_owned())
        );
    }

    #[test]
    fn a_connector_name_is_matched_exactly() {
        assert_eq!(pick("DP-1", &["dp-1.png"]), Choice::Fallback);
    }

    #[test]
    fn replacement_cleanup_matches_only_the_exact_output_stem() {
        assert!(belongs_to_output("DP-1", "DP-1.png"));
        assert!(belongs_to_output("DP-1", "DP-1.JPEG"));
        assert!(!belongs_to_output("DP-1", "DP-10.png"));
        assert!(!belongs_to_output("DP-1", "default.png"));
        assert!(!belongs_to_output("DP-1", "DP-1.svg"));
    }

    #[test]
    fn an_explicit_choice_beats_the_named_and_default_images() {
        let available = names(&["DP-1.png", "default.png", "sunset.jpg"]);
        assert_eq!(
            choose("DP-1", Some("sunset.jpg"), &available),
            Choice::Image("sunset.jpg".to_owned())
        );
    }

    #[test]
    fn an_explicit_choice_that_is_missing_or_unshowable_is_ignored() {
        let available = names(&["DP-1.png", "notes.txt", "default.png"]);
        assert_eq!(
            choose("DP-1", Some("gone.jpg"), &available),
            Choice::Image("DP-1.png".to_owned())
        );
        assert_eq!(
            choose("DP-1", Some("notes.txt"), &available),
            Choice::Image("DP-1.png".to_owned())
        );
        assert_eq!(
            choose("HDMI-A-1", Some("../etc.png"), &available),
            Choice::Image("default.png".to_owned())
        );
        assert_eq!(
            choose("HDMI-A-1", Some("gone.png"), &names(&[])),
            Choice::Fallback
        );
    }

    #[test]
    fn mode_uses_lowercase_names_and_defaults_to_fill() {
        assert_eq!(Mode::default(), Mode::Fill);
        for (mode, name) in [
            (Mode::Fill, "\"fill\""),
            (Mode::Fit, "\"fit\""),
            (Mode::Stretch, "\"stretch\""),
            (Mode::Center, "\"center\""),
        ] {
            assert_eq!(serde_json::to_string(&mode).ok().as_deref(), Some(name));
            assert_eq!(serde_json::from_str::<Mode>(name).ok(), Some(mode));
        }
    }

    const MODES: [Mode; 4] = [Mode::Fill, Mode::Fit, Mode::Stretch, Mode::Center];

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn fill_covers_and_crops_centred() {
        // Wide image on a 16:9 target: scale by height, crop the sides.
        let r = placement(Mode::Fill, (4000, 1000), (1920, 1080));
        assert!(close(r.h, 1080.0) && close(r.w, 4320.0));
        assert!(close(r.x, (1920.0 - 4320.0) / 2.0) && close(r.y, 0.0));
        // Tall image: scale by width, crop top and bottom.
        let r = placement(Mode::Fill, (1000, 4000), (1920, 1080));
        assert!(close(r.w, 1920.0) && close(r.h, 7680.0));
        assert!(close(r.x, 0.0) && close(r.y, (1080.0 - 7680.0) / 2.0));
    }

    #[test]
    fn fit_contains_and_centres() {
        let r = placement(Mode::Fit, (4000, 1000), (1920, 1080));
        assert!(close(r.w, 1920.0) && close(r.h, 480.0));
        assert!(close(r.x, 0.0) && close(r.y, 300.0));
        let r = placement(Mode::Fit, (1000, 4000), (1920, 1080));
        assert!(close(r.h, 1080.0) && close(r.w, 270.0));
        assert!(close(r.x, 825.0) && close(r.y, 0.0));
    }

    #[test]
    fn stretch_is_the_whole_target_and_center_is_one_to_one() {
        let r = placement(Mode::Stretch, (10, 3000), (1920, 1080));
        assert_eq!(
            r,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0
            }
        );
        let r = placement(Mode::Center, (800, 600), (1920, 1080));
        assert_eq!(
            r,
            Rect {
                x: 560.0,
                y: 240.0,
                w: 800.0,
                h: 600.0
            }
        );
        // Larger than the output: centred and overflowing equally.
        let r = placement(Mode::Center, (2920, 1080), (1920, 1080));
        assert!(close(r.x, -500.0) && close(r.w, 2920.0));
    }

    #[test]
    fn a_matching_aspect_ratio_is_the_whole_target_in_every_scaling_mode() {
        for mode in [Mode::Fill, Mode::Fit, Mode::Stretch] {
            let r = placement(mode, (960, 540), (1920, 1080));
            assert!(close(r.x, 0.0) && close(r.y, 0.0), "{mode:?}");
            assert!(close(r.w, 1920.0) && close(r.h, 1080.0), "{mode:?}");
        }
    }

    #[test]
    fn square_and_portrait_targets_are_finite_and_non_empty() {
        let images = [(1, 1), (1000, 1000), (1000, 2000), (3000, 1000), (1, 9000)];
        let targets = [
            (1080, 1080),
            (1080, 1920),
            (1920, 1080),
            (1, 1),
            (3840, 2160),
        ];
        for mode in MODES {
            for image in images {
                for target in targets {
                    let r = placement(mode, image, target);
                    assert!(
                        [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()),
                        "{mode:?} {image:?} {target:?}"
                    );
                    assert!(r.w > 0.0 && r.h > 0.0, "{mode:?} {image:?} {target:?}");
                }
            }
        }
    }

    #[test]
    fn degenerate_sizes_never_produce_nan_or_infinity() {
        let zeros = [(0, 0), (0, 100), (100, 0)];
        for mode in MODES {
            for degenerate in zeros {
                for other in [(1920, 1080), (0, 0)] {
                    for (image, target) in [(degenerate, other), (other, degenerate)] {
                        let r = placement(mode, image, target);
                        assert!(
                            [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()),
                            "{mode:?} {image:?} {target:?}"
                        );
                    }
                }
            }
            // A zero image still covers a real output rather than vanishing.
            let r = placement(mode, (0, 0), (1920, 1080));
            assert_eq!(
                r,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0
                }
            );
        }
    }

    #[test]
    fn next_walks_the_sorted_folder_and_wraps_both_ways() {
        let folder = names(&["b.png", "c.webp", "a.jpg"]);
        assert_eq!(next(&folder, Some("a.jpg"), 1).as_deref(), Some("b.png"));
        assert_eq!(next(&folder, Some("c.webp"), 1).as_deref(), Some("a.jpg"));
        assert_eq!(next(&folder, Some("a.jpg"), -1).as_deref(), Some("c.webp"));
        assert_eq!(next(&folder, Some("b.png"), -1).as_deref(), Some("a.jpg"));
        assert_eq!(next(&folder, Some("b.png"), 0).as_deref(), Some("b.png"));
        assert_eq!(next(&folder, Some("a.jpg"), 4).as_deref(), Some("b.png"));
        assert_eq!(next(&folder, Some("a.jpg"), -4).as_deref(), Some("c.webp"));
    }

    #[test]
    fn next_without_a_usable_current_starts_at_an_end() {
        let folder = names(&["a.jpg", "b.png"]);
        assert_eq!(next(&folder, None, 1).as_deref(), Some("a.jpg"));
        assert_eq!(next(&folder, None, -1).as_deref(), Some("b.png"));
        assert_eq!(next(&folder, Some("gone.png"), 1).as_deref(), Some("a.jpg"));
        assert_eq!(
            next(&folder, Some("gone.png"), -1).as_deref(),
            Some("b.png")
        );
    }

    #[test]
    fn next_ignores_unshowable_names_and_an_empty_folder_has_none() {
        let folder = names(&["a.png", "notes.txt", "b.svg", ".c.png", "d.jpg"]);
        assert_eq!(next(&folder, Some("a.png"), 1).as_deref(), Some("d.jpg"));
        assert_eq!(next(&folder, Some("d.jpg"), 1).as_deref(), Some("a.png"));
        // A current that is not showable counts as absent.
        assert_eq!(
            next(&folder, Some("notes.txt"), 1).as_deref(),
            Some("a.png")
        );
        assert_eq!(next(&names(&["x.txt"]), None, 1), None);
        assert_eq!(next(&[], Some("a.png"), -1), None);
        // One image: every step lands on it.
        assert_eq!(
            next(&names(&["a.png"]), Some("a.png"), 1).as_deref(),
            Some("a.png")
        );
    }

    #[test]
    fn next_does_not_depend_on_the_order_files_arrive_in() {
        let forwards = names(&["a.jpg", "b.png", "c.webp"]);
        let backwards = names(&["c.webp", "b.png", "a.jpg", "a.jpg"]);
        for step in [-2, -1, 1, 2] {
            assert_eq!(
                next(&forwards, Some("b.png"), step),
                next(&backwards, Some("b.png"), step)
            );
        }
    }
}
