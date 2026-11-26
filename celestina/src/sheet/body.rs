//! What a capsule sheet shows under its row: a body, laid out in its own
//! coordinates ((0, 0) is the body box's top-left) and painted translated to
//! where the sheet puts it. Part A has one body, the volume's level card.

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Role, Theme, Weight};
use celestina_ui::{fit_middle, Tree};
use celestina_wayland::InputEvent;

use super::Measure;
use crate::{
    island::view::text_style,
    osd::popup::{LevelCore, Outcome},
};

/// A sheet's body.
#[derive(Debug)]
pub enum SheetBody {
    /// The volume's (or, later, the brightness') sliders and rows.
    Level(Box<LevelCore>),
}

/// What a body asks of its sheet after an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyOutcome {
    /// The sheet stays; `changed` when the body looks different.
    Keep { changed: bool },
    /// The body is done (Escape): the sheet closes.
    Close,
}

impl SheetBody {
    /// The body box's size, logical px.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        match self {
            SheetBody::Level(core) => core.size(),
        }
    }

    /// Whether the wheel at (`x`, `y`) of the body belongs to the body (a
    /// slider under the pointer) rather than to the sheet's scroll.
    #[must_use]
    pub fn takes_wheel(&self, x: f32, y: f32, now: Instant) -> bool {
        match self {
            SheetBody::Level(core) => core.slider_at(x, y, now).is_some(),
        }
    }

    /// Whether the pointer is held on the body (a drag): every pointer event
    /// then belongs to it, wherever it is.
    #[must_use]
    pub fn held(&self) -> bool {
        match self {
            SheetBody::Level(core) => core.held(),
        }
    }

    /// Sends what the body's throttle holds, now: the last thing the person
    /// chose is never lost to a close or a switch.
    pub fn flush(&mut self, now: Instant) {
        match self {
            SheetBody::Level(core) => {
                core.flush(now, true);
            }
        }
    }

    /// Lays the body out in its own size with `lay`.
    pub fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        match self {
            SheetBody::Level(core) => core.layout_in(lay),
        }
    }

    /// Paints the body at its own coordinates (the caller translates).
    pub fn paint(&mut self, painter: &mut Painter, now: Instant) {
        match self {
            SheetBody::Level(core) => core.paint(painter, now),
        }
    }

    /// Whether the body's own animations still run.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        match self {
            SheetBody::Level(core) => core.is_animating(now),
        }
    }

    /// How long until something the body holds is due (a throttled send, a
    /// level held while its monitor settles).
    #[must_use]
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        match self {
            SheetBody::Level(core) => [core.throttle_wait(now), core.graph_wait(now)]
                .into_iter()
                .flatten()
                .min(),
        }
    }

    /// The rect of the body's node `key`, in body coordinates (tests).
    #[cfg(test)]
    #[must_use]
    pub fn node_rect(&self, key: celestina_ui::Key) -> Option<celestina_paint::Rect> {
        match self {
            SheetBody::Level(core) => core.node_rect(key),
        }
    }

    /// Whether the body is the volume's or brightness' level card.
    #[must_use]
    pub fn is_level(&self) -> bool {
        matches!(self, SheetBody::Level(_))
    }

    /// Handles `ev` (already in body coordinates). Names are cut with
    /// `measure`, in hand for this call only.
    pub fn input(
        &mut self,
        ev: &InputEvent,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> BodyOutcome {
        match self {
            SheetBody::Level(core) => {
                let style = core.name_style();
                let mut fit = |name: &str, width: f32| {
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                };
                match core.input(ev, now, &mut fit) {
                    Outcome::Close => BodyOutcome::Close,
                    Outcome::Keep { changed } => BodyOutcome::Keep { changed },
                }
            }
        }
    }

    /// Takes in a new theme and the app's newest readings, sends what is due
    /// and says whether the look changed. Names are cut with `measure`.
    pub fn sync(&mut self, theme: &Theme, now: Instant, measure: &mut Measure<'_>) -> bool {
        match self {
            SheetBody::Level(core) => {
                core.cut_monitors(theme, now, |name, width, weight: Weight| {
                    let style = text_style(theme.type_scale.body, weight, false, Role::TextHi);
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                });
                let style = text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi);
                let changed = core.sync(theme, now, |name, width| {
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                });
                core.flush(now, false);
                changed
            }
        }
    }
}
