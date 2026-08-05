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

use crate::bounded;

/// The longest file name this shell will carry. A wallpaper directory is the
/// author's own, but a name is still text from outside the program.
pub const MAX_NAME_CHARS: usize = 255;

/// What this shell knows how to show. Deliberately short: these are the formats
/// Qt decodes without a plugin the session may not have, and a wallpaper that
/// depends on an optional decoder would work on one machine and not the next.
const SHOWABLE: [&str; 5] = ["png", "jpg", "jpeg", "webp", "avif"];

/// Whether a file name is one this shell can show.
///
/// Extension only: reading the file to find out would mean opening every entry
/// in a directory on every start, and a mislabelled file fails at decode time
/// anyway — where the surface already has an honest fallback.
#[must_use]
pub fn is_showable(name: &str) -> bool {
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || name.starts_with('.') {
        return false;
    }
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && SHOWABLE
                .iter()
                .any(|showable| extension.eq_ignore_ascii_case(showable))
    })
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
/// An output is matched by name first — `DP-1.png` is for `DP-1`, and matching
/// by name is what lets a two-screen session hold two different pictures. A
/// `default` image serves any output that has no named one. Anything else,
/// including a directory full of files this shell cannot show, is
/// [`Choice::Fallback`].
///
/// `available` is the file names present, in any order; the choice does not
/// depend on that order, so two outputs never race to a different answer.
#[must_use]
pub fn choose(output: &str, available: &[String]) -> Choice {
    let showable = |name: &&String| is_showable(name);

    // The output's own image, by stem. Case-sensitive: connector names are, and
    // matching `dp-1.png` to `DP-1` would be guessing.
    if let Some(named) = available
        .iter()
        .filter(showable)
        .filter(|name| stem_of(name) == output)
        .min()
    {
        return Choice::Image(bounded(named, MAX_NAME_CHARS));
    }

    // The session's own image, for every output without one of its own.
    if let Some(shared) = available
        .iter()
        .filter(showable)
        .filter(|name| stem_of(name) == "default")
        .min()
    {
        return Choice::Image(bounded(shared, MAX_NAME_CHARS));
    }

    // Deliberately not "the first image in the directory": an output showing a
    // picture chosen for another screen is a lie about which file it is.
    Choice::Fallback
}

fn stem_of(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|entry| (*entry).to_owned()).collect()
    }

    #[test]
    fn only_formats_every_session_can_decode_are_showable() {
        assert!(is_showable("DP-1.png"));
        assert!(is_showable("photo.JPEG"));
        assert!(is_showable("a.webp"));
        // Not showable without a decoder this session may not have.
        assert!(!is_showable("photo.svg"));
        assert!(!is_showable("photo.heic"));
        assert!(!is_showable("photo.mp4"));
        // Not a file this shell should be picking up at all.
        assert!(!is_showable("notes.txt"));
        assert!(!is_showable("README"));
        assert!(!is_showable(".hidden.png"));
        assert!(!is_showable(".png"));
        assert!(!is_showable(""));
    }

    #[test]
    fn an_output_gets_the_image_named_for_it() {
        let available = names(&["DP-1.png", "DP-2.jpg", "default.png"]);
        assert_eq!(
            choose("DP-1", &available),
            Choice::Image("DP-1.png".to_owned())
        );
        assert_eq!(
            choose("DP-2", &available),
            Choice::Image("DP-2.jpg".to_owned())
        );
    }

    #[test]
    fn an_output_without_its_own_image_falls_back_to_the_shared_one() {
        let available = names(&["DP-1.png", "default.png"]);
        assert_eq!(
            choose("HDMI-A-1", &available),
            Choice::Image("default.png".to_owned())
        );
    }

    #[test]
    fn an_output_never_shows_a_picture_chosen_for_another_screen() {
        // A directory with one image in it, named for a screen that is not
        // this one. The temptation is to show it anyway; that would be a lie
        // about which file this output is displaying.
        let available = names(&["DP-1.png"]);
        assert_eq!(choose("HDMI-A-1", &available), Choice::Fallback);
    }

    #[test]
    fn a_directory_with_nothing_showable_in_it_is_a_fallback() {
        assert_eq!(choose("DP-1", &names(&[])), Choice::Fallback);
        assert_eq!(
            choose("DP-1", &names(&["notes.txt", "photo.svg", ".DP-1.png"])),
            Choice::Fallback
        );
    }

    #[test]
    fn the_choice_does_not_depend_on_the_order_files_arrive_in() {
        let forwards = names(&["DP-1.png", "DP-1.jpg", "default.png"]);
        let backwards = names(&["default.png", "DP-1.jpg", "DP-1.png"]);
        // Two candidates for the same output resolve the same way whichever
        // order the directory happened to be read in.
        assert_eq!(choose("DP-1", &forwards), choose("DP-1", &backwards));
        assert_eq!(
            choose("DP-1", &forwards),
            Choice::Image("DP-1.jpg".to_owned())
        );
    }

    #[test]
    fn a_connector_name_is_matched_exactly() {
        let available = names(&["dp-1.png"]);
        // Connector names are case-sensitive; matching loosely would be
        // guessing which screen the author meant.
        assert_eq!(choose("DP-1", &available), Choice::Fallback);
    }
}
