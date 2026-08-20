//! What plays when the current thing ends.
//!
//! Until now reaching the end of a track was the end of listening: the session
//! stopped and the next song in the folder sat there waiting to be clicked.
//! This module owns the one rule that fixes it — *given what just ended and
//! what is around it, what plays next* — as a pure function over a position in
//! a list.
//!
//! Two things it deliberately does not do:
//!
//! - **It never advances on its own.** It answers a question the host asks
//!   after the engine has *confirmed* the file ended. A prediction made when a
//!   file was merely near its end would skip a track whose last seconds failed
//!   to decode.
//! - **It never advances a still.** An image has no end to reach, so a library
//!   of photographs cannot turn itself into a slideshow by accident.

use crate::media::MediaKind;

/// What to do when the current item ends.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Continuation {
    /// Stop. The item stays open at its end, which is what the player already
    /// does and what someone listening to one song expects.
    #[default]
    Stop,
    /// Play the next item in the folder, and stop after the last one.
    ///
    /// Stopping rather than wrapping is the honest default: a list that starts
    /// again from the top has no end, and a person who left the room would come
    /// back to find it still going.
    Folder,
    /// Play the same item again.
    RepeatOne,
}

impl Continuation {
    /// The three, in the order a menu offers them.
    pub const ALL: [Self; 3] = [Self::Stop, Self::Folder, Self::RepeatOne];

    /// Which item to open after the one at `index` ended, if any.
    ///
    /// `count` is how many items the folder holds and `kind` is what just
    /// ended. Returns `None` whenever nothing should start: a still, an empty
    /// or single-item folder under `Folder`, the last item, an index that does
    /// not name anything, or [`Continuation::Stop`].
    #[must_use]
    pub fn next(self, kind: MediaKind, index: usize, count: usize) -> Option<usize> {
        // A still never ends, so nothing follows one. Reaching this with an
        // image means a host confirmed an end that cannot happen.
        if !kind.capabilities().timed || index >= count {
            return None;
        }
        match self {
            Self::Stop => None,
            Self::RepeatOne => Some(index),
            Self::Folder => {
                let next = index.checked_add(1)?;
                (next < count).then_some(next)
            }
        }
    }

    /// Whether this mode keeps something playing at all. A surface uses it to
    /// say whether listening will continue, without knowing where in a folder
    /// the person is.
    #[must_use]
    pub const fn continues(self) -> bool {
        !matches!(self, Self::Stop)
    }
}

#[cfg(test)]
mod tests {
    use super::Continuation;
    use crate::media::MediaKind;

    #[test]
    fn stopping_is_the_default_and_starts_nothing() {
        let mode = Continuation::default();
        assert_eq!(mode, Continuation::Stop);
        assert_eq!(mode.next(MediaKind::Audio, 0, 10), None);
        assert!(!mode.continues());
    }

    #[test]
    fn the_folder_plays_on_and_stops_after_the_last_one() {
        let mode = Continuation::Folder;
        assert_eq!(mode.next(MediaKind::Audio, 0, 3), Some(1));
        assert_eq!(mode.next(MediaKind::Audio, 1, 3), Some(2));
        assert_eq!(
            mode.next(MediaKind::Audio, 2, 3),
            None,
            "a list that starts again from the top has no end"
        );
        assert!(mode.continues());
    }

    #[test]
    fn repeating_returns_the_same_position() {
        assert_eq!(
            Continuation::RepeatOne.next(MediaKind::Video, 4, 9),
            Some(4)
        );
    }

    #[test]
    fn a_still_never_leads_anywhere_whatever_the_mode() {
        for mode in Continuation::ALL {
            assert_eq!(
                mode.next(MediaKind::Image, 0, 10),
                None,
                "{mode:?} turned a gallery into a slideshow"
            );
        }
    }

    #[test]
    fn a_position_that_names_nothing_starts_nothing() {
        for mode in Continuation::ALL {
            assert_eq!(mode.next(MediaKind::Audio, 5, 5), None);
            assert_eq!(mode.next(MediaKind::Audio, 0, 0), None);
        }
    }

    #[test]
    fn a_folder_of_one_ends_where_it_started() {
        assert_eq!(Continuation::Folder.next(MediaKind::Audio, 0, 1), None);
        assert_eq!(
            Continuation::RepeatOne.next(MediaKind::Audio, 0, 1),
            Some(0),
            "one track on repeat is still a thing a person asks for"
        );
    }
}
