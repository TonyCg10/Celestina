//! Who paints the Island's capsule on the output of a notices surface
//! (design §5.1). The surface paints the rest Island in its first frame, and
//! only then does the panel lend its capsule; at rest the panel takes it
//! back first and the surface goes one loop turn later, so no frame shows
//! the bar without an Island. Pure: the app does what each answer says.

/// Where the capsule is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Holder {
    /// The panel paints it; the surface has not drawn yet (or it was
    /// returned and the surface has not woken again).
    Panel,
    /// The surface drew its first frame; the lend is posted.
    Asked,
    /// The surface paints it.
    Lent,
    /// The surface found everything at rest; the return is posted. `lent`
    /// is whether the panel had lent the capsule before.
    Resting { lent: bool },
    /// The panel paints it again; the release is posted.
    Returned,
}

/// What a frame asks the app to post to itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Post {
    /// Lend the panel's capsule (the first frame is out).
    Lend,
    /// Everything is at rest: take the capsule back.
    Rest,
}

/// What the app does when a posted lend or rest comes back round the loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The panel lends its capsule.
    Lend,
    /// The panel paints its capsule (it already does when it was never
    /// lent), is redrawn, and the release is posted.
    Return,
    /// Nothing: the message is stale.
    Nothing,
}

/// The hand-over of one notices surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handover {
    holder: Holder,
}

impl Default for Handover {
    fn default() -> Self {
        Handover::new()
    }
}

impl Handover {
    /// A surface just created: the panel holds the capsule.
    #[must_use]
    pub fn new() -> Handover {
        Handover {
            holder: Holder::Panel,
        }
    }

    /// A frame of the surface was drawn; `wants` is whether it still has
    /// something to show. What to post, if anything. A frame drawn after a
    /// return asks to lend again: a notice that arrives during the shrink
    /// or the turn before the release reuses the surface.
    pub fn frame(&mut self, wants: bool) -> Option<Post> {
        match (self.holder, wants) {
            (Holder::Panel | Holder::Returned, true) => {
                self.holder = Holder::Asked;
                Some(Post::Lend)
            }
            (Holder::Lent, false) => {
                self.holder = Holder::Resting { lent: true };
                Some(Post::Rest)
            }
            // Drew with nothing to show and never held the capsule: the
            // return still redraws the panel, which skipped its own
            // redraws while this surface was there.
            (Holder::Panel, false) => {
                self.holder = Holder::Resting { lent: false };
                Some(Post::Rest)
            }
            _ => None,
        }
    }

    /// The posted lend came back round the loop: the panel lends its
    /// capsule now, unless the surface has nothing left to show.
    pub fn lend(&mut self, wants: bool) -> Answer {
        if self.holder != Holder::Asked {
            return Answer::Nothing;
        }
        if wants {
            self.holder = Holder::Lent;
            Answer::Lend
        } else {
            self.holder = Holder::Returned;
            Answer::Return
        }
    }

    /// The posted rest came back round the loop: the panel takes its
    /// capsule back now (and the app posts the release), unless something
    /// arrived meanwhile.
    pub fn rest(&mut self, wants: bool) -> Answer {
        let Holder::Resting { lent } = self.holder else {
            return Answer::Nothing;
        };
        if wants {
            // A capsule the panel never lent is asked for again by the next
            // frame.
            self.holder = if lent { Holder::Lent } else { Holder::Panel };
            Answer::Nothing
        } else {
            self.holder = Holder::Returned;
            Answer::Return
        }
    }

    /// Whether the surface holds, or is about to hold, the capsule: the
    /// clock face it paints must roll over at the minute.
    #[must_use]
    pub fn holds(&self) -> bool {
        matches!(
            self.holder,
            Holder::Asked | Holder::Lent | Holder::Resting { .. }
        )
    }

    /// Whether a lend or a return is still to come, and the panel's redraw
    /// with it: a change of what the panel shows waits for that redraw. Once
    /// the surface holds the capsule steadily (or the panel has it back) the
    /// panel redraws for its own changes, such as the bell, like any other.
    #[must_use]
    pub fn answer_pending(&self) -> bool {
        matches!(
            self.holder,
            Holder::Panel | Holder::Asked | Holder::Resting { .. }
        )
    }

    /// The release came back round the loop. Whether the surface goes now:
    /// the capsule is back and nothing wants the surface.
    #[must_use]
    pub fn release(&self, wants: bool) -> bool {
        self.holder == Holder::Returned && !wants
    }

    /// The surface is going for another reason (its output lost the focus):
    /// whether the panel must take its capsule back, which it does at once.
    pub fn retire(&mut self) -> bool {
        let lent = !matches!(
            self.holder,
            Holder::Panel | Holder::Returned | Holder::Resting { lent: false }
        );
        self.holder = Holder::Returned;
        lent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capsule_is_lent_after_the_first_frame_and_taken_back_before_release() {
        let mut h = Handover::new();
        // Nothing is lent before a frame is out.
        assert_eq!(h.lend(true), Answer::Nothing, "no frame, no lend");
        assert!(!h.release(false));
        assert_eq!(h.frame(true), Some(Post::Lend));
        assert_eq!(
            h.lend(true),
            Answer::Lend,
            "the first frame is out: the panel lends"
        );
        // Animating frames post nothing.
        assert_eq!(h.frame(true), None);
        // Everything at rest: the return comes first.
        assert_eq!(h.frame(false), Some(Post::Rest));
        assert!(!h.release(false), "the panel has not taken it back yet");
        assert_eq!(
            h.rest(false),
            Answer::Return,
            "the panel paints its capsule again"
        );
        assert!(h.release(false), "only now may the surface go");
    }

    #[test]
    fn a_toast_during_the_shrink_keeps_the_surface() {
        let mut h = Handover::new();
        h.frame(true);
        assert_eq!(h.lend(true), Answer::Lend);
        // The shrink ends and posts the return...
        assert_eq!(h.frame(false), Some(Post::Rest));
        // ...but a notice arrived before it came round: the panel keeps
        // the capsule lent, so it never paints under the grown Island.
        assert_eq!(h.rest(true), Answer::Nothing);
        assert!(!h.release(true));
        assert_eq!(h.frame(true), None, "the surface is reused as it is");
        // The next rest runs the whole way again.
        assert_eq!(h.frame(false), Some(Post::Rest));
        assert_eq!(h.rest(false), Answer::Return);
    }

    #[test]
    fn a_toast_in_the_turn_before_the_release_lends_again_and_keeps_the_surface() {
        let mut h = Handover::new();
        h.frame(true);
        h.lend(true);
        h.frame(false);
        assert_eq!(h.rest(false), Answer::Return);
        // A notice arrives in the wait: the release finds the surface
        // wanted and leaves it.
        assert!(!h.release(true));
        // Its first frame at the rest Island is drawn before the panel
        // lends the capsule once more.
        assert_eq!(h.frame(true), Some(Post::Lend));
        assert_eq!(h.lend(true), Answer::Lend);
        // The old release is a stale message now.
        assert!(!h.release(false));
    }

    #[test]
    fn a_lend_that_finds_nothing_to_show_is_not_done() {
        let mut h = Handover::new();
        assert_eq!(h.frame(true), Some(Post::Lend));
        // The panel never lent: it just goes back to the release.
        assert_eq!(h.lend(false), Answer::Return);
        assert!(h.release(false));
        assert_eq!(h.frame(true), Some(Post::Lend), "it can be asked again");
    }

    #[test]
    fn a_surface_that_draws_with_nothing_to_show_returns_without_a_lend() {
        let mut h = Handover::new();
        assert_eq!(h.frame(false), Some(Post::Rest));
        assert_eq!(h.rest(false), Answer::Return);
        assert!(h.release(false));
    }

    #[test]
    fn a_toast_after_a_rest_that_never_lent_still_asks_for_the_lend() {
        let mut h = Handover::new();
        assert_eq!(h.frame(false), Some(Post::Rest), "nothing to show yet");
        // A notice arrives before the rest came back: the panel never lent,
        // so the surface must ask, or the panel's capsule would show under
        // the grown notice.
        assert_eq!(h.rest(true), Answer::Nothing);
        assert_eq!(h.frame(true), Some(Post::Lend));
        assert_eq!(h.lend(true), Answer::Lend);
        // And what was lent is still kept when the same happens after one.
        assert_eq!(h.frame(false), Some(Post::Rest));
        assert_eq!(h.rest(true), Answer::Nothing);
        assert_eq!(h.frame(true), None);
    }

    #[test]
    fn the_panel_redraws_for_its_own_changes_unless_a_lend_or_return_is_coming() {
        let mut h = Handover::new();
        assert!(h.answer_pending(), "the first frame is yet to come");
        h.frame(true);
        assert!(h.answer_pending(), "the lend is posted");
        h.lend(true);
        assert!(!h.answer_pending(), "lent and steady: the bell may change");
        h.frame(false);
        assert!(h.answer_pending(), "the return is posted");
        h.rest(false);
        assert!(!h.answer_pending(), "the panel has it back");
    }

    #[test]
    fn the_surface_holds_the_capsule_from_the_ask_to_the_return() {
        let mut h = Handover::new();
        assert!(!h.holds());
        h.frame(true);
        assert!(h.holds(), "asked");
        h.lend(true);
        assert!(h.holds(), "lent");
        h.frame(false);
        assert!(h.holds(), "resting");
        h.rest(false);
        assert!(!h.holds(), "returned");
    }

    #[test]
    fn retiring_hands_a_lent_capsule_back_and_only_that() {
        let mut lent = Handover::new();
        lent.frame(true);
        lent.lend(true);
        assert!(lent.retire());
        let mut fresh = Handover::new();
        assert!(!fresh.retire(), "never lent: nothing to hand back");
        let mut asked = Handover::new();
        asked.frame(true);
        assert!(asked.retire(), "a lend in flight is cancelled too");
        assert_eq!(
            asked.lend(true),
            Answer::Nothing,
            "and it finds nothing to do"
        );
    }
}
