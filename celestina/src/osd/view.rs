//! The on-screen display's look (CEL-2-C, plan decision 3): a Haze card with
//! the kind's icon, a bar of the level and the level as `NN %`, near the
//! bottom edge of its surface. Every colour is a theme role and every size a
//! theme scalar; the words come from [`crate::copy`].
//!
//! [`OsdView`] owns the surface's tree. Its first content appears at once (it
//! is feedback for a key the person just pressed); taking the content away
//! drops the card from the root, so it leaves on the tree's one exit fade (`Motion::Exit`, an opacity fade that reduced motion keeps, as the
//! motion contract says), and its draw asks for frames only while that fade
//! runs: a settled display draws nothing more.

use std::time::Instant;

use celestina_paint::{silhouette_slices, Painter, Rect, Role, Theme, Weight};
use celestina_shell::osd::{Kind, Shown};
use celestina_ui::{view, Key, Tree, View};
use celestina_wayland::FrameCx;
use taffy::JustifyContent;

use crate::copy;
use crate::island::{
    state::SILHOUETTE_STEP,
    view::{centred, text_style},
};

/// The card, the root's one child while something is shown.
#[must_use]
pub fn card_key() -> Key {
    Key::named("osd-card")
}

/// The surface-wide root, kept while the surface lives so the card's going
/// is a child leaving, never a new root fading in.
fn root_key() -> Key {
    Key::named("osd")
}

/// The card's measures, all from `theme`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// The card's padding along its length.
    pub padding: f32,
    /// The square of the icon.
    pub icon: f32,
    /// The gap between the icon, the bar and the level.
    pub gap: f32,
    /// The bar's length and thickness.
    pub track: (f32, f32),
    /// The width kept for the level, wide enough for `100 %`.
    pub label: f32,
    /// The card's size.
    pub card: (f32, f32),
    /// The space between the card and the output's edges.
    pub margin: f32,
}

impl Metrics {
    /// The measures of `theme`: one row tall with the small spacing above and
    /// below, a bar five rows long, and a label four body glyphs wide so the
    /// level never moves the bar as it changes.
    #[must_use]
    pub fn of(theme: &Theme) -> Metrics {
        let padding = theme.spacing.lg;
        let icon = theme.spacing.xxl;
        let gap = theme.spacing.md;
        let track = (5.0 * theme.row_height, theme.linear_track_height);
        let label = 4.0 * theme.type_scale.body;
        let card = (
            2.0 * padding + icon + gap + track.0 + gap + label,
            theme.row_height + 2.0 * theme.spacing.sm,
        );
        Metrics {
            padding,
            icon,
            gap,
            track,
            label,
            card,
            margin: theme.spacing.xxxl,
        }
    }

    /// The surface the card sits in, logical px: the card with the margin on
    /// both sides and below it, so the same surface sits centred on the
    /// bottom edge or in the bottom-right corner. Above and beside the card
    /// it is transparent and takes no input.
    #[must_use]
    pub fn surface(&self) -> (u32, u32) {
        (
            (self.card.0 + 2.0 * self.margin).ceil() as u32,
            (self.card.1 + self.margin).ceil() as u32,
        )
    }
}

/// The semantic icon of what is shown: the kind's, or its muted one.
#[must_use]
pub fn icon_of(shown: Shown) -> &'static str {
    match (shown.kind, shown.reading.muted) {
        (Kind::Volume, false) => "media-volume",
        (Kind::Volume, true) => "media-volume-muted",
        (Kind::Microphone, false) => "mic",
        (Kind::Microphone, true) => "mic-off",
        (Kind::Brightness, _) => "sun",
    }
}

/// The view of `shown` on a surface: the card at its place, or the empty
/// root while nothing is shown.
#[must_use]
pub fn view(theme: &Theme, shown: Option<Shown>) -> View {
    let m = Metrics::of(theme);
    let (w, h) = m.surface();
    let root = view::col(root_key()).size(w as f32, h as f32);
    match shown {
        Some(shown) => root.child(card(theme, &m, shown)),
        None => root,
    }
}

/// The card of `shown`: icon, bar and level.
fn card(theme: &Theme, m: &Metrics, shown: Shown) -> View {
    let percent = shown.reading.percent.min(100);
    // A muted device keeps its level, drawn faint.
    let level = if shown.reading.muted {
        Role::TextFaint
    } else {
        Role::Accent
    };
    let filled = m.track.0 * f32::from(percent) / 100.0;
    let track = view::row(Key::named("osd-track"))
        .size(m.track.0, m.track.1)
        .radius(m.track.1 / 2.0)
        .fill(Role::ControlFill)
        .clip()
        .child(
            view::row(Key::named("osd-level"))
                .size(filled, m.track.1)
                .radius(m.track.1 / 2.0)
                .fill(level),
        );
    let mut label = centred(view::row(Key::named("osd-label"))).width(m.label);
    label.style.justify_content = JustifyContent::FLEX_END;
    let label = label.child(view::text(
        Key::named("osd-label-text"),
        &copy::osd_percent(percent),
        text_style(theme.type_scale.body, Weight::Semibold, true, Role::TextHi),
    ));
    centred(view::row(card_key()))
        .absolute(m.margin, 0.0)
        .size(m.card.0, m.card.1)
        .haze(theme.radius_lg)
        .padding_xy(m.padding, 0.0)
        .gap(m.gap)
        .child(view::icon(Key::named("osd-icon"), icon_of(shown), m.icon).tint(Role::TextHi))
        .child(track)
        .child(label)
}

/// The tree of one OSD surface and what it last showed.
#[derive(Debug)]
pub struct OsdView {
    theme: Theme,
    tree: Tree,
    /// What the tree was last given; `None` until the first frame.
    applied: Option<Option<Shown>>,
    size: (f32, f32),
    dpr: f32,
}

impl OsdView {
    /// A surface's tree in `theme` under `reduced` motion, nothing applied.
    #[must_use]
    pub fn new(theme: &Theme, reduced: bool) -> OsdView {
        OsdView {
            theme: theme.clone(),
            tree: Tree::new(reduced, theme),
            applied: None,
            size: (0.0, 0.0),
            dpr: 1.0,
        }
    }

    /// Gives the tree `shown` when it differs from what it has. Returns
    /// whether it did.
    fn show(&mut self, shown: Option<Shown>, now: Instant) -> bool {
        if self.applied == Some(shown) {
            return false;
        }
        self.tree.apply(view(&self.theme, shown), now);
        self.applied = Some(shown);
        true
    }

    /// The blur region at `now`: the card's silhouette while it is drawn,
    /// leaving or not.
    fn blur(&self, now: Instant) -> Vec<Rect> {
        self.tree
            .node_by_key(card_key())
            .filter(|node| self.tree.effective_opacity(*node, now) > 0.0)
            .map(|node| {
                silhouette_slices(self.tree.rect(node), self.theme.radius_lg, SILHOUETTE_STEP)
            })
            .unwrap_or_default()
    }

    /// Draws one frame of `shown`: the card (or its fading), the blur region
    /// of what was painted and the empty input region. Asks for another
    /// frame only while the tree animates.
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        painter: &mut Painter,
        shown: Option<Shown>,
        reduced: bool,
    ) {
        let now = cx.now;
        if reduced != self.tree.reduced_motion() {
            self.tree.set_reduced_motion(reduced);
        }
        if painter.theme() != &self.theme {
            self.theme = painter.theme().clone();
            self.tree.set_theme(&self.theme);
            // Rebuilt with the new measures.
            self.applied = None;
        }
        self.size = cx.size;
        self.dpr = cx.scale as f32;
        self.show(shown, now);
        self.tree.layout(self.size, painter.text(), self.dpr);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "osd: layout fault");
        }
        painter.begin(self.size, self.dpr, cx.blur_available);
        self.tree.paint(painter, now);
        painter.end();
        cx.ctl.set_blur_region(&self.blur(now));
        // The display never takes the pointer: a click goes to what is under.
        cx.ctl.set_input_region(&[]);
        if self.tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::TextStyle;
    use celestina_shell::osd::Reading;

    use super::*;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn shown(kind: Kind, percent: u8, muted: bool) -> Shown {
        Shown {
            kind,
            reading: Reading { percent, muted },
        }
    }

    fn laid_out(theme: &Theme, what: Option<Shown>, now: Instant) -> OsdView {
        let mut osd = OsdView::new(theme, false);
        osd.show(what, now);
        let size = Metrics::of(theme).surface();
        osd.tree
            .layout_measured((size.0 as f32, size.1 as f32), fake_measure);
        osd
    }

    #[test]
    fn every_measure_comes_from_the_theme() {
        let theme = Theme::dark();
        let m = Metrics::of(&theme);
        assert_eq!(m.track.1, theme.linear_track_height);
        assert_eq!(m.card.1, theme.row_height + 2.0 * theme.spacing.sm);
        assert_eq!(
            m.card.0,
            2.0 * theme.spacing.lg
                + theme.spacing.xxl
                + 2.0 * theme.spacing.md
                + m.track.0
                + m.label
        );
        // A larger text scale widens the label, never the bar.
        let large = Metrics::of(&theme.scaled(1.3));
        assert!(large.label > m.label);
        assert_eq!(large.track, m.track);
        let (w, h) = m.surface();
        assert_eq!(w as f32, (m.card.0 + 2.0 * m.margin).ceil());
        assert_eq!(h as f32, (m.card.1 + m.margin).ceil());
    }

    #[test]
    fn the_card_sits_on_the_bottom_margin_with_the_bar_filled_to_the_level() {
        let theme = Theme::dark();
        let m = Metrics::of(&theme);
        let osd = laid_out(&theme, Some(shown(Kind::Volume, 40, false)), Instant::now());
        let card = osd
            .tree
            .rect(osd.tree.node_by_key(card_key()).expect("card"));
        assert_eq!((card.x, card.y), (m.margin, 0.0));
        assert_eq!((card.w, card.h), m.card);
        let surface = m.surface();
        assert_eq!(surface.1 as f32 - (card.y + card.h), m.margin.ceil());
        let track = osd.tree.rect(
            osd.tree
                .node_by_key(Key::named("osd-track"))
                .expect("track"),
        );
        let level = osd.tree.rect(
            osd.tree
                .node_by_key(Key::named("osd-level"))
                .expect("level"),
        );
        assert_eq!(track.w, m.track.0);
        assert!((level.w - 0.4 * m.track.0).abs() < 0.01, "{}", level.w);
        assert_eq!(level.x, track.x);
    }

    #[test]
    fn icons_follow_the_kind_and_the_mute() {
        assert_eq!(icon_of(shown(Kind::Volume, 40, false)), "media-volume");
        assert_eq!(icon_of(shown(Kind::Volume, 40, true)), "media-volume-muted");
        assert_eq!(icon_of(shown(Kind::Microphone, 40, false)), "mic");
        assert_eq!(icon_of(shown(Kind::Microphone, 40, true)), "mic-off");
        assert_eq!(icon_of(shown(Kind::Brightness, 40, false)), "sun");
    }

    #[test]
    fn muted_keeps_the_remembered_level_in_a_faint_bar() {
        let theme = Theme::dark();
        let m = Metrics::of(&theme);
        let osd = laid_out(&theme, Some(shown(Kind::Volume, 70, true)), Instant::now());
        let level = osd.tree.rect(
            osd.tree
                .node_by_key(Key::named("osd-level"))
                .expect("level"),
        );
        assert!((level.w - 0.7 * m.track.0).abs() < 0.01);
        let fill = |muted: bool| {
            let built = view(&theme, Some(shown(Kind::Volume, 70, muted)));
            match &built.children[0].children[1].children[0].kind {
                celestina_ui::ViewKind::Box { fill, .. } => *fill,
                other => panic!("the level is a box, not {other:?}"),
            }
        };
        assert_eq!(fill(true), Some(Role::TextFaint));
        assert_eq!(fill(false), Some(Role::Accent));
    }

    #[test]
    fn the_level_reads_as_spanish_copy() {
        assert_eq!(copy::osd_percent(7), "7 %");
        assert_eq!(copy::osd_percent(100), "100 %");
    }

    #[test]
    fn taking_the_content_away_fades_the_card_out_and_then_rests() {
        let theme = Theme::dark();
        let t0 = Instant::now();
        let mut osd = laid_out(&theme, Some(shown(Kind::Brightness, 50, false)), t0);
        assert!(
            !osd.tree.is_animating(t0),
            "the first content appears at once"
        );
        assert!(!osd.blur(t0).is_empty());
        assert!(osd.show(None, t0));
        assert!(!osd.show(None, t0), "the same content is not applied twice");
        assert!(
            osd.tree.is_animating(t0),
            "the card leaves on the exit fade"
        );
        let exit = std::time::Duration::from_millis(u64::from(theme.motion.exit));
        let after = t0 + exit + std::time::Duration::from_millis(1);
        assert!(!osd.tree.is_animating(after));
        assert!(osd.blur(after).is_empty());
        // Reduced motion keeps the fade (an opacity, not a movement).
        let mut reduced = OsdView::new(&theme, true);
        reduced.show(Some(shown(Kind::Volume, 50, false)), t0);
        reduced.show(None, t0);
        assert!(reduced.tree.is_animating(t0));
    }
}
