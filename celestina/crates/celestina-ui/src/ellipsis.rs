//! Fitting a text to a width by cutting its middle.
//!
//! A name that only differs at its end ("... (HDMI)" and "... (HDMI 2)") must
//! stay distinct when it is cut, so the cut keeps the head and a longer tail
//! and puts the ellipsis between them. The width of a candidate is asked of
//! the caller (the text system in the product, a fixed advance in the tests),
//! so the cut follows what is drawn, never a count of bytes or characters.
#![forbid(unsafe_code)]

use unicode_segmentation::UnicodeSegmentation;

/// The mark that stands for the cut text.
pub const ELLIPSIS: char = '\u{2026}';

/// `text` whole if `width_of` says it fits `max_width`; else its start and its
/// end around an ellipsis (about two fifths of what is kept at the end, never
/// splitting a grapheme cluster), keeping as much as still fits. When not
/// even the ellipsis fits it is returned alone.
///
/// A cut's width grows with what it keeps, so the longest cut that fits is
/// found by bisection: `width_of` runs about `log2` of the cluster count
/// times, which bounds the cost of a hostile, very long name.
#[must_use]
pub fn fit_middle(text: &str, max_width: f32, mut width_of: impl FnMut(&str) -> f32) -> String {
    if width_of(text) <= max_width {
        return text.to_owned();
    }
    let clusters: Vec<&str> = text.graphemes(true).collect();
    let cut = |kept: usize| {
        let tail = (kept * 2).div_ceil(5);
        let mut cut = clusters[..kept - tail].concat();
        cut.push(ELLIPSIS);
        cut.push_str(&clusters[clusters.len() - tail..].concat());
        cut
    };
    // Invariant: a cut keeping `fits` clusters fits (0 is the bare ellipsis)
    // and one keeping `wide` does not (the whole text did not).
    let (mut fits, mut wide) = (0, clusters.len());
    let mut best = ELLIPSIS.to_string();
    while wide - fits > 1 {
        let kept = fits + (wide - fits) / 2;
        let candidate = cut(kept);
        if width_of(&candidate) <= max_width {
            fits = kept;
            best = candidate;
        } else {
            wide = kept;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 7 px per character.
    fn width(s: &str) -> f32 {
        7.0 * s.chars().count() as f32
    }

    #[test]
    fn a_text_that_fits_is_left_alone() {
        assert_eq!(fit_middle("Zen", 100.0, width), "Zen");
        assert_eq!(fit_middle("abcdef", 42.0, width), "abcdef");
    }

    #[test]
    fn names_that_differ_only_at_the_end_stay_distinct_and_fit() {
        let a = "Navi 48 HDMI/DP Audio Controller Digital Stereo (HDMI)";
        let b = "Navi 48 HDMI/DP Audio Controller Digital Stereo (HDMI 2)";
        let (cut_a, cut_b) = (fit_middle(a, 200.0, width), fit_middle(b, 200.0, width));
        assert_ne!(cut_a, cut_b);
        assert!(width(&cut_a) <= 200.0 && width(&cut_b) <= 200.0);
        assert!(cut_a.contains(ELLIPSIS) && cut_b.starts_with("Navi 48"));
        assert!(cut_b.ends_with("(HDMI 2)"), "{cut_b}");
        // It keeps as much as fits: one more cluster would not.
        assert!(width(&cut_b) > 200.0 - 7.0 * 2.0, "{cut_b}");
    }

    #[test]
    fn it_cuts_between_grapheme_clusters_and_survives_no_room() {
        // A flag is one cluster of two scalar values.
        let flags = "\u{1f1ea}\u{1f1f8}".repeat(10);
        let cut = fit_middle(&flags, 70.0, |s| 7.0 * s.graphemes(true).count() as f32);
        assert!(cut
            .chars()
            .all(|c| c == ELLIPSIS || ('\u{1f1e6}'..='\u{1f1ff}').contains(&c)));
        assert_eq!(cut.chars().filter(|c| *c != ELLIPSIS).count() % 2, 0);
        assert_eq!(fit_middle("abcdef", 1.0, width), ELLIPSIS.to_string());
        assert_eq!(fit_middle("", 0.0, width), "");
    }

    #[test]
    fn a_very_long_name_costs_a_logarithmic_number_of_measures() {
        let name = "x".repeat(100_000);
        let mut calls = 0;
        let cut = fit_middle(&name, 200.0, |s| {
            calls += 1;
            width(s)
        });
        assert!(calls <= 20, "{calls} measures");
        assert!(
            width(&cut) <= 200.0 && width(&cut) > 200.0 - 7.0 * 2.0,
            "{cut}"
        );
    }
}
